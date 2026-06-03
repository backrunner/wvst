use serde::Serialize;

use super::report::{RuntimeProbeResult, u64_field};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeProcessOutputSummary {
    pub reported_cases: usize,
    pub output_event_cases: usize,
    pub output_parameter_change_cases: usize,
    pub advanced_payload_cases: usize,
    pub advanced_payload_problem_cases: usize,
    pub filtered_output_event_cases: usize,
    pub filtered_output_parameter_change_cases: usize,
    pub total_output_events: u64,
    pub total_output_parameter_changes: u64,
    pub total_raw_output_events: u64,
    pub total_normalized_output_events: u64,
    pub total_filtered_output_events: u64,
    pub total_advanced_output_events: u64,
    pub total_advanced_data_output_events: u64,
    pub total_advanced_note_expression_output_events: u64,
    pub total_advanced_chord_output_events: u64,
    pub total_advanced_scale_output_events: u64,
    pub total_advanced_payload_events: u64,
    pub total_advanced_payload_bytes: u64,
    pub total_advanced_raw_payload_events: u64,
    pub total_advanced_text_payload_events: u64,
    pub total_advanced_truncated_payload_events: u64,
    pub total_advanced_unavailable_payload_events: u64,
    pub total_advanced_invalid_text_payload_events: u64,
    pub total_raw_output_parameter_changes: u64,
    pub total_normalized_output_parameter_changes: u64,
    pub total_filtered_output_parameter_changes: u64,
    pub max_output_events: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_events_case: Option<String>,
    pub max_output_parameter_changes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_parameter_changes_case: Option<String>,
}

impl RuntimeProbeProcessOutputSummary {
    pub(crate) fn from_results(results: &[RuntimeProbeResult]) -> Self {
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
            let output_events = u64_field(process, "outputEvents");
            let output_parameter_changes = u64_field(process, "outputParameterChanges");
            summary.total_output_events = summary.total_output_events.saturating_add(output_events);
            summary.total_output_parameter_changes = summary
                .total_output_parameter_changes
                .saturating_add(output_parameter_changes);

            if output_events > 0 {
                summary.output_event_cases += 1;
            }
            if output_parameter_changes > 0 {
                summary.output_parameter_change_cases += 1;
            }
            if output_events > summary.max_output_events {
                summary.max_output_events = output_events;
                summary.max_output_events_case = Some(result.case_name.clone());
            }
            if output_parameter_changes > summary.max_output_parameter_changes {
                summary.max_output_parameter_changes = output_parameter_changes;
                summary.max_output_parameter_changes_case = Some(result.case_name.clone());
            }

            summary.observe_diagnostics(process);
        }

        summary
    }

    fn observe_diagnostics(&mut self, process: &serde_json::Value) {
        let Some(diagnostics) = process.get("diagnostics") else {
            return;
        };

        let raw_output_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "rawEvents"))
            .unwrap_or(0);
        let normalized_output_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "normalizedEvents"))
            .unwrap_or(0);
        let filtered_output_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "filteredEvents"))
            .unwrap_or(0);
        let advanced_output_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedEvents"))
            .unwrap_or(0);
        let advanced_data_output_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedDataEvents"))
            .unwrap_or(0);
        let advanced_note_expression_output_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedNoteExpressionEvents"))
            .unwrap_or(0);
        let advanced_chord_output_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedChordEvents"))
            .unwrap_or(0);
        let advanced_scale_output_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedScaleEvents"))
            .unwrap_or(0);
        let advanced_payload_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedPayloadEvents"))
            .unwrap_or(0);
        let advanced_payload_bytes = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedPayloadBytes"))
            .unwrap_or(0);
        let advanced_raw_payload_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedRawPayloadEvents"))
            .unwrap_or(0);
        let advanced_text_payload_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedTextPayloadEvents"))
            .unwrap_or(0);
        let advanced_truncated_payload_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedTruncatedPayloadEvents"))
            .unwrap_or(0);
        let advanced_unavailable_payload_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedUnavailablePayloadEvents"))
            .unwrap_or(0);
        let advanced_invalid_text_payload_events = diagnostics
            .get("outputEvents")
            .map(|events| u64_field(events, "advancedInvalidTextPayloadEvents"))
            .unwrap_or(0);
        let raw_output_parameter_changes = diagnostics
            .get("outputParameterChanges")
            .map(|changes| u64_field(changes, "rawPoints"))
            .unwrap_or(0);
        let normalized_output_parameter_changes = diagnostics
            .get("outputParameterChanges")
            .map(|changes| u64_field(changes, "normalizedPoints"))
            .unwrap_or(0);
        let filtered_output_parameter_changes = diagnostics
            .get("outputParameterChanges")
            .map(|changes| u64_field(changes, "filteredPoints"))
            .unwrap_or(0);

        self.total_raw_output_events = self
            .total_raw_output_events
            .saturating_add(raw_output_events);
        self.total_normalized_output_events = self
            .total_normalized_output_events
            .saturating_add(normalized_output_events);
        self.total_filtered_output_events = self
            .total_filtered_output_events
            .saturating_add(filtered_output_events);
        self.total_advanced_output_events = self
            .total_advanced_output_events
            .saturating_add(advanced_output_events);
        self.total_advanced_data_output_events = self
            .total_advanced_data_output_events
            .saturating_add(advanced_data_output_events);
        self.total_advanced_note_expression_output_events = self
            .total_advanced_note_expression_output_events
            .saturating_add(advanced_note_expression_output_events);
        self.total_advanced_chord_output_events = self
            .total_advanced_chord_output_events
            .saturating_add(advanced_chord_output_events);
        self.total_advanced_scale_output_events = self
            .total_advanced_scale_output_events
            .saturating_add(advanced_scale_output_events);
        self.total_advanced_payload_events = self
            .total_advanced_payload_events
            .saturating_add(advanced_payload_events);
        self.total_advanced_payload_bytes = self
            .total_advanced_payload_bytes
            .saturating_add(advanced_payload_bytes);
        self.total_advanced_raw_payload_events = self
            .total_advanced_raw_payload_events
            .saturating_add(advanced_raw_payload_events);
        self.total_advanced_text_payload_events = self
            .total_advanced_text_payload_events
            .saturating_add(advanced_text_payload_events);
        self.total_advanced_truncated_payload_events = self
            .total_advanced_truncated_payload_events
            .saturating_add(advanced_truncated_payload_events);
        self.total_advanced_unavailable_payload_events = self
            .total_advanced_unavailable_payload_events
            .saturating_add(advanced_unavailable_payload_events);
        self.total_advanced_invalid_text_payload_events = self
            .total_advanced_invalid_text_payload_events
            .saturating_add(advanced_invalid_text_payload_events);
        self.total_raw_output_parameter_changes = self
            .total_raw_output_parameter_changes
            .saturating_add(raw_output_parameter_changes);
        self.total_normalized_output_parameter_changes = self
            .total_normalized_output_parameter_changes
            .saturating_add(normalized_output_parameter_changes);
        self.total_filtered_output_parameter_changes = self
            .total_filtered_output_parameter_changes
            .saturating_add(filtered_output_parameter_changes);

        if advanced_payload_events > 0 {
            self.advanced_payload_cases += 1;
        }
        if advanced_truncated_payload_events > 0
            || advanced_unavailable_payload_events > 0
            || advanced_invalid_text_payload_events > 0
        {
            self.advanced_payload_problem_cases += 1;
        }
        if filtered_output_events > 0 {
            self.filtered_output_event_cases += 1;
        }
        if filtered_output_parameter_changes > 0 {
            self.filtered_output_parameter_change_cases += 1;
        }
    }
}
