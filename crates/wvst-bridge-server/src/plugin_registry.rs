use std::path::PathBuf;
use std::sync::Mutex;

use wvst_scanner::{ScanReport, default_vst3_paths, scan_paths};

#[derive(Debug, Default)]
pub struct PluginRegistry {
    report: Mutex<ScanReport>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> ScanReport {
        self.report
            .lock()
            .map(|report| report.clone())
            .unwrap_or_default()
    }

    pub fn scan_default_paths(&self) -> ScanReport {
        self.scan_paths(default_vst3_paths())
    }

    pub fn scan_paths(&self, paths: Vec<PathBuf>) -> ScanReport {
        let report = scan_paths(&paths);

        if let Ok(mut current) = self.report.lock() {
            *current = report.clone();
        }

        report
    }
}
