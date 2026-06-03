use serde_json::Value;

use super::{expectations::RuntimeProbeExpectations, report::RuntimeProbeResult};

pub(super) fn evaluate_audio_bus_expectations(
    expectations: &RuntimeProbeExpectations,
    result: &RuntimeProbeResult,
    failures: &mut Vec<String>,
) {
    if !has_audio_bus_expectations(expectations) {
        return;
    }

    let Some(selected_audio_buses) = result
        .probe_report
        .as_ref()
        .and_then(|report| report.get("selectedAudioBuses"))
    else {
        failures.push("missing runtime-probe selected audio bus diagnostics".to_string());
        return;
    };

    if expectations.require_no_input_bus {
        match selected_audio_buses.get("input") {
            None | Some(Value::Null) => {}
            Some(input) => failures.push(format!(
                "expected no selected input audio bus, got index {}",
                optional_i64_field(input, "selectedIndex")
                    .map_or_else(|| "<missing>".to_string(), |index| index.to_string())
            )),
        }
    }

    evaluate_bus_selection(
        selected_audio_buses,
        failures,
        "input",
        expectations.expected_input_bus_index,
        expectations.expected_input_bus_channels,
    );
    evaluate_bus_selection(
        selected_audio_buses,
        failures,
        "output",
        expectations.expected_output_bus_index,
        expectations.expected_output_bus_channels,
    );
}

fn has_audio_bus_expectations(expectations: &RuntimeProbeExpectations) -> bool {
    expectations.require_no_input_bus
        || expectations.expected_input_bus_index.is_some()
        || expectations.expected_output_bus_index.is_some()
        || expectations.expected_input_bus_channels.is_some()
        || expectations.expected_output_bus_channels.is_some()
}

fn evaluate_bus_selection(
    selected_audio_buses: &Value,
    failures: &mut Vec<String>,
    direction: &'static str,
    expected_index: Option<i32>,
    expected_channels: Option<u16>,
) {
    if expected_index.is_none() && expected_channels.is_none() {
        return;
    }

    let Some(bus) = selected_audio_buses
        .get(direction)
        .filter(|value| value.is_object())
    else {
        failures.push(format!(
            "missing selected {direction} audio bus diagnostics"
        ));
        return;
    };

    if let Some(expected) = expected_index {
        match optional_i64_field(bus, "selectedIndex") {
            Some(actual) if actual == i64::from(expected) => {}
            Some(actual) => failures.push(format!(
                "expected selected {direction} audio bus index {expected}, got {actual}"
            )),
            None => failures.push(format!(
                "missing selected {direction} audio bus index diagnostics"
            )),
        }
    }

    if let Some(expected) = expected_channels {
        let actual = bus
            .get("selected")
            .and_then(|selected| selected.get("channelCount"))
            .and_then(Value::as_u64);
        match actual {
            Some(actual) if actual == u64::from(expected) => {}
            Some(actual) => failures.push(format!(
                "expected selected {direction} audio bus channels {expected}, got {actual}"
            )),
            None => failures.push(format!(
                "missing selected {direction} audio bus channel diagnostics"
            )),
        }
    }
}

fn optional_i64_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}
