use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::audio_in_flight::AudioInFlightLimiter;
use crate::audio_stream_tracker::AudioStreamTracker;
use crate::component_handler_events::ComponentHandlerEventPublisher;
use crate::events::{BridgeEventBus, BridgeEventKind};
use crate::host_worker::HostWorkerClient;
use crate::instance_registry::{
    InstanceCreateParams, InstanceRegistry, InstanceState, WorkerState,
};
use crate::metrics::BridgeMetrics;
use crate::plugin_registry::PluginRegistry;
use crate::worker_supervisor::WorkerSupervisor;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct RequestContext<'a> {
    config: &'a BridgeConfig,
    host_worker: &'a HostWorkerClient,
    instances: &'a InstanceRegistry,
    component_handler_events: &'a ComponentHandlerEventPublisher,
    events: &'a BridgeEventBus,
    metrics: &'a BridgeMetrics,
    plugins: &'a PluginRegistry,
    stream_tracker: &'a AudioStreamTracker,
    audio_in_flight: &'a AudioInFlightLimiter,
    workers: &'a WorkerSupervisor,
}

#[cfg(unix)]
#[tokio::test]
async fn creates_lists_and_destroys_instance() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let worker_path = serve_worker_script();
    let workers = WorkerSupervisor::new_for_test(worker_path.clone(), Duration::from_secs(5));
    let context = RequestContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        component_handler_events: &component_handler_events,
        events: &events,
        metrics: &metrics,
        plugins: &plugins,
        stream_tracker: &stream_tracker,
        audio_in_flight: &audio_in_flight,
        workers: &workers,
    };

    let create_request = serde_json::json!({
        "id": 1,
        "method": "instance.create",
        "params": {
            "pluginId": plugin_id,
            "classId": "class-a",
            "sampleRate": 48000,
            "maxBlockFrames": 128,
            "inputChannels": 2,
            "outputChannels": 2
        }
    })
    .to_string();
    let create_value = request_json(&create_request, context).await;
    assert!(create_value.get("error").is_none(), "{create_value}");
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");
    let stream_id = create_value["result"]["streamId"]
        .as_u64()
        .expect("stream id");

    assert_eq!(create_value["result"]["state"], "ready");
    assert_eq!(create_value["result"]["workerState"], "ready");
    assert_eq!(create_value["result"]["streamState"], "open");
    assert_eq!(create_value["result"]["backend"], "passthrough");
    assert_eq!(
        create_value["result"]["runtimeCapabilities"]["schemaVersion"],
        1
    );
    assert_eq!(
        create_value["result"]["runtimeCapabilities"]["binaryAudioProcess"],
        true
    );
    assert_eq!(
        create_value["result"]["runtimeCapabilities"]["parameters"],
        true
    );
    assert_eq!(
        create_value["result"]["runtimeCapabilities"]["componentState"],
        false
    );
    assert_eq!(create_value["result"]["latencySamples"], 0);
    assert_eq!(create_value["result"]["tailSamples"], 0);

    let list_value =
        request_json(r#"{"id":2,"method":"instance.list","params":{}}"#, context).await;
    assert_eq!(list_value["result"].as_array().expect("instances").len(), 1);
    assert_eq!(
        list_value["result"][0]["runtimeCapabilities"]["parameters"],
        true
    );

    let status_request = serde_json::json!({
        "id": 3,
        "method": "instance.status",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let status_value = request_json(&status_request, context).await;
    assert_eq!(status_value["result"]["instance"]["workerState"], "ready");
    assert_eq!(
        status_value["result"]["instance"]["runtimeCapabilities"]["parameters"],
        true
    );
    assert_eq!(status_value["result"]["worker"]["ipcVersion"], 1);
    assert_eq!(status_value["result"]["worker"]["instances"], 1);
    assert_eq!(
        status_value["result"]["worker"]["runtime"][0]["runtimeCapabilities"]["schemaVersion"],
        1
    );
    assert_eq!(
        status_value["result"]["worker"]["runtime"][0]["runtimeCapabilities"]["parameters"],
        true
    );
    assert_eq!(
        status_value["result"]["worker"]["runtime"][0]["diagnostics"]["componentHandler"]["totalEvents"],
        3
    );
    assert_eq!(
        status_value["result"]["worker"]["runtime"][0]["diagnostics"]["passthroughReason"]["kind"],
        "non-bundle-path"
    );

    let handler_events_value =
        request_json(r#"{"id":72,"method":"bridge.events","params":{}}"#, context).await;
    let handler_events = handler_events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["kind"]["type"] == "vst3-component-handler-event")
        .collect::<Vec<_>>();
    assert_eq!(handler_events.len(), 3);
    assert_eq!(handler_events[0]["kind"]["handlerKind"], "begin-edit");
    assert_eq!(handler_events[0]["kind"]["parameterId"], 42);
    assert_eq!(handler_events[1]["kind"]["handlerKind"], "perform-edit");
    assert_eq!(handler_events[1]["kind"]["valueNormalized"], 0.75);
    assert_eq!(
        handler_events[2]["kind"]["handlerKind"],
        "restart-component"
    );
    assert_eq!(
        handler_events[2]["kind"]["restartFlags"]["latencyChanged"],
        true
    );
    assert_eq!(
        handler_events[2]["kind"]["restartFlags"]["paramTitlesChanged"],
        true
    );
    let metadata_events = handler_events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["kind"]["type"] == "vst3-metadata-invalidated")
        .collect::<Vec<_>>();
    assert_eq!(metadata_events.len(), 1);
    assert_eq!(metadata_events[0]["kind"]["handlerSequence"], 3);
    assert_eq!(
        metadata_events[0]["kind"]["reasons"],
        serde_json::json!(["parameter-info", "latency"])
    );
    assert_eq!(
        metadata_events[0]["kind"]["refreshPolicy"],
        "refresh-metadata"
    );

    let second_status_value = request_json(&status_request, context).await;
    assert_eq!(
        second_status_value["result"]["worker"]["runtime"][0]["diagnostics"]["componentHandler"]["totalEvents"],
        3
    );
    let deduped_events_value =
        request_json(r#"{"id":73,"method":"bridge.events","params":{}}"#, context).await;
    let deduped_handler_events = deduped_events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["kind"]["type"] == "vst3-component-handler-event")
        .count();
    assert_eq!(deduped_handler_events, 3);
    let deduped_metadata_events = deduped_events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["kind"]["type"] == "vst3-metadata-invalidated")
        .count();
    assert_eq!(deduped_metadata_events, 1);

    let parameters_request = serde_json::json!({
        "id": 31,
        "method": "instance.parameters",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let parameters_value = request_json(&parameters_request, context).await;
    assert_eq!(parameters_value["result"]["parameters"][0]["id"], 42);
    assert_eq!(parameters_value["result"]["parameters"][0]["title"], "Gain");

    let metadata_refresh_request = serde_json::json!({
        "id": 30,
        "method": "instance.metadata.refresh",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let metadata_refresh_value = request_json(&metadata_refresh_request, context).await;
    assert_eq!(metadata_refresh_value["result"]["instanceId"], instance_id);
    assert_eq!(metadata_refresh_value["result"]["parameters"][0]["id"], 42);
    assert_eq!(metadata_refresh_value["result"]["unitInfo"], Value::Null);
    assert_eq!(metadata_refresh_value["result"]["state"], Value::Null);
    assert_eq!(metadata_refresh_value["result"]["worker"]["instances"], 1);

    let runtime_snapshot_request = serde_json::json!({
        "id": 74,
        "method": "instance.runtime.snapshot",
        "params": { "instanceId": instance_id, "includeRecentEvents": true }
    })
    .to_string();
    let runtime_snapshot_value = request_json(&runtime_snapshot_request, context).await;
    assert_eq!(
        runtime_snapshot_value["result"]["instance"]["instanceId"],
        instance_id
    );
    assert_eq!(
        runtime_snapshot_value["result"]["metadata"]["parameters"][0]["id"],
        42
    );
    assert_eq!(
        runtime_snapshot_value["result"]["metadata"]["worker"]["instances"],
        1
    );
    assert!(
        runtime_snapshot_value["result"]["recentEvents"]
            .as_array()
            .expect("recent events")
            .iter()
            .any(|event| event["kind"]["type"] == "worker-ready")
    );

    let parameter_info_request = serde_json::json!({
        "id": 32,
        "method": "instance.parameter.info",
        "params": { "instanceId": instance_id, "parameterId": 42, "valueNormalized": 0.25 }
    })
    .to_string();
    let parameter_info_value = request_json(&parameter_info_request, context).await;
    assert_eq!(parameter_info_value["result"]["valuePlain"], 25.0);
    assert_eq!(parameter_info_value["result"]["valueString"], "25 dB");

    let parameter_value_by_string_request = serde_json::json!({
        "id": 33,
        "method": "instance.parameter.valueByString",
        "params": { "instanceId": instance_id, "parameterId": 42, "value": "50 dB" }
    })
    .to_string();
    let parameter_value_by_string_value =
        request_json(&parameter_value_by_string_request, context).await;
    assert_eq!(
        parameter_value_by_string_value["result"]["valueNormalized"],
        0.5
    );
    assert_eq!(
        parameter_value_by_string_value["result"]["valueString"],
        "50 dB"
    );

    let parameter_normalized_by_plain_request = serde_json::json!({
        "id": 34,
        "method": "instance.parameter.normalizedByPlain",
        "params": { "instanceId": instance_id, "parameterId": 42, "valuePlain": 75.0 }
    })
    .to_string();
    let parameter_normalized_by_plain_value =
        request_json(&parameter_normalized_by_plain_request, context).await;
    assert_eq!(
        parameter_normalized_by_plain_value["result"]["valueNormalized"],
        0.75
    );
    assert_eq!(
        parameter_normalized_by_plain_value["result"]["valueString"],
        "75 dB"
    );

    let parameter_begin_edit_request = serde_json::json!({
        "id": 35,
        "method": "instance.parameter.beginEdit",
        "params": { "instanceId": instance_id, "parameterId": 42 }
    })
    .to_string();
    let parameter_begin_edit_value = request_json(&parameter_begin_edit_request, context).await;
    assert_eq!(
        parameter_begin_edit_value["result"]["editKind"],
        "begin-edit"
    );

    let parameter_perform_edit_request = serde_json::json!({
        "id": 36,
        "method": "instance.parameter.performEdit",
        "params": { "instanceId": instance_id, "parameterId": 42, "valueNormalized": 0.66 }
    })
    .to_string();
    let parameter_perform_edit_value = request_json(&parameter_perform_edit_request, context).await;
    assert_eq!(
        parameter_perform_edit_value["result"]["editKind"],
        "perform-edit"
    );
    assert_eq!(
        parameter_perform_edit_value["result"]["valueNormalized"],
        0.66
    );

    let parameter_end_edit_request = serde_json::json!({
        "id": 37,
        "method": "instance.parameter.endEdit",
        "params": { "instanceId": instance_id, "parameterId": 42 }
    })
    .to_string();
    let parameter_end_edit_value = request_json(&parameter_end_edit_request, context).await;
    assert_eq!(parameter_end_edit_value["result"]["editKind"], "end-edit");

    let parameter_edit_request = serde_json::json!({
        "id": 41,
        "method": "instance.parameter.edit",
        "params": { "instanceId": instance_id, "parameterId": 42, "valueNormalized": 0.66 }
    })
    .to_string();
    let parameter_edit_value = request_json(&parameter_edit_request, context).await;
    assert_eq!(
        parameter_edit_value["result"]["beginEdit"]["editKind"],
        "begin-edit"
    );
    assert_eq!(
        parameter_edit_value["result"]["performEdit"]["editKind"],
        "perform-edit"
    );
    assert_eq!(
        parameter_edit_value["result"]["performEdit"]["valueNormalized"],
        0.66
    );
    assert_eq!(
        parameter_edit_value["result"]["endEdit"]["editKind"],
        "end-edit"
    );

    let notify_component_request = serde_json::json!({
        "id": 38,
        "method": "instance.connection.notifyComponent",
        "params": {
            "instanceId": instance_id,
            "messageId": "TextMessage",
            "attributes": {
                "answer": { "type": "int", "value": 42 },
                "label": { "type": "string", "value": "ok" }
            }
        }
    })
    .to_string();
    let notify_component_value = request_json(&notify_component_request, context).await;
    assert_eq!(notify_component_value["result"]["target"], "component");
    assert_eq!(notify_component_value["result"]["messageId"], "TextMessage");
    assert_eq!(notify_component_value["result"]["attributeCount"], 2);
    assert_eq!(notify_component_value["result"]["notified"], true);

    let notify_controller_request = serde_json::json!({
        "id": 39,
        "method": "instance.connection.notifyController",
        "params": { "instanceId": instance_id, "messageId": "TextMessage" }
    })
    .to_string();
    let notify_controller_value = request_json(&notify_controller_request, context).await;
    assert_eq!(notify_controller_value["result"]["target"], "controller");
    assert_eq!(
        notify_controller_value["result"]["messageId"],
        "TextMessage"
    );
    assert_eq!(notify_controller_value["result"]["attributeCount"], 0);
    assert_eq!(notify_controller_value["result"]["notified"], true);

    let notify_component_refresh_request = serde_json::json!({
        "id": 46,
        "method": "instance.connection.notifyComponentAndRefresh",
        "params": {
            "instanceId": instance_id,
            "messageId": "TextMessage",
            "attributes": {
                "answer": { "type": "int", "value": 42 }
            }
        }
    })
    .to_string();
    let notify_component_refresh_value =
        request_json(&notify_component_refresh_request, context).await;
    assert_eq!(
        notify_component_refresh_value["result"]["notifyComponent"]["target"],
        "component"
    );
    assert_eq!(
        notify_component_refresh_value["result"]["metadata"]["parameters"][0]["id"],
        42
    );

    let notify_controller_refresh_request = serde_json::json!({
        "id": 47,
        "method": "instance.connection.notifyControllerAndRefresh",
        "params": {
            "instanceId": instance_id,
            "messageId": "TextMessage",
            "includeWorkerMetrics": false
        }
    })
    .to_string();
    let notify_controller_refresh_value =
        request_json(&notify_controller_refresh_request, context).await;
    assert_eq!(
        notify_controller_refresh_value["result"]["notifyController"]["target"],
        "controller"
    );
    assert_eq!(
        notify_controller_refresh_value["result"]["metadata"]["worker"],
        Value::Null
    );

    let invalid_notify_request = serde_json::json!({
        "id": 40,
        "method": "instance.connection.notifyComponent",
        "params": {
            "instanceId": instance_id,
            "messageId": "TextMessage",
            "attributes": []
        }
    })
    .to_string();
    let invalid_notify_value = request_json(&invalid_notify_request, context).await;
    assert_eq!(invalid_notify_value["error"]["code"], -32602);
    assert_eq!(
        invalid_notify_value["error"]["message"],
        "attributes must be an object"
    );

    let set_state_and_refresh_request = serde_json::json!({
        "id": 42,
        "method": "instance.state.setAndRefresh",
        "params": {
            "instanceId": instance_id,
            "controllerStateBase64": "AQID",
            "includeState": true
        }
    })
    .to_string();
    let set_state_and_refresh_value = request_json(&set_state_and_refresh_request, context).await;
    assert_eq!(
        set_state_and_refresh_value["result"]["setState"]["controllerStateBytes"],
        3
    );
    assert_eq!(
        set_state_and_refresh_value["result"]["metadata"]["parameters"][0]["id"],
        42
    );
    assert_eq!(
        set_state_and_refresh_value["result"]["metadata"]["state"]["stateBase64"],
        "AQID"
    );
    assert_eq!(
        set_state_and_refresh_value["result"]["metadata"]["worker"]["instances"],
        1
    );

    let set_unit_program_data_refresh_request = serde_json::json!({
        "id": 43,
        "method": "instance.setUnitProgramDataAndRefresh",
        "params": {
            "instanceId": instance_id,
            "listOrUnitId": 1,
            "programIndex": 2,
            "dataBase64": "AQID"
        }
    })
    .to_string();
    let set_unit_program_data_refresh_value =
        request_json(&set_unit_program_data_refresh_request, context).await;
    assert_eq!(
        set_unit_program_data_refresh_value["result"]["setUnitProgramData"]["dataBytes"],
        3
    );
    assert_eq!(
        set_unit_program_data_refresh_value["result"]["metadata"]["parameters"][0]["id"],
        42
    );

    let set_program_data_refresh_request = serde_json::json!({
        "id": 44,
        "method": "instance.programData.setAndRefresh",
        "params": {
            "instanceId": instance_id,
            "listId": 1,
            "programIndex": 2,
            "dataBase64": "AQID"
        }
    })
    .to_string();
    let set_program_data_refresh_value =
        request_json(&set_program_data_refresh_request, context).await;
    assert_eq!(
        set_program_data_refresh_value["result"]["setProgramData"]["dataBytes"],
        3
    );
    assert_eq!(
        set_program_data_refresh_value["result"]["metadata"]["worker"]["instances"],
        1
    );

    let set_unit_data_refresh_request = serde_json::json!({
        "id": 45,
        "method": "instance.unitData.setAndRefresh",
        "params": {
            "instanceId": instance_id,
            "unitId": 1,
            "dataBase64": "AQID",
            "includeWorkerMetrics": false
        }
    })
    .to_string();
    let set_unit_data_refresh_value = request_json(&set_unit_data_refresh_request, context).await;
    assert_eq!(
        set_unit_data_refresh_value["result"]["setUnitData"]["dataBytes"],
        3
    );
    assert_eq!(
        set_unit_data_refresh_value["result"]["metadata"]["worker"],
        Value::Null
    );

    let close_stream_request = serde_json::json!({
        "id": 4,
        "method": "stream.close",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let close_stream_value = request_json(&close_stream_request, context).await;
    assert_eq!(close_stream_value["result"]["streamState"], "closed");

    let open_stream_request = serde_json::json!({
        "id": 5,
        "method": "stream.open",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let open_stream_value = request_json(&open_stream_request, context).await;
    assert_eq!(open_stream_value["result"]["streamState"], "open");

    let start_request = serde_json::json!({
        "id": 6,
        "method": "instance.start",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let start_value = request_json(&start_request, context).await;
    assert_eq!(start_value["result"]["instance"]["state"], "processing");
    assert_eq!(start_value["result"]["worker"]["workerState"], "processing");

    let stop_request = serde_json::json!({
        "id": 7,
        "method": "instance.stop",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let stop_value = request_json(&stop_request, context).await;
    assert_eq!(stop_value["result"]["instance"]["state"], "stopped");
    assert_eq!(stop_value["result"]["worker"]["workerState"], "stopped");

    let events_value =
        request_json(r#"{"id":71,"method":"bridge.events","params":{}}"#, context).await;
    let event_types = events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .map(|event| event["kind"]["type"].as_str().expect("event type"))
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"worker-starting"));
    assert!(event_types.contains(&"worker-ready"));
    assert!(event_types.contains(&"worker-processing-starting"));
    assert!(event_types.contains(&"worker-processing"));
    assert!(event_types.contains(&"worker-processing-stopping"));
    assert!(event_types.contains(&"worker-stopped"));

    let destroy_request = serde_json::json!({
        "id": 8,
        "method": "instance.destroy",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let destroy_value = request_json(&destroy_request, context).await;
    assert_eq!(destroy_value["result"]["state"], "destroyed");
    assert!(instances.list().is_empty());
    let destroy_events_value =
        request_json(r#"{"id":74,"method":"bridge.events","params":{}}"#, context).await;
    let destroy_events = destroy_events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| {
            matches!(
                event["kind"]["type"].as_str(),
                Some("worker-destroying" | "worker-destroyed")
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(destroy_events.len(), 2);
    assert_eq!(
        destroy_events[0]["kind"]["instanceId"].as_u64(),
        Some(instance_id)
    );
    assert_eq!(
        destroy_events[0]["kind"]["streamId"].as_u64(),
        Some(stream_id)
    );
    assert_eq!(
        destroy_events[0]["kind"]["pluginId"].as_str(),
        Some(plugin_id.as_str())
    );
    assert_eq!(
        destroy_events[1]["kind"]["instanceId"].as_u64(),
        Some(instance_id)
    );
    assert_eq!(
        destroy_events[1]["kind"]["streamId"].as_u64(),
        Some(stream_id)
    );
    assert_eq!(
        destroy_events[1]["kind"]["pluginId"].as_str(),
        Some(plugin_id.as_str())
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn marks_instance_failed_when_heartbeat_worker_exits() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"))
        .with_worker_auto_restart(false);
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let worker_path = serve_worker_exits_on_metrics_script();
    let workers = WorkerSupervisor::new_for_test(worker_path.clone(), Duration::from_secs(5));
    let context = RequestContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        component_handler_events: &component_handler_events,
        events: &events,
        metrics: &metrics,
        plugins: &plugins,
        stream_tracker: &stream_tracker,
        audio_in_flight: &audio_in_flight,
        workers: &workers,
    };

    let create_request = instance_create_request(1, &plugin_id);
    let create_value = request_json(&create_request, context).await;
    assert!(create_value.get("error").is_none(), "{create_value}");
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");

    let status_value = request_json(
        &instance_request(2, "instance.status", instance_id),
        context,
    )
    .await;
    assert_eq!(status_value["error"]["code"], 5037);
    assert_eq!(status_value["error"]["data"]["kind"], "protocol");

    let failed = instances.get(instance_id).expect("failed instance");
    assert_eq!(failed.state, InstanceState::Failed);
    assert_eq!(failed.worker_state, WorkerState::Failed);

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_instance_create_when_worker_limit_is_reached() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let worker_path = serve_worker_script();
    let workers = WorkerSupervisor::with_options(
        crate::worker_supervisor::WorkerSupervisorOptions::new(worker_path.clone())
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false)
            .with_framed_control_ipc(false)
            .with_max_instances(1),
    );
    let context = RequestContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        component_handler_events: &component_handler_events,
        events: &events,
        metrics: &metrics,
        plugins: &plugins,
        stream_tracker: &stream_tracker,
        audio_in_flight: &audio_in_flight,
        workers: &workers,
    };

    let first = request_json(&instance_create_request(1, &plugin_id), context).await;
    assert!(first.get("error").is_none(), "{first}");

    let second = request_json(&instance_create_request(2, &plugin_id), context).await;

    assert_eq!(second["error"]["code"], 4290);
    assert_eq!(second["error"]["data"]["kind"], "resource-limit-exceeded");
    assert_eq!(second["error"]["data"]["resource"], "worker-instances");
    assert_eq!(instances.list().len(), 1);
    assert_eq!(metrics.snapshot().worker_failures, 1);

    let events_value =
        request_json(r#"{"id":5,"method":"bridge.events","params":{}}"#, context).await;
    let policy = worker_policy_decision(
        &events_value,
        "resource-limit",
        "reject",
        "worker-instance-limit",
    );
    assert_eq!(policy["pluginId"], plugin_id);
    assert_eq!(policy["data"]["kind"], "resource-limit-exceeded");
    assert_eq!(policy["data"]["resource"], "worker-instances");

    let first_instance = first["result"]["instanceId"].as_u64().expect("instance id");
    let _ = request_json(
        &instance_request(3, "instance.destroy", first_instance),
        context,
    )
    .await;

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn auto_recovers_processing_instance_when_heartbeat_worker_exits() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let worker_path = serve_worker_exits_on_metrics_script();
    let workers = WorkerSupervisor::new_for_test(worker_path.clone(), Duration::from_secs(5));
    let context = RequestContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        component_handler_events: &component_handler_events,
        events: &events,
        metrics: &metrics,
        plugins: &plugins,
        stream_tracker: &stream_tracker,
        audio_in_flight: &audio_in_flight,
        workers: &workers,
    };

    let create_value = request_json(&instance_create_request(1, &plugin_id), context).await;
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");

    let start_value =
        request_json(&instance_request(2, "instance.start", instance_id), context).await;
    assert_eq!(start_value["result"]["instance"]["state"], "processing");

    let status_value = request_json(
        &instance_request(3, "instance.status", instance_id),
        context,
    )
    .await;
    assert_eq!(status_value["result"]["recovered"], true);
    assert_eq!(status_value["result"]["instance"]["state"], "processing");
    assert_eq!(
        status_value["result"]["instance"]["workerState"],
        "processing"
    );
    assert_eq!(
        status_value["result"]["worker"]["workerState"],
        "processing"
    );

    let metrics_value =
        request_json(r#"{"id":4,"method":"bridge.metrics","params":{}}"#, context).await;
    assert_eq!(metrics_value["result"]["workerFailures"], 1);
    assert_eq!(metrics_value["result"]["workerRestarts"], 1);
    assert_eq!(metrics_value["result"]["workerAutoRestarts"], 1);

    let events_value =
        request_json(r#"{"id":41,"method":"bridge.events","params":{}}"#, context).await;
    let event_types = events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .map(|event| event["kind"]["type"].as_str().expect("event type"))
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"worker-recovering"));
    assert!(event_types.contains(&"worker-recovered"));
    let recovering = events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| event["kind"]["type"] == "worker-recovering")
        .expect("recovering event");
    assert_eq!(recovering["kind"]["mode"], "auto-heartbeat");
    assert_eq!(recovering["kind"]["reason"], "heartbeat-failed");
    assert!(recovering["kind"]["errorData"].is_object());
    let policy = worker_policy_decision(
        &events_value,
        "worker-recovery",
        "restart",
        "heartbeat-failed",
    );
    assert_eq!(policy["instanceId"].as_u64(), Some(instance_id));
    assert_eq!(policy["pluginId"], plugin_id);
    assert!(policy["data"].is_object());
    let recovered = events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| event["kind"]["type"] == "worker-recovered")
        .expect("recovered event");
    assert_eq!(recovered["kind"]["mode"], "auto-heartbeat");
    assert_eq!(recovered["kind"]["processingRestored"], true);

    let _ = request_json(
        &instance_request(5, "instance.destroy", instance_id),
        context,
    )
    .await;

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn emits_recovery_failed_when_auto_restart_recreate_fails() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let worker_path = serve_worker_rejects_recreate_after_metrics_exit_script();
    let workers = WorkerSupervisor::new_for_test(worker_path.clone(), Duration::from_secs(5));
    let context = RequestContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        component_handler_events: &component_handler_events,
        events: &events,
        metrics: &metrics,
        plugins: &plugins,
        stream_tracker: &stream_tracker,
        audio_in_flight: &audio_in_flight,
        workers: &workers,
    };

    let create_value = request_json(&instance_create_request(1, &plugin_id), context).await;
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");
    let start_value =
        request_json(&instance_request(2, "instance.start", instance_id), context).await;
    assert_eq!(start_value["result"]["instance"]["state"], "processing");

    let status_value = request_json(
        &instance_request(3, "instance.status", instance_id),
        context,
    )
    .await;
    assert_eq!(status_value["error"]["code"], 4220);
    assert_eq!(status_value["error"]["data"]["kind"], "worker-rejected");

    let failed = instances.get(instance_id).expect("failed instance");
    assert_eq!(failed.state, InstanceState::Failed);
    assert_eq!(failed.worker_state, WorkerState::Failed);

    let events_value =
        request_json(r#"{"id":41,"method":"bridge.events","params":{}}"#, context).await;
    let recovery_failed = events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| event["kind"]["type"] == "worker-recovery-failed")
        .expect("recovery failed event");
    assert_eq!(recovery_failed["kind"]["mode"], "auto-heartbeat");
    assert_eq!(recovery_failed["kind"]["reason"], "restart-failed");
    assert_eq!(
        recovery_failed["kind"]["errorData"]["kind"],
        "worker-rejected"
    );
    assert_eq!(
        recovery_failed["kind"]["errorData"]["workerData"]["stage"],
        "component.create"
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn restarts_failed_instance_with_same_stream() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"))
        .with_worker_auto_restart(false);
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let worker_path = serve_worker_exits_on_metrics_script();
    let workers = WorkerSupervisor::new_for_test(worker_path.clone(), Duration::from_secs(5));
    let context = RequestContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        component_handler_events: &component_handler_events,
        events: &events,
        metrics: &metrics,
        plugins: &plugins,
        stream_tracker: &stream_tracker,
        audio_in_flight: &audio_in_flight,
        workers: &workers,
    };

    let create_request = instance_create_request(1, &plugin_id);
    let create_value = request_json(&create_request, context).await;
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");
    let stream_id = create_value["result"]["streamId"]
        .as_u64()
        .expect("stream id");

    let failing_status_value = request_json(
        &instance_request(2, "instance.status", instance_id),
        context,
    )
    .await;
    assert_eq!(failing_status_value["error"]["code"], 5037);

    let restart_value = request_json(
        &instance_request(3, "instance.restart", instance_id),
        context,
    )
    .await;
    assert_eq!(
        restart_value["result"]["instance"]["instanceId"], instance_id,
        "restart response: {restart_value}"
    );
    assert_eq!(restart_value["result"]["instance"]["streamId"], stream_id);
    assert_eq!(restart_value["result"]["instance"]["state"], "ready");
    assert_eq!(
        restart_value["result"]["instance"]["backend"],
        "passthrough"
    );
    assert_eq!(restart_value["result"]["instance"]["latencySamples"], 0);
    assert_eq!(restart_value["result"]["instance"]["tailSamples"], 0);
    assert_eq!(restart_value["result"]["worker"]["workerState"], "ready");

    let metrics_value =
        request_json(r#"{"id":4,"method":"bridge.metrics","params":{}}"#, context).await;
    assert_eq!(metrics_value["result"]["workerFailures"], 1);
    assert_eq!(metrics_value["result"]["workerRestarts"], 1);

    let events_value =
        request_json(r#"{"id":41,"method":"bridge.events","params":{}}"#, context).await;
    let recovering = events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| event["kind"]["type"] == "worker-recovering")
        .expect("recovering event");
    assert_eq!(recovering["kind"]["mode"], "manual-restart");
    assert_eq!(recovering["kind"]["reason"], "manual-restart");
    assert!(recovering["kind"].get("errorData").is_none());
    let policy = worker_policy_decision(
        &events_value,
        "worker-recovery",
        "restart",
        "manual-restart",
    );
    assert_eq!(policy["instanceId"].as_u64(), Some(instance_id));
    assert_eq!(policy["pluginId"], plugin_id);
    assert_eq!(policy["data"]["wasProcessing"], false);
    let recovered = events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| event["kind"]["type"] == "worker-recovered")
        .expect("recovered event");
    assert_eq!(recovered["kind"]["mode"], "manual-restart");
    assert_eq!(recovered["kind"]["processingRestored"], false);

    let _ = request_json(
        &instance_request(5, "instance.destroy", instance_id),
        context,
    )
    .await;

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[tokio::test]
async fn stream_close_waits_for_in_flight_audio_to_drain() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let plugin = plugins.find(&plugin_id).expect("plugin");
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let workers = WorkerSupervisor::new_for_test(
        PathBuf::from("missing-wvst-host-worker"),
        Duration::from_secs(5),
    );
    let record = instances
        .create(
            InstanceCreateParams {
                plugin_id,
                class_id: Some("class-a".to_string()),
                sample_rate: 48_000,
                max_block_frames: 128,
                input_channels: 2,
                output_channels: 2,
            },
            &plugin,
        )
        .expect("instance");
    let guard = audio_in_flight
        .try_acquire(record.stream_id)
        .expect("in-flight stream");
    let context = RequestContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        component_handler_events: &component_handler_events,
        events: &events,
        metrics: &metrics,
        plugins: &plugins,
        stream_tracker: &stream_tracker,
        audio_in_flight: &audio_in_flight,
        workers: &workers,
    };
    let close_request = serde_json::json!({
        "id": 1,
        "method": "stream.close",
        "params": { "instanceId": record.instance_id }
    })
    .to_string();
    let mut close = Box::pin(request_json(&close_request, context));

    tokio::select! {
        _ = &mut close => panic!("stream.close returned before in-flight audio drained"),
        _ = tokio::time::sleep(Duration::from_millis(25)) => {}
    }
    drop(guard);
    let close_value = close.await;

    assert_eq!(close_value["result"]["streamState"], "closed");
    assert_eq!(
        instances
            .get(record.instance_id)
            .expect("instance")
            .stream_state,
        crate::instance_registry::StreamState::Closed
    );
    let close_events = events.recent_since(None);
    assert_eq!(close_events.len(), 2);
    assert!(matches!(
        &close_events[0].kind,
        BridgeEventKind::StreamClosing {
            instance_id,
            plugin_id: _,
            stream_id,
        } if *instance_id == record.instance_id && *stream_id == record.stream_id
    ));
    assert!(matches!(
        &close_events[1].kind,
        BridgeEventKind::StreamClosed {
            instance_id,
            plugin_id: _,
            stream_id,
            drain_timed_out: false,
        } if *instance_id == record.instance_id && *stream_id == record.stream_id
    ));

    let open_request = serde_json::json!({
        "id": 2,
        "method": "stream.open",
        "params": { "instanceId": record.instance_id }
    })
    .to_string();
    let open_value = request_json(&open_request, context).await;

    assert_eq!(open_value["result"]["streamState"], "open");
    let open_events = events.recent_since(close_events.last().map(|event| event.sequence));
    assert_eq!(open_events.len(), 1);
    assert!(matches!(
        &open_events[0].kind,
        BridgeEventKind::StreamOpened {
            instance_id,
            plugin_id: _,
            stream_id,
        } if *instance_id == record.instance_id && *stream_id == record.stream_id
    ));

    let _ = std::fs::remove_dir_all(root);
}

async fn request_json(text: &str, context: RequestContext<'_>) -> Value {
    let response = handle_control_text(
        text,
        ControlContext {
            config: context.config,
            host_worker: context.host_worker,
            instances: context.instances,
            component_handler_events: context.component_handler_events,
            events: context.events,
            metrics: context.metrics,
            plugins: context.plugins,
            origin: None,
            stream_tracker: context.stream_tracker,
            audio_in_flight: context.audio_in_flight,
            session_authorized: true,
            workers: context.workers,
        },
    )
    .await;

    serde_json::from_str(&response.text).expect("valid control json")
}

fn worker_policy_decision<'a>(
    events_value: &'a Value,
    policy: &str,
    decision: &str,
    reason: &str,
) -> &'a Value {
    events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .find_map(|event| {
            let kind = &event["kind"];
            (kind["type"].as_str() == Some("worker-policy-decision")
                && kind["policy"].as_str() == Some(policy)
                && kind["decision"].as_str() == Some(decision)
                && kind["reason"].as_str() == Some(reason))
            .then_some(kind)
        })
        .unwrap_or_else(|| {
            panic!("missing worker-policy-decision {policy}/{decision}/{reason}: {events_value}")
        })
}

fn test_host_worker() -> HostWorkerClient {
    HostWorkerClient::new_for_test(
        PathBuf::from("missing-wvst-host-worker"),
        Duration::from_secs(5),
    )
}

fn scanned_plugin_registry() -> (PluginRegistry, PathBuf, String) {
    let root = unique_temp_dir();
    let bundle = root.join("Test.vst3").join("Contents");
    std::fs::create_dir_all(&bundle).expect("bundle directory");
    std::fs::write(
        bundle.join("moduleinfo.json"),
        r#"{"Name":"Test","Classes":[{"CID":"class-a","Name":"Test Class","Category":"Fx"}]}"#,
    )
    .expect("moduleinfo");

    let plugins = PluginRegistry::new();
    let report = plugins.scan_paths(vec![root.clone()]);
    let plugin_id = report.plugins[0].plugin_id.clone();

    (plugins, root, plugin_id)
}

fn instance_create_request(id: u64, plugin_id: &str) -> String {
    serde_json::json!({
        "id": id,
        "method": "instance.create",
        "params": {
            "pluginId": plugin_id,
            "classId": "class-a",
            "sampleRate": 48000,
            "maxBlockFrames": 128,
            "inputChannels": 2,
            "outputChannels": 2
        }
    })
    .to_string()
}

fn instance_request(id: u64, method: &str, instance_id: u64) -> String {
    serde_json::json!({
        "id": id,
        "method": method,
        "params": { "instanceId": instance_id }
    })
    .to_string()
}

#[cfg(unix)]
fn serve_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("serve-worker.sh");
    std::fs::write(
        &worker,
r#"#!/bin/sh
runtime_capabilities='"runtimeCapabilities":{"schemaVersion":1,"binaryAudioProcess":true,"componentState":false,"controller":true,"controllerState":true,"parameters":true,"parameterAutomation":true,"units":false,"unitProgramData":false,"programListData":false,"unitData":false,"midiMapping":false,"outputEvents":true,"outputParameterChanges":true,"componentHandlerEvents":true,"connectionPoints":true,"processContext":false}'
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"test-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true,"binaryAudioProcess":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready","backend":"passthrough",%s,"latencySamples":0,"tailSamples":0}}\n' "$id" "$runtime_capabilities" ;;
    *instance.parameters*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameters":[{"id":42,"title":"Gain","shortTitle":"Gain","units":"dB","stepCount":0,"defaultNormalizedValue":0.5,"unitId":0,"flags":{"raw":1,"canAutomate":true,"readOnly":false,"wrapAround":false,"list":false,"hidden":false,"programChange":false,"bypass":false}}]}}\n' "$id" ;;
    *instance.units*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"unitInfo":null}}\n' "$id" ;;
    *instance.parameter.info*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"valueNormalized":0.25,"valuePlain":25.0,"valueString":"25 dB"}}\n' "$id" ;;
    *instance.parameter.valueByString*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"valueNormalized":0.5,"valuePlain":50.0,"valueString":"50 dB"}}\n' "$id" ;;
    *instance.parameter.normalizedByPlain*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"valueNormalized":0.75,"valuePlain":75.0,"valueString":"75 dB"}}\n' "$id" ;;
    *instance.parameter.beginEdit*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"editKind":"begin-edit","valueNormalized":null}}\n' "$id" ;;
    *instance.parameter.performEdit*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"editKind":"perform-edit","valueNormalized":0.66}}\n' "$id" ;;
    *instance.parameter.endEdit*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"editKind":"end-edit","valueNormalized":null}}\n' "$id" ;;
    *instance.getState*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"componentStateBase64":null,"controllerStateBase64":"AQID","stateBase64":"AQID"}}\n' "$id" ;;
    *instance.setState*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"componentStateBytes":null,"controllerStateBytes":3,"stateBytes":3}}\n' "$id" ;;
    *instance.setUnitProgramData*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"listOrUnitId":1,"programIndex":2,"dataBytes":3}}\n' "$id" ;;
    *instance.programData.set*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"listId":1,"programIndex":2,"dataBytes":3}}\n' "$id" ;;
    *instance.unitData.set*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"unitId":1,"dataBytes":3}}\n' "$id" ;;
    *instance.connection.notifyComponent*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"target":"component","messageId":"TextMessage","attributeCount":2,"notified":true}}\n' "$id" ;;
    *instance.connection.notifyController*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"target":"controller","messageId":"TextMessage","attributeCount":0,"notified":true}}\n' "$id" ;;
    *instance.startProcessing*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"processing"}}\n' "$id" ;;
    *instance.stopProcessing*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"stopped"}}\n' "$id" ;;
    *worker.metrics*) printf '{"jsonrpc":"2.0","id":%s,"result":{"ipcVersion":1,"instances":1,"runtime":[{"streamId":1,"backend":"passthrough",%s,"latencySamples":0,"tailSamples":0,"diagnostics":{"passthroughReason":{"kind":"non-bundle-path","message":"test fallback"},"componentHandler":{"totalEvents":3,"recentEvents":[{"sequence":1,"kind":"begin-edit","parameterId":42},{"sequence":2,"kind":"perform-edit","parameterId":42,"valueNormalized":0.75},{"sequence":3,"kind":"restart-component","flags":24,"restartFlags":{"raw":24,"reloadComponent":false,"ioChanged":false,"paramValuesChanged":false,"latencyChanged":true,"paramTitlesChanged":true,"midiCcAssignmentChanged":false,"noteExpressionChanged":false,"ioTitlesChanged":false,"prefetchableSupportChanged":false,"routingInfoChanged":false,"keyswitchChanged":false,"paramIdMappingChanged":false,"unknownBits":0}}]}}}]}}\n' "$id" "$runtime_capabilities" ;;
    *instance.destroy*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"destroyed"}}\n' "$id"; exit 0 ;;
    *) printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32601,"message":"unknown"}}\n' "$id" ;;
  esac
