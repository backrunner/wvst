use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    audio_bus_expectations::evaluate_audio_bus_expectations,
    controller_expectations::evaluate_controller_expectations,
    output_expectations::evaluate_process_output_expectations,
    process_timing_expectations::evaluate_process_timing_expectations,
    report::{
        RuntimeProbeResult, RuntimeProbeStatus, collect_diagnostic_categories, f64_field, u64_field,
    },
    runtime_characteristics_expectations::evaluate_runtime_characteristics_expectations,
};

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
    pub min_parameter_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_automatable_parameters: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_component_state: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_component_state_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_component_state_roundtrip: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_controller_state: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_controller_state_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_controller_state_roundtrip: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_controller_component_state_sync: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_controller_component_state_sync_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_unit_info: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_unit_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_program_list_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_total_programs: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_program_list_data: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_program_list_data_supported: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_unit_data: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_unit_data_supported: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_component_handler: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_component_handler_edit_probe: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_connection_points: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_connection_notify_probe: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_component_handler_events: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_no_input_bus: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_input_bus_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output_bus_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_input_bus_channels: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output_bus_channels: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_output_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_output_parameter_changes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_normalized_output_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_output_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_data_output_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_note_expression_output_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_chord_output_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_scale_output_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_payload_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_payload_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_raw_payload_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_advanced_text_payload_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_advanced_truncated_payload_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_advanced_unavailable_payload_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_advanced_invalid_text_payload_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_normalized_output_parameter_changes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_filtered_output_events: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_filtered_output_parameter_changes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_process_time_p50_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_process_time_p95_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_process_time_p99_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_process_time_max_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_latency_samples: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tail_samples: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_tail_info: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_tail_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_process_context_requirements: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_process_context_requirements: Option<u64>,
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

    pub const fn min_parameter_count(mut self, count: u64) -> Self {
        self.min_parameter_count = Some(count);
        self
    }

    pub const fn min_automatable_parameters(mut self, count: u64) -> Self {
        self.min_automatable_parameters = Some(count);
        self
    }

    pub const fn require_component_state(mut self) -> Self {
        self.require_component_state = true;
        self
    }

    pub const fn min_component_state_bytes(mut self, bytes: u64) -> Self {
        self.min_component_state_bytes = Some(bytes);
        self
    }

    pub const fn require_component_state_roundtrip(mut self) -> Self {
        self.require_component_state_roundtrip = true;
        self
    }

    pub const fn require_controller_state(mut self) -> Self {
        self.require_controller_state = true;
        self
    }

    pub const fn min_controller_state_bytes(mut self, bytes: u64) -> Self {
        self.min_controller_state_bytes = Some(bytes);
        self
    }

    pub const fn require_controller_state_roundtrip(mut self) -> Self {
        self.require_controller_state_roundtrip = true;
        self
    }

    pub const fn require_controller_component_state_sync(mut self) -> Self {
        self.require_controller_component_state_sync = true;
        self
    }

    pub const fn min_controller_component_state_sync_bytes(mut self, bytes: u64) -> Self {
        self.min_controller_component_state_sync_bytes = Some(bytes);
        self
    }

    pub const fn require_unit_info(mut self) -> Self {
        self.require_unit_info = true;
        self
    }

    pub const fn min_unit_count(mut self, count: u64) -> Self {
        self.min_unit_count = Some(count);
        self
    }

    pub const fn min_program_list_count(mut self, count: u64) -> Self {
        self.min_program_list_count = Some(count);
        self
    }

    pub const fn min_total_programs(mut self, count: u64) -> Self {
        self.min_total_programs = Some(count);
        self
    }

    pub const fn require_program_list_data(mut self) -> Self {
        self.require_program_list_data = true;
        self
    }

    pub const fn min_program_list_data_supported(mut self, count: u64) -> Self {
        self.min_program_list_data_supported = Some(count);
        self
    }

    pub const fn require_unit_data(mut self) -> Self {
        self.require_unit_data = true;
        self
    }

    pub const fn min_unit_data_supported(mut self, count: u64) -> Self {
        self.min_unit_data_supported = Some(count);
        self
    }

    pub const fn require_component_handler(mut self) -> Self {
        self.require_component_handler = true;
        self
    }

    pub const fn require_component_handler_edit_probe(mut self) -> Self {
        self.require_component_handler_edit_probe = true;
        self
    }

    pub const fn require_connection_points(mut self) -> Self {
        self.require_connection_points = true;
        self
    }

    pub const fn require_connection_notify_probe(mut self) -> Self {
        self.require_connection_notify_probe = true;
        self
    }

    pub const fn min_component_handler_events(mut self, count: u64) -> Self {
        self.min_component_handler_events = Some(count);
        self
    }

    pub const fn require_no_input_bus(mut self) -> Self {
        self.require_no_input_bus = true;
        self
    }

    pub const fn expected_input_bus_index(mut self, index: i32) -> Self {
        self.expected_input_bus_index = Some(index);
        self
    }

    pub const fn expected_output_bus_index(mut self, index: i32) -> Self {
        self.expected_output_bus_index = Some(index);
        self
    }

    pub const fn expected_input_bus_channels(mut self, channels: u16) -> Self {
        self.expected_input_bus_channels = Some(channels);
        self
    }

    pub const fn expected_output_bus_channels(mut self, channels: u16) -> Self {
        self.expected_output_bus_channels = Some(channels);
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
        evaluate_controller_expectations(self, result, &mut failures);
        evaluate_audio_bus_expectations(self, result, &mut failures);
        evaluate_process_output_expectations(self, result, &mut failures);
        evaluate_process_timing_expectations(self, result, &mut failures);
        evaluate_runtime_characteristics_expectations(self, result, &mut failures);
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

fn status_name(status: RuntimeProbeStatus) -> &'static str {
    match status {
        RuntimeProbeStatus::Passed => "passed",
        RuntimeProbeStatus::Failed => "failed",
        RuntimeProbeStatus::LaunchFailed => "launch-failed",
        RuntimeProbeStatus::TimedOut => "timed-out",
        RuntimeProbeStatus::ExpectationFailed => "expectation-failed",
    }
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
