use serde::{Deserialize, Serialize};
use wvst_vst3_host::Vst3LoadedComponent;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkerRuntimeCapabilities {
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
}

impl WorkerRuntimeCapabilities {
    pub(super) fn passthrough() -> Self {
        Self {
            binary_audio_process: true,
            component_state: false,
            controller: false,
            controller_state: false,
            parameters: false,
            parameter_automation: false,
            units: false,
            unit_program_data: false,
            program_list_data: false,
            unit_data: false,
            midi_mapping: false,
            output_events: false,
            output_parameter_changes: false,
            component_handler_events: false,
            connection_points: false,
            process_context: false,
        }
    }

    pub(super) fn from_vst3_component(component: &Vst3LoadedComponent, midi_mapping: bool) -> Self {
        let controller = component.controller().is_some();
        let units = component
            .controller()
            .and_then(|controller| controller.unit_info().ok().flatten())
            .is_some();
        let program_list_data = component.has_program_list_data().unwrap_or(false);
        let unit_data = component.has_unit_data().unwrap_or(false);
        let component_connection_point = component.instance().connection_point().ok().flatten();
        let controller_connection_point = component
            .controller()
            .and_then(|controller| controller.connection_point().ok().flatten());

        Self {
            binary_audio_process: true,
            component_state: true,
            controller,
            controller_state: controller,
            parameters: controller,
            parameter_automation: true,
            units,
            unit_program_data: units,
            program_list_data,
            unit_data,
            midi_mapping,
            output_events: true,
            output_parameter_changes: true,
            component_handler_events: controller,
            connection_points: component_connection_point.is_some()
                && controller_connection_point.is_some(),
            process_context: component
                .instance()
                .process_context_requirements()
                .is_some(),
        }
    }
}
