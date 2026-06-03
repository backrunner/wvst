use std::path::PathBuf;

use super::{LinuxOptions, MacosOptions, WindowsOptions};

#[test]
fn parses_macos_options() {
    let options = MacosOptions::parse(vec![
        "--bridge-server".into(),
        "/tmp/bridge".into(),
        "--host-worker".into(),
        "/tmp/worker".into(),
        "--out".into(),
        "/tmp/out".into(),
        "--install-prefix".into(),
        "/tmp/install".into(),
        "--launchd-label".into(),
        "top.backrunner.test".into(),
        "--bind-addr".into(),
        "127.0.0.1:4000".into(),
    ])
    .expect("options");

    assert_eq!(options.bridge_server, Some(PathBuf::from("/tmp/bridge")));
    assert_eq!(options.host_worker, Some(PathBuf::from("/tmp/worker")));
    assert_eq!(options.output_dir, PathBuf::from("/tmp/out"));
    assert_eq!(options.install_prefix, Some(PathBuf::from("/tmp/install")));
    assert_eq!(options.launchd_label, Some("top.backrunner.test".into()));
    assert_eq!(options.bind_addr, Some("127.0.0.1:4000".into()));
}

#[test]
fn defaults_macos_output_dir() {
    let options = MacosOptions::parse(Vec::new()).expect("options");

    assert_eq!(options.output_dir, PathBuf::from("target/wvst-package"));
}

#[test]
fn parses_linux_options() {
    let options = LinuxOptions::parse(vec![
        "--bridge-server".into(),
        "/tmp/bridge".into(),
        "--host-worker".into(),
        "/tmp/worker".into(),
        "--out".into(),
        "/tmp/out".into(),
        "--install-prefix".into(),
        "/tmp/install".into(),
        "--systemd-service".into(),
        "top.backrunner.test".into(),
        "--bind-addr".into(),
        "127.0.0.1:4000".into(),
    ])
    .expect("options");

    assert_eq!(options.bridge_server, Some(PathBuf::from("/tmp/bridge")));
    assert_eq!(options.host_worker, Some(PathBuf::from("/tmp/worker")));
    assert_eq!(options.output_dir, PathBuf::from("/tmp/out"));
    assert_eq!(options.install_prefix, Some(PathBuf::from("/tmp/install")));
    assert_eq!(options.systemd_service, Some("top.backrunner.test".into()));
    assert_eq!(options.bind_addr, Some("127.0.0.1:4000".into()));
}

#[test]
fn defaults_linux_output_dir() {
    let options = LinuxOptions::parse(Vec::new()).expect("options");

    assert_eq!(options.output_dir, PathBuf::from("target/wvst-package"));
}

#[test]
fn parses_windows_options() {
    let options = WindowsOptions::parse(vec![
        "--bridge-server".into(),
        "/tmp/bridge.exe".into(),
        "--host-worker".into(),
        "/tmp/worker.exe".into(),
        "--out".into(),
        "/tmp/out".into(),
        "--install-prefix".into(),
        "/tmp/install".into(),
        "--scheduled-task".into(),
        "WVSTBridgeTest".into(),
        "--bind-addr".into(),
        "127.0.0.1:4000".into(),
    ])
    .expect("options");

    assert_eq!(
        options.bridge_server,
        Some(PathBuf::from("/tmp/bridge.exe"))
    );
    assert_eq!(options.host_worker, Some(PathBuf::from("/tmp/worker.exe")));
    assert_eq!(options.output_dir, PathBuf::from("/tmp/out"));
    assert_eq!(options.install_prefix, Some(PathBuf::from("/tmp/install")));
    assert_eq!(options.scheduled_task, Some("WVSTBridgeTest".into()));
    assert_eq!(options.bind_addr, Some("127.0.0.1:4000".into()));
}

#[test]
fn defaults_windows_output_dir() {
    let options = WindowsOptions::parse(Vec::new()).expect("options");

    assert_eq!(options.output_dir, PathBuf::from("target/wvst-package"));
}
