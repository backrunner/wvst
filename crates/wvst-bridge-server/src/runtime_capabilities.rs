use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub binary_audio_process: bool,
    pub component_state: bool,
    pub controller: bool,
    pub controller_state: bool,
    pub parameters: bool,
    pub parameter_automation: bool,
    pub units: bool,
    pub unit_program_data: bool,
    pub program_list_data: bool,
    pub unit_data: bool,
    pub midi_mapping: bool,
    pub output_events: bool,
    pub output_parameter_changes: bool,
    pub component_handler_events: bool,
    pub connection_points: bool,
    pub process_context: bool,
}

impl RuntimeCapabilities {
    pub fn from_worker_result(value: Option<&Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };

        Self {
            binary_audio_process: json_bool(value, "binaryAudioProcess"),
            component_state: json_bool(value, "componentState"),
            controller: json_bool(value, "controller"),
            controller_state: json_bool(value, "controllerState"),
            parameters: json_bool(value, "parameters"),
            parameter_automation: json_bool(value, "parameterAutomation"),
            units: json_bool(value, "units"),
            unit_program_data: json_bool(value, "unitProgramData"),
            program_list_data: json_bool(value, "programListData"),
            unit_data: json_bool(value, "unitData"),
            midi_mapping: json_bool(value, "midiMapping"),
            output_events: json_bool(value, "outputEvents"),
            output_parameter_changes: json_bool(value, "outputParameterChanges"),
            component_handler_events: json_bool(value, "componentHandlerEvents"),
            connection_points: json_bool(value, "connectionPoints"),
            process_context: json_bool(value, "processContext"),
        }
    }
}

fn json_bool(value: &Value, key: &'static str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}
