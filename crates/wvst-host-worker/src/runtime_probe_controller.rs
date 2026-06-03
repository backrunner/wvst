use serde::Serialize;
use wvst_vst3_host::{
    HostResult, Vst3ControllerComponentStateSync, Vst3HostMessage, Vst3LoadedComponent,
    Vst3ParameterInfo,
};

#[path = "runtime_probe_controller_units.rs"]
mod runtime_probe_controller_units;

use runtime_probe_controller_units::{
    RuntimeProbeDataSupportSummary, RuntimeProbeUnitSummary, program_list_data_summary,
    unit_data_summary, unit_summary,
};

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeProbeControllerSummary {
    controller_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    controller_component_state_sync: Option<Vst3ControllerComponentStateSync>,
    parameters: RuntimeProbeParameterSummary,
    component_state: RuntimeProbeStateSummary,
    controller_state: RuntimeProbeStateSummary,
    units: RuntimeProbeUnitSummary,
    program_list_data: RuntimeProbeDataSupportSummary,
    unit_data: RuntimeProbeDataSupportSummary,
    component_handler: RuntimeProbeComponentHandlerSummary,
    connection_points: RuntimeProbeConnectionPointSummary,
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
    roundtrip: RuntimeProbeStateRoundtripSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeStateRoundtripSummary {
    attempted: bool,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_before: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_after: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeComponentHandlerSummary {
    available: bool,
    total_events: u64,
    recent_event_count: usize,
    edit_probe: RuntimeProbeComponentHandlerEditProbeSummary,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeComponentHandlerEditProbeSummary {
    attempted: bool,
    success: bool,
    event_delta: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameter_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_normalized: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    before_events: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after_events: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeConnectionPointSummary {
    connected: bool,
    notify_probe: RuntimeProbeConnectionNotifyProbeSummary,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeConnectionNotifyProbeSummary {
    attempted: bool,
    success: bool,
    component: RuntimeProbeConnectionNotifyTargetSummary,
    controller: RuntimeProbeConnectionNotifyTargetSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_reason: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeConnectionNotifyTargetSummary {
    attempted: bool,
    success: bool,
    notified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub(crate) fn controller_summary(
    loaded: &mut Vst3LoadedComponent,
    parameters: &[Vst3ParameterInfo],
    run_edit_probe: bool,
    edit_probe_parameter_id: Option<u32>,
    run_connection_notify_probe: bool,
    run_state_roundtrip_probe: bool,
) -> RuntimeProbeControllerSummary {
    let units = unit_summary(loaded);
    RuntimeProbeControllerSummary {
        controller_available: loaded.controller().is_some(),
        controller_component_state_sync: loaded.controller_component_state_sync().cloned(),
        parameters: parameter_summary(parameters),
        component_state: component_state_summary(loaded, run_state_roundtrip_probe),
        controller_state: controller_state_summary(loaded, run_state_roundtrip_probe),
        program_list_data: program_list_data_summary(loaded, &units),
        unit_data: unit_data_summary(loaded, &units),
        component_handler: component_handler_summary(
            loaded,
            parameters,
            run_edit_probe,
            edit_probe_parameter_id,
        ),
        connection_points: connection_point_summary(loaded, run_connection_notify_probe),
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

fn component_state_summary(
    loaded: &mut Vst3LoadedComponent,
    run_roundtrip_probe: bool,
) -> RuntimeProbeStateSummary {
    match loaded.component_state() {
        Ok(state) => {
            let roundtrip = if run_roundtrip_probe {
                component_state_roundtrip(loaded, &state)
            } else {
                skipped_state_roundtrip("disabled-by-options")
            };
            RuntimeProbeStateSummary {
                available: true,
                bytes: Some(state.len()),
                roundtrip,
                error: None,
            }
        }
        Err(error) => RuntimeProbeStateSummary {
            available: false,
            bytes: None,
            roundtrip: skipped_state_roundtrip("state-unavailable"),
            error: Some(error.to_string()),
        },
    }
}

fn controller_state_summary(
    loaded: &mut Vst3LoadedComponent,
    run_roundtrip_probe: bool,
) -> RuntimeProbeStateSummary {
    match loaded.controller_state() {
        Ok(Some(state)) => RuntimeProbeStateSummary {
            available: true,
            bytes: Some(state.len()),
            roundtrip: if run_roundtrip_probe {
                controller_state_roundtrip(loaded, &state)
            } else {
                skipped_state_roundtrip("disabled-by-options")
            },
            error: None,
        },
        Ok(None) => RuntimeProbeStateSummary::default(),
        Err(error) => RuntimeProbeStateSummary {
            available: false,
            bytes: None,
            roundtrip: skipped_state_roundtrip("state-unavailable"),
            error: Some(error.to_string()),
        },
    }
}

fn component_state_roundtrip(
    loaded: &mut Vst3LoadedComponent,
    state: &[u8],
) -> RuntimeProbeStateRoundtripSummary {
    let mut summary = started_state_roundtrip(state);
    if let Err(error) = loaded.set_component_state(state) {
        summary.error = Some(error.to_string());
        return summary;
    }
    finish_state_roundtrip(
        summary,
        state,
        loaded.component_state().map_err(|error| error.to_string()),
    )
}

fn controller_state_roundtrip(
    loaded: &mut Vst3LoadedComponent,
    state: &[u8],
) -> RuntimeProbeStateRoundtripSummary {
    let mut summary = started_state_roundtrip(state);
    if let Err(error) = loaded.set_controller_state(state) {
        summary.error = Some(error.to_string());
        return summary;
    }
    finish_state_roundtrip(
        summary,
        state,
        loaded
            .controller_state()
            .map_err(|error| error.to_string())
            .and_then(|state| state.ok_or_else(|| "controller state unavailable".to_string())),
    )
}

fn started_state_roundtrip(state: &[u8]) -> RuntimeProbeStateRoundtripSummary {
    RuntimeProbeStateRoundtripSummary {
        attempted: true,
        bytes_before: Some(state.len()),
        ..RuntimeProbeStateRoundtripSummary::default()
    }
}

fn finish_state_roundtrip(
    mut summary: RuntimeProbeStateRoundtripSummary,
    state: &[u8],
    after_result: Result<Vec<u8>, String>,
) -> RuntimeProbeStateRoundtripSummary {
    match after_result {
        Ok(after) => {
            summary.bytes_after = Some(after.len());
            if after == state {
                summary.success = true;
            } else {
                summary.error = Some("state bytes changed after roundtrip".to_string());
            }
        }
        Err(error) => {
            summary.error = Some(error);
        }
    }
    summary
}

fn skipped_state_roundtrip(reason: &'static str) -> RuntimeProbeStateRoundtripSummary {
    RuntimeProbeStateRoundtripSummary {
        skipped_reason: Some(reason.to_string()),
        ..RuntimeProbeStateRoundtripSummary::default()
    }
}

fn component_handler_summary(
    loaded: &mut Vst3LoadedComponent,
    parameters: &[Vst3ParameterInfo],
    run_edit_probe: bool,
    edit_probe_parameter_id: Option<u32>,
) -> RuntimeProbeComponentHandlerSummary {
    let edit_probe = if run_edit_probe {
        component_handler_edit_probe(loaded, parameters, edit_probe_parameter_id)
    } else {
        skipped_edit_probe("disabled-by-options")
    };
    let Some(snapshot) = loaded.component_handler_snapshot() else {
        return RuntimeProbeComponentHandlerSummary {
            edit_probe,
            ..RuntimeProbeComponentHandlerSummary::default()
        };
    };
    RuntimeProbeComponentHandlerSummary {
        available: true,
        total_events: snapshot.total_events,
        recent_event_count: snapshot.recent_events.len(),
        edit_probe,
    }
}

fn component_handler_edit_probe(
    loaded: &mut Vst3LoadedComponent,
    parameters: &[Vst3ParameterInfo],
    requested_parameter_id: Option<u32>,
) -> RuntimeProbeComponentHandlerEditProbeSummary {
    let Some(before_snapshot) = loaded.component_handler_snapshot() else {
        return skipped_edit_probe("component-handler-unavailable");
    };
    let parameter = match edit_probe_parameter(parameters, requested_parameter_id) {
        Ok(parameter) => parameter,
        Err(reason) => {
            return RuntimeProbeComponentHandlerEditProbeSummary {
                parameter_id: requested_parameter_id,
                before_events: Some(before_snapshot.total_events),
                skipped_reason: Some(reason.to_string()),
                ..RuntimeProbeComponentHandlerEditProbeSummary::default()
            };
        }
    };
    let Some(controller) = loaded.controller() else {
        return skipped_edit_probe("controller-unavailable");
    };
    let value_normalized = controller.get_param_normalized(parameter.id);
    let mut summary = RuntimeProbeComponentHandlerEditProbeSummary {
        attempted: true,
        parameter_id: Some(parameter.id),
        value_normalized: Some(value_normalized),
        before_events: Some(before_snapshot.total_events),
        ..RuntimeProbeComponentHandlerEditProbeSummary::default()
    };

    if !is_normalized_value(value_normalized) {
        summary.error = Some("controller returned valueNormalized outside [0, 1]".to_string());
        return summary;
    }

    let edit_result = perform_component_handler_edit_probe(loaded, parameter.id, value_normalized);
    let after_events = loaded
        .component_handler_snapshot()
        .map(|snapshot| snapshot.total_events);
    summary.after_events = after_events;
    summary.event_delta = after_events
        .unwrap_or(before_snapshot.total_events)
        .saturating_sub(before_snapshot.total_events);

    match edit_result {
        Ok(()) if summary.event_delta >= 3 => summary.success = true,
        Ok(()) => {
            summary.error =
                Some("component handler edit probe recorded fewer than 3 events".to_string());
        }
        Err(error) => summary.error = Some(error.to_string()),
    }

    summary
}

fn perform_component_handler_edit_probe(
    loaded: &mut Vst3LoadedComponent,
    parameter_id: u32,
    value_normalized: f64,
) -> HostResult<()> {
    let Some(controller) = loaded.controller_mut() else {
        return Ok(());
    };
    controller.begin_edit(parameter_id)?;
    let perform_result = controller.perform_edit(parameter_id, value_normalized);
    let end_result = controller.end_edit(parameter_id);
    perform_result?;
    end_result
}

fn edit_probe_parameter(
    parameters: &[Vst3ParameterInfo],
    requested_parameter_id: Option<u32>,
) -> Result<&Vst3ParameterInfo, &'static str> {
    if let Some(parameter_id) = requested_parameter_id {
        let Some(parameter) = parameters
            .iter()
            .find(|parameter| parameter.id == parameter_id)
        else {
            return Err("requested-parameter-not-found");
        };
        return if parameter.flags.can_automate && !parameter.flags.read_only {
            Ok(parameter)
        } else {
            Err("requested-parameter-not-eligible")
        };
    }

    parameters
        .iter()
        .find(|parameter| {
            parameter.flags.can_automate && !parameter.flags.read_only && !parameter.flags.hidden
        })
        .or_else(|| {
            parameters
                .iter()
                .find(|parameter| parameter.flags.can_automate && !parameter.flags.read_only)
        })
        .ok_or("no-automatable-writable-parameter")
}

fn skipped_edit_probe(reason: &'static str) -> RuntimeProbeComponentHandlerEditProbeSummary {
    RuntimeProbeComponentHandlerEditProbeSummary {
        skipped_reason: Some(reason.to_string()),
        ..RuntimeProbeComponentHandlerEditProbeSummary::default()
    }
}

fn is_normalized_value(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn connection_point_summary(
    loaded: &Vst3LoadedComponent,
    run_notify_probe: bool,
) -> RuntimeProbeConnectionPointSummary {
    RuntimeProbeConnectionPointSummary {
        connected: loaded.connection_points_connected(),
        notify_probe: connection_notify_probe(loaded, run_notify_probe),
    }
}

fn connection_notify_probe(
    loaded: &Vst3LoadedComponent,
    run_notify_probe: bool,
) -> RuntimeProbeConnectionNotifyProbeSummary {
    if !run_notify_probe {
        return skipped_connection_notify_probe("disabled-by-options");
    }
    if !loaded.connection_points_connected() {
        return skipped_connection_notify_probe("connection-points-unavailable");
    }

    let component = connection_notify_target_probe(loaded, ConnectionNotifyProbeTarget::Component);
    let controller =
        connection_notify_target_probe(loaded, ConnectionNotifyProbeTarget::Controller);
    RuntimeProbeConnectionNotifyProbeSummary {
        attempted: true,
        success: component.success && controller.success,
        component,
        controller,
        skipped_reason: None,
    }
}

fn skipped_connection_notify_probe(
    reason: &'static str,
) -> RuntimeProbeConnectionNotifyProbeSummary {
    RuntimeProbeConnectionNotifyProbeSummary {
        skipped_reason: Some(reason.to_string()),
        ..RuntimeProbeConnectionNotifyProbeSummary::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionNotifyProbeTarget {
    Component,
    Controller,
}

fn connection_notify_target_probe(
    loaded: &Vst3LoadedComponent,
    target: ConnectionNotifyProbeTarget,
) -> RuntimeProbeConnectionNotifyTargetSummary {
    let mut summary = RuntimeProbeConnectionNotifyTargetSummary {
        attempted: true,
        ..RuntimeProbeConnectionNotifyTargetSummary::default()
    };
    let mut message = match connection_notify_probe_message() {
        Ok(message) => message,
        Err(error) => {
            summary.error = Some(error);
            return summary;
        }
    };
    let result = match target {
        ConnectionNotifyProbeTarget::Component => loaded.notify_component(&mut message),
        ConnectionNotifyProbeTarget::Controller => loaded.notify_controller(&mut message),
    };
    match result {
        Ok(Some(())) => {
            summary.success = true;
            summary.notified = true;
        }
        Ok(None) => {
            summary.skipped_reason = Some("connection-points-unavailable".to_string());
        }
        Err(error) => {
            summary.error = Some(error.to_string());
        }
    }
    summary
}

fn connection_notify_probe_message() -> Result<Vst3HostMessage, String> {
    let mut message = Vst3HostMessage::new();
    message
        .set_id("WVST.RuntimeProbe.ConnectionNotify")
        .map_err(|error| error.to_string())?;
    message
        .attributes_mut()
        .set_int("wvstProbeVersion", 1)
        .map_err(|error| error.to_string())?;
    message
        .attributes_mut()
        .set_float("wvstProbeValue", 0.25)
        .map_err(|error| error.to_string())?;
    message
        .attributes_mut()
        .set_string("wvstProbeText", "ok")
        .map_err(|error| error.to_string())?;
    message
        .attributes_mut()
        .set_binary("wvstProbeBytes", &[1, 2, 3, 4])
        .map_err(|error| error.to_string())?;
    Ok(message)
}

#[cfg(test)]
#[path = "runtime_probe_controller_tests.rs"]
mod runtime_probe_controller_tests;
