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
struct InstanceParameterInfoParams {
    instance_id: u64,
    parameter_id: u32,
    #[serde(default)]
    value_normalized: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceParameterValueByStringParams {
    instance_id: u64,
    parameter_id: u32,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceParameterNormalizedByPlainParams {
    instance_id: u64,
    parameter_id: u32,
    value_plain: f64,
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
    #[serde(default)]
    state_base64: Option<String>,
    #[serde(default)]
    component_state_base64: Option<String>,
    #[serde(default)]
    controller_state_base64: Option<String>,
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

pub(super) fn handle_instance_units(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceStateParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance units params: {error}"),
            );
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.unit_metadata() {
        Ok(unit_info) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "unitInfo": unit_info,
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
    if !is_normalized_value(value) {
        return response_error(
            id,
            4220,
            "controller returned valueNormalized outside [0, 1]",
        );
    }

    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "parameterId": params.parameter_id,
            "valueNormalized": value,
        }),
    )
}

pub(super) fn handle_instance_parameter_info(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterInfoParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid parameter info params: {error}"),
            );
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    let value_normalized = match params.value_normalized {
        Some(value) => {
            if !is_normalized_value(value) {
                return response_error(id, 4220, "valueNormalized must be finite in [0, 1]");
            }
            value
        }
        None => match instance.backend.get_param_normalized(params.parameter_id) {
            Some(value) if is_normalized_value(value) => value,
            Some(_) => {
                return response_error(
                    id,
                    4220,
                    "controller returned valueNormalized outside [0, 1]",
                );
            }
            None => return response_error(id, 4040, "edit controller not available"),
        },
    };
    parameter_info_response(
        id,
        params.instance_id,
        params.parameter_id,
        value_normalized,
        instance,
    )
}

pub(super) fn handle_instance_parameter_value_by_string(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterValueByStringParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid parameter value-by-string params: {error}"),
            );
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    let value_normalized = match instance
        .backend
        .param_value_by_string(params.parameter_id, &params.value)
    {
        Ok(value) if is_normalized_value(value) => value,
        Ok(_) => return response_error(id, 4220, "converted valueNormalized is outside [0, 1]"),
        Err(error) => return response_error(id, 4220, error),
    };
    parameter_info_response(
        id,
        params.instance_id,
        params.parameter_id,
        value_normalized,
        instance,
    )
}

pub(super) fn handle_instance_parameter_normalized_by_plain(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterNormalizedByPlainParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid parameter normalized-by-plain params: {error}"),
            );
        }
    };
    if !params.value_plain.is_finite() {
        return response_error(id, 4220, "valuePlain must be finite");
    }
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    let Some(value_normalized) = instance
        .backend
        .plain_param_to_normalized(params.parameter_id, params.value_plain)
    else {
        return response_error(id, 4040, "edit controller not available");
    };
    if !is_normalized_value(value_normalized) {
        return response_error(id, 4220, "converted valueNormalized is outside [0, 1]");
    }

    parameter_info_response(
        id,
        params.instance_id,
        params.parameter_id,
        value_normalized,
        instance,
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
    if !is_normalized_value(params.value_normalized) {
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

pub(super) fn handle_instance_parameter_begin_edit(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid parameter begin edit params: {error}"),
            );
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.begin_param_edit(params.parameter_id) {
        Ok(()) => parameter_edit_response(
            id,
            params.instance_id,
            params.parameter_id,
            "begin-edit",
            None,
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_parameter_perform_edit(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterSetParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid parameter perform edit params: {error}"),
            );
        }
    };
    if !is_normalized_value(params.value_normalized) {
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
        .perform_param_edit(params.parameter_id, params.value_normalized)
    {
        Ok(()) => parameter_edit_response(
            id,
            params.instance_id,
            params.parameter_id,
            "perform-edit",
            Some(params.value_normalized),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_parameter_end_edit(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid parameter end edit params: {error}"),
            );
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.end_param_edit(params.parameter_id) {
        Ok(()) => parameter_edit_response(
            id,
            params.instance_id,
            params.parameter_id,
            "end-edit",
            None,
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

fn is_normalized_value(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn parameter_info_response(
    id: Value,
    instance_id: u64,
    parameter_id: u32,
    value_normalized: f64,
    instance: &super::WorkerInstance,
) -> String {
    let value_plain = instance
        .backend
        .normalized_param_to_plain(parameter_id, value_normalized)
        .and_then(finite_plain_value);
    let value_string = match instance
        .backend
        .param_string_by_value(parameter_id, value_normalized)
    {
        Ok(value) => value,
        Err(error) => return response_error(id, 4220, error),
    };

    response_result(
        id,
        json!({
            "instanceId": instance_id,
            "parameterId": parameter_id,
            "valueNormalized": value_normalized,
            "valuePlain": value_plain,
            "valueString": value_string,
        }),
    )
}

fn parameter_edit_response(
    id: Value,
    instance_id: u64,
    parameter_id: u32,
    edit_kind: &'static str,
    value_normalized: Option<f64>,
) -> String {
    response_result(
        id,
        json!({
            "instanceId": instance_id,
            "parameterId": parameter_id,
            "editKind": edit_kind,
            "valueNormalized": value_normalized,
        }),
    )
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
    let component_state = match decode_optional_base64(
        "componentStateBase64",
        params.component_state_base64.as_deref(),
    ) {
        Ok(bytes) => bytes,
        Err(error) => return response_error(id, 4220, error),
    };
    let controller_state = match decode_optional_base64(
        "controllerStateBase64",
        params
            .controller_state_base64
            .as_deref()
            .or(params.state_base64.as_deref()),
    ) {
        Ok(bytes) => bytes,
        Err(error) => return response_error(id, 4220, error),
    };
    if component_state.is_none() && controller_state.is_none() {
        return response_error(
            id,
            -32602,
            "set state requires componentStateBase64, controllerStateBase64, or stateBase64",
        );
    }

    let Some(instance) = state.instances.get_mut(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    if let Some(state_bytes) = component_state.as_deref()
        && let Err(error) = instance.backend.set_component_state(state_bytes)
    {
        return response_error(id, 4220, error);
    }
    if let Some(state_bytes) = controller_state.as_deref()
        && let Err(error) = instance.backend.set_controller_state(state_bytes)
    {
        return response_error(id, 4220, error);
    }

    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "componentStateBytes": component_state.as_ref().map(Vec::len),
            "controllerStateBytes": controller_state.as_ref().map(Vec::len),
            "stateBytes": controller_state.as_ref().map(Vec::len),
        }),
    )
}

fn decode_optional_base64(
    label: &'static str,
    value: Option<&str>,
) -> Result<Option<Vec<u8>>, String> {
    value.map(|value| decode_base64(label, value)).transpose()
}

fn decode_base64(label: &'static str, value: &str) -> Result<Vec<u8>, String> {
    BASE64
        .decode(value.as_bytes())
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn finite_plain_value(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_normalized_values() {
        assert!(is_normalized_value(0.0));
        assert!(is_normalized_value(1.0));
        assert!(!is_normalized_value(-0.01));
        assert!(!is_normalized_value(1.01));
        assert!(!is_normalized_value(f64::NAN));
    }

    #[test]
    fn filters_non_finite_plain_values() {
        assert_eq!(finite_plain_value(12.5), Some(12.5));
        assert_eq!(finite_plain_value(f64::NAN), None);
        assert_eq!(finite_plain_value(f64::INFINITY), None);
        assert_eq!(finite_plain_value(f64::NEG_INFINITY), None);
    }
}
