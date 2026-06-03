use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

#[test]
fn builds_macos_bundle_with_launchd_and_scripts() {
    let root = unique_temp_dir();
    let bridge = root.join("source").join("wvst-bridge-server");
    let worker = root.join("source").join("wvst-host-worker");
    fs::create_dir_all(bridge.parent().expect("source dir")).expect("source dir");
    fs::write(&bridge, b"bridge").expect("bridge binary");
    fs::write(&worker, b"worker").expect("worker binary");

    let mut config = MacosPackageConfig::new(&bridge, &worker, root.join("out"));
    config.install_prefix = root.join("install prefix");
    config.launchd_label = "top.backrunner.wvst.test".to_string();

    let manifest = build_macos_package(&config).expect("package");

    assert!(manifest.package_root.ends_with("wvst-macos"));
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

    let env = fs::read_to_string(
        manifest
            .package_root
            .join("config")
            .join("wvst.env.example"),
    )
    .expect("env example");
    assert!(env.contains("WVST_HOST_WORKER="));
    assert!(env.contains("WVST_TOKEN="));

    let plist = fs::read_to_string(
        manifest
            .package_root
            .join("launchd")
            .join("top.backrunner.wvst.test.plist"),
    )
    .expect("plist");
    assert!(plist.contains("<string>top.backrunner.wvst.test</string>"));
    assert!(plist.contains("WVST_CONFIG_FILE"));

    let install = fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("install-macos.sh"),
    )
    .expect("install script");
    assert!(install.contains("launchctl bootstrap"));
    assert!(install.contains("openssl rand -hex 24"));
    assert!(install.contains("wvst-log-rotate"));

    let diagnose = fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("diagnose-macos.sh"),
    )
    .expect("diagnose script");
    assert!(diagnose.contains("wvst-bridge-server\" diagnose"));
    assert!(diagnose.contains("WVST_HOST_WORKER"));
    assert!(diagnose.contains("launchctl print"));
    assert!(diagnose.contains("WVST log directory"));

    let rotate = fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("rotate-logs-macos.sh"),
    )
    .expect("rotate script");
    assert!(rotate.contains("WVST_LOG_MAX_BYTES"));
    assert!(rotate.contains("cp -p \"$file\" \"$file.1\""));
    assert!(rotate.contains(": > \"$file\""));

    let verify = fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("verify-macos.sh"),
    )
    .expect("verify script");
    assert!(verify.contains("WVST_STRICT_CODESIGN_VERIFY"));
    assert!(verify.contains("plutil -lint"));
    assert!(verify.contains("codesign --verify --strict"));
    assert!(verify.contains("spctl --assess --type execute"));
    assert!(verify.contains("wvst-bridge-server\" diagnose"));
    assert!(verify.contains("WVST_VERIFY_REPORT"));
    assert!(verify.contains("wvst-verify-report.json"));
    assert!(verify.contains("checksSkipped"));
    assert!(verify.contains("\"checks\": [\"bundle-files\""));
    assert!(verify.contains("\"bridge-diagnose\""));

    let sign = fs::read_to_string(
        manifest
            .package_root
            .join("scripts")
            .join("sign-notarize-macos.sh"),
    )
    .expect("sign script");
    assert!(sign.contains("WVST_CODESIGN_IDENTITY"));
    assert!(sign.contains("codesign --force --timestamp --options runtime"));
    assert!(sign.contains("xcrun notarytool submit"));
    assert!(sign.contains("xcrun stapler staple"));
    assert!(sign.contains("spctl --assess --type execute"));
    assert!(sign.contains("WVST_SKIP_NOTARIZATION"));

    let readme = fs::read_to_string(manifest.package_root.join("README.md")).expect("readme");
    assert!(readme.contains("./scripts/verify-macos.sh"));
    assert!(readme.contains("WVST_STRICT_CODESIGN_VERIFY=1"));
    assert!(readme.contains("wvst-package-manifest.json"));
    assert!(readme.contains("wvst-verify-report.json"));

    let evidence: serde_json::Value = serde_json::from_slice(
        &fs::read(manifest.package_root.join("wvst-package-manifest.json")).expect("manifest json"),
    )
    .expect("manifest json");
    assert_eq!(evidence["schemaVersion"], 1);
    assert_eq!(evidence["platform"], "macos");
    assert_eq!(evidence["install"]["serviceKind"], "launchd-user-agent");
    assert_eq!(
        evidence["verification"]["reportPath"],
        "wvst-verify-report.json"
    );
    assert_eq!(
        evidence["verification"]["checks"]
            .as_array()
            .expect("verification checks")
            .len(),
        8
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_missing_binaries() {
    let root = unique_temp_dir();
    let config = MacosPackageConfig::new(
        root.join("missing-bridge"),
        root.join("missing-worker"),
        root.join("out"),
    );

    let error = build_macos_package(&config).expect_err("missing binary");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    let _ = fs::remove_dir_all(root);
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let nonce = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "wvst-packager-{}-{nanos}-{nonce}",
        std::process::id()
    ))
}
