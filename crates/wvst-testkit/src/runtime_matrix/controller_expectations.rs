use serde_json::Value;

use super::{
    expectations::RuntimeProbeExpectations,
    report::{RuntimeProbeResult, availability_field, u64_field},
};

pub(super) fn evaluate_controller_expectations(
    expectations: &RuntimeProbeExpectations,
    result: &RuntimeProbeResult,
    failures: &mut Vec<String>,
) {
    if !has_controller_expectations(expectations) {
        return;
    }

    let Some(controller) = result
        .probe_report
        .as_ref()
        .and_then(|report| report.get("controller"))
    else {
        failures.push("missing runtime-probe controller diagnostics".to_string());
        return;
    };

    let parameter_count = controller
        .get("parameters")
        .map(|parameters| u64_field(parameters, "count"))
        .unwrap_or(0);
    let automatable = controller
        .get("parameters")
        .map(|parameters| u64_field(parameters, "automatable"))
        .unwrap_or(0);

    if let Some(min) = expectations.min_parameter_count
        && parameter_count < min
    {
        failures.push(format!(
            "expected at least {min} parameters, got {parameter_count}"
        ));
    }
    if let Some(min) = expectations.min_automatable_parameters
        && automatable < min
    {
        failures.push(format!(
            "expected at least {min} automatable parameters, got {automatable}"
        ));
    }
    if expectations.require_component_state && !availability_field(controller, "componentState") {
        failures.push("expected component state to be available".to_string());
    }
    if let Some(min) = expectations.min_component_state_bytes {
        let bytes = controller
            .get("componentState")
            .map(|state| u64_field(state, "bytes"))
            .unwrap_or(0);
        if bytes < min {
            failures.push(format!(
                "expected component state to contain at least {min} bytes, got {bytes}"
            ));
        }
    }
    if expectations.require_component_state_roundtrip
        && !state_roundtrip_succeeded(controller, "componentState")
    {
        failures.push("expected component state roundtrip to succeed".to_string());
    }
    if expectations.require_controller_state && !availability_field(controller, "controllerState") {
        failures.push("expected controller state to be available".to_string());
    }
    if let Some(min) = expectations.min_controller_state_bytes {
        let bytes = controller
            .get("controllerState")
            .map(|state| u64_field(state, "bytes"))
            .unwrap_or(0);
        if bytes < min {
            failures.push(format!(
                "expected controller state to contain at least {min} bytes, got {bytes}"
            ));
        }
    }
    if expectations.require_controller_state_roundtrip
        && !state_roundtrip_succeeded(controller, "controllerState")
    {
        failures.push("expected controller state roundtrip to succeed".to_string());
    }
    if expectations.require_controller_component_state_sync
        && !controller_component_state_sync_succeeded(controller)
    {
        failures.push("expected controller component state sync to succeed".to_string());
    }
    if let Some(min) = expectations.min_controller_component_state_sync_bytes {
        let bytes = controller_component_state_sync_bytes(controller);
        if bytes < min {
            failures.push(format!(
                "expected controller component state sync to carry at least {min} bytes, got {bytes}"
            ));
        }
    }
    if expectations.require_unit_info && !availability_field(controller, "units") {
        failures.push("expected unit info to be available".to_string());
    }
    if let Some(min) = expectations.min_unit_count {
        let count = controller
            .get("units")
            .map(|units| u64_field(units, "unitCount"))
            .unwrap_or(0);
        if count < min {
            failures.push(format!("expected at least {min} units, got {count}"));
        }
    }
    if let Some(min) = expectations.min_program_list_count {
        let count = controller
            .get("units")
            .map(|units| u64_field(units, "programListCount"))
            .unwrap_or(0);
        if count < min {
            failures.push(format!(
                "expected at least {min} program lists, got {count}"
            ));
        }
    }
    if let Some(min) = expectations.min_total_programs {
        let count = controller
            .get("units")
            .map(|units| u64_field(units, "totalPrograms"))
            .unwrap_or(0);
        if count < min {
            failures.push(format!(
                "expected at least {min} total programs, got {count}"
            ));
        }
    }
    if expectations.require_program_list_data && !availability_field(controller, "programListData")
    {
        failures.push("expected program list data to be available".to_string());
    }
    if let Some(min) = expectations.min_program_list_data_supported {
        let count = controller
            .get("programListData")
            .map(|data| u64_field(data, "supported"))
            .unwrap_or(0);
        if count < min {
            failures.push(format!(
                "expected at least {min} program list data entries to be supported, got {count}"
            ));
        }
    }
    if expectations.require_unit_data && !availability_field(controller, "unitData") {
        failures.push("expected unit data to be available".to_string());
    }
    if let Some(min) = expectations.min_unit_data_supported {
        let count = controller
            .get("unitData")
            .map(|data| u64_field(data, "supported"))
            .unwrap_or(0);
        if count < min {
            failures.push(format!(
                "expected at least {min} unit data entries to be supported, got {count}"
            ));
        }
    }
    if expectations.require_component_handler && !availability_field(controller, "componentHandler")
    {
        failures.push("expected component handler to be available".to_string());
    }
    if expectations.require_component_handler_edit_probe
        && !component_handler_edit_probe_succeeded(controller)
    {
        failures.push("expected component handler edit probe to succeed".to_string());
    }
    if expectations.require_connection_points && !connection_points_connected(controller) {
        failures
            .push("expected component/controller connection points to be connected".to_string());
    }
    if expectations.require_connection_notify_probe
        && !connection_notify_probe_succeeded(controller)
    {
        failures
            .push("expected component/controller connection notify probe to succeed".to_string());
    }
    if let Some(min) = expectations.min_component_handler_events {
        let total_events = controller
            .get("componentHandler")
            .map(|handler| u64_field(handler, "totalEvents"))
            .unwrap_or(0);
        if total_events < min {
            failures.push(format!(
                "expected at least {min} component handler events, got {total_events}"
            ));
        }
    }
}

