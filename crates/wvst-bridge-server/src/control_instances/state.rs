use serde::Deserialize;
use serde_json::Value;

use super::super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::instance_registry::{InstanceSetStateParams, InstanceStatusParams};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstanceStateSetAndRefreshParams {
    instance_id: u64,
    #[serde(default)]
    component_state_base64: Option<String>,
    #[serde(default)]
    controller_state_base64: Option<String>,
    #[serde(default)]
    include_state: bool,
    #[serde(default = "default_include_worker_metrics")]
    include_worker_metrics: bool,
}

const fn default_include_worker_metrics() -> bool {
    true
}

pub async fn handle_instance_get_state(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceStatusParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid get state params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context.workers.get_state(params.instance_id).await {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_set_state(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceSetStateParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid set state params: {error}"));
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .set_state(
            params.instance_id,
            params.component_state_base64,
            params.controller_state_base64,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

pub async fn handle_instance_state_set_and_refresh(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceStateSetAndRefreshParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid state set-and-refresh params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .workers
        .set_state_and_refresh(
            params.instance_id,
            params.component_state_base64,
            params.controller_state_base64,
            params.include_state,
            params.include_worker_metrics,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn set_and_refresh_rejects_removed_state_base64_alias() {
        let error = serde_json::from_value::<InstanceStateSetAndRefreshParams>(json!({
            "instanceId": 1,
            "stateBase64": "AQID",
        }))
        .expect_err("removed alias should be rejected");

        assert!(error.to_string().contains("unknown field `stateBase64`"));
    }
}
