use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::report::RuntimeProbeResult;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeAudioBusSummary {
    pub reported_cases: usize,
    pub input_bus_cases: usize,
    pub no_input_bus_cases: usize,
    pub output_bus_cases: usize,
    pub missing_output_bus_cases: usize,
    pub input_channel_mismatch_cases: usize,
    pub output_channel_mismatch_cases: usize,
    pub requested_input_channels: BTreeMap<u16, usize>,
    pub requested_output_channels: BTreeMap<u16, usize>,
    pub selected_input_channels: BTreeMap<u16, usize>,
    pub selected_output_channels: BTreeMap<u16, usize>,
    pub selected_input_bus_indexes: BTreeMap<i32, usize>,
    pub selected_output_bus_indexes: BTreeMap<i32, usize>,
    pub selected_input_bus_types: BTreeMap<String, usize>,
    pub selected_output_bus_types: BTreeMap<String, usize>,
    pub max_available_input_buses: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_available_input_buses_case: Option<String>,
    pub max_available_output_buses: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_available_output_buses_case: Option<String>,
}

impl RuntimeProbeAudioBusSummary {
    pub(crate) fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(selected_audio_buses) = result
                .probe_report
                .as_ref()
                .and_then(|report| report.get("selectedAudioBuses"))
            else {
                continue;
            };

            summary.reported_cases += 1;
            summary.observe_input(result, selected_audio_buses);
            summary.observe_output(result, selected_audio_buses);
        }

        summary
    }

    fn observe_input(&mut self, result: &RuntimeProbeResult, selected_audio_buses: &Value) {
        match selected_audio_buses.get("input") {
            Some(Value::Null) => self.no_input_bus_cases += 1,
            Some(input) if input.is_object() => {
                self.input_bus_cases += 1;
                if observe_bus(
                    input,
                    BusSummaryTarget {
                        requested_channels: &mut self.requested_input_channels,
                        selected_channels: &mut self.selected_input_channels,
                        selected_indexes: &mut self.selected_input_bus_indexes,
                        selected_bus_types: &mut self.selected_input_bus_types,
                        max_available_buses: &mut self.max_available_input_buses,
                        max_available_buses_case: &mut self.max_available_input_buses_case,
                    },
                    &result.case_name,
                ) {
                    self.input_channel_mismatch_cases += 1;
                }
            }
            _ => {}
        }
    }

    fn observe_output(&mut self, result: &RuntimeProbeResult, selected_audio_buses: &Value) {
        match selected_audio_buses.get("output") {
            Some(output) if output.is_object() => {
                self.output_bus_cases += 1;
                if observe_bus(
                    output,
                    BusSummaryTarget {
                        requested_channels: &mut self.requested_output_channels,
                        selected_channels: &mut self.selected_output_channels,
                        selected_indexes: &mut self.selected_output_bus_indexes,
                        selected_bus_types: &mut self.selected_output_bus_types,
                        max_available_buses: &mut self.max_available_output_buses,
                        max_available_buses_case: &mut self.max_available_output_buses_case,
                    },
                    &result.case_name,
                ) {
                    self.output_channel_mismatch_cases += 1;
                }
            }
            _ => self.missing_output_bus_cases += 1,
        }
    }
}

struct BusSummaryTarget<'a> {
    requested_channels: &'a mut BTreeMap<u16, usize>,
    selected_channels: &'a mut BTreeMap<u16, usize>,
    selected_indexes: &'a mut BTreeMap<i32, usize>,
    selected_bus_types: &'a mut BTreeMap<String, usize>,
    max_available_buses: &'a mut usize,
    max_available_buses_case: &'a mut Option<String>,
}

fn observe_bus(bus: &Value, target: BusSummaryTarget<'_>, case_name: &str) -> bool {
    let requested = u16_field(bus, "requestedChannels");
    if let Some(requested) = requested {
        increment(target.requested_channels, requested);
    }

    let selected = bus.get("selected").filter(|value| value.is_object());
    let selected_channel_count = selected.and_then(|selected| u16_field(selected, "channelCount"));
    if let Some(channels) = selected_channel_count {
        increment(target.selected_channels, channels);
    }
    if let Some(index) = i32_field(bus, "selectedIndex") {
        increment(target.selected_indexes, index);
    }
    if let Some(bus_type) = selected
        .and_then(|selected| selected.get("busType"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|bus_type| !bus_type.is_empty())
    {
        increment(target.selected_bus_types, bus_type.to_string());
    }

    let available = bus
        .get("available")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if available > *target.max_available_buses {
        *target.max_available_buses = available;
        *target.max_available_buses_case = Some(case_name.to_string());
    }

    matches!(
        (requested, selected_channel_count),
        (Some(requested), Some(selected_channel_count)) if requested != selected_channel_count
    )
}

fn u16_field(value: &Value, field: &str) -> Option<u16> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
}

fn i32_field(value: &Value, field: &str) -> Option<i32> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn increment<K>(map: &mut BTreeMap<K, usize>, key: K)
where
    K: Ord,
{
    *map.entry(key).or_default() += 1;
}
