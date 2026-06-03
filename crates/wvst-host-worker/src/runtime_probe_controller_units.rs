use serde::Serialize;
use wvst_vst3_host::{HostResult, Vst3LoadedComponent, Vst3UnitMetadata};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeProbeUnitSummary {
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
pub(super) struct RuntimeProbeDataSupportSummary {
    available: bool,
    checked: usize,
    supported: usize,
    unsupported: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub(super) fn unit_summary(loaded: &Vst3LoadedComponent) -> RuntimeProbeUnitSummary {
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

pub(super) fn program_list_data_summary(
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

pub(super) fn unit_data_summary(
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
}
