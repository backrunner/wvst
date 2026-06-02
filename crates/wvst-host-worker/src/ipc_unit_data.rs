use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{WorkerIpcState, response_error, response_result};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceProgramDataParams {
    instance_id: u64,
    list_id: i32,
    program_index: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceSetProgramDataParams {
    instance_id: u64,
    list_id: i32,
    program_index: i32,
    data_base64: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceUnitDataParams {
    instance_id: u64,
    unit_id: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceSetUnitDataParams {
    instance_id: u64,
    unit_id: i32,
    data_base64: String,
}

pub(super) fn handle_instance_program_data_supported(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceProgramDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid program data params: {error}"));
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.program_data_supported(params.list_id) {
        Ok(supported) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "listId": params.list_id,
                "programIndex": params.program_index,
                "supported": supported,
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_get_program_data(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceProgramDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid get program data params: {error}"),
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

    match instance
        .backend
        .get_program_data(params.list_id, params.program_index)
    {
        Ok(data) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "listId": params.list_id,
                "programIndex": params.program_index,
                "dataBase64": BASE64.encode(&data),
                "dataBytes": data.len(),
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_set_program_data(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceSetProgramDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid set program data params: {error}"),
            );
        }
    };
    let data = match decode_base64("dataBase64", &params.data_base64) {
        Ok(data) => data,
        Err(error) => return response_error(id, 4220, error),
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance
        .backend
        .set_program_data(params.list_id, params.program_index, &data)
    {
        Ok(()) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "listId": params.list_id,
                "programIndex": params.program_index,
                "dataBytes": data.len(),
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_unit_data_supported(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid unit data params: {error}"));
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.unit_data_supported(params.unit_id) {
        Ok(supported) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "unitId": params.unit_id,
                "supported": supported,
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_get_unit_data(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid get unit data params: {error}"));
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.get_unit_data(params.unit_id) {
        Ok(data) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "unitId": params.unit_id,
                "dataBase64": BASE64.encode(&data),
                "dataBytes": data.len(),
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_set_unit_data(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceSetUnitDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid set unit data params: {error}"));
        }
    };
    let data = match decode_base64("dataBase64", &params.data_base64) {
        Ok(data) => data,
        Err(error) => return response_error(id, 4220, error),
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.set_unit_data(params.unit_id, &data) {
        Ok(()) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "unitId": params.unit_id,
                "dataBytes": data.len(),
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

fn decode_base64(label: &'static str, value: &str) -> Result<Vec<u8>, String> {
    BASE64
        .decode(value.as_bytes())
        .map_err(|error| format!("invalid {label}: {error}"))
}
