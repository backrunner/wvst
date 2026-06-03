use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn builds_windows_bundle_with_scheduled_task_scripts() {
    let root = unique_temp_dir();
    let bridge = root.join("source").join("wvst-bridge-server.exe");
    let worker = root.join("source").join("wvst-host-worker.exe");
    std::fs::create_dir_all(bridge.parent().expect("source dir")).expect("source dir");
    std::fs::write(&bridge, b"bridge").expect("bridge binary");
    std::fs::write(&worker, b"worker").expect("worker binary");

    let mut config = WindowsPackageConfig::new(&bridge, &worker, root.join("out"));
    config.install_prefix = root.join("Install Prefix");
    config.scheduled_task_name = "WVSTBridgeTest".to_string();

    let manifest = build_windows_package(&config).expect("package");

    assert!(manifest.package_root.ends_with("wvst-windows"));
    assert!(
        manifest
            .files
            .iter()
            .any(|path| path.ends_with("README.md"))
    );
    assert!(
        manifest
            .files
            .iter()
            .any(|path| path.ends_with("wvst-package-manifest.json"))
    );
    assert!(
        manifest
            .files
            .iter()
            .any(|path| path.ends_with("bin/wvst-bridge-launcher.ps1"))
    );

    let env = std::fs::read_to_string(
        manifest
            .package_root
            .join("config")
            .join("wvst.env.example"),
    )
    .expect("env example");
    assert!(env.contains("WVST_HOST_WORKER="));
    assert!(env.contains("WVST_TOKEN="));

    let install = std::fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("install-windows.ps1"),
    )
    .expect("install script");
    assert!(install.contains("New-ScheduledTaskAction"));
    assert!(install.contains("Register-ScheduledTask"));
    assert!(install.contains("WVST_TOKEN=$Token"));
    assert!(install.contains("wvst-log-rotate.ps1"));
    assert!(install.contains("$LogDir"));

    let launcher = std::fs::read_to_string(
        manifest
            .package_root
            .join("bin")
            .join("wvst-bridge-launcher.ps1"),
    )
    .expect("launcher");
    assert!(launcher.contains("WVST_HOST_WORKER"));
    assert!(launcher.contains("wvst-bridge-server.exe"));
    assert!(launcher.contains("bridge.log"));
    assert!(launcher.contains("2>> $StderrLog"));

    let diagnose = std::fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("diagnose-windows.ps1"),
    )
    .expect("diagnose script");
    assert!(diagnose.contains("wvst-bridge-server.exe') diagnose"));
    assert!(diagnose.contains("WVST_HOST_WORKER"));
    assert!(diagnose.contains("Get-ScheduledTask"));
    assert!(diagnose.contains("[Console]::Error.WriteLine"));
    assert!(diagnose.contains("WVST log directory"));
    assert!(!diagnose.contains("Write-Error"));

    let rotate = std::fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("rotate-logs-windows.ps1"),
    )
    .expect("rotate script");
    assert!(rotate.contains("WVST_LOG_MAX_BYTES"));
    assert!(rotate.contains("Copy-Item -Force"));
    assert!(rotate.contains("Clear-Content"));

    let verify = std::fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("verify-windows.ps1"),
    )
    .expect("verify script");
    assert!(verify.contains("WVST_STRICT_AUTHENTICODE_VERIFY"));
    assert!(verify.contains("Get-AuthenticodeSignature"));
    assert!(verify.contains("Register-ScheduledTask"));
    assert!(verify.contains("& $Bridge diagnose"));
    assert!(verify.contains("WVST_VERIFY_REPORT"));
    assert!(verify.contains("wvst-verify-report.json"));
    assert!(verify.contains("ConvertTo-Json"));
    assert!(verify.contains("checks = @('bundle-files'"));
    assert!(verify.contains("'bridge-diagnose'"));

    let readme = std::fs::read_to_string(manifest.package_root.join("README.md")).expect("readme");
    assert!(readme.contains(".\\scripts\\verify-windows.ps1"));
    assert!(readme.contains("wvst-package-manifest.json"));
    assert!(readme.contains("wvst-verify-report.json"));

    let evidence: serde_json::Value = serde_json::from_slice(
        &std::fs::read(manifest.package_root.join("wvst-package-manifest.json"))
            .expect("manifest json"),
    )
    .expect("manifest json");
    assert_eq!(evidence["schemaVersion"], 1);
    assert_eq!(evidence["platform"], "windows");
    assert_eq!(evidence["install"]["serviceKind"], "windows-scheduled-task");
    assert_eq!(
        evidence["verification"]["reportPath"],
        "wvst-verify-report.json"
    );
    assert_eq!(
        evidence["verification"]["checks"]
            .as_array()
            .expect("verification checks")
            .len(),
        5
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rejects_missing_binaries() {
    let root = unique_temp_dir();
    let config = WindowsPackageConfig::new(
        root.join("missing-bridge.exe"),
        root.join("missing-worker.exe"),
        root.join("out"),
    );

    let error = build_windows_package(&config).expect_err("missing binary");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let _ = std::fs::remove_dir_all(root);
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "wvst-windows-packager-{}-{nanos}",
        std::process::id()
    ))
}
