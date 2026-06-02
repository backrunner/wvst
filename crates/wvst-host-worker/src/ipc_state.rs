use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    WorkerIpcState, ipc_payload::decode_control_base64, response_backend_error, response_error,
    response_result,
};

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
        Err(error) => return response_backend_error(id, 4220, &error),
    };
    let controller_state = match instance.backend.controller_state() {
        Ok(state) => state,
        Err(error) => return response_backend_error(id, 4220, &error),
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
        return response_backend_error(id, 4220, &error);
    }
    if let Some(state_bytes) = controller_state.as_deref()
        && let Err(error) = instance.backend.set_controller_state(state_bytes)
    {
        return response_backend_error(id, 4220, &error);
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
    value
        .map(|value| decode_control_base64(label, value))
        .transpose()
}
