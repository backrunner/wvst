use serde::Serialize;
use serde_json::Value;

use super::{RuntimeProbeResult, availability_field, u64_field};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeControllerHealthSummary {
    pub reported_cases: usize,
    pub parameter_cases: usize,
    pub total_parameters: u64,
    pub total_automatable_parameters: u64,
    pub max_parameter_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_parameter_count_case: Option<String>,
    pub max_automatable_parameters: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_automatable_parameters_case: Option<String>,
    pub component_state_cases: usize,
    pub component_state_roundtrip_cases: usize,
    pub total_component_state_bytes: u64,
    pub max_component_state_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_component_state_bytes_case: Option<String>,
    pub controller_state_cases: usize,
    pub controller_state_roundtrip_cases: usize,
    pub controller_component_state_sync_attempted_cases: usize,
    pub controller_component_state_sync_cases: usize,
    pub controller_component_state_sync_failed_cases: usize,
    pub total_controller_component_state_sync_bytes: u64,
    pub max_controller_component_state_sync_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_controller_component_state_sync_bytes_case: Option<String>,
    pub total_controller_state_bytes: u64,
    pub max_controller_state_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_controller_state_bytes_case: Option<String>,
    pub unit_info_cases: usize,
    pub total_units: u64,
    pub total_program_lists: u64,
    pub total_programs: u64,
    pub max_total_programs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_total_programs_case: Option<String>,
    pub program_list_data_cases: usize,
    pub total_program_list_data_checked: u64,
    pub total_program_list_data_supported: u64,
    pub total_program_list_data_unsupported: u64,
    pub unit_data_cases: usize,
    pub total_unit_data_checked: u64,
    pub total_unit_data_supported: u64,
    pub total_unit_data_unsupported: u64,
    pub component_handler_cases: usize,
    pub component_handler_edit_probe_cases: usize,
    pub connection_point_cases: usize,
    pub connection_notify_probe_cases: usize,
    pub component_connection_notify_probe_cases: usize,
    pub controller_connection_notify_probe_cases: usize,
    pub total_component_handler_events: u64,
    pub total_component_handler_edit_probe_event_delta: u64,
}

impl RuntimeProbeControllerHealthSummary {
    pub(super) fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(controller) = result
                .probe_report
                .as_ref()
                .and_then(|report| report.get("controller"))
            else {
                continue;
            };

            summary.reported_cases += 1;

            let parameter_count = controller
                .get("parameters")
                .map(|parameters| u64_field(parameters, "count"))
                .unwrap_or(0);
            let automatable = controller
                .get("parameters")
                .map(|parameters| u64_field(parameters, "automatable"))
                .unwrap_or(0);

            if parameter_count > 0 {
                summary.parameter_cases += 1;
            }
            summary.total_parameters = summary.total_parameters.saturating_add(parameter_count);
            summary.total_automatable_parameters = summary
                .total_automatable_parameters
                .saturating_add(automatable);
            if parameter_count > summary.max_parameter_count {
                summary.max_parameter_count = parameter_count;
                summary.max_parameter_count_case = Some(result.case_name.clone());
            }
            if automatable > summary.max_automatable_parameters {
                summary.max_automatable_parameters = automatable;
                summary.max_automatable_parameters_case = Some(result.case_name.clone());
            }

