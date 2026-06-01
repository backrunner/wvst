use serde_json::{Value, json};

use super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::instance_registry::{
    InstanceCreateParams, InstanceDestroyParams, InstanceError, InstanceRestartParams,
    InstanceStatusParams, StreamLifecycleParams,
};

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
            context.metrics.increment_worker_failures();
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

pub async fn handle_instance_restart(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceRestartParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance restart params: {error}"),
            );
        }
    };

    let record = match context.instances.get(params.instance_id) {
        Ok(record) => record,
        Err(error) => return response_instance_error(id, error),
    };

    match context.workers.restart_instance(&record).await {
        Ok(worker) => {
            context.metrics.increment_worker_restarts();
            match context.instances.mark_worker_ready(record.instance_id) {
                Ok(instance) => {
                    response_result(id, json!({ "instance": instance, "worker": worker }))
                }
                Err(error) => response_instance_error(id, error),
            }
        }
        Err(error) => {
            context.metrics.increment_worker_failures();
            let _ = context.instances.mark_worker_failed(record.instance_id);
            response_worker_supervisor_error(id, error)
        }
    }
}

pub async fn handle_instance_status(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceStatusParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance status params: {error}"),
            );
        }
    };

    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context.workers.heartbeat_instance(params.instance_id).await {
        Ok(worker) => match context.instances.mark_worker_ready(params.instance_id) {
            Ok(instance) => response_result(id, json!({ "instance": instance, "worker": worker })),
            Err(error) => response_instance_error(id, error),
        },
        Err(error) => {
            context.metrics.increment_worker_failures();
            let _ = context.instances.mark_worker_failed(params.instance_id);
            response_worker_supervisor_error(id, error)
        }
    }
}

pub fn handle_stream_open(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = match serde_json::from_value::<StreamLifecycleParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid stream open params: {error}"));
        }
    };

    match context.instances.open_stream(params) {
        Ok(record) => response_result(id, json!(record)),
        Err(error) => response_instance_error(id, error),
    }
}

pub fn handle_stream_close(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = match serde_json::from_value::<StreamLifecycleParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid stream close params: {error}"));
        }
    };

    match context.instances.close_stream(params) {
        Ok(record) => response_result(id, json!(record)),
        Err(error) => response_instance_error(id, error),
    }
}
