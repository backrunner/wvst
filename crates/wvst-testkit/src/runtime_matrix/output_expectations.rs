use serde_json::Value;

use super::{
    expectations::RuntimeProbeExpectations,
    report::{RuntimeProbeResult, u64_field},
};

impl RuntimeProbeExpectations {
    pub const fn min_output_events(mut self, events: u64) -> Self {
        self.min_output_events = Some(events);
        self
    }

    pub const fn min_output_parameter_changes(mut self, changes: u64) -> Self {
        self.min_output_parameter_changes = Some(changes);
        self
    }

    pub const fn min_normalized_output_events(mut self, events: u64) -> Self {
        self.min_normalized_output_events = Some(events);
        self
    }

    pub const fn min_advanced_output_events(mut self, events: u64) -> Self {
        self.min_advanced_output_events = Some(events);
        self
    }

    pub const fn min_advanced_data_output_events(mut self, events: u64) -> Self {
        self.min_advanced_data_output_events = Some(events);
        self
    }

    pub const fn min_advanced_note_expression_output_events(mut self, events: u64) -> Self {
        self.min_advanced_note_expression_output_events = Some(events);
        self
    }

    pub const fn min_advanced_chord_output_events(mut self, events: u64) -> Self {
        self.min_advanced_chord_output_events = Some(events);
        self
    }

    pub const fn min_advanced_scale_output_events(mut self, events: u64) -> Self {
        self.min_advanced_scale_output_events = Some(events);
        self
    }

    pub const fn min_advanced_payload_events(mut self, events: u64) -> Self {
        self.min_advanced_payload_events = Some(events);
        self
    }

    pub const fn min_advanced_payload_bytes(mut self, bytes: u64) -> Self {
        self.min_advanced_payload_bytes = Some(bytes);
        self
    }

    pub const fn min_advanced_raw_payload_events(mut self, events: u64) -> Self {
        self.min_advanced_raw_payload_events = Some(events);
        self
    }

    pub const fn min_advanced_text_payload_events(mut self, events: u64) -> Self {
        self.min_advanced_text_payload_events = Some(events);
        self
    }

    pub const fn min_normalized_output_parameter_changes(mut self, changes: u64) -> Self {
        self.min_normalized_output_parameter_changes = Some(changes);
        self
    }

    pub const fn max_advanced_truncated_payload_events(mut self, events: u64) -> Self {
        self.max_advanced_truncated_payload_events = Some(events);
        self
    }

    pub const fn max_advanced_unavailable_payload_events(mut self, events: u64) -> Self {
        self.max_advanced_unavailable_payload_events = Some(events);
        self
    }

    pub const fn max_advanced_invalid_text_payload_events(mut self, events: u64) -> Self {
        self.max_advanced_invalid_text_payload_events = Some(events);
        self
    }

    pub const fn max_filtered_output_events(mut self, events: u64) -> Self {
        self.max_filtered_output_events = Some(events);
        self
    }

    pub const fn max_filtered_output_parameter_changes(mut self, changes: u64) -> Self {
        self.max_filtered_output_parameter_changes = Some(changes);
        self
    }
}

