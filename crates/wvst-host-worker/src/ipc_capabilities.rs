use serde::{Deserialize, Serialize};
use wvst_vst3_host::Vst3LoadedComponent;

const RUNTIME_CAPABILITIES_SCHEMA_VERSION: u16 = 2;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkerRuntimeCapabilities {
    pub(super) schema_version: u16,
    pub(super) binary_audio_process: bool,
    pub(super) component_state: bool,
    pub(super) controller: bool,
    pub(super) controller_state: bool,
    pub(super) parameters: bool,
    pub(super) parameter_automation: bool,
    pub(super) units: bool,
    pub(super) unit_program_data: bool,
    pub(super) program_list_data: bool,
    pub(super) unit_data: bool,
    pub(super) midi_mapping: bool,
    pub(super) output_events: bool,
    pub(super) output_parameter_changes: bool,
    pub(super) component_handler_events: bool,
    pub(super) connection_points: bool,
    pub(super) process_context: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) unavailable: Vec<WorkerRuntimeCapabilityDiagnostic>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkerRuntimeCapabilityDiagnostic {
    capability: WorkerRuntimeCapability,
    reason: WorkerRuntimeCapabilityUnavailableReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    hint: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WorkerRuntimeCapability {
    ComponentState,
    Controller,
    ControllerState,
    Parameters,
    Units,
    UnitProgramData,
    ProgramListData,
    UnitData,
    MidiMapping,
    ComponentHandlerEvents,
    ConnectionPoints,
    ProcessContext,
    OutputEvents,
    OutputParameterChanges,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WorkerRuntimeCapabilityUnavailableReason {
    ControllerUnavailable,
    InterfaceUnavailable,
    FeatureUnavailable,
    ProbeFailed,
}

impl WorkerRuntimeCapabilities {
    pub(super) fn from_vst3_component(component: &Vst3LoadedComponent, midi_mapping: bool) -> Self {
        let controller = component.controller();
        let has_controller = controller.is_some();
        let unit_probe = match controller {
            Some(controller) => capability_probe(controller.unit_info()),
            None => CapabilityProbe::unavailable(
                WorkerRuntimeCapabilityUnavailableReason::ControllerUnavailable,
                None,
            ),
        };
        let program_list_data_probe = bool_capability_probe(component.has_program_list_data());
        let unit_data_probe = bool_capability_probe(component.has_unit_data());
        let component_connection_probe = capability_probe(component.instance().connection_point());
        let controller_connection_probe = match controller {
            Some(controller) => capability_probe(controller.connection_point()),
            None => CapabilityProbe::unavailable(
                WorkerRuntimeCapabilityUnavailableReason::ControllerUnavailable,
                None,
            ),
        };
        let connection_points =
            component_connection_probe.available && controller_connection_probe.available;
        let process_context = component
            .instance()
            .process_context_requirements()
            .is_some();

        let mut unavailable = Vec::new();
        if !has_controller {
            unavailable.extend(controller_unavailable());
        }
        if !unit_probe.available {
            push_unavailable(
                &mut unavailable,
                WorkerRuntimeCapability::Units,
                &unit_probe,
                "Load a plugin that exposes VST3 IUnitInfo to enumerate units and programs.",
            );
            push_unavailable(
                &mut unavailable,
                WorkerRuntimeCapability::UnitProgramData,
                &unit_probe,
                "VST3 unit program data needs an edit controller with IUnitInfo support.",
            );
        }
        if !program_list_data_probe.available {
            push_unavailable(
                &mut unavailable,
                WorkerRuntimeCapability::ProgramListData,
                &program_list_data_probe,
                "Use component/controller state when the plugin does not expose IProgramListData.",
            );
        }
        if !unit_data_probe.available {
            push_unavailable(
                &mut unavailable,
                WorkerRuntimeCapability::UnitData,
                &unit_data_probe,
                "Use component/controller state when the plugin does not expose IUnitData.",
            );
        }
        if !midi_mapping {
            unavailable.push(WorkerRuntimeCapabilityDiagnostic::new(
                WorkerRuntimeCapability::MidiMapping,
                if has_controller {
                    WorkerRuntimeCapabilityUnavailableReason::FeatureUnavailable
                } else {
                    WorkerRuntimeCapabilityUnavailableReason::ControllerUnavailable
                },
                None,
                "Send note events directly; MIDI CC mapping requires plugin IMidiMapping assignments.",
            ));
        }
        if !connection_points {
            push_connection_point_unavailable(
                &mut unavailable,
                &component_connection_probe,
                &controller_connection_probe,
            );
        }
        if !process_context {
            unavailable.push(WorkerRuntimeCapabilityDiagnostic::new(
                WorkerRuntimeCapability::ProcessContext,
                WorkerRuntimeCapabilityUnavailableReason::InterfaceUnavailable,
                None,
                "The host still supplies a process context; the plugin did not expose IProcessContextRequirements.",
            ));
        }

        Self {
            schema_version: RUNTIME_CAPABILITIES_SCHEMA_VERSION,
            binary_audio_process: true,
            component_state: true,
            controller: has_controller,
            controller_state: has_controller,
            parameters: has_controller,
            parameter_automation: true,
            units: unit_probe.available,
            unit_program_data: unit_probe.available,
            program_list_data: program_list_data_probe.available,
            unit_data: unit_data_probe.available,
            midi_mapping,
            output_events: true,
            output_parameter_changes: true,
            component_handler_events: has_controller,
            connection_points,
            process_context,
            unavailable,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CapabilityProbe {
    available: bool,
    reason: WorkerRuntimeCapabilityUnavailableReason,
    message: Option<String>,
}

impl CapabilityProbe {
    fn unavailable(
        reason: WorkerRuntimeCapabilityUnavailableReason,
        message: Option<String>,
    ) -> Self {
        Self {
            available: false,
            reason,
            message,
        }
    }
}

impl WorkerRuntimeCapabilityDiagnostic {
    fn new(
        capability: WorkerRuntimeCapability,
        reason: WorkerRuntimeCapabilityUnavailableReason,
        message: Option<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            capability,
            reason,
            message,
            hint: hint.into(),
        }
    }
}

fn capability_probe<T, E: std::fmt::Display>(result: Result<Option<T>, E>) -> CapabilityProbe {
    match result {
        Ok(Some(_)) => CapabilityProbe {
            available: true,
            reason: WorkerRuntimeCapabilityUnavailableReason::InterfaceUnavailable,
            message: None,
        },
        Ok(None) => CapabilityProbe::unavailable(
            WorkerRuntimeCapabilityUnavailableReason::InterfaceUnavailable,
            None,
        ),
        Err(error) => CapabilityProbe::unavailable(
            WorkerRuntimeCapabilityUnavailableReason::ProbeFailed,
            Some(error.to_string()),
        ),
    }
}

fn bool_capability_probe<E: std::fmt::Display>(result: Result<bool, E>) -> CapabilityProbe {
    match result {
        Ok(true) => CapabilityProbe {
            available: true,
            reason: WorkerRuntimeCapabilityUnavailableReason::InterfaceUnavailable,
            message: None,
        },
        Ok(false) => CapabilityProbe::unavailable(
            WorkerRuntimeCapabilityUnavailableReason::InterfaceUnavailable,
            None,
        ),
        Err(error) => CapabilityProbe::unavailable(
            WorkerRuntimeCapabilityUnavailableReason::ProbeFailed,
            Some(error.to_string()),
        ),
    }
}

fn controller_unavailable() -> Vec<WorkerRuntimeCapabilityDiagnostic> {
    const HINT: &str = "Load a plugin class that exposes an initialized VST3 IEditController.";
    [
        WorkerRuntimeCapability::Controller,
        WorkerRuntimeCapability::ControllerState,
        WorkerRuntimeCapability::Parameters,
        WorkerRuntimeCapability::ComponentHandlerEvents,
    ]
    .into_iter()
    .map(|capability| {
        WorkerRuntimeCapabilityDiagnostic::new(
            capability,
            WorkerRuntimeCapabilityUnavailableReason::ControllerUnavailable,
            None,
            HINT,
        )
    })
    .collect()
}

fn push_unavailable(
    unavailable: &mut Vec<WorkerRuntimeCapabilityDiagnostic>,
    capability: WorkerRuntimeCapability,
    probe: &CapabilityProbe,
    hint: &'static str,
) {
    unavailable.push(WorkerRuntimeCapabilityDiagnostic::new(
        capability,
        probe.reason,
        probe.message.clone(),
        hint,
    ));
}

fn push_connection_point_unavailable(
    unavailable: &mut Vec<WorkerRuntimeCapabilityDiagnostic>,
    component_probe: &CapabilityProbe,
    controller_probe: &CapabilityProbe,
) {
    let probe = if !component_probe.available {
        component_probe
    } else {
        controller_probe
    };
    push_unavailable(
        unavailable,
        WorkerRuntimeCapability::ConnectionPoints,
        probe,
        "Both component and controller must expose IConnectionPoint for message notify support.",
    );
}
