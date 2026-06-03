use serde::Deserialize;
use serde_json::Value;

use super::super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::instance_registry::InstanceConnectionNotifyParams;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceConnectionNotifyAndRefreshParams {
    instance_id: u64,
    message_id: String,
    #[serde(default)]
    attributes: Value,
    #[serde(default)]
    include_state: bool,
    #[serde(default = "default_include_worker_metrics")]
    include_worker_metrics: bool,
}

const fn default_include_worker_metrics() -> bool {
    true
}

pub async fn handle_instance_notify_component(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    handle_instance_connection_notify(id, params, context, ConnectionNotifyTarget::Component).await
}

pub async fn handle_instance_notify_controller(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    handle_instance_connection_notify(id, params, context, ConnectionNotifyTarget::Controller).await
}

pub async fn handle_instance_notify_component_and_refresh(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    handle_instance_connection_notify_and_refresh(
        id,
        params,
        context,
        ConnectionNotifyTarget::Component,
    )
    .await
}

pub async fn handle_instance_notify_controller_and_refresh(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    handle_instance_connection_notify_and_refresh(
        id,
        params,
        context,
        ConnectionNotifyTarget::Controller,
    )
    .await
}

#[derive(Debug, Clone, Copy)]
enum ConnectionNotifyTarget {
    Component,
    Controller,
}

async fn handle_instance_connection_notify(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
    target: ConnectionNotifyTarget,
) -> String {
    let params = match serde_json::from_value::<InstanceConnectionNotifyParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid connection notify params: {error}"),
            );
        }
    };
    if params.message_id.is_empty() {
        return response_error(id, -32602, "messageId is required");
    }
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    let attributes = match normalize_connection_notify_attributes(params.attributes) {
        Ok(attributes) => attributes,
        Err(message) => return response_error(id, -32602, message),
    };

    let result = match target {
        ConnectionNotifyTarget::Component => {
            context
                .workers
                .notify_component(params.instance_id, params.message_id, attributes)
                .await
        }
        ConnectionNotifyTarget::Controller => {
            context
                .workers
                .notify_controller(params.instance_id, params.message_id, attributes)
                .await
        }
    };

    match result {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

async fn handle_instance_connection_notify_and_refresh(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
    target: ConnectionNotifyTarget,
) -> String {
    let params = match serde_json::from_value::<InstanceConnectionNotifyAndRefreshParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid connection notify-and-refresh params: {error}"),
            );
        }
    };
    if params.message_id.is_empty() {
        return response_error(id, -32602, "messageId is required");
    }
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    let attributes = match normalize_connection_notify_attributes(params.attributes) {
        Ok(attributes) => attributes,
        Err(message) => return response_error(id, -32602, message),
    };

    let result = match target {
        ConnectionNotifyTarget::Component => {
            context
                .workers
                .notify_component_and_refresh(
                    params.instance_id,
                    params.message_id,
                    attributes,
                    params.include_state,
                    params.include_worker_metrics,
                )
                .await
        }
        ConnectionNotifyTarget::Controller => {
            context
                .workers
                .notify_controller_and_refresh(
                    params.instance_id,
                    params.message_id,
                    attributes,
                    params.include_state,
                    params.include_worker_metrics,
                )
                .await
        }
    };

    match result {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

fn normalize_connection_notify_attributes(attributes: Value) -> Result<Value, &'static str> {
    if attributes.is_object() {
        Ok(attributes)
    } else if attributes.is_null() {
        Ok(serde_json::json!({}))
    } else {
        Err("attributes must be an object")
    }
}
