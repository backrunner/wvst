use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub schema_version: u16,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable: Vec<RuntimeCapabilityDiagnostic>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilityDiagnostic {
    pub capability: RuntimeCapability,
    pub reason: RuntimeCapabilityUnavailableReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub hint: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCapability {
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
pub enum RuntimeCapabilityUnavailableReason {
    ControllerUnavailable,
    InterfaceUnavailable,
    FeatureUnavailable,
    ProbeFailed,
}

impl RuntimeCapabilities {
    pub fn from_worker_result(value: Option<&Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };

        Self {
            schema_version: json_u16(value, "schemaVersion"),
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
            unavailable: diagnostics(value.get("unavailable")),
        }
    }
}

fn json_bool(value: &Value, key: &'static str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn json_u16(value: &Value, key: &'static str) -> u16 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(0)
}

fn diagnostics(value: Option<&Value>) -> Vec<RuntimeCapabilityDiagnostic> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_runtime_capability_diagnostics() {
        let capabilities = RuntimeCapabilities::from_worker_result(Some(&json!({
            "schemaVersion": 2,
            "binaryAudioProcess": true,
            "componentState": false,
            "unavailable": [
                {
                    "capability": "component-state",
                    "reason": "interface-unavailable",
                    "hint": "The plugin does not expose component state."
                }
            ]
        })));

        assert_eq!(capabilities.schema_version, 2);
        assert!(capabilities.binary_audio_process);
        assert_eq!(capabilities.unavailable.len(), 1);
        assert_eq!(
            capabilities.unavailable[0].capability,
            RuntimeCapability::ComponentState
        );
        assert_eq!(
            capabilities.unavailable[0].reason,
            RuntimeCapabilityUnavailableReason::InterfaceUnavailable
        );
    }

    #[test]
    fn ignores_unknown_or_absent_diagnostics() {
        let missing = RuntimeCapabilities::from_worker_result(Some(&json!({
            "schemaVersion": 1,
            "binaryAudioProcess": true
        })));
        assert!(missing.unavailable.is_empty());

        let malformed = RuntimeCapabilities::from_worker_result(Some(&json!({
            "schemaVersion": 2,
            "unavailable": [
                { "capability": "future-capability" },
                {
                    "capability": "parameters",
                    "reason": "controller-unavailable",
                    "hint": "Load a plugin class that exposes an initialized VST3 IEditController."
                }
            ]
        })));
        assert_eq!(malformed.unavailable.len(), 1);
        assert_eq!(
            malformed.unavailable[0].capability,
            RuntimeCapability::Parameters
        );
    }
}
