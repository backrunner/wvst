use serde::Serialize;
use wvst_vst3_host::{
    Vst3AudioBusInfo, Vst3BusDirection, Vst3BusType, Vst3SelectedAudioBus, Vst3SelectedAudioBuses,
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkerAudioBusDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<WorkerSelectedAudioBus>,
    output: WorkerSelectedAudioBus,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerSelectedAudioBus {
    direction: WorkerAudioBusDirection,
    requested_channels: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested_index: Option<i32>,
    selected_index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected: Option<WorkerAudioBusInfo>,
    available: Vec<WorkerAudioBusInfo>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerAudioBusInfo {
    index: i32,
    direction: WorkerAudioBusDirection,
    channel_count: i32,
    bus_type: WorkerAudioBusType,
    default_active: bool,
    control_voltage: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WorkerAudioBusDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerAudioBusType {
    kind: WorkerAudioBusTypeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw: Option<i32>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WorkerAudioBusTypeKind {
    Main,
    Aux,
    Unknown,
}

impl From<Vst3SelectedAudioBuses> for WorkerAudioBusDiagnostics {
    fn from(value: Vst3SelectedAudioBuses) -> Self {
        Self {
            input: value.input.map(WorkerSelectedAudioBus::from),
            output: WorkerSelectedAudioBus::from(value.output),
        }
    }
}

impl From<Vst3SelectedAudioBus> for WorkerSelectedAudioBus {
    fn from(value: Vst3SelectedAudioBus) -> Self {
        Self {
            direction: WorkerAudioBusDirection::from(value.direction),
            requested_channels: value.requested_channels,
            requested_index: value.requested_index,
            selected_index: value.selected_index,
            selected: value.selected.map(WorkerAudioBusInfo::from),
            available: value
                .available
                .into_iter()
                .map(WorkerAudioBusInfo::from)
                .collect(),
        }
    }
}

impl From<Vst3AudioBusInfo> for WorkerAudioBusInfo {
    fn from(value: Vst3AudioBusInfo) -> Self {
        Self {
            index: value.index,
            direction: WorkerAudioBusDirection::from(value.direction),
            channel_count: value.channel_count,
            bus_type: WorkerAudioBusType::from(value.bus_type),
            default_active: value.default_active,
            control_voltage: value.control_voltage,
            name: value.name,
        }
    }
}

impl From<Vst3BusDirection> for WorkerAudioBusDirection {
    fn from(value: Vst3BusDirection) -> Self {
        match value {
            Vst3BusDirection::Input => Self::Input,
            Vst3BusDirection::Output => Self::Output,
        }
    }
}

impl From<Vst3BusType> for WorkerAudioBusType {
    fn from(value: Vst3BusType) -> Self {
        match value {
            Vst3BusType::Main => Self {
                kind: WorkerAudioBusTypeKind::Main,
                raw: None,
            },
            Vst3BusType::Aux => Self {
                kind: WorkerAudioBusTypeKind::Aux,
                raw: None,
            },
            Vst3BusType::Unknown(raw) => Self {
                kind: WorkerAudioBusTypeKind::Unknown,
                raw: Some(raw),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_selected_audio_bus_diagnostics() {
        let diagnostics = WorkerAudioBusDiagnostics::from(Vst3SelectedAudioBuses {
            input: Some(Vst3SelectedAudioBus {
                direction: Vst3BusDirection::Input,
                requested_channels: 2,
                requested_index: Some(1),
                selected_index: 1,
                selected: Some(bus(1, Vst3BusDirection::Input, Vst3BusType::Main)),
                available: vec![
                    bus(0, Vst3BusDirection::Input, Vst3BusType::Aux),
                    bus(1, Vst3BusDirection::Input, Vst3BusType::Main),
                ],
            }),
            output: Vst3SelectedAudioBus {
                direction: Vst3BusDirection::Output,
                requested_channels: 2,
                requested_index: None,
                selected_index: 3,
                selected: Some(bus(3, Vst3BusDirection::Output, Vst3BusType::Unknown(99))),
                available: vec![bus(3, Vst3BusDirection::Output, Vst3BusType::Unknown(99))],
            },
        });

        let value = serde_json::to_value(diagnostics).expect("diagnostics json");

        assert_eq!(value["input"]["direction"], "input");
        assert_eq!(value["input"]["requestedChannels"], 2);
        assert_eq!(value["input"]["requestedIndex"], 1);
        assert_eq!(value["input"]["selectedIndex"], 1);
        assert_eq!(value["input"]["selected"]["busType"]["kind"], "main");
        assert_eq!(value["input"]["available"][0]["busType"]["kind"], "aux");
        assert_eq!(value["output"]["direction"], "output");
        assert_eq!(value["output"]["selected"]["busType"]["kind"], "unknown");
        assert_eq!(value["output"]["selected"]["busType"]["raw"], 99);
    }

    fn bus(index: i32, direction: Vst3BusDirection, bus_type: Vst3BusType) -> Vst3AudioBusInfo {
        Vst3AudioBusInfo {
            index,
            direction,
            channel_count: 2,
            bus_type,
            default_active: index == 1,
            control_voltage: false,
            name: Some(format!("bus-{index}")),
        }
    }
}
