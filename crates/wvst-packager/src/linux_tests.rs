use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

#[test]
fn builds_linux_bundle_with_systemd_and_scripts() {
    let root = unique_temp_dir();
    let bridge = root.join("source").join("wvst-bridge-server");
    let worker = root.join("source").join("wvst-host-worker");
    std::fs::create_dir_all(bridge.parent().expect("source dir")).expect("source dir");
    std::fs::write(&bridge, b"bridge").expect("bridge binary");
    std::fs::write(&worker, b"worker").expect("worker binary");

    let mut config = LinuxPackageConfig::new(&bridge, &worker, root.join("out"));
    config.install_prefix = root.join("install prefix");
    config.systemd_service_name = "top.backrunner.wvst.test".to_string();

    let manifest = build_linux_package(&config).expect("package");

    assert!(manifest.package_root.ends_with("wvst-linux"));
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
            .any(|path| path.ends_with("bin/wvst-bridge-launcher"))
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

    let unit = std::fs::read_to_string(
        manifest
            .package_root
            .join("systemd")
            .join("top.backrunner.wvst.test.service"),
    )
    .expect("systemd unit");
    assert!(unit.contains("[Service]"));
    assert!(unit.contains("ExecStart="));
    assert!(unit.contains("Restart=on-failure"));
    assert!(unit.contains("KillMode=control-group"));
    assert!(unit.contains("SyslogIdentifier=wvst-bridge"));
    assert!(unit.contains("StandardOutput=journal"));
    assert!(unit.contains("top.backrunner.wvst.test"));

    let install = std::fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("install-linux.sh"),
    )
    .expect("install script");
    assert!(install.contains("systemctl --user enable --now"));
    assert!(install.contains("openssl rand -hex 24"));
    assert!(install.contains("wvst-log-rotate"));

    let diagnose = std::fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("diagnose-linux.sh"),
    )
    .expect("diagnose script");
    assert!(diagnose.contains("wvst-bridge-server\" diagnose"));
    assert!(diagnose.contains("WVST_HOST_WORKER"));
    assert!(diagnose.contains("systemctl --user is-active"));
    assert!(diagnose.contains("journalctl --user -u"));

    let rotate = std::fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("rotate-logs-linux.sh"),
    )
    .expect("rotate script");
    assert!(rotate.contains("WVST_JOURNAL_VACUUM_SIZE"));
    assert!(rotate.contains("journalctl --user --vacuum-size"));
    assert!(rotate.contains("top.backrunner.wvst.test"));

    let verify = std::fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("verify-linux.sh"),
    )
    .expect("verify script");
    assert!(verify.contains("systemd-analyze verify"));
    assert!(verify.contains("wvst-bridge-server\" diagnose"));
    assert!(verify.contains("journalctl --user"));
    assert!(verify.contains("WVST_VERIFY_REPORT"));
    assert!(verify.contains("wvst-verify-report.json"));
    assert!(verify.contains("checksSkipped"));
    assert!(verify.contains("\"checks\": [\"bundle-files\""));
    assert!(verify.contains("\"bridge-diagnose\""));

    let readme = std::fs::read_to_string(manifest.package_root.join("README.md")).expect("readme");
    assert!(readme.contains("./scripts/verify-linux.sh"));
    assert!(readme.contains("wvst-package-manifest.json"));
    assert!(readme.contains("wvst-verify-report.json"));

    let evidence: serde_json::Value = serde_json::from_slice(
        &std::fs::read(manifest.package_root.join("wvst-package-manifest.json"))
            .expect("manifest json"),
    )
    .expect("manifest json");
    assert_eq!(evidence["schemaVersion"], 1);
    assert_eq!(evidence["platform"], "linux");
    assert_eq!(evidence["install"]["serviceKind"], "systemd-user-service");
    assert_eq!(
        evidence["verification"]["reportPath"],
        "wvst-verify-report.json"
    );
    assert_eq!(
        evidence["verification"]["checks"]
            .as_array()
            .expect("verification checks")
            .len(),
        6
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rejects_missing_binaries() {
    let root = unique_temp_dir();
    let config = LinuxPackageConfig::new(
        root.join("missing-bridge"),
        root.join("missing-worker"),
        root.join("out"),
    );

    let error = build_linux_package(&config).expect_err("missing binary");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let _ = std::fs::remove_dir_all(root);
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let nonce = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "wvst-linux-packager-{}-{nanos}-{nonce}",
        std::process::id()
    ))
}
