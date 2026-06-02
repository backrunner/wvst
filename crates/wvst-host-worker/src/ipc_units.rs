use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::{Value, json};
use wvst_vst3_host::Vst3BusDirection;

use super::{WorkerIpcState, response_error, response_result};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceSelectUnitParams {
    instance_id: u64,
    unit_id: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceUnitByBusParams {
    instance_id: u64,
    direction: String,
    bus_index: i32,
    channel: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceUnitProgramDataParams {
    instance_id: u64,
    list_or_unit_id: i32,
    program_index: i32,
    data_base64: String,
}

pub(super) fn handle_instance_select_unit(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceSelectUnitParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid select unit params: {error}"));
        }
    };
    let Some(instance) = state.instances.get(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    match instance.backend.select_unit(params.unit_id) {
        Ok(selected_unit_id) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "unitId": params.unit_id,
                "selectedUnitId": selected_unit_id,
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_unit_by_bus(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitByBusParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid unit by bus params: {error}"));
        }
    };
    let direction = match parse_audio_bus_direction(&params.direction) {
        Ok(direction) => direction,
        Err(error) => return response_error(id, -32602, error),
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
        .unit_by_audio_bus(direction, params.bus_index, params.channel)
    {
        Ok(unit_id) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "direction": params.direction,
                "busIndex": params.bus_index,
                "channel": params.channel,
                "unitId": unit_id,
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

pub(super) fn handle_instance_set_unit_program_data(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitProgramDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid unit program data params: {error}"),
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

    match instance.backend.set_unit_program_data(
        params.list_or_unit_id,
        params.program_index,
        &data,
    ) {
        Ok(()) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "listOrUnitId": params.list_or_unit_id,
                "programIndex": params.program_index,
                "dataBytes": data.len(),
            }),
        ),
        Err(error) => response_error(id, 4220, error),
    }
}

fn parse_audio_bus_direction(value: &str) -> Result<Vst3BusDirection, String> {
    match value {
        "input" => Ok(Vst3BusDirection::Input),
        "output" => Ok(Vst3BusDirection::Output),
        _ => Err(format!("invalid audio bus direction: {value}")),
    }
}

fn decode_base64(label: &'static str, value: &str) -> Result<Vec<u8>, String> {
    BASE64
        .decode(value.as_bytes())
        .map_err(|error| format!("invalid {label}: {error}"))
}
