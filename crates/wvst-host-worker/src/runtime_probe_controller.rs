use serde::Serialize;
use wvst_vst3_host::{HostResult, Vst3LoadedComponent, Vst3ParameterInfo, Vst3UnitMetadata};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeProbeControllerSummary {
    controller_available: bool,
    parameters: RuntimeProbeParameterSummary,
    component_state: RuntimeProbeStateSummary,
    controller_state: RuntimeProbeStateSummary,
    units: RuntimeProbeUnitSummary,
    program_list_data: RuntimeProbeDataSupportSummary,
    unit_data: RuntimeProbeDataSupportSummary,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeParameterSummary {
    count: usize,
    automatable: usize,
    read_only: usize,
    hidden: usize,
    bypass: usize,
    program_change: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeStateSummary {
    available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeUnitSummary {
    available: bool,
    unit_count: usize,
    program_list_count: usize,
    total_programs: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_unit_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip)]
    unit_ids: Vec<i32>,
    #[serde(skip)]
    program_list_ids: Vec<i32>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeDataSupportSummary {
    available: bool,
    checked: usize,
    supported: usize,
    unsupported: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub(crate) fn controller_summary(
    loaded: &Vst3LoadedComponent,
    parameters: &[Vst3ParameterInfo],
) -> RuntimeProbeControllerSummary {
    let units = unit_summary(loaded);
    RuntimeProbeControllerSummary {
        controller_available: loaded.controller().is_some(),
        parameters: parameter_summary(parameters),
        component_state: component_state_summary(loaded),
        controller_state: controller_state_summary(loaded),
        program_list_data: program_list_data_summary(loaded, &units),
        unit_data: unit_data_summary(loaded, &units),
        units,
    }
}

fn parameter_summary(parameters: &[Vst3ParameterInfo]) -> RuntimeProbeParameterSummary {
    let mut summary = RuntimeProbeParameterSummary {
        count: parameters.len(),
        ..RuntimeProbeParameterSummary::default()
    };
    for parameter in parameters {
        if parameter.flags.can_automate {
            summary.automatable += 1;
        }
        if parameter.flags.read_only {
            summary.read_only += 1;
        }
        if parameter.flags.hidden {
            summary.hidden += 1;
        }
        if parameter.flags.bypass {
            summary.bypass += 1;
        }
        if parameter.flags.program_change {
            summary.program_change += 1;
        }
    }
    summary
}

fn component_state_summary(loaded: &Vst3LoadedComponent) -> RuntimeProbeStateSummary {
    state_summary(loaded.component_state().map(Some))
}

fn controller_state_summary(loaded: &Vst3LoadedComponent) -> RuntimeProbeStateSummary {
    state_summary(loaded.controller_state())
}

fn state_summary(result: HostResult<Option<Vec<u8>>>) -> RuntimeProbeStateSummary {
    match result {
        Ok(Some(state)) => RuntimeProbeStateSummary {
            available: true,
            bytes: Some(state.len()),
            error: None,
        },
        Ok(None) => RuntimeProbeStateSummary::default(),
        Err(error) => RuntimeProbeStateSummary {
            available: false,
            bytes: None,
            error: Some(error.to_string()),
        },
    }
}

fn unit_summary(loaded: &Vst3LoadedComponent) -> RuntimeProbeUnitSummary {
    let Some(controller) = loaded.controller() else {
        return RuntimeProbeUnitSummary::default();
    };
    let unit_info = match controller.unit_info() {
        Ok(Some(unit_info)) => unit_info,
        Ok(None) => return RuntimeProbeUnitSummary::default(),
        Err(error) => {
            return RuntimeProbeUnitSummary {
                error: Some(error.to_string()),
                ..RuntimeProbeUnitSummary::default()
            };
        }
    };
    match unit_info.metadata() {
        Ok(metadata) => unit_summary_from_metadata(&metadata),
        Err(error) => RuntimeProbeUnitSummary {
            error: Some(error.to_string()),
            ..RuntimeProbeUnitSummary::default()
        },
    }
}

fn unit_summary_from_metadata(metadata: &Vst3UnitMetadata) -> RuntimeProbeUnitSummary {
    RuntimeProbeUnitSummary {
        available: true,
        unit_count: metadata.units.len(),
        program_list_count: metadata.program_lists.len(),
        total_programs: metadata
            .program_lists
            .iter()
            .map(|list| list.programs.len())
            .sum(),
        selected_unit_id: Some(metadata.selected_unit_id),
        error: None,
        unit_ids: metadata.units.iter().map(|unit| unit.id).collect(),
        program_list_ids: metadata.program_lists.iter().map(|list| list.id).collect(),
    }
}

fn program_list_data_summary(
    loaded: &Vst3LoadedComponent,
    units: &RuntimeProbeUnitSummary,
) -> RuntimeProbeDataSupportSummary {
    let has_data = match loaded.has_program_list_data() {
        Ok(value) => value,
        Err(error) => {
            return RuntimeProbeDataSupportSummary {
                error: Some(error.to_string()),
                ..RuntimeProbeDataSupportSummary::default()
            };
        }
    };
    if !has_data {
        return RuntimeProbeDataSupportSummary::default();
    }
    count_support(&units.program_list_ids, |id| {
        loaded.program_data_supported(id)
    })
}

fn unit_data_summary(
    loaded: &Vst3LoadedComponent,
    units: &RuntimeProbeUnitSummary,
) -> RuntimeProbeDataSupportSummary {
    let has_data = match loaded.has_unit_data() {
        Ok(value) => value,
        Err(error) => {
            return RuntimeProbeDataSupportSummary {
                error: Some(error.to_string()),
                ..RuntimeProbeDataSupportSummary::default()
            };
        }
    };
    if !has_data {
        return RuntimeProbeDataSupportSummary::default();
    }
    count_support(&units.unit_ids, |id| loaded.unit_data_supported(id))
}

fn count_support(
    ids: &[i32],
    mut supported: impl FnMut(i32) -> HostResult<Option<bool>>,
) -> RuntimeProbeDataSupportSummary {
    let mut summary = RuntimeProbeDataSupportSummary {
        available: true,
        checked: ids.len(),
        ..RuntimeProbeDataSupportSummary::default()
    };
    for id in ids {
        match supported(*id) {
            Ok(Some(true)) => summary.supported += 1,
            Ok(Some(false) | None) => summary.unsupported += 1,
            Err(error) => {
                summary.error = Some(error.to_string());
                break;
            }
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use wvst_vst3_host::{Vst3ParameterFlags, Vst3ParameterInfo};

    #[test]
    fn summarizes_parameter_flags() {
        let summary = parameter_summary(&[
            parameter(flags(true, false, false, false, false)),
            parameter(flags(false, true, true, true, true)),
        ]);

        assert_eq!(summary.count, 2);
        assert_eq!(summary.automatable, 1);
        assert_eq!(summary.read_only, 1);
        assert_eq!(summary.hidden, 1);
        assert_eq!(summary.bypass, 1);
        assert_eq!(summary.program_change, 1);
    }

    #[test]
    fn counts_optional_support() {
        let summary = count_support(&[10, 20, 30], |id| match id {
            10 => Ok(Some(true)),
            20 => Ok(Some(false)),
            _ => Ok(None),
        });

        assert!(summary.available);
        assert_eq!(summary.checked, 3);
        assert_eq!(summary.supported, 1);
        assert_eq!(summary.unsupported, 2);
        assert_eq!(summary.error, None);
    }

    fn parameter(flags: Vst3ParameterFlags) -> Vst3ParameterInfo {
        Vst3ParameterInfo {
            id: 42,
            title: None,
            short_title: None,
            units: None,
            step_count: 0,
            default_normalized_value: 0.0,
            unit_id: 0,
            flags,
        }
    }

    fn flags(
        can_automate: bool,
        read_only: bool,
        hidden: bool,
        program_change: bool,
        bypass: bool,
    ) -> Vst3ParameterFlags {
        Vst3ParameterFlags {
            raw: 0,
            can_automate,
            read_only,
            wrap_around: false,
            list: false,
            hidden,
            program_change,
            bypass,
        }
    }
}
