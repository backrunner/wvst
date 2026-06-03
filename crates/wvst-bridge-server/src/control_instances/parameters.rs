use serde_json::Value;

use super::super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::instance_registry::{
    InstanceParameterInfoParams, InstanceParameterNormalizedByPlainParams, InstanceParameterParams,
    InstanceParameterSetParams, InstanceParameterValueByStringParams,
};

pub async fn handle_instance_parameter_get(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid parameter get params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_get(params.instance_id, params.parameter_id)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_parameter_info(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Some(value) = params.value_normalized
        && !is_normalized_value(value)
    {
        return response_error(id, 4220, "valueNormalized must be finite in [0, 1]");
    }
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_info(
            params.instance_id,
            params.parameter_id,
            params.value_normalized,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_parameter_value_by_string(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if params.value.is_empty() {
        return response_error(id, 4220, "value must not be empty");
    }
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_value_by_string(params.instance_id, params.parameter_id, params.value)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_parameter_normalized_by_plain(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_normalized_by_plain(params.instance_id, params.parameter_id, params.value_plain)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_parameter_set(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_set(
            params.instance_id,
            params.parameter_id,
            params.value_normalized,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_parameter_begin_edit(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_begin_edit(params.instance_id, params.parameter_id)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_parameter_perform_edit(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_perform_edit(
            params.instance_id,
            params.parameter_id,
            params.value_normalized,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_parameter_end_edit(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
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
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_end_edit(params.instance_id, params.parameter_id)
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_parameter_edit(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceParameterSetParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid parameter edit params: {error}"),
            );
        }
    };
    if !is_normalized_value(params.value_normalized) {
        return response_error(id, 4220, "valueNormalized must be finite in [0, 1]");
    }
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .parameter_edit(
            params.instance_id,
            params.parameter_id,
            params.value_normalized,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

fn is_normalized_value(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
