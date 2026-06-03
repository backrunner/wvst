use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::runtime_matrix_manifest::RuntimeProbeEvidenceRequirements;

use super::{
    RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION, RuntimeProbeCaseEvidence, RuntimeProbePluginKind,
    audio_bus_summary::RuntimeProbeAudioBusSummary, coverage_audit::RuntimeProbeCoverageAudit,
    note_timing::RuntimeProbeNoteTimingHealthSummary,
    process_output_summary::RuntimeProbeProcessOutputSummary,
    process_timing_summary::RuntimeProbeProcessTimingSummary,
    runtime_characteristics_summary::RuntimeProbeRuntimeCharacteristicsSummary,
};

#[path = "report/controller.rs"]
mod controller;

pub use controller::RuntimeProbeControllerHealthSummary;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProbeStatus {
    #[default]
    Passed,
    Failed,
    LaunchFailed,
    TimedOut,
    ExpectationFailed,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeResult {
    pub case_name: String,
    pub plugin_path: PathBuf,
    pub class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<RuntimeProbeCaseEvidence>,
    pub status: RuntimeProbeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_status: Option<RuntimeProbeStatus>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_report: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expectation_failures: Vec<String>,
    pub duration_millis: u64,
}

impl RuntimeProbeResult {
    pub const fn passed(&self) -> bool {
        matches!(self.status, RuntimeProbeStatus::Passed)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeMatrixReport {
    pub schema_version: u16,
    pub results: Vec<RuntimeProbeResult>,
    pub passed: usize,
    pub failed: usize,
    pub launch_failed: usize,
    pub timed_out: usize,
    pub expectation_failed: usize,
    pub audio_health: RuntimeProbeAudioHealthSummary,
    pub audio_buses: RuntimeProbeAudioBusSummary,
    pub note_timing: RuntimeProbeNoteTimingHealthSummary,
    pub process_output: RuntimeProbeProcessOutputSummary,
    pub process_timing: RuntimeProbeProcessTimingSummary,
    pub runtime_characteristics: RuntimeProbeRuntimeCharacteristicsSummary,
    pub controller_health: RuntimeProbeControllerHealthSummary,
    pub diagnostics: RuntimeProbeDiagnosticsSummary,
    pub evidence: RuntimeProbeEvidenceSummary,
    pub coverage_audit: RuntimeProbeCoverageAudit,
}

impl RuntimeProbeMatrixReport {
    #[cfg(test)]
    pub(crate) fn new(results: Vec<RuntimeProbeResult>) -> Self {
        Self::new_with_coverage_requirements(results, None)
    }

    pub(crate) fn new_with_coverage_requirements(
        results: Vec<RuntimeProbeResult>,
        coverage_requirements: Option<&RuntimeProbeEvidenceRequirements>,
    ) -> Self {
        let passed = results.iter().filter(|result| result.passed()).count();
        let failed = count_status(&results, RuntimeProbeStatus::Failed);
        let launch_failed = count_status(&results, RuntimeProbeStatus::LaunchFailed);
        let timed_out = count_status(&results, RuntimeProbeStatus::TimedOut);
        let expectation_failed = count_status(&results, RuntimeProbeStatus::ExpectationFailed);
        let audio_health = RuntimeProbeAudioHealthSummary::from_results(&results);
        let audio_buses = RuntimeProbeAudioBusSummary::from_results(&results);
        let note_timing = RuntimeProbeNoteTimingHealthSummary::from_results(&results);
        let process_output = RuntimeProbeProcessOutputSummary::from_results(&results);
        let process_timing = RuntimeProbeProcessTimingSummary::from_results(&results);
        let runtime_characteristics =
            RuntimeProbeRuntimeCharacteristicsSummary::from_results(&results);
        let controller_health = RuntimeProbeControllerHealthSummary::from_results(&results);
        let diagnostics = RuntimeProbeDiagnosticsSummary::from_results(&results);
        let evidence = RuntimeProbeEvidenceSummary::from_results(&results);
        let coverage_audit =
            RuntimeProbeCoverageAudit::from_results(&results, coverage_requirements);

        Self {
            schema_version: RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION,
            results,
            passed,
            failed,
            launch_failed,
            timed_out,
            expectation_failed,
            audio_health,
            audio_buses,
            note_timing,
            process_output,
            process_timing,
            runtime_characteristics,
            controller_health,
            diagnostics,
            evidence,
            coverage_audit,
        }
    }

    pub const fn all_passed(&self) -> bool {
        self.failed == 0
            && self.launch_failed == 0
            && self.timed_out == 0
            && self.expectation_failed == 0
            && self.coverage_audit.passed
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeEvidenceSummary {
    pub reported_cases: usize,
    pub missing_evidence_cases: usize,
    pub third_party_cases: usize,
    pub effect_cases: usize,
    pub instrument_cases: usize,
    pub hybrid_cases: usize,
    pub unknown_kind_cases: usize,
    pub missing_vendor_cases: usize,
    pub missing_plugin_name_cases: usize,
    pub vendors: BTreeMap<String, usize>,
    pub tags: BTreeMap<String, usize>,
}

impl RuntimeProbeEvidenceSummary {
    fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(evidence) = &result.evidence else {
                summary.missing_evidence_cases += 1;
                continue;
            };

            summary.reported_cases += 1;
            if evidence.third_party {
                summary.third_party_cases += 1;
            }
            match evidence.plugin_kind {
                RuntimeProbePluginKind::Effect => summary.effect_cases += 1,
                RuntimeProbePluginKind::Instrument => summary.instrument_cases += 1,
                RuntimeProbePluginKind::Hybrid => summary.hybrid_cases += 1,
                RuntimeProbePluginKind::Unknown => summary.unknown_kind_cases += 1,
            }

            if let Some(vendor) = non_empty(&evidence.vendor) {
                increment(&mut summary.vendors, vendor.to_string());
            } else {
                summary.missing_vendor_cases += 1;
            }
            if non_empty(&evidence.plugin_name).is_none() {
                summary.missing_plugin_name_cases += 1;
            }
            for tag in &evidence.tags {
                let tag = tag.trim();
                if !tag.is_empty() {
                    increment(&mut summary.tags, tag.to_string());
                }
            }
        }

        summary
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeAudioHealthSummary {
    pub reported_cases: usize,
    pub fully_silent_cases: usize,
    pub non_zero_cases: usize,
    pub non_finite_cases: usize,
    pub clipped_cases: usize,
    pub total_non_finite_output_samples: u64,
    pub total_clipped_output_samples: u64,
    pub total_silent_output_blocks: u64,
    pub max_output_peak: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_peak_case: Option<String>,
    pub max_output_rms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_rms_case: Option<String>,
}

impl RuntimeProbeAudioHealthSummary {
    fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(process) = result
                .probe_report
                .as_ref()
                .and_then(|report| report.get("process"))
            else {
                continue;
            };

            summary.reported_cases += 1;
            let total_blocks = u64_field(process, "totalBlocks");
            let silent_blocks = u64_field(process, "silentOutputBlocks");
            let non_zero_blocks = u64_field(process, "nonZeroOutputBlocks");
            let non_finite_samples = u64_field(process, "nonFiniteOutputSamples");
            let clipped_samples = u64_field(process, "clippedOutputSamples");
            let peak = f64_field(process, "maxOutputPeak");
            let rms = f64_field(process, "outputRms");

            if total_blocks > 0 && silent_blocks == total_blocks {
                summary.fully_silent_cases += 1;
            }
            if non_zero_blocks > 0 {
                summary.non_zero_cases += 1;
            }
            if non_finite_samples > 0 {
                summary.non_finite_cases += 1;
            }
            if clipped_samples > 0 {
                summary.clipped_cases += 1;
            }
            summary.total_non_finite_output_samples = summary
                .total_non_finite_output_samples
                .saturating_add(non_finite_samples);
            summary.total_clipped_output_samples = summary
                .total_clipped_output_samples
                .saturating_add(clipped_samples);
            summary.total_silent_output_blocks = summary
                .total_silent_output_blocks
                .saturating_add(silent_blocks);

            if peak > summary.max_output_peak {
                summary.max_output_peak = peak;
                summary.max_output_peak_case = Some(result.case_name.clone());
            }
            if rms > summary.max_output_rms {
                summary.max_output_rms = rms;
                summary.max_output_rms_case = Some(result.case_name.clone());
            }
        }

        summary
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeDiagnosticsSummary {
    pub compatibility_categories: BTreeMap<String, usize>,
    pub classification_categories: BTreeMap<String, usize>,
    pub failure_kinds: BTreeMap<String, usize>,
}

impl RuntimeProbeDiagnosticsSummary {
    fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(report) = &result.probe_report else {
                continue;
            };
            for category in collect_diagnostic_categories(report, "compatibility") {
                increment(&mut summary.compatibility_categories, category);
            }
            for category in collect_diagnostic_categories(report, "classification") {
                increment(&mut summary.classification_categories, category);
            }
            if let Some(kind) = report
                .get("data")
                .and_then(|data| data.get("kind"))
                .and_then(Value::as_str)
            {
                increment(&mut summary.failure_kinds, kind.to_string());
            }
        }

        summary
    }
}

pub(crate) fn collect_diagnostic_categories(
    value: &Value,
    diagnostic_key: &'static str,
) -> Vec<String> {
    let mut categories = Vec::new();
    collect_diagnostic_categories_into(value, diagnostic_key, &mut categories);
    categories
}

fn collect_diagnostic_categories_into(
    value: &Value,
    diagnostic_key: &'static str,
    categories: &mut Vec<String>,
) {
    match value {
        Value::Object(object) => {
            if let Some(category) = object
                .get(diagnostic_key)
                .and_then(|diagnostic| diagnostic.get("category"))
                .and_then(Value::as_str)
            {
                categories.push(category.to_string());
            }
            for value in object.values() {
                collect_diagnostic_categories_into(value, diagnostic_key, categories);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_diagnostic_categories_into(value, diagnostic_key, categories);
            }
        }
        _ => {}
    }
}

fn count_status(results: &[RuntimeProbeResult], status: RuntimeProbeStatus) -> usize {
    results
        .iter()
        .filter(|result| result.status == status)
        .count()
}

fn increment(map: &mut BTreeMap<String, usize>, key: String) {
    *map.entry(key).or_default() += 1;
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

pub(crate) fn f64_field(value: &Value, field: &str) -> f64 {
    value.get(field).and_then(Value::as_f64).unwrap_or(0.0)
}

pub(crate) fn availability_field(value: &Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(|field| field.get("available"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
