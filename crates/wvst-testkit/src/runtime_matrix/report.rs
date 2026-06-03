use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProbeStatus {
    #[default]
    Passed,
    Failed,
    LaunchFailed,
    ExpectationFailed,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeResult {
    pub case_name: String,
    pub plugin_path: PathBuf,
    pub class_id: String,
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeExpectations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_status: Option<RuntimeProbeStatus>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_non_zero_output: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_silent_output: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_non_finite_output_samples: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_clipped_output_samples: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_silent_output_blocks: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_output_rms_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_peak_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_note_response: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_note_to_audio_frames: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_note_to_audio_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_compatibility_category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_classification_category: Option<String>,
}

impl RuntimeProbeExpectations {
    pub const fn expected_status(mut self, status: RuntimeProbeStatus) -> Self {
        self.expected_status = Some(status);
        self
    }

    pub const fn require_non_zero_output(mut self) -> Self {
        self.require_non_zero_output = true;
        self
    }

    pub const fn require_silent_output(mut self) -> Self {
        self.require_silent_output = true;
        self
    }

    pub const fn max_non_finite_output_samples(mut self, samples: u64) -> Self {
        self.max_non_finite_output_samples = Some(samples);
        self
    }

    pub const fn max_clipped_output_samples(mut self, samples: u64) -> Self {
        self.max_clipped_output_samples = Some(samples);
        self
    }

    pub const fn max_silent_output_blocks(mut self, blocks: u64) -> Self {
        self.max_silent_output_blocks = Some(blocks);
        self
    }

    pub const fn min_output_rms_milli(mut self, rms_milli: u32) -> Self {
        self.min_output_rms_milli = Some(rms_milli);
        self
    }

    pub const fn max_output_peak_milli(mut self, peak_milli: u32) -> Self {
        self.max_output_peak_milli = Some(peak_milli);
        self
    }

    pub const fn require_note_response(mut self) -> Self {
        self.require_note_response = true;
        self
    }

    pub const fn max_note_to_audio_frames(mut self, frames: u64) -> Self {
        self.max_note_to_audio_frames = Some(frames);
        self
    }

    pub const fn max_note_to_audio_micros(mut self, micros: u64) -> Self {
        self.max_note_to_audio_micros = Some(micros);
        self
    }

    pub fn expected_compatibility_category(mut self, category: impl Into<String>) -> Self {
        self.expected_compatibility_category = Some(category.into());
        self
    }

    pub fn expected_classification_category(mut self, category: impl Into<String>) -> Self {
        self.expected_classification_category = Some(category.into());
        self
    }

    pub(crate) fn apply_to(&self, result: &mut RuntimeProbeResult) {
        let raw_status = result.status;
        let failures = self.evaluate(result, raw_status);
        result.probe_status = Some(raw_status);

        if failures.is_empty() {
            if self.expected_status == Some(raw_status) {
                result.status = RuntimeProbeStatus::Passed;
            }
            return;
        }

        result.status = RuntimeProbeStatus::ExpectationFailed;
        result.expectation_failures = failures;
    }

    fn evaluate(&self, result: &RuntimeProbeResult, raw_status: RuntimeProbeStatus) -> Vec<String> {
        let mut failures = Vec::new();

        if let Some(expected_status) = self.expected_status
            && raw_status != expected_status
        {
            failures.push(format!(
                "expected probe status {}, got {}",
                status_name(expected_status),
                status_name(raw_status)
            ));
        }

        self.evaluate_audio(result, &mut failures);
        self.evaluate_note_timing(result, &mut failures);
        self.evaluate_category(
            result,
            &mut failures,
            "compatibility",
            self.expected_compatibility_category.as_deref(),
        );
        self.evaluate_category(
            result,
            &mut failures,
            "classification",
            self.expected_classification_category.as_deref(),
        );

        failures
    }

    fn evaluate_audio(&self, result: &RuntimeProbeResult, failures: &mut Vec<String>) {
        if !self.has_audio_expectations() {
            return;
        }

        let Some(process) = result
            .probe_report
            .as_ref()
            .and_then(|report| report.get("process"))
        else {
            failures.push("missing runtime-probe process diagnostics".to_string());
            return;
        };

        let total_blocks = u64_field(process, "totalBlocks");
        let silent_blocks = u64_field(process, "silentOutputBlocks");
        let non_zero_blocks = u64_field(process, "nonZeroOutputBlocks");
        let non_finite_samples = u64_field(process, "nonFiniteOutputSamples");
        let clipped_samples = u64_field(process, "clippedOutputSamples");
        let peak = f64_field(process, "maxOutputPeak");
        let rms = f64_field(process, "outputRms");

        if self.require_non_zero_output && non_zero_blocks == 0 {
            failures.push("expected non-zero output, got only silent blocks".to_string());
        }
        if self.require_silent_output && total_blocks > 0 && silent_blocks != total_blocks {
            failures.push(format!(
                "expected silent output, got {non_zero_blocks} non-zero blocks"
            ));
        }
        if let Some(max) = self.max_non_finite_output_samples
            && non_finite_samples > max
        {
            failures.push(format!(
                "expected at most {max} non-finite output samples, got {non_finite_samples}"
            ));
        }
        if let Some(max) = self.max_clipped_output_samples
            && clipped_samples > max
        {
            failures.push(format!(
                "expected at most {max} clipped output samples, got {clipped_samples}"
            ));
        }
        if let Some(max) = self.max_silent_output_blocks
            && silent_blocks > max
        {
            failures.push(format!(
                "expected at most {max} silent output blocks, got {silent_blocks}"
            ));
        }
        if let Some(min_milli) = self.min_output_rms_milli {
            let min = f64::from(min_milli) / 1000.0;
            if rms < min {
                failures.push(format!("expected output RMS >= {min:.3}, got {rms:.6}"));
            }
        }
        if let Some(max_milli) = self.max_output_peak_milli {
            let max = f64::from(max_milli) / 1000.0;
            if peak > max {
                failures.push(format!("expected output peak <= {max:.3}, got {peak:.6}"));
            }
        }
    }

    fn evaluate_note_timing(&self, result: &RuntimeProbeResult, failures: &mut Vec<String>) {
        if !self.has_note_timing_expectations() {
            return;
        }

        let Some(note_timing) = result
            .probe_report
            .as_ref()
            .and_then(|report| report.get("process"))
            .and_then(|process| process.get("noteTiming"))
        else {
            failures.push("missing runtime-probe note timing diagnostics".to_string());
            return;
        };

        let note_present = bool_field(note_timing, "notePresent");
        let note_on_frame = optional_i64_field(note_timing, "noteOnAbsoluteFrame");
        let output_frame = optional_i64_field(note_timing, "firstNonZeroOutputAbsoluteFrame");
        let latency_frames =
            optional_i64_field(note_timing, "framesFromNoteOnToFirstNonZeroOutput");
        let latency_micros =
            optional_i64_field(note_timing, "microsFromNoteOnToFirstNonZeroOutput");

        if self.require_note_response {
            if !note_present {
                failures.push("expected note response, but probe did not send a note".to_string());
            }
            if note_on_frame.is_none() {
                failures.push("expected note response, but note-on timing is missing".to_string());
            }
            if output_frame.is_none() {
                failures.push(
                    "expected note response, but no non-zero output was observed".to_string(),
                );
            }
            if let Some(frames) = latency_frames
                && frames < 0
            {
                failures.push(format!(
                    "expected note response after note-on, got first output {frames} frames before note-on"
                ));
            }
        }

        if let Some(max) = self.max_note_to_audio_frames {
            match latency_frames {
                Some(frames) if frames >= 0 && frames as u64 <= max => {}
                Some(frames) => failures.push(format!(
                    "expected note-to-audio latency <= {max} frames, got {frames}"
                )),
                None => failures.push("missing note-to-audio frame latency".to_string()),
            }
        }

        if let Some(max) = self.max_note_to_audio_micros {
            match latency_micros {
                Some(micros) if micros >= 0 && micros as u64 <= max => {}
                Some(micros) => failures.push(format!(
                    "expected note-to-audio latency <= {max} us, got {micros}"
                )),
                None => failures.push("missing note-to-audio latency in microseconds".to_string()),
            }
        }
    }

    fn evaluate_category(
        &self,
        result: &RuntimeProbeResult,
        failures: &mut Vec<String>,
        diagnostic_key: &'static str,
        expected: Option<&str>,
    ) {
        let Some(expected) = expected else {
            return;
        };
        let categories = result
            .probe_report
            .as_ref()
            .map(|report| collect_diagnostic_categories(report, diagnostic_key))
            .unwrap_or_default();
        if categories.iter().any(|category| category == expected) {
            return;
        }
        failures.push(format!(
            "expected {diagnostic_key} category {expected:?}, got {:?}",
            categories
        ));
    }

    fn has_audio_expectations(&self) -> bool {
        self.require_non_zero_output
            || self.require_silent_output
            || self.max_non_finite_output_samples.is_some()
            || self.max_clipped_output_samples.is_some()
            || self.max_silent_output_blocks.is_some()
            || self.min_output_rms_milli.is_some()
            || self.max_output_peak_milli.is_some()
    }

    fn has_note_timing_expectations(&self) -> bool {
        self.require_note_response
            || self.max_note_to_audio_frames.is_some()
            || self.max_note_to_audio_micros.is_some()
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
    pub expectation_failed: usize,
    pub audio_health: RuntimeProbeAudioHealthSummary,
    pub diagnostics: RuntimeProbeDiagnosticsSummary,
}

impl RuntimeProbeMatrixReport {
    pub(crate) fn new(results: Vec<RuntimeProbeResult>) -> Self {
        let passed = results.iter().filter(|result| result.passed()).count();
        let failed = count_status(&results, RuntimeProbeStatus::Failed);
        let launch_failed = count_status(&results, RuntimeProbeStatus::LaunchFailed);
        let expectation_failed = count_status(&results, RuntimeProbeStatus::ExpectationFailed);
        let audio_health = RuntimeProbeAudioHealthSummary::from_results(&results);
        let diagnostics = RuntimeProbeDiagnosticsSummary::from_results(&results);

        Self {
            schema_version: RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION,
            results,
            passed,
            failed,
            launch_failed,
            expectation_failed,
            audio_health,
            diagnostics,
        }
    }

    pub const fn all_passed(&self) -> bool {
        self.failed == 0 && self.launch_failed == 0 && self.expectation_failed == 0
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

fn collect_diagnostic_categories(value: &Value, diagnostic_key: &'static str) -> Vec<String> {
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

fn status_name(status: RuntimeProbeStatus) -> &'static str {
    match status {
        RuntimeProbeStatus::Passed => "passed",
        RuntimeProbeStatus::Failed => "failed",
        RuntimeProbeStatus::LaunchFailed => "launch-failed",
        RuntimeProbeStatus::ExpectationFailed => "expectation-failed",
    }
}

fn u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn f64_field(value: &Value, field: &str) -> f64 {
    value.get(field).and_then(Value::as_f64).unwrap_or(0.0)
}

fn bool_field(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn optional_i64_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}

const fn is_false(value: &bool) -> bool {
    !*value
}