            summary.record_state(controller, result);
            summary.record_units(controller, result);
            summary.record_controller_events(controller);
        }

        summary
    }

    fn record_state(&mut self, controller: &Value, result: &RuntimeProbeResult) {
        if availability_field(controller, "componentState") {
            self.component_state_cases += 1;
            if let Some(state) = controller.get("componentState") {
                if state
                    .get("roundtrip")
                    .and_then(|roundtrip| roundtrip.get("success"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    self.component_state_roundtrip_cases += 1;
                }
                let bytes = u64_field(state, "bytes");
                self.total_component_state_bytes =
                    self.total_component_state_bytes.saturating_add(bytes);
                if bytes > self.max_component_state_bytes {
                    self.max_component_state_bytes = bytes;
                    self.max_component_state_bytes_case = Some(result.case_name.clone());
                }
            }
        }
        if availability_field(controller, "controllerState") {
            self.controller_state_cases += 1;
            if let Some(state) = controller.get("controllerState") {
                if state
                    .get("roundtrip")
                    .and_then(|roundtrip| roundtrip.get("success"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    self.controller_state_roundtrip_cases += 1;
                }
                let bytes = u64_field(state, "bytes");
                self.total_controller_state_bytes =
                    self.total_controller_state_bytes.saturating_add(bytes);
                if bytes > self.max_controller_state_bytes {
                    self.max_controller_state_bytes = bytes;
                    self.max_controller_state_bytes_case = Some(result.case_name.clone());
                }
            }
        }
        if let Some(sync) = controller.get("controllerComponentStateSync") {
            if sync
                .get("attempted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                self.controller_component_state_sync_attempted_cases += 1;
            }
            if sync
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                self.controller_component_state_sync_cases += 1;
            } else if sync.get("error").is_some() {
                self.controller_component_state_sync_failed_cases += 1;
            }
            let bytes = u64_field(sync, "componentStateBytes");
            self.total_controller_component_state_sync_bytes = self
                .total_controller_component_state_sync_bytes
                .saturating_add(bytes);
            if bytes > self.max_controller_component_state_sync_bytes {
                self.max_controller_component_state_sync_bytes = bytes;
                self.max_controller_component_state_sync_bytes_case =
                    Some(result.case_name.clone());
            }
        }
    }

    fn record_units(&mut self, controller: &Value, result: &RuntimeProbeResult) {
        if availability_field(controller, "units") {
            self.unit_info_cases += 1;
            if let Some(units) = controller.get("units") {
                let unit_count = u64_field(units, "unitCount");
                let program_list_count = u64_field(units, "programListCount");
                let total_programs = u64_field(units, "totalPrograms");
                self.total_units = self.total_units.saturating_add(unit_count);
                self.total_program_lists =
                    self.total_program_lists.saturating_add(program_list_count);
                self.total_programs = self.total_programs.saturating_add(total_programs);
                if total_programs > self.max_total_programs {
                    self.max_total_programs = total_programs;
                    self.max_total_programs_case = Some(result.case_name.clone());
                }
            }
        }
        if availability_field(controller, "programListData") {
            self.program_list_data_cases += 1;
            if let Some(data) = controller.get("programListData") {
                self.total_program_list_data_checked = self
                    .total_program_list_data_checked
                    .saturating_add(u64_field(data, "checked"));
                self.total_program_list_data_supported = self
                    .total_program_list_data_supported
                    .saturating_add(u64_field(data, "supported"));
                self.total_program_list_data_unsupported = self
                    .total_program_list_data_unsupported
                    .saturating_add(u64_field(data, "unsupported"));
            }
        }
        if availability_field(controller, "unitData") {
            self.unit_data_cases += 1;
            if let Some(data) = controller.get("unitData") {
                self.total_unit_data_checked = self
                    .total_unit_data_checked
                    .saturating_add(u64_field(data, "checked"));
                self.total_unit_data_supported = self
                    .total_unit_data_supported
                    .saturating_add(u64_field(data, "supported"));
                self.total_unit_data_unsupported = self
                    .total_unit_data_unsupported
                    .saturating_add(u64_field(data, "unsupported"));
            }
        }
    }

    fn record_controller_events(&mut self, controller: &Value) {
        if availability_field(controller, "componentHandler") {
            self.component_handler_cases += 1;
            if let Some(handler) = controller.get("componentHandler") {
                self.total_component_handler_events = self
                    .total_component_handler_events
                    .saturating_add(u64_field(handler, "totalEvents"));
                if let Some(edit_probe) = handler.get("editProbe")
                    && edit_probe
                        .get("success")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                {
                    self.component_handler_edit_probe_cases += 1;
                    self.total_component_handler_edit_probe_event_delta = self
                        .total_component_handler_edit_probe_event_delta
                        .saturating_add(u64_field(edit_probe, "eventDelta"));
                }
            }
        }
        if controller
            .get("connectionPoints")
            .and_then(|connection_points| connection_points.get("connected"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            self.connection_point_cases += 1;
        }
        if let Some(notify_probe) = controller
            .get("connectionPoints")
            .and_then(|connection_points| connection_points.get("notifyProbe"))
        {
            if notify_probe
                .get("component")
                .and_then(|component| component.get("success"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                self.component_connection_notify_probe_cases += 1;
            }
            if notify_probe
                .get("controller")
                .and_then(|controller| controller.get("success"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                self.controller_connection_notify_probe_cases += 1;
            }
            if notify_probe
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                self.connection_notify_probe_cases += 1;
            }
        }
    }
}
