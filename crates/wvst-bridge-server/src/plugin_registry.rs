use std::path::PathBuf;
use std::sync::Mutex;

use wvst_scanner::{PluginDescriptor, ScanReport, default_vst3_paths, scan_paths};

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

    pub fn find(&self, plugin_id: &str) -> Option<PluginDescriptor> {
        self.report.lock().ok().and_then(|report| {
            report
                .plugins
                .iter()
                .find(|plugin| plugin.plugin_id == plugin_id)
                .cloned()
        })
    }

    pub fn scan_default_paths(&self) -> ScanReport {
        self.scan_paths(default_vst3_paths())
    }

    pub fn default_paths() -> Vec<PathBuf> {
        default_vst3_paths()
    }

    pub fn scan_paths_report(paths: Vec<PathBuf>) -> ScanReport {
        scan_paths(&paths)
    }

    pub fn replace_report(&self, report: ScanReport) {
        if let Ok(mut current) = self.report.lock() {
            *current = report;
        }
    }

    pub fn scan_paths(&self, paths: Vec<PathBuf>) -> ScanReport {
        let report = Self::scan_paths_report(paths);
        self.replace_report(report.clone());

        report
    }
}