fn has_controller_expectations(expectations: &RuntimeProbeExpectations) -> bool {
    expectations.min_parameter_count.is_some()
        || expectations.min_automatable_parameters.is_some()
        || expectations.require_component_state
        || expectations.min_component_state_bytes.is_some()
        || expectations.require_component_state_roundtrip
        || expectations.require_controller_state
        || expectations.min_controller_state_bytes.is_some()
        || expectations.require_controller_state_roundtrip
        || expectations.require_controller_component_state_sync
        || expectations
            .min_controller_component_state_sync_bytes
            .is_some()
        || expectations.require_unit_info
        || expectations.min_unit_count.is_some()
        || expectations.min_program_list_count.is_some()
        || expectations.min_total_programs.is_some()
        || expectations.require_program_list_data
        || expectations.min_program_list_data_supported.is_some()
        || expectations.require_unit_data
        || expectations.min_unit_data_supported.is_some()
        || expectations.require_component_handler
        || expectations.require_component_handler_edit_probe
        || expectations.require_connection_points
        || expectations.require_connection_notify_probe
        || expectations.min_component_handler_events.is_some()
}

fn state_roundtrip_succeeded(controller: &Value, field: &str) -> bool {
    controller
        .get(field)
        .and_then(|state| state.get("roundtrip"))
        .and_then(|roundtrip| roundtrip.get("success"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn controller_component_state_sync_succeeded(controller: &Value) -> bool {
    let Some(sync) = controller.get("controllerComponentStateSync") else {
        return false;
    };
    let attempted = sync
        .get("attempted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let success = sync
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    attempted && success
}

fn controller_component_state_sync_bytes(controller: &Value) -> u64 {
    controller
        .get("controllerComponentStateSync")
        .map(|sync| u64_field(sync, "componentStateBytes"))
        .unwrap_or(0)
}

fn component_handler_edit_probe_succeeded(controller: &Value) -> bool {
    let Some(edit_probe) = controller
        .get("componentHandler")
        .and_then(|handler| handler.get("editProbe"))
    else {
        return false;
    };
    let success = edit_probe
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let event_delta = u64_field(edit_probe, "eventDelta");
    success && event_delta >= 3
}

fn connection_points_connected(controller: &Value) -> bool {
    controller
        .get("connectionPoints")
        .and_then(|connection_points| connection_points.get("connected"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn connection_notify_probe_succeeded(controller: &Value) -> bool {
    let Some(notify_probe) = controller
        .get("connectionPoints")
        .and_then(|connection_points| connection_points.get("notifyProbe"))
    else {
        return false;
    };
    let success = notify_probe
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let component_success = notify_probe
        .get("component")
        .and_then(|component| component.get("success"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let controller_success = notify_probe
        .get("controller")
        .and_then(|controller| controller.get("success"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    success && component_success && controller_success
}
