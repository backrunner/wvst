use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "wvst-release-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        let fixture = Self(path);
        fixture.write("Cargo.toml", "[workspace]\nmembers = [\"crates/wvst-core\"]\n[workspace.package]\nversion = \"0.1.0\"\n");
        fixture.write(
            "crates/wvst-core/Cargo.toml",
            "[package]\nname = \"wvst-core\"\nversion.workspace = true\n",
        );
        fixture.write("Cargo.lock", "version = 4\n[[package]]\nname = \"wvst-core\"\nversion = \"0.1.0\"\n[[package]]\nname = \"third-party\"\nversion = \"0.1.0\"\nsource = \"registry+test\"\n");
        for file in [
            "package.json",
            "packages/wvst-web/package.json",
            "packages/wvst-web-examples/package.json",
            "docs/package.json",
        ] {
            fixture.write(file, "{\"version\":\"0.1.0\",\"private\":true}");
        }
        fixture.write("package-lock.json", r#"{"version":"0.1.0","packages":{"":{"version":"0.1.0"},"docs":{"version":"0.1.0"},"packages/wvst-web":{"version":"0.1.0"},"packages/wvst-web-examples":{"version":"0.1.0"},"node_modules/third-party":{"version":"0.1.0"}}}"#);
        fixture.write("packages/wvst-web/src/version.ts", "");
        fixture.write(
            "CHANGELOG.md",
            "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- First release.\n",
        );
        sync(&fixture.0).unwrap();
        fixture
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.0.join(path)).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn detects_drift_and_tag_mismatch() {
    let f = Fixture::new();
    assert!(check(&f.0, None).is_ok());
    assert!(check(&f.0, Some("v0.2.0")).is_err());
    assert!(
        check(&f.0, Some("v0.1.0")).is_err(),
        "missing release notes"
    );
    f.write("docs/package.json", "{\"version\":\"0.0.0\"}");
    assert!(check(&f.0, None).unwrap_err().contains("docs/package.json"));
    sync(&f.0).unwrap();
    assert!(check(&f.0, None).is_ok());
}
#[test]
fn accepts_windows_line_endings_but_rejects_changed_sdk_version() {
    let f = Fixture::new();
    let path = "packages/wvst-web/src/version.ts";
    let crlf = f.read(path).replace('\n', "\r\n");
    f.write(path, &crlf);
    assert!(check(&f.0, None).is_ok());
    f.write(path, &crlf.replace("0.1.0", "0.2.0"));
    assert_eq!(
        check(&f.0, None).unwrap_err(),
        "generated SDK version drift"
    );
}

#[test]
fn prepares_prerelease_and_preserves_dependency_versions() {
    let f = Fixture::new();
    prepare(&f.0, "0.1.0-alpha.1", "2026-09-07").unwrap();
    assert!(check(&f.0, Some("v0.1.0-alpha.1")).is_ok());
    assert!(
        notes(&f.0, "0.1.0-alpha.1")
            .unwrap()
            .contains("First release.")
    );
    assert!(
        f.read("CHANGELOG.md")
            .contains("## [Unreleased]\n\n## [0.1.0-alpha.1]")
    );
    let lock: serde_json::Value = serde_json::from_str(&f.read("package-lock.json")).unwrap();
    assert_eq!(
        lock["packages"]["node_modules/third-party"]["version"],
        "0.1.0"
    );
    assert!(
        f.read("Cargo.lock")
            .contains("name = \"third-party\"\nversion = \"0.1.0\"")
    );
    assert!(
        f.read("packages/wvst-web/src/version.ts")
            .contains("0.1.0-alpha.1")
    );
}
#[test]
fn rejects_invalid_versions_dates_and_empty_notes_without_writes() {
    let f = Fixture::new();
    let before = f.read("Cargo.toml");
    for (version, date) in [
        ("v0.1.0", "2026-09-07"),
        ("0.1.0+build", "2026-09-07"),
        ("0.1.0", "2026-02-30"),
        ("0.1.0", "2026-13-01"),
    ] {
        assert!(prepare(&f.0, version, date).is_err());
        assert_eq!(before, f.read("Cargo.toml"));
    }
    f.write("CHANGELOG.md", "# Changelog\n\n## [Unreleased]\n");
    assert!(prepare(&f.0, "0.1.0-alpha.1", "2026-09-07").is_err());
    assert_eq!(before, f.read("Cargo.toml"));
}
#[test]
fn releases_cannot_reuse_or_regress_recorded_versions() {
    let f = Fixture::new();
    prepare(&f.0, "0.1.0-alpha.1", "2026-09-07").unwrap();
    let text = f
        .read("CHANGELOG.md")
        .replace("## [Unreleased]\n", "## [Unreleased]\n\n- Next change.\n");
    f.write("CHANGELOG.md", &text);
    assert!(prepare(&f.0, "0.1.0-alpha.1", "2026-09-07").is_err());
    assert!(prepare(&f.0, "0.0.9", "2026-09-07").is_err());
    prepare(&f.0, "0.1.0-alpha.2", "2026-09-08").unwrap();
    assert!(
        notes(&f.0, "0.1.0-alpha.2")
            .unwrap()
            .contains("Next change.")
    );
    assert!(
        !notes(&f.0, "0.1.0-alpha.2")
            .unwrap()
            .contains("First release.")
    );
}
#[test]
fn checksums_require_the_complete_versioned_artifact_set() {
    let f = Fixture::new();
    let directory = f.0.join("artifacts");
    fs::create_dir(&directory).unwrap();
    let commit = "a".repeat(40);
    assert!(checksums(&f.0, &directory, &commit).is_err());
    for target in release_targets() {
        let ext = if target.contains("windows") {
            "zip"
        } else if target.contains("apple") {
            "dmg"
        } else {
            "tar.gz"
        };
        fs::write(
            directory.join(format!("wvst-0.1.0-{target}.{ext}")),
            b"test archive",
        )
        .unwrap();
    }
    fs::write(directory.join("wvst-web-0.1.0.tgz"), b"sdk archive").unwrap();
    checksums(&f.0, &directory, &commit).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(directory.join("release-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["artifacts"].as_array().unwrap().len(), 5);
    assert_eq!(manifest["commit"], commit);
    let before = fs::read_to_string(directory.join("SHA256SUMS")).unwrap();
    assert_eq!(before.lines().count(), 6);
    assert!(
        before
            .lines()
            .all(|line| line.split_whitespace().next().unwrap().len() == 64)
    );
    checksums(&f.0, &directory, &commit).unwrap();
    assert_eq!(
        before,
        fs::read_to_string(directory.join("SHA256SUMS")).unwrap()
    );
    fs::write(directory.join("unexpected.zip"), b"extra").unwrap();
    assert!(checksums(&f.0, &directory, &commit).is_err());
}
#[test]
fn bundle_rejects_unsupported_targets_and_invalid_commits() {
    let f = Fixture::new();
    assert!(
        bundle(
            &f.0,
            "unknown",
            Path::new("missing"),
            Path::new("out"),
            &"a".repeat(40)
        )
        .is_err()
    );
    assert!(
        bundle(
            &f.0,
            "aarch64-apple-darwin",
            Path::new("missing"),
            Path::new("out"),
            "not-a-commit"
        )
        .is_err()
    );
}
