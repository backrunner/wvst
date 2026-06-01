use serde_json::{Value, json};

use super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::instance_registry::{InstanceCreateParams, InstanceDestroyParams, InstanceError};

pub async fn handle_instance_create(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceCreateParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance create params: {error}"),
            );
        }
    };

    let Some(plugin) = context.plugins.find(&params.plugin_id) else {
        return response_instance_error(id, InstanceError::PluginNotFound(params.plugin_id));
    };

    let record = match context.instances.create(params, &plugin) {
        Ok(record) => record,
        Err(error) => return response_instance_error(id, error),
    };

    match context.workers.start_instance(&record).await {
        Ok(_) => match context.instances.mark_worker_ready(record.instance_id) {
            Ok(record) => response_result(id, json!(record)),
            Err(error) => response_instance_error(id, error),
        },
        Err(error) => {
            let _ = context.instances.mark_worker_failed(record.instance_id);
            let _ = context.instances.destroy(InstanceDestroyParams {
                instance_id: record.instance_id,
            });
            response_worker_supervisor_error(id, error)
        }
    }
}

pub async fn handle_instance_destroy(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceDestroyParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance destroy params: {error}"),
            );
        }
    };

    let _ = context.workers.destroy_instance(params.instance_id).await;

    match context.instances.destroy(params) {
        Ok(result) => response_result(id, json!(result)),
        Err(error) => response_instance_error(id, error),
    }
}
