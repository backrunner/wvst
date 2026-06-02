use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{WorkerIpcState, response_error, response_result};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceParameterParams {
    instance_id: u64,
    parameter_id: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceParameterSetParams {
    instance_id: u64,
    parameter_id: u32,
    value_normalized: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceStateParams {
    instance_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceSetStateParams {
    instance_id: u64,
    state_base64: String,
}

pub(super) fn handle_instance_parameters(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceStateParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid instance params: {error}"));
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.parameters() {
        Ok(parameters) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "parameters": parameters,
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_parameter_get(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid parameter get params: {error}"));
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };
    let Some(value) = instance.backend.get_param_normalized(params.parameter_id) else {
        return response_error(id, 4040, "edit controller not available");
    };

    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "parameterId": params.parameter_id,
            "valueNormalized": value,
        }),
    )
}

pub(super) fn handle_instance_parameter_set(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterSetParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid parameter set params: {error}"));
        }
    };
    if !params.value_normalized.is_finite() || !(0.0..=1.0).contains(&params.value_normalized) {
        return response_error(id, 4220, "valueNormalized must be finite in [0, 1]");
    }
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance
        .backend
        .set_param_normalized(params.parameter_id, params.value_normalized)
    {
        Ok(()) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "parameterId": params.parameter_id,
                "valueNormalized": params.value_normalized,
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_get_state(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceStateParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid get state params: {error}"));
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    let component_state = match instance.backend.component_state() {
        Ok(state) => state,
        Err(error) => return response_error(id, 4220, error),
    };
    let controller_state = match instance.backend.controller_state() {
        Ok(state) => state,
        Err(error) => return response_error(id, 4220, error),
    };
    if component_state.is_none() && controller_state.is_none() {
        return response_error(id, 4040, "plugin state not available");
    }

    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "componentStateBase64": component_state.map(|state| BASE64.encode(state)),
            "controllerStateBase64": controller_state.as_ref().map(|state| BASE64.encode(state)),
            "stateBase64": controller_state.map(|state| BASE64.encode(state)),
        }),
    )
}

pub(super) fn handle_instance_set_state(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceSetStateParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid set state params: {error}"));
        }
    };
    let state_bytes = match BASE64.decode(params.state_base64.as_bytes()) {
        Ok(bytes) => bytes,
        Err(error) => return response_error(id, 4220, format!("invalid stateBase64: {error}")),
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.set_controller_state(&state_bytes) {
        Ok(()) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "stateBytes": state_bytes.len(),
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}
