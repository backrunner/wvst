use serde::Deserialize;
use serde_json::{Value, json};

use super::super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::instance_registry::InstanceStatusParams;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceMetadataRefreshParams {
    instance_id: u64,
    #[serde(default)]
    include_state: bool,
    #[serde(default = "default_include_worker_metrics")]
    include_worker_metrics: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceRuntimeSnapshotParams {
    instance_id: u64,
    #[serde(default)]
    include_state: bool,
    #[serde(default = "default_include_worker_metrics")]
    include_worker_metrics: bool,
    #[serde(default)]
    include_recent_events: bool,
    #[serde(default)]
    after_event_sequence: Option<u64>,
}

const fn default_include_worker_metrics() -> bool {
    true
}

pub async fn handle_instance_parameters(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceStatusParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid instance params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context.workers.parameters(params.instance_id).await {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_units(
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
                format!("invalid instance units params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context.workers.units(params.instance_id).await {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_metadata_refresh(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceMetadataRefreshParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid metadata refresh params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .metadata_refresh(
            params.instance_id,
            params.include_state,
            params.include_worker_metrics,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_runtime_snapshot(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceRuntimeSnapshotParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid runtime snapshot params: {error}"),
            );
        }
    };
    let instance = match context.instances.get(params.instance_id) {
        Ok(instance) => instance,
        Err(error) => return response_instance_error(id, error),
    };

    match context
        .workers
        .metadata_refresh(
            params.instance_id,
            params.include_state,
            params.include_worker_metrics,
        )
        .await
    {
        Ok(metadata) => response_result(
            id,
            json!({
                "instance": instance,
                "metadata": metadata,
                "dataPlane": crate::runtime_snapshot::data_plane_snapshot(
                    params.instance_id,
                    &context,
                ),
                "recentEvents": if params.include_recent_events {
                    json!(context.events.recent_since(params.after_event_sequence))
                } else {
                    Value::Null
                },
            }),
        ),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}
