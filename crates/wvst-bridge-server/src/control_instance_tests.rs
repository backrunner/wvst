use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::audio_stream_tracker::AudioStreamTracker;
use crate::component_handler_events::ComponentHandlerEventPublisher;
use crate::events::BridgeEventBus;
use crate::host_worker::HostWorkerClient;
use crate::instance_registry::{InstanceRegistry, InstanceState, WorkerState};
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

    assert_eq!(create_value["result"]["state"], "ready");
    assert_eq!(create_value["result"]["workerState"], "ready");
    assert_eq!(create_value["result"]["streamState"], "open");
    assert_eq!(create_value["result"]["backend"], "passthrough");
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
        status_value["result"]["worker"]["runtime"][0]["runtimeCapabilities"]["parameters"],
        true
    );
    assert_eq!(
        status_value["result"]["worker"]["runtime"][0]["diagnostics"]["componentHandler"]["totalEvents"],
        2
    );

    let handler_events_value =
        request_json(r#"{"id":72,"method":"bridge.events","params":{}}"#, context).await;
    let handler_events = handler_events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["kind"]["type"] == "vst3-component-handler-event")
        .collect::<Vec<_>>();
    assert_eq!(handler_events.len(), 2);
    assert_eq!(handler_events[0]["kind"]["handlerKind"], "begin-edit");
    assert_eq!(handler_events[0]["kind"]["parameterId"], 42);
    assert_eq!(handler_events[1]["kind"]["handlerKind"], "perform-edit");
    assert_eq!(handler_events[1]["kind"]["valueNormalized"], 0.75);

    let second_status_value = request_json(&status_request, context).await;
    assert_eq!(
        second_status_value["result"]["worker"]["runtime"][0]["diagnostics"]["componentHandler"]["totalEvents"],
        2
    );
    let deduped_events_value =
        request_json(r#"{"id":73,"method":"bridge.events","params":{}}"#, context).await;
    let deduped_handler_events = deduped_events_value["result"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["kind"]["type"] == "vst3-component-handler-event")
        .count();
    assert_eq!(deduped_handler_events, 2);

    let parameters_request = serde_json::json!({
        "id": 31,
        "method": "instance.parameters",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let parameters_value = request_json(&parameters_request, context).await;
    assert_eq!(parameters_value["result"]["parameters"][0]["id"], 42);
    assert_eq!(parameters_value["result"]["parameters"][0]["title"], "Gain");

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
    assert!(event_types.contains(&"worker-processing"));
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
    let worker_path = serve_worker_script();
    let workers = WorkerSupervisor::with_options(
        crate::worker_supervisor::WorkerSupervisorOptions::new(worker_path.clone())
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false)
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

    let _ = request_json(
        &instance_request(5, "instance.destroy", instance_id),
        context,
    )
    .await;

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
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
            session_authorized: true,
            workers: context.workers,
        },
    )
    .await;

    serde_json::from_str(&response.text).expect("valid control json")
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
runtime_capabilities='"runtimeCapabilities":{"binaryAudioProcess":true,"componentState":false,"controller":true,"controllerState":true,"parameters":true,"parameterAutomation":true,"units":false,"unitProgramData":false,"programListData":false,"unitData":false,"midiMapping":false,"outputEvents":true,"outputParameterChanges":true,"componentHandlerEvents":true,"connectionPoints":false,"processContext":false}'
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"test-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true,"binaryAudioProcess":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready","backend":"passthrough",%s,"latencySamples":0,"tailSamples":0}}\n' "$id" "$runtime_capabilities" ;;
    *instance.parameters*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameters":[{"id":42,"title":"Gain","shortTitle":"Gain","units":"dB","stepCount":0,"defaultNormalizedValue":0.5,"unitId":0,"flags":{"raw":1,"canAutomate":true,"readOnly":false,"wrapAround":false,"list":false,"hidden":false,"programChange":false,"bypass":false}}]}}\n' "$id" ;;
    *instance.parameter.info*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"valueNormalized":0.25,"valuePlain":25.0,"valueString":"25 dB"}}\n' "$id" ;;
    *instance.parameter.valueByString*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"valueNormalized":0.5,"valuePlain":50.0,"valueString":"50 dB"}}\n' "$id" ;;
    *instance.parameter.normalizedByPlain*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"parameterId":42,"valueNormalized":0.75,"valuePlain":75.0,"valueString":"75 dB"}}\n' "$id" ;;
    *instance.startProcessing*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"processing"}}\n' "$id" ;;
    *instance.stopProcessing*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"stopped"}}\n' "$id" ;;
    *worker.metrics*) printf '{"jsonrpc":"2.0","id":%s,"result":{"ipcVersion":1,"instances":1,"runtime":[{"streamId":1,"backend":"passthrough",%s,"latencySamples":0,"tailSamples":0,"diagnostics":{"componentHandler":{"totalEvents":2,"recentEvents":[{"sequence":1,"kind":"begin-edit","parameterId":42},{"sequence":2,"kind":"perform-edit","parameterId":42,"valueNormalized":0.75}]}}}]}}\n' "$id" "$runtime_capabilities" ;;
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