done
"#,
    )
    .expect("script");
    let mut permissions = std::fs::metadata(&worker).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&worker, permissions).expect("permissions");
    worker
}

#[cfg(unix)]
fn serve_worker_exits_on_metrics_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("serve-worker-exits.sh");
    std::fs::write(
        &worker,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"test-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true,"binaryAudioProcess":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready","backend":"passthrough","latencySamples":0,"tailSamples":0}}\n' "$id" ;;
    *instance.startProcessing*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"processing"}}\n' "$id" ;;
    *instance.stopProcessing*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"stopped"}}\n' "$id" ;;
    *worker.metrics*) exit 0 ;;
    *instance.destroy*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"destroyed"}}\n' "$id"; exit 0 ;;
    *) printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32601,"message":"unknown"}}\n' "$id" ;;
  esac
done
"#,
    )
    .expect("script");
    let mut permissions = std::fs::metadata(&worker).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&worker, permissions).expect("permissions");
    worker
}

#[cfg(unix)]
fn serve_worker_rejects_recreate_after_metrics_exit_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let marker = directory.join("created-once");
    let worker = directory.join("serve-worker-rejects-recreate.sh");
    std::fs::write(
        &worker,
        format!(
            r#"#!/bin/sh
marker={marker:?}
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{{"jsonrpc":"2.0","id":%s,"result":{{"workerName":"test-worker","ipcVersion":1,"capabilities":{{"instanceLifecycle":true,"binaryAudioProcess":true}}}}}}\n' "$id" ;;
    *instance.create*)
      if [ -f "$marker" ]; then
        printf '{{"jsonrpc":"2.0","id":%s,"error":{{"code":4220,"message":"component create failed","data":{{"kind":"vst3-runtime-init","stage":"component.create","hostError":"factory-create-instance-failed","message":"component create failed"}}}}}}\n' "$id"
      else
        : > "$marker"
        printf '{{"jsonrpc":"2.0","id":%s,"result":{{"instanceId":1,"streamId":1,"workerState":"ready","backend":"passthrough","latencySamples":0,"tailSamples":0}}}}\n' "$id"
      fi
      ;;
    *instance.startProcessing*) printf '{{"jsonrpc":"2.0","id":%s,"result":{{"instanceId":1,"streamId":1,"workerState":"processing"}}}}\n' "$id" ;;
    *worker.metrics*) exit 0 ;;
    *) printf '{{"jsonrpc":"2.0","id":%s,"error":{{"code":-32601,"message":"unknown"}}}}\n' "$id" ;;
  esac
done
"#,
            marker = marker.to_string_lossy()
        ),
    )
    .expect("script");
    let mut permissions = std::fs::metadata(&worker).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&worker, permissions).expect("permissions");
    worker
}

fn unique_temp_dir() -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "wvst-control-instance-test-{}-{counter}-{suffix}",
        std::process::id()
    ))
}
