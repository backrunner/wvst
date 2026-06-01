use super::*;
use std::path::PathBuf;
use std::time::Duration;

use crate::host_worker::HostWorkerClient;
use crate::instance_registry::{InstanceRegistry, InstanceState, WorkerState};
use crate::metrics::BridgeMetrics;
use crate::plugin_registry::PluginRegistry;
use crate::worker_supervisor::WorkerSupervisor;

#[cfg(unix)]
#[tokio::test]
async fn creates_lists_and_destroys_instance() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let worker_path = serve_worker_script();
    let workers = WorkerSupervisor::new_for_test(worker_path.clone(), Duration::from_secs(5));

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
    let create_value = request_json(
        &create_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    assert!(create_value.get("error").is_none(), "{create_value}");
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");

    assert_eq!(create_value["result"]["state"], "ready");
    assert_eq!(create_value["result"]["workerState"], "ready");

    let list_value = request_json(
        r#"{"id":2,"method":"instance.list","params":{}}"#,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    assert_eq!(list_value["result"].as_array().expect("instances").len(), 1);

    let status_request = serde_json::json!({
        "id": 3,
        "method": "instance.status",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let status_value = request_json(
        &status_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    assert_eq!(status_value["result"]["instance"]["workerState"], "ready");
    assert_eq!(status_value["result"]["worker"]["ipcVersion"], 1);
    assert_eq!(status_value["result"]["worker"]["instances"], 1);

    let destroy_request = serde_json::json!({
        "id": 4,
        "method": "instance.destroy",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let destroy_value = request_json(
        &destroy_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    assert_eq!(destroy_value["result"]["state"], "destroyed");
    assert!(instances.list().is_empty());

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn marks_instance_failed_when_heartbeat_worker_exits() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let worker_path = serve_worker_exits_on_metrics_script();
    let workers = WorkerSupervisor::new_for_test(worker_path.clone(), Duration::from_secs(5));

    let create_request = instance_create_request(1, &plugin_id);
    let create_value = request_json(
        &create_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    assert!(create_value.get("error").is_none(), "{create_value}");
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");

    let status_request = serde_json::json!({
        "id": 2,
        "method": "instance.status",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let status_value = request_json(
        &status_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
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
async fn restarts_failed_instance_with_same_stream() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let metrics = BridgeMetrics::new();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let worker_path = serve_worker_exits_on_metrics_script();
    let workers = WorkerSupervisor::new_for_test(worker_path.clone(), Duration::from_secs(5));

    let create_request = instance_create_request(1, &plugin_id);
    let create_value = request_json(
        &create_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");
    let stream_id = create_value["result"]["streamId"]
        .as_u64()
        .expect("stream id");

    let failing_status_request = serde_json::json!({
        "id": 2,
        "method": "instance.status",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let failing_status_value = request_json(
        &failing_status_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    assert_eq!(failing_status_value["error"]["code"], 5037);

    let restart_request = serde_json::json!({
        "id": 3,
        "method": "instance.restart",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let restart_value = request_json(
        &restart_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    assert_eq!(
        restart_value["result"]["instance"]["instanceId"],
        instance_id
    );
    assert_eq!(restart_value["result"]["instance"]["streamId"], stream_id);
    assert_eq!(restart_value["result"]["instance"]["state"], "ready");
    assert_eq!(restart_value["result"]["worker"]["workerState"], "ready");

    let metrics_value = request_json(
        r#"{"id":4,"method":"bridge.metrics","params":{}}"#,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;
    assert_eq!(metrics_value["result"]["workerFailures"], 1);
    assert_eq!(metrics_value["result"]["workerRestarts"], 1);

    let destroy_request = serde_json::json!({
        "id": 5,
        "method": "instance.destroy",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let _ = request_json(
        &destroy_request,
        &config,
        &host_worker,
        &instances,
        &metrics,
        &plugins,
        &workers,
    )
    .await;

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

async fn request_json(
    text: &str,
    config: &BridgeConfig,
    host_worker: &HostWorkerClient,
    instances: &InstanceRegistry,
    metrics: &BridgeMetrics,
    plugins: &PluginRegistry,
    workers: &WorkerSupervisor,
) -> Value {
    let response = handle_control_text(
        text,
        ControlContext {
            config,
            host_worker,
            instances,
            metrics,
            plugins,
            origin: None,
            session_authorized: true,
            workers,
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

#[cfg(unix)]
fn serve_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("serve-worker.sh");
    std::fs::write(
        &worker,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"test-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true,"binaryAudioProcess":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready"}}\n' "$id" ;;
    *worker.metrics*) printf '{"jsonrpc":"2.0","id":%s,"result":{"ipcVersion":1,"instances":1}}\n' "$id" ;;
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
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready"}}\n' "$id" ;;
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
    std::env::temp_dir().join(format!("wvst-control-instance-test-{suffix}"))
}
