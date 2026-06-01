use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::metadata::{PluginDescriptor, parse_vst3_bundle};

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub plugins: Vec<PluginDescriptor>,
    pub failures: Vec<ScanFailure>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanFailure {
    pub path: String,
    pub reason: String,
}

pub fn scan_paths(paths: &[PathBuf]) -> ScanReport {
    let mut report = ScanReport::default();

    for path in paths {
        scan_path(path, &mut report);
    }

    report.plugins.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.path.cmp(&right.path))
    });
    report
        .failures
        .sort_by(|left, right| left.path.cmp(&right.path));
    report
}

fn scan_path(path: &Path, report: &mut ScanReport) {
    if !path.exists() {
        report.failures.push(ScanFailure {
            path: path_to_string(path),
            reason: "path does not exist".to_string(),
        });
        return;
    }

    if is_vst3_bundle(path) {
        parse_bundle(path, report);
        return;
    }

    scan_directory(path, report);
}

fn scan_directory(path: &Path, report: &mut ScanReport) {
    let mut stack = vec![path.to_path_buf()];

    while let Some(directory) = stack.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                report.failures.push(ScanFailure {
                    path: path_to_string(&directory),
                    reason: error.to_string(),
                });
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if is_vst3_bundle(&path) {
                parse_bundle(&path, report);
            } else if path.is_dir() {
                stack.push(path);
            }
        }
    }
}

fn parse_bundle(path: &Path, report: &mut ScanReport) {
    match parse_vst3_bundle(path) {
        Ok(plugin) => report.plugins.push(plugin),
        Err(error) => report.failures.push(ScanFailure {
            path: path_to_string(path),
            reason: error.to_string(),
        }),
    }
}

fn is_vst3_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vst3"))
        && path.is_dir()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, remove_dir_all, write};

    #[test]
    fn scans_nested_vst3_bundles() {
        let root = test_root("scan");
        let bundle = root.join("nested").join("Echo.vst3");
        create_dir_all(bundle.join("Contents")).expect("bundle directory");
        write(
            bundle.join("Contents/moduleinfo.json"),
            r#"{"Name":"Echo","Classes":[{"Name":"Echo","Category":"Fx"}]}"#,
        )
        .expect("moduleinfo");

        let report = scan_paths(std::slice::from_ref(&root));
        assert_eq!(report.plugins.len(), 1);
        assert_eq!(report.plugins[0].name, "Echo");
        assert!(report.failures.is_empty());

        remove_dir_all(root).expect("cleanup");
    }

    fn test_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("wvst-scanner-{name}-{}", std::process::id()));
        let _ = remove_dir_all(&root);
        root
    }
}
