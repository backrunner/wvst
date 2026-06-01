use super::*;
use std::path::PathBuf;
use std::time::Duration;

use crate::instance_registry::InstanceRegistry;
use crate::metrics::BridgeMetrics;
use crate::plugin_registry::PluginRegistry;
use crate::worker_supervisor::WorkerSupervisor;

#[tokio::test]
async fn responds_to_hello() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let metrics = BridgeMetrics::new();
    let plugins = PluginRegistry::new();
    let workers = test_workers();
    let context = ControlContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        metrics: &metrics,
        plugins: &plugins,
        origin: Some("http://localhost:5173"),
        session_authorized: false,
        workers: &workers,
    };

    let response = handle_control_text(
        r#"{"jsonrpc":"2.0","id":1,"method":"bridge.hello","params":{"clientName":"test","clientVersion":"0.1.0","protocolMin":{"major":1,"minor":0},"protocolMax":{"major":1,"minor":0},"audioFrameVersion":1}}"#,
        context,
    )
    .await;
    let value: Value = serde_json::from_str(&response.text).expect("valid json");

    assert!(response.session_authorized);
    assert_eq!(value["id"], 1);
    assert_eq!(value["result"]["bridgeName"], "wvst-bridge");
}

#[tokio::test]
async fn rejects_denied_origin() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let metrics = BridgeMetrics::new();
    let plugins = PluginRegistry::new();
    let workers = test_workers();
    let context = ControlContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        metrics: &metrics,
        plugins: &plugins,
        origin: Some("https://example.com"),
        session_authorized: false,
        workers: &workers,
    };

    let response = handle_control_text(
        r#"{"id":1,"method":"bridge.hello","params":{"clientName":"test","clientVersion":"0.1.0","protocolMin":{"major":1,"minor":0},"protocolMax":{"major":1,"minor":0},"audioFrameVersion":1}}"#,
        context,
    )
    .await;
    let value: Value = serde_json::from_str(&response.text).expect("valid json");

    assert!(!response.session_authorized);
    assert_eq!(value["error"]["code"], 4010);
}

#[tokio::test]
async fn rejects_plugin_list_before_hello() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let metrics = BridgeMetrics::new();
    let plugins = PluginRegistry::new();
    let workers = test_workers();
    let context = ControlContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        metrics: &metrics,
        plugins: &plugins,
        origin: None,
        session_authorized: false,
        workers: &workers,
    };

    let response =
        handle_control_text(r#"{"id":1,"method":"plugin.list","params":{}}"#, context).await;
    let value: Value = serde_json::from_str(&response.text).expect("valid json");

    assert_eq!(value["error"]["code"], 4012);
}

#[tokio::test]
async fn lists_cached_plugins_after_hello() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let host_worker = test_host_worker();
    let instances = InstanceRegistry::new();
    let metrics = BridgeMetrics::new();
    let plugins = PluginRegistry::new();
    let workers = test_workers();
    let context = ControlContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        metrics: &metrics,
        plugins: &plugins,
        origin: None,
        session_authorized: true,
        workers: &workers,
    };

    let response =
        handle_control_text(r#"{"id":1,"method":"plugin.list","params":{}}"#, context).await;
    let value: Value = serde_json::from_str(&response.text).expect("valid json");

    assert_eq!(
        value["result"]["plugins"]
            .as_array()
            .expect("plugins")
            .len(),
        0
    );
}

#[cfg(unix)]
#[tokio::test]
async fn routes_factory_info_to_host_worker() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let worker_path = factory_info_worker_script();
    let host_worker = HostWorkerClient::new_for_test(worker_path.clone(), Duration::from_secs(5));
    let instances = InstanceRegistry::new();
    let metrics = BridgeMetrics::new();
    let plugins = PluginRegistry::new();
    let workers = test_workers();
    let context = ControlContext {
        config: &config,
        host_worker: &host_worker,
        instances: &instances,
        metrics: &metrics,
        plugins: &plugins,
        origin: None,
        session_authorized: true,
        workers: &workers,
    };

    let response = handle_control_text(
        r#"{"id":1,"method":"plugin.factoryInfo","params":{"path":"/tmp/Fake.vst3"}}"#,
        context,
    )
    .await;
    let value: Value = serde_json::from_str(&response.text).expect("valid json");

    assert_eq!(value["result"]["vendor"], "WVST");
    assert_eq!(value["result"]["classes"][0]["name"], "Probe");

    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

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
    let create = handle_control_text(
        &create_request,
        ControlContext {
            config: &config,
            host_worker: &host_worker,
            instances: &instances,
            metrics: &metrics,
            plugins: &plugins,
            origin: None,
            session_authorized: true,
            workers: &workers,
        },
    )
    .await;
    let create_value: Value = serde_json::from_str(&create.text).expect("valid create json");
    assert!(create_value.get("error").is_none(), "{create_value}");
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");

    assert_eq!(create_value["result"]["state"], "ready");
    assert_eq!(create_value["result"]["workerState"], "ready");

    let list = handle_control_text(
        r#"{"id":2,"method":"instance.list","params":{}}"#,
        ControlContext {
            config: &config,
            host_worker: &host_worker,
            instances: &instances,
            metrics: &metrics,
            plugins: &plugins,
            origin: None,
            session_authorized: true,
            workers: &workers,
        },
    )
    .await;
    let list_value: Value = serde_json::from_str(&list.text).expect("valid list json");
    assert_eq!(list_value["result"].as_array().expect("instances").len(), 1);

    let destroy_request = serde_json::json!({
        "id": 3,
        "method": "instance.destroy",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let destroy = handle_control_text(
        &destroy_request,
        ControlContext {
            config: &config,
            host_worker: &host_worker,
            instances: &instances,
            metrics: &metrics,
            plugins: &plugins,
            origin: None,
            session_authorized: true,
            workers: &workers,
        },
    )
    .await;
    let destroy_value: Value = serde_json::from_str(&destroy.text).expect("valid destroy json");
    assert_eq!(destroy_value["result"]["state"], "destroyed");
    assert!(instances.list().is_empty());

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

fn test_host_worker() -> HostWorkerClient {
    HostWorkerClient::new_for_test(
        PathBuf::from("missing-wvst-host-worker"),
        Duration::from_secs(5),
    )
}

fn test_workers() -> WorkerSupervisor {
    WorkerSupervisor::new_for_test(
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
  case "$line" in
    *worker.hello*) printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"workerName":"test-worker","ipcVersion":1}}' ;;
    *instance.create*) printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"instanceId":1,"streamId":1,"workerState":"ready"}}' ;;
    *instance.destroy*) printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"instanceId":1,"streamId":1,"workerState":"destroyed"}}'; exit 0 ;;
    *) printf '%s\n' '{"jsonrpc":"2.0","id":0,"error":{"code":-32601,"message":"unknown"}}' ;;
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
fn factory_info_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("worker.sh");
    std::fs::write(
        &worker,
        "#!/bin/sh\nprintf '{\"vendor\":\"WVST\",\"url\":null,\"email\":null,\"flags\":0,\"classes\":[{\"classId\":\"00000000000000000000000000000000\",\"cardinality\":1,\"category\":\"Audio Module Class\",\"name\":\"Probe\"}]}\\n'\n",
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
    std::env::temp_dir().join(format!("wvst-control-test-{suffix}"))
}
