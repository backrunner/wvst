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
