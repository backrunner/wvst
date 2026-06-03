use serde::Deserialize;
use serde_json::Value;

use super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::instance_registry::{
    InstanceProgramDataParams, InstanceSelectUnitParams, InstanceSetProgramDataParams,
    InstanceSetUnitDataParams, InstanceUnitByBusParams, InstanceUnitDataParams,
    InstanceUnitProgramDataParams,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceUnitProgramDataSetAndRefreshParams {
    instance_id: u64,
    list_or_unit_id: i32,
    program_index: i32,
    data_base64: String,
    #[serde(default)]
    include_state: bool,
    #[serde(default = "default_include_worker_metrics")]
    include_worker_metrics: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceProgramDataSetAndRefreshParams {
    instance_id: u64,
    list_id: i32,
    program_index: i32,
    data_base64: String,
    #[serde(default)]
    include_state: bool,
    #[serde(default = "default_include_worker_metrics")]
    include_worker_metrics: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceUnitDataSetAndRefreshParams {
    instance_id: u64,
    unit_id: i32,
    data_base64: String,
    #[serde(default)]
    include_state: bool,
    #[serde(default = "default_include_worker_metrics")]
    include_worker_metrics: bool,
}

const fn default_include_worker_metrics() -> bool {
    true
}

pub async fn handle_instance_select_unit(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceSelectUnitParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid select unit params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .select_unit(params.instance_id, params.unit_id)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_unit_by_bus(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitByBusParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid unit by bus params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .unit_by_bus(
            params.instance_id,
            params.direction,
            params.bus_index,
            params.channel,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_set_unit_program_data(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .set_unit_program_data(
            params.instance_id,
            params.list_or_unit_id,
            params.program_index,
            params.data_base64,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_set_unit_program_data_and_refresh(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitProgramDataSetAndRefreshParams>(params)
    {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid unit program data set-and-refresh params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .set_unit_program_data_and_refresh(
            params.instance_id,
            params.list_or_unit_id,
            params.program_index,
            params.data_base64,
            params.include_state,
            params.include_worker_metrics,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_program_data_supported(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceProgramDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid program data params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .program_data_supported(params.instance_id, params.list_id, params.program_index)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_get_program_data(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .get_program_data(params.instance_id, params.list_id, params.program_index)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_set_program_data(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .set_program_data(
            params.instance_id,
            params.list_id,
            params.program_index,
            params.data_base64,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_set_program_data_and_refresh(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceProgramDataSetAndRefreshParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid set program data set-and-refresh params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .set_program_data_and_refresh(
            params.instance_id,
            params.list_id,
            params.program_index,
            params.data_base64,
            params.include_state,
            params.include_worker_metrics,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_unit_data_supported(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid unit data params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .unit_data_supported(params.instance_id, params.unit_id)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_set_unit_data_and_refresh(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitDataSetAndRefreshParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid set unit data set-and-refresh params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .set_unit_data_and_refresh(
            params.instance_id,
            params.unit_id,
            params.data_base64,
            params.include_state,
            params.include_worker_metrics,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_get_unit_data(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceUnitDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid get unit data params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .get_unit_data(params.instance_id, params.unit_id)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_set_unit_data(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceSetUnitDataParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid set unit data params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .set_unit_data(params.instance_id, params.unit_id, params.data_base64)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}
