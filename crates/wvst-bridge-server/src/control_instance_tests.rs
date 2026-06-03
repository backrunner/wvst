#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use super::*;
use std::path::PathBuf;
use std::sync::Arc;
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
use crate::stream_shared_memory::SharedMemoryStreamRegistry;
use crate::stream_shared_memory_pump::SharedMemoryPumpRegistry;
use crate::worker_supervisor::WorkerSupervisor;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct RequestContext<'a> {
    config: &'a BridgeConfig,
    host_worker: &'a HostWorkerClient,
    instances: &'a InstanceRegistry,
    component_handler_events: &'a ComponentHandlerEventPublisher,
    events: &'a BridgeEventBus,
    metrics: &'a Arc<BridgeMetrics>,
    plugins: &'a PluginRegistry,
    stream_tracker: &'a AudioStreamTracker,
    audio_in_flight: &'a AudioInFlightLimiter,
    shared_memory: &'a Arc<SharedMemoryStreamRegistry>,
    shared_memory_pumps: &'a SharedMemoryPumpRegistry,
    workers: &'a Arc<WorkerSupervisor>,
}

#[cfg(unix)]
#[tokio::test]
async fn creates_lists_and_destroys_instance() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let worker_path = serve_worker_script();
    let workers = Arc::new(WorkerSupervisor::new_for_test(
        worker_path.clone(),
        Duration::from_secs(5),
    ));
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
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
    assert_eq!(create_value["result"]["backend"], "vst3-runtime");
    assert_eq!(
        create_value["result"]["runtimeCapabilities"]["schemaVersion"],
        2
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
    assert_eq!(
        create_value["result"]["runtimeCapabilities"]["unavailable"][0]["capability"],
        "component-state"
    );
    assert_eq!(create_value["result"]["latencySamples"], 0);
    assert_eq!(create_value["result"]["tailSamples"], 0);
    assert_eq!(create_value["result"]["tailInfo"]["kind"], "none");
    assert_eq!(create_value["result"]["tailInfo"]["finiteSamples"], 0);

    let list_value =
        request_json(r#"{"id":2,"method":"instance.list","params":{}}"#, context).await;
    assert_eq!(list_value["result"].as_array().expect("instances").len(), 1);
    assert_eq!(
        list_value["result"][0]["runtimeCapabilities"]["parameters"],
        true
    );
    assert_eq!(list_value["result"][0]["tailInfo"]["kind"], "none");

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
        2
    );
    assert_eq!(
        status_value["result"]["worker"]["runtime"][0]["runtimeCapabilities"]["parameters"],
        true
    );
    assert_eq!(
        status_value["result"]["instance"]["runtimeCapabilities"]["unavailable"][0]["reason"],
        "interface-unavailable"
    );
    assert_eq!(
        status_value["result"]["worker"]["runtime"][0]["diagnostics"]["componentHandler"]["totalEvents"],
        3
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
    assert_eq!(
        runtime_snapshot_value["result"]["dataPlane"]["sharedMemory"]["attached"],
        false
    );
    assert_eq!(
        runtime_snapshot_value["result"]["dataPlane"]["sharedMemoryPump"]["running"],
        false
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
        set_state_and_refresh_value["result"]["metadata"]["state"]["controllerStateBase64"],
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

    let shared_memory_request = serde_json::json!({
        "id": 75,
        "method": "stream.sharedMemory.create",
        "params": { "instanceId": instance_id, "capacityBlocks": 3 }
    })
    .to_string();
    let shared_memory_value = request_json(&shared_memory_request, context).await;
    assert_eq!(shared_memory_value["result"]["schemaVersion"], 1);
    assert_eq!(
        shared_memory_value["result"]["transport"],
        "file-backed-mmap"
    );
    assert_eq!(shared_memory_value["result"]["instanceId"], instance_id);
    assert_eq!(shared_memory_value["result"]["streamId"], stream_id);
    assert_eq!(
        shared_memory_value["result"]["layout"]["config"]["capacityBlocks"],
        3
    );
    assert_eq!(shared_memory_value["result"]["worker"]["attached"], true);
    assert_eq!(
        shared_memory_value["result"]["descriptorBytes"]
            .as_array()
            .expect("descriptor bytes")
            .len(),
        usize::from(wvst_shm_transport::SHARED_AUDIO_DESCRIPTOR_BYTES)
    );
    let first_shared_memory_path = PathBuf::from(
        shared_memory_value["result"]["path"]
            .as_str()
            .expect("shared memory path"),
    );
    assert!(first_shared_memory_path.exists());

    let shared_memory_status_request = serde_json::json!({
        "id": 79,
        "method": "stream.sharedMemory.status",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let shared_memory_status_value = request_json(&shared_memory_status_request, context).await;
    assert_eq!(shared_memory_status_value["result"]["attached"], true);
    assert_eq!(
        shared_memory_status_value["result"]["status"]["descriptor"]["streamId"],
        stream_id
    );
    assert_eq!(
        shared_memory_status_value["result"]["status"]["pathExists"],
        true
    );
    assert_eq!(
        shared_memory_status_value["result"]["status"]["input"]["cursor"]["readFrame"],
        0
    );
    assert_eq!(
        shared_memory_status_value["result"]["status"]["input"]["cursor"]["writeFrame"],
        0
    );
    assert_eq!(
        shared_memory_status_value["result"]["status"]["input"]["readableFrames"],
        0
    );
    assert_eq!(
        shared_memory_status_value["result"]["status"]["input"]["writableFrames"],
        384
    );
    assert_eq!(
        shared_memory_status_value["result"]["status"]["output"]["readableFrames"],
        0
    );
    assert_eq!(
        shared_memory_status_value["result"]["status"]["output"]["writableFrames"],
        384
    );

    let destroy_shared_memory_request = serde_json::json!({
        "id": 76,
        "method": "stream.sharedMemory.destroy",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let destroy_shared_memory_value = request_json(&destroy_shared_memory_request, context).await;
    assert_eq!(destroy_shared_memory_value["result"]["destroyed"], true);
    assert_eq!(destroy_shared_memory_value["result"]["worker"]["ok"], true);
    assert_eq!(
        destroy_shared_memory_value["result"]["worker"]["result"]["detached"],
        true
    );
    assert!(!first_shared_memory_path.exists());

    let detached_shared_memory_status_value =
        request_json(&shared_memory_status_request, context).await;
    assert_eq!(
        detached_shared_memory_status_value["result"]["attached"],
        false
    );
    assert_eq!(
        detached_shared_memory_status_value["result"]["status"],
        Value::Null
    );

    let shared_memory_again_value = request_json(&shared_memory_request, context).await;
    let shared_memory_path = PathBuf::from(
        shared_memory_again_value["result"]["path"]
            .as_str()
            .expect("shared memory path"),
    );
    assert!(shared_memory_path.exists());

    let start_request = serde_json::json!({
        "id": 6,
        "method": "instance.start",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let start_value = request_json(&start_request, context).await;
    assert_eq!(start_value["result"]["instance"]["state"], "processing");
    assert_eq!(start_value["result"]["worker"]["workerState"], "processing");

    let process_shared_memory_request = serde_json::json!({
        "id": 77,
        "method": "stream.sharedMemory.process",
        "params": {
            "instanceId": instance_id,
            "frames": 2,
            "midiEvents": [{
                "sampleOffset": 1,
                "kind": 1,
                "channel": 0,
                "data1": 60,
                "data2": 100
            }],
            "parameterEvents": [{
                "sampleOffset": 1,
                "parameterId": 42,
                "valueNormalized": 0.5
            }]
        }
    })
    .to_string();
    let process_shared_memory_value = request_json(&process_shared_memory_request, context).await;
    assert_eq!(process_shared_memory_value["result"]["frames"], 2);
    assert_eq!(
        process_shared_memory_value["result"]["transport"],
        "file-backed-mmap"
    );
    assert_eq!(
        process_shared_memory_value["result"]["inputEvents"]["midiEvents"],
        1
    );
    assert_eq!(
        process_shared_memory_value["result"]["inputEvents"]["parameterEvents"],
        1
    );
    let shared_memory_metrics_value = request_json(
        r#"{"id":78,"method":"bridge.metrics","params":{}}"#,
        context,
    )
    .await;
    assert_eq!(
        shared_memory_metrics_value["result"]["sharedMemoryProcessBlocks"],
        1
    );
    assert_eq!(
        shared_memory_metrics_value["result"]["sharedMemoryProcessFrames"],
        2
    );
    assert_eq!(
        shared_memory_metrics_value["result"]["sharedMemoryProcessFailures"],
        0
    );
    assert_eq!(
        shared_memory_metrics_value["result"]["sharedMemoryPumpOverruns"],
        0
    );
    assert_eq!(
        shared_memory_metrics_value["result"]["sharedMemoryProcessLatency"]["count"],
        1
    );

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
    assert!(!shared_memory_path.exists());
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
async fn shared_memory_pump_runs_until_stopped() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let worker_path = serve_worker_script();
    let workers = Arc::new(WorkerSupervisor::new_for_test(
        worker_path.clone(),
        Duration::from_secs(5),
    ));
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
        workers: &workers,
    };

    let create = request_json(&instance_create_request(1, &plugin_id), context).await;
    let instance_id = create["result"]["instanceId"]
        .as_u64()
        .expect("instance id");
    let shared_memory_create = serde_json::json!({
        "id": 2,
        "method": "stream.sharedMemory.create",
        "params": { "instanceId": instance_id, "capacityBlocks": 2 }
    })
    .to_string();
    let shared_memory_value = request_json(&shared_memory_create, context).await;
    assert_eq!(shared_memory_value["result"]["worker"]["attached"], true);

    let start = request_json(&instance_request(3, "instance.start", instance_id), context).await;
    assert_eq!(start["result"]["instance"]["state"], "processing");

    let pump_start = serde_json::json!({
        "id": 4,
        "method": "stream.sharedMemory.pump.start",
        "params": { "instanceId": instance_id, "frames": 2, "intervalMicros": 1_000 }
    })
    .to_string();
    let pump_start_value = request_json(&pump_start, context).await;
    assert_eq!(pump_start_value["result"]["started"], true);
    assert_eq!(pump_start_value["result"]["status"]["config"]["frames"], 2);
    assert_eq!(
        pump_start_value["result"]["status"]["config"]["maxQueuedEvents"],
        1024
    );
    assert_eq!(pump_start_value["result"]["status"]["overruns"], 0);
    assert_eq!(pump_start_value["result"]["status"]["inputUnderruns"], 0);
    assert_eq!(
        pump_start_value["result"]["status"]["outputBackpressure"],
        0
    );
    assert_eq!(pump_start_value["result"]["status"]["workerErrors"], 0);
    assert_eq!(pump_start_value["result"]["status"]["lastProcessMicros"], 0);
    assert_eq!(pump_start_value["result"]["status"]["maxProcessMicros"], 0);
    assert_eq!(
        pump_start_value["result"]["status"]["lastOverrunMicros"],
        Value::Null
    );
    assert_eq!(
        pump_start_value["result"]["status"]["lastOutcome"],
        Value::Null
    );

    let pump_status = wait_for_pump_success(instance_id, context).await;
    assert_eq!(pump_status["result"]["running"], true);
    assert!(
        pump_status["result"]["status"]["successes"]
            .as_u64()
            .expect("successes")
            >= 1
    );
    assert_eq!(pump_status["result"]["status"]["lastSuccessFrames"], 2);
    assert!(
        pump_status["result"]["status"]["lastProcessMicros"]
            .as_u64()
            .is_some()
    );
    assert!(
        pump_status["result"]["status"]["maxProcessMicros"]
            .as_u64()
            .is_some()
    );
    assert!(
        pump_status["result"]["status"]["overruns"]
            .as_u64()
            .is_some()
    );
    assert_eq!(pump_status["result"]["status"]["lastOutcome"], "success");

    let enqueue_events = serde_json::json!({
        "id": 5,
        "method": "stream.sharedMemory.pump.enqueueEvents",
        "params": {
            "instanceId": instance_id,
            "delayIterations": 1,
            "midiEvents": [
                {
                    "sampleOffset": 0,
                    "kind": 1,
                    "channel": 0,
                    "data1": 60,
                    "data2": 100,
                    "data3": 0,
                    "dataLength": 2,
                    "noteId": 11
                }
            ],
            "parameterEvents": [
                {
                    "sampleOffset": 0,
                    "parameterId": 42,
                    "valueNormalized": 0.5
                }
            ]
        }
    })
    .to_string();
    let enqueue_value = request_json(&enqueue_events, context).await;
    assert_eq!(enqueue_value["result"]["queuedEvents"], 2);
    assert_eq!(enqueue_value["result"]["status"]["pendingEvents"], 2);

    let drained_status = wait_for_pump_drained_events(instance_id, 2, context).await;
    assert!(
        drained_status["result"]["status"]["events"]["drainedEvents"]
            .as_u64()
            .expect("drained events")
            >= 2
    );

    let enqueue_future_events = serde_json::json!({
        "id": 51,
        "method": "stream.sharedMemory.pump.enqueueEvents",
        "params": {
            "instanceId": instance_id,
            "delayIterations": 10_000,
            "midiEvents": [{ "sampleOffset": 0, "kind": 1 }]
        }
    })
    .to_string();
    let enqueue_future_value = request_json(&enqueue_future_events, context).await;
    assert_eq!(enqueue_future_value["result"]["queuedEvents"], 1);

    let clear_events = serde_json::json!({
        "id": 52,
        "method": "stream.sharedMemory.pump.clearEvents",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let clear_value = request_json(&clear_events, context).await;
    assert_eq!(clear_value["result"]["clearedEvents"], 1);
    assert_eq!(clear_value["result"]["status"]["pendingEvents"], 0);

    let metrics_value =
        request_json(r#"{"id":6,"method":"bridge.metrics","params":{}}"#, context).await;
    assert!(
        metrics_value["result"]["sharedMemoryProcessBlocks"]
            .as_u64()
            .expect("process blocks")
            >= 1
    );
    assert!(
        metrics_value["result"]["sharedMemoryPumpOverruns"]
            .as_u64()
            .is_some()
    );
    assert!(
        metrics_value["result"]["sharedMemoryPumpInputUnderruns"]
            .as_u64()
            .is_some()
    );
    assert!(
        metrics_value["result"]["sharedMemoryPumpOutputBackpressure"]
            .as_u64()
            .is_some()
    );
    assert!(
        metrics_value["result"]["sharedMemoryPumpWorkerErrors"]
            .as_u64()
            .is_some()
    );
    assert!(
        metrics_value["result"]["sharedMemoryPumpEventsEnqueued"]
            .as_u64()
            .expect("pump events enqueued")
            >= 3
    );
    assert!(
        metrics_value["result"]["sharedMemoryPumpEventsDrained"]
            .as_u64()
            .expect("pump events drained")
            >= 2
    );
    assert!(
        metrics_value["result"]["sharedMemoryPumpEventsCleared"]
            .as_u64()
            .expect("pump events cleared")
            >= 1
    );

    let pump_stop = serde_json::json!({
        "id": 7,
        "method": "stream.sharedMemory.pump.stop",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let pump_stop_value = request_json(&pump_stop, context).await;
    assert_eq!(pump_stop_value["result"]["stopped"], true);
    assert_eq!(pump_stop_value["result"]["status"]["running"], false);

    let stopped_status = request_json(
        &serde_json::json!({
            "id": 8,
            "method": "stream.sharedMemory.pump.status",
            "params": { "instanceId": instance_id }
        })
        .to_string(),
        context,
    )
    .await;
    assert_eq!(stopped_status["result"]["running"], false);
    assert_eq!(stopped_status["result"]["status"], Value::Null);

    let _ = request_json(
        &instance_request(9, "instance.destroy", instance_id),
        context,
    )
    .await;
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
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let worker_path = serve_worker_exits_on_metrics_script();
    let workers = Arc::new(WorkerSupervisor::new_for_test(
        worker_path.clone(),
        Duration::from_secs(5),
    ));
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
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
    assert_eq!(status_value["error"]["code"], 5036);
    assert_eq!(status_value["error"]["data"]["kind"], "io");

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
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let worker_path = serve_worker_script();
    let workers = Arc::new(WorkerSupervisor::with_options(
        crate::worker_supervisor::WorkerSupervisorOptions::new(worker_path.clone())
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false)
            .with_max_instances(1),
    ));
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
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
async fn reclaims_closed_idle_instance_when_worker_limit_is_reached() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let component_handler_events = ComponentHandlerEventPublisher::new();
    let events = BridgeEventBus::new();
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let worker_path = serve_worker_script();
    let workers = Arc::new(WorkerSupervisor::with_options(
        crate::worker_supervisor::WorkerSupervisorOptions::new(worker_path.clone())
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false)
            .with_max_instances(1),
    ));
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
        workers: &workers,
    };

    let first = request_json(&instance_create_request(1, &plugin_id), context).await;
    assert!(first.get("error").is_none(), "{first}");
    let first_instance = first["result"]["instanceId"].as_u64().expect("first id");
    let first_stream = first["result"]["streamId"].as_u64().expect("first stream");

    let close = request_json(
        &instance_request(2, "stream.close", first_instance),
        context,
    )
    .await;
    assert_eq!(close["result"]["streamState"], "closed");

    let second = request_json(&instance_create_request(3, &plugin_id), context).await;
    assert!(second.get("error").is_none(), "{second}");
    let second_instance = second["result"]["instanceId"].as_u64().expect("second id");
    assert_ne!(second_instance, first_instance);
    assert_eq!(second["result"]["state"], "ready");
    assert_eq!(instances.list().len(), 1);
    assert!(instances.get(first_instance).is_err());
    assert!(instances.get(second_instance).is_ok());
    assert_eq!(metrics.snapshot().worker_failures, 0);

    let events_value =
        request_json(r#"{"id":5,"method":"bridge.events","params":{}}"#, context).await;
    let policy = worker_policy_decision(&events_value, "resource-limit", "reclaim", "idle-worker");
    assert_eq!(policy["instanceId"].as_u64(), Some(second_instance));
    assert_eq!(policy["pluginId"], plugin_id);
    assert_eq!(policy["data"]["kind"], "idle-worker-reclaimed");
    assert_eq!(policy["data"]["resource"], "worker-instances");
    assert_eq!(
        policy["data"]["reclaimedInstanceId"].as_u64(),
        Some(first_instance)
    );
    assert_eq!(
        policy["data"]["reclaimedStreamId"].as_u64(),
        Some(first_stream)
    );

    let destroyed = events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| {
            event["kind"]["type"] == "worker-destroyed"
                && event["kind"]["instanceId"].as_u64() == Some(first_instance)
        })
        .expect("worker destroyed event");
    assert_eq!(destroyed["kind"]["streamId"].as_u64(), Some(first_stream));

    let _ = request_json(
        &instance_request(4, "instance.destroy", second_instance),
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
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let worker_path = serve_worker_exits_on_metrics_script();
    let workers = Arc::new(WorkerSupervisor::new_for_test(
        worker_path.clone(),
        Duration::from_secs(5),
    ));
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
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
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let worker_path = serve_worker_rejects_recreate_after_metrics_exit_script();
    let workers = Arc::new(WorkerSupervisor::new_for_test(
        worker_path.clone(),
        Duration::from_secs(5),
    ));
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
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
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let worker_path = serve_worker_exits_on_metrics_script();
    let workers = Arc::new(WorkerSupervisor::new_for_test(
        worker_path.clone(),
        Duration::from_secs(5),
    ));
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
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
    assert_eq!(failing_status_value["error"]["code"], 5036);

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
        "vst3-runtime"
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
    let metrics = Arc::new(BridgeMetrics::new());
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let plugin = plugins.find(&plugin_id).expect("plugin");
    let stream_tracker = AudioStreamTracker::new();
    let audio_in_flight = AudioInFlightLimiter::new();
    let shared_memory = Arc::new(SharedMemoryStreamRegistry::new());
    let shared_memory_pumps = SharedMemoryPumpRegistry::default();
    let workers = Arc::new(WorkerSupervisor::new_for_test(
        PathBuf::from("missing-wvst-host-worker"),
        Duration::from_secs(5),
    ));
    let record = instances
        .create(
            InstanceCreateParams {
                plugin_id,
                class_id: Some("class-a".to_string()),
                sample_rate: 48_000,
                max_block_frames: 128,
                input_channels: 2,
                output_channels: 2,
                input_bus_index: None,
                output_bus_index: None,
            },
            &plugin,
        )
        .expect("instance");
    let shared_memory_descriptor = shared_memory
        .create_for_instance(&record, Some(2))
        .expect("shared memory");
    let shared_memory_path = PathBuf::from(&shared_memory_descriptor.path);
    assert!(shared_memory_path.exists());
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
        shared_memory: &shared_memory,
        shared_memory_pumps: &shared_memory_pumps,
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
    assert!(!shared_memory_path.exists());
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
            shared_memory: context.shared_memory,
            shared_memory_pumps: context.shared_memory_pumps,
            session_authorized: true,
            workers: context.workers,
        },
    )
    .await;

    serde_json::from_str(&response.text).expect("valid control json")
}

async fn wait_for_pump_success(instance_id: u64, context: RequestContext<'_>) -> Value {
    let status_request = serde_json::json!({
        "id": 99,
        "method": "stream.sharedMemory.pump.status",
        "params": { "instanceId": instance_id }
    })
    .to_string();

    let mut last_status = Value::Null;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        last_status = request_json(&status_request, context).await;
        if last_status["result"]["status"]["successes"]
            .as_u64()
            .unwrap_or(0)
            > 0
        {
            return last_status;
        }
    }

    last_status
}

async fn wait_for_pump_drained_events(
    instance_id: u64,
    drained_events: u64,
    context: RequestContext<'_>,
) -> Value {
    let status_request = serde_json::json!({
        "id": 100,
        "method": "stream.sharedMemory.pump.status",
        "params": { "instanceId": instance_id }
    })
    .to_string();

    let mut last_status = Value::Null;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        last_status = request_json(&status_request, context).await;
        if last_status["result"]["status"]["events"]["drainedEvents"]
            .as_u64()
            .unwrap_or(0)
            >= drained_events
        {
            return last_status;
        }
    }

    last_status
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
    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("serve-worker.py");
    let source = format!("{FRAMED_WORKER_PREAMBLE}{BASE_WORKER_FIXTURE}");
    write_framed_worker_script(&worker, &source);
    worker
}

#[cfg(unix)]
fn serve_worker_exits_on_metrics_script() -> PathBuf {
    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("serve-worker-exits.py");
    let source = format!("{FRAMED_WORKER_PREAMBLE}{EXIT_ON_METRICS_WORKER_FIXTURE}");
    write_framed_worker_script(&worker, &source);
    worker
}

#[cfg(unix)]
fn serve_worker_rejects_recreate_after_metrics_exit_script() -> PathBuf {
    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let marker = directory.join("created-once");
    let worker = directory.join("serve-worker-rejects-recreate.py");
    let source = format!("{FRAMED_WORKER_PREAMBLE}{REJECT_RECREATE_WORKER_FIXTURE}")
        .replace("__MARKER__", &marker.to_string_lossy());
    write_framed_worker_script(&worker, &source);
    worker
}

#[cfg(unix)]
fn write_framed_worker_script(path: &std::path::Path, source: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, source).expect("script");
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("permissions");
}

#[cfg(unix)]
const FRAMED_WORKER_PREAMBLE: &str = r#"#!/usr/bin/env python3
import json
import os
import struct
import sys

MAGIC = int.from_bytes(b"WVCI", "little")
VERSION = 1
HEADER_LEN = 24
KIND_REQUEST = 1
KIND_RESPONSE = 2
KIND_ERROR = 3
KIND_BATCH_REQUEST = 4
KIND_BATCH_RESPONSE = 5
MAX_BODY = 16 * 1024 * 1024

def read_exact(size):
    data = sys.stdin.buffer.read(size)
    if not data:
        return None
    while len(data) < size:
        chunk = sys.stdin.buffer.read(size - len(data))
        if not chunk:
            return None
        data += chunk
    return data

def pack_frame(kind, status, sequence, body):
    return struct.pack("<IHHHHIQ", MAGIC, VERSION, HEADER_LEN, kind, status, len(body), sequence) + body

def ok(request, result):
    body = json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}, separators=(",", ":")).encode()
    return KIND_RESPONSE, 0, body, False

def err(request, code, message, data=None):
    payload = {"jsonrpc": "2.0", "id": request.get("id"), "error": {"code": code, "message": message}}
    if data is not None:
        payload["error"]["data"] = data
    body = json.dumps(payload, separators=(",", ":")).encode()
    return KIND_ERROR, code & 0xFFFF, body, False

def hello(request):
    return ok(request, {
        "workerName": "test-worker",
        "ipcVersion": 1,
        "capabilities": {
            "instanceLifecycle": True,
            "binaryAudioProcess": True,
            "framedControlIpc": True,
            "framedControlIpcVersion": 1,
            "framedControlMaxBodyBytes": MAX_BODY,
            "framedControlSequenceIds": True,
            "framedControlStatusCodes": True,
            "framedControlErrorResponses": True,
            "framedControlBatching": True,
        },
    })

def ready_result(state="ready"):
    return {
        "instanceId": 1,
        "streamId": 1,
        "workerState": state,
        "backend": "vst3-runtime",
        "latencySamples": 0,
        "tailSamples": 0,
        "tailInfo": {"samples": 0, "kind": "none", "finiteSamples": 0},
    }

def run():
    while True:
        header = read_exact(HEADER_LEN)
        if header is None:
            return
        magic, version, header_len, kind, status, body_len, sequence = struct.unpack("<IHHHHIQ", header)
        body = read_exact(body_len)
        if body is None:
            return
        if kind == KIND_BATCH_REQUEST:
            output = bytearray()
            offset = 0
            while offset < len(body):
                child_header = body[offset:offset + HEADER_LEN]
                cmagic, cversion, cheader_len, ckind, cstatus, cbody_len, csequence = struct.unpack("<IHHHHIQ", child_header)
                offset += HEADER_LEN
                child_body = body[offset:offset + cbody_len]
                offset += cbody_len
                request = json.loads(child_body.decode())
                rkind, rstatus, rbody, should_exit = handle(request)
                output.extend(pack_frame(rkind, rstatus, csequence, rbody))
                if should_exit:
                    sys.stdout.buffer.write(pack_frame(KIND_BATCH_RESPONSE, 0, sequence, bytes(output)))
                    sys.stdout.buffer.flush()
                    return
            sys.stdout.buffer.write(pack_frame(KIND_BATCH_RESPONSE, 0, sequence, bytes(output)))
            sys.stdout.buffer.flush()
            continue
        request = json.loads(body.decode())
        rkind, rstatus, rbody, should_exit = handle(request)
        sys.stdout.buffer.write(pack_frame(rkind, rstatus, sequence, rbody))
        sys.stdout.buffer.flush()
        if should_exit:
            return

"#;

#[cfg(unix)]
const BASE_WORKER_FIXTURE: &str = r#"
RUNTIME_CAPABILITIES = {
    "schemaVersion": 2,
    "binaryAudioProcess": True,
    "componentState": False,
    "controller": True,
    "controllerState": True,
    "parameters": True,
    "parameterAutomation": True,
    "units": False,
    "unitProgramData": False,
    "programListData": False,
    "unitData": False,
    "midiMapping": False,
    "outputEvents": True,
    "outputParameterChanges": True,
    "componentHandlerEvents": True,
    "connectionPoints": True,
    "processContext": False,
    "unavailable": [
        {"capability": "component-state", "reason": "interface-unavailable", "message": "component state interface unavailable", "hint": "Use controller state or reload the plugin if component state is required."},
        {"capability": "unit-data", "reason": "interface-unavailable", "hint": "Use component/controller state when the plugin does not expose IUnitData."},
    ],
}

def create_ready():
    result = ready_result()
    result["runtimeCapabilities"] = RUNTIME_CAPABILITIES
    return result

def metrics_result():
    return {
        "ipcVersion": 1,
        "instances": 1,
        "runtime": [{
            "streamId": 1,
            "backend": "vst3-runtime",
            "runtimeCapabilities": RUNTIME_CAPABILITIES,
            "latencySamples": 0,
            "tailSamples": 0,
            "tailInfo": {"samples": 0, "kind": "none", "finiteSamples": 0},
            "diagnostics": {
                "componentHandler": {
                    "totalEvents": 3,
                    "recentEvents": [
                        {"sequence": 1, "kind": "begin-edit", "parameterId": 42},
                        {"sequence": 2, "kind": "perform-edit", "parameterId": 42, "valueNormalized": 0.75},
                        {"sequence": 3, "kind": "restart-component", "flags": 24, "restartFlags": {"raw": 24, "reloadComponent": False, "ioChanged": False, "paramValuesChanged": False, "latencyChanged": True, "paramTitlesChanged": True, "midiCcAssignmentChanged": False, "noteExpressionChanged": False, "ioTitlesChanged": False, "prefetchableSupportChanged": False, "routingInfoChanged": False, "keyswitchChanged": False, "paramIdMappingChanged": False, "unknownBits": 0}},
                    ],
                },
            },
        }],
    }

def handle(request):
    method = request.get("method", "")
    params = request.get("params") or {}
    if method == "worker.hello":
        return hello(request)
    if method == "instance.create":
        return ok(request, create_ready())
    if method == "stream.sharedMemory.attach":
        return ok(request, {"instanceId": 1, "streamId": 1, "attached": True, "sharedMemory": {"schemaVersion": 1, "transport": "file-backed-mmap"}})
    if method == "stream.sharedMemory.detach":
        return ok(request, {"instanceId": 1, "streamId": 1, "detached": True})
    if method == "stream.sharedMemory.process":
        has_events = bool(params.get("midiEvents"))
        return ok(request, {"instanceId": 1, "streamId": 1, "transport": "file-backed-mmap", "frames": 2, "inputEvents": {"midiEvents": 1 if has_events else 0, "parameterEvents": 1 if has_events else 0, "vst3InputEvents": 1 if has_events else 0, "vst3ParameterChanges": 1 if has_events else 0}})
    if method == "instance.parameters":
        return ok(request, {"instanceId": 1, "parameters": [{"id": 42, "title": "Gain", "shortTitle": "Gain", "units": "dB", "stepCount": 0, "defaultNormalizedValue": 0.5, "unitId": 0, "flags": {"raw": 1, "canAutomate": True, "readOnly": False, "wrapAround": False, "list": False, "hidden": False, "programChange": False, "bypass": False}}]})
    if method == "instance.units":
        return ok(request, {"instanceId": 1, "unitInfo": None})
    if method == "instance.parameter.info":
        return ok(request, {"instanceId": 1, "parameterId": 42, "valueNormalized": 0.25, "valuePlain": 25.0, "valueString": "25 dB"})
    if method == "instance.parameter.valueByString":
        return ok(request, {"instanceId": 1, "parameterId": 42, "valueNormalized": 0.5, "valuePlain": 50.0, "valueString": "50 dB"})
    if method == "instance.parameter.normalizedByPlain":
        return ok(request, {"instanceId": 1, "parameterId": 42, "valueNormalized": 0.75, "valuePlain": 75.0, "valueString": "75 dB"})
    if method == "instance.parameter.beginEdit":
        return ok(request, {"instanceId": 1, "parameterId": 42, "editKind": "begin-edit", "valueNormalized": None})
    if method == "instance.parameter.performEdit":
        return ok(request, {"instanceId": 1, "parameterId": 42, "editKind": "perform-edit", "valueNormalized": 0.66})
    if method == "instance.parameter.endEdit":
        return ok(request, {"instanceId": 1, "parameterId": 42, "editKind": "end-edit", "valueNormalized": None})
    if method == "instance.getState":
        return ok(request, {"instanceId": 1, "componentStateBase64": None, "controllerStateBase64": "AQID"})
    if method == "instance.setState":
        return ok(request, {"instanceId": 1, "componentStateBytes": None, "controllerStateBytes": 3})
    if method == "instance.setUnitProgramData":
        return ok(request, {"instanceId": 1, "listOrUnitId": 1, "programIndex": 2, "dataBytes": 3})
    if method == "instance.programData.set":
        return ok(request, {"instanceId": 1, "listId": 1, "programIndex": 2, "dataBytes": 3})
    if method == "instance.unitData.set":
        return ok(request, {"instanceId": 1, "unitId": 1, "dataBytes": 3})
    if method == "instance.connection.notifyComponent":
        return ok(request, {"instanceId": 1, "target": "component", "messageId": "TextMessage", "attributeCount": 2, "notified": True})
    if method == "instance.connection.notifyController":
        return ok(request, {"instanceId": 1, "target": "controller", "messageId": "TextMessage", "attributeCount": 0, "notified": True})
    if method == "instance.startProcessing":
        return ok(request, {"instanceId": 1, "streamId": 1, "workerState": "processing"})
    if method == "instance.stopProcessing":
        return ok(request, {"instanceId": 1, "streamId": 1, "workerState": "stopped"})
    if method == "worker.metrics":
        return ok(request, metrics_result())
    if method == "instance.destroy":
        return ok(request, {"instanceId": 1, "streamId": 1, "workerState": "destroyed"})
    return err(request, -32601, "unknown")

run()
"#;

#[cfg(unix)]
const EXIT_ON_METRICS_WORKER_FIXTURE: &str = r#"
def handle(request):
    method = request.get("method", "")
    if method == "worker.hello":
        return hello(request)
    if method == "instance.create":
        return ok(request, ready_result())
    if method == "instance.startProcessing":
        return ok(request, {"instanceId": 1, "streamId": 1, "workerState": "processing"})
    if method == "instance.stopProcessing":
        return ok(request, {"instanceId": 1, "streamId": 1, "workerState": "stopped"})
    if method == "worker.metrics":
        sys.exit(0)
    if method == "instance.destroy":
        return ok(request, {"instanceId": 1, "streamId": 1, "workerState": "destroyed"})
    return err(request, -32601, "unknown")

run()
"#;

#[cfg(unix)]
const REJECT_RECREATE_WORKER_FIXTURE: &str = r#"
MARKER = "__MARKER__"

def handle(request):
    method = request.get("method", "")
    if method == "worker.hello":
        return hello(request)
    if method == "instance.create":
        if os.path.exists(MARKER):
            return err(request, 4220, "component create failed", {"kind": "vst3-runtime-init", "stage": "component.create", "hostError": "factory-create-instance-failed", "message": "component create failed"})
        open(MARKER, "w").close()
        return ok(request, ready_result())
    if method == "instance.startProcessing":
        return ok(request, {"instanceId": 1, "streamId": 1, "workerState": "processing"})
    if method == "worker.metrics":
        sys.exit(0)
    return err(request, -32601, "unknown")

run()
"#;

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