pub(super) fn evaluate_process_output_expectations(
    expectations: &RuntimeProbeExpectations,
    result: &RuntimeProbeResult,
    failures: &mut Vec<String>,
) {
    if !has_process_output_expectations(expectations) {
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

    push_min(
        failures,
        "output events",
        u64_field(process, "outputEvents"),
        expectations.min_output_events,
    );
    push_min(
        failures,
        "output parameter changes",
        u64_field(process, "outputParameterChanges"),
        expectations.min_output_parameter_changes,
    );

    evaluate_diagnostics(expectations, process, failures);
}

fn evaluate_diagnostics(
    expectations: &RuntimeProbeExpectations,
    process: &Value,
    failures: &mut Vec<String>,
) {
    let Some(diagnostics) = process.get("diagnostics") else {
        if has_diagnostic_expectations(expectations) {
            failures.push("missing runtime-probe process output diagnostics".to_string());
        }
        return;
    };

    let output_events = diagnostics.get("outputEvents");
    let output_parameter_changes = diagnostics.get("outputParameterChanges");

    push_min(
        failures,
        "normalized output events",
        output_events
            .map(|value| u64_field(value, "normalizedEvents"))
            .unwrap_or(0),
        expectations.min_normalized_output_events,
    );
    push_min(
        failures,
        "advanced output events",
        output_events
            .map(|value| u64_field(value, "advancedEvents"))
            .unwrap_or(0),
        expectations.min_advanced_output_events,
    );
    push_min(
        failures,
        "advanced data output events",
        output_events
            .map(|value| u64_field(value, "advancedDataEvents"))
            .unwrap_or(0),
        expectations.min_advanced_data_output_events,
    );
    push_min(
        failures,
        "advanced note-expression output events",
        output_events
            .map(|value| u64_field(value, "advancedNoteExpressionEvents"))
            .unwrap_or(0),
        expectations.min_advanced_note_expression_output_events,
    );
    push_min(
        failures,
        "advanced chord output events",
        output_events
            .map(|value| u64_field(value, "advancedChordEvents"))
            .unwrap_or(0),
        expectations.min_advanced_chord_output_events,
    );
    push_min(
        failures,
        "advanced scale output events",
        output_events
            .map(|value| u64_field(value, "advancedScaleEvents"))
            .unwrap_or(0),
        expectations.min_advanced_scale_output_events,
    );
    push_min(
        failures,
        "advanced output payload events",
        output_events
            .map(|value| u64_field(value, "advancedPayloadEvents"))
            .unwrap_or(0),
        expectations.min_advanced_payload_events,
    );
    push_min(
        failures,
        "advanced output payload bytes",
        output_events
            .map(|value| u64_field(value, "advancedPayloadBytes"))
            .unwrap_or(0),
        expectations.min_advanced_payload_bytes,
    );
    push_min(
        failures,
        "advanced raw output payload events",
        output_events
            .map(|value| u64_field(value, "advancedRawPayloadEvents"))
            .unwrap_or(0),
        expectations.min_advanced_raw_payload_events,
    );
    push_min(
        failures,
        "advanced text output payload events",
        output_events
            .map(|value| u64_field(value, "advancedTextPayloadEvents"))
            .unwrap_or(0),
        expectations.min_advanced_text_payload_events,
    );
    push_min(
        failures,
        "normalized output parameter changes",
        output_parameter_changes
            .map(|value| u64_field(value, "normalizedPoints"))
            .unwrap_or(0),
        expectations.min_normalized_output_parameter_changes,
    );
    push_max(
        failures,
        "filtered output events",
        output_events
            .map(|value| u64_field(value, "filteredEvents"))
            .unwrap_or(0),
        expectations.max_filtered_output_events,
    );
    push_max(
        failures,
        "advanced truncated output payload events",
        output_events
            .map(|value| u64_field(value, "advancedTruncatedPayloadEvents"))
            .unwrap_or(0),
        expectations.max_advanced_truncated_payload_events,
    );
    push_max(
        failures,
        "advanced unavailable output payload events",
        output_events
            .map(|value| u64_field(value, "advancedUnavailablePayloadEvents"))
            .unwrap_or(0),
        expectations.max_advanced_unavailable_payload_events,
    );
    push_max(
        failures,
        "advanced invalid-text output payload events",
        output_events
            .map(|value| u64_field(value, "advancedInvalidTextPayloadEvents"))
            .unwrap_or(0),
        expectations.max_advanced_invalid_text_payload_events,
    );
    push_max(
        failures,
        "filtered output parameter changes",
        output_parameter_changes
            .map(|value| u64_field(value, "filteredPoints"))
            .unwrap_or(0),
        expectations.max_filtered_output_parameter_changes,
    );
}

fn has_process_output_expectations(expectations: &RuntimeProbeExpectations) -> bool {
    expectations.min_output_events.is_some()
        || expectations.min_output_parameter_changes.is_some()
        || has_diagnostic_expectations(expectations)
}

fn has_diagnostic_expectations(expectations: &RuntimeProbeExpectations) -> bool {
    expectations.min_normalized_output_events.is_some()
        || expectations.min_advanced_output_events.is_some()
        || expectations.min_advanced_data_output_events.is_some()
        || expectations
            .min_advanced_note_expression_output_events
            .is_some()
        || expectations.min_advanced_chord_output_events.is_some()
        || expectations.min_advanced_scale_output_events.is_some()
        || expectations.min_advanced_payload_events.is_some()
        || expectations.min_advanced_payload_bytes.is_some()
        || expectations.min_advanced_raw_payload_events.is_some()
        || expectations.min_advanced_text_payload_events.is_some()
        || expectations
            .min_normalized_output_parameter_changes
            .is_some()
        || expectations.max_filtered_output_events.is_some()
        || expectations.max_advanced_truncated_payload_events.is_some()
        || expectations
            .max_advanced_unavailable_payload_events
            .is_some()
        || expectations
            .max_advanced_invalid_text_payload_events
            .is_some()
        || expectations.max_filtered_output_parameter_changes.is_some()
}

fn push_min(failures: &mut Vec<String>, label: &'static str, actual: u64, expected: Option<u64>) {
    if let Some(expected) = expected
        && actual < expected
    {
        failures.push(format!(
            "expected at least {expected} {label}, got {actual}"
        ));
    }
}

fn push_max(failures: &mut Vec<String>, label: &'static str, actual: u64, expected: Option<u64>) {
    if let Some(expected) = expected
        && actual > expected
    {
        failures.push(format!("expected at most {expected} {label}, got {actual}"));
    }
}
