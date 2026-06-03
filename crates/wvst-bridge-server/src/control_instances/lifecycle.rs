use serde_json::{Value, json};

use super::super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use super::recovery::{
    handle_status_worker_failure, mark_restarted_instance, start_instance_with_idle_reclaim,
};
use super::worker_events::{
    destroy_instance_record, emit_policy_decision, emit_recovery_failed, emit_worker_destroyed,
    emit_worker_destroying, emit_worker_error, emit_worker_start_policy_rejection,
};
use crate::events::{BridgeEventKind, WorkerRecoveryMode};
use crate::instance_registry::{
    InstanceCreateParams, InstanceDestroyParams, InstanceError, InstanceProcessingParams,
    InstanceRestartParams, InstanceState, InstanceStatusParams, WorkerRuntimeInfo,
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
    let plugin_id = params.plugin_id.clone();
    if context
        .workers
        .release_expired_quarantine(&plugin_id)
        .await
        .is_some()
    {
        emit_policy_decision(
            &context,
            None,
            Some(&plugin_id),
            "plugin-quarantine",
            "release",
            "quarantine-expired",
            None,
        );
        context
            .events
            .emit(BridgeEventKind::WorkerQuarantineReleased {
                plugin_id: plugin_id.clone(),
            });
    }

    let record = match context.instances.create(params, &plugin) {
        Ok(record) => record,
        Err(error) => return response_instance_error(id, error),
    };
    context
        .component_handler_events
        .reset_instance(record.instance_id);
    let _ = context.instances.mark_worker_starting(record.instance_id);
    context.events.emit(BridgeEventKind::WorkerStarting {
        instance_id: record.instance_id,
        plugin_id: record.plugin_id.clone(),
    });

    match start_instance_with_idle_reclaim(&record, &context).await {
        Ok(worker) => match context.instances.mark_worker_ready_with_runtime(
            record.instance_id,
            WorkerRuntimeInfo::from_worker_result(&worker),
        ) {
            Ok(record) => {
                context.events.emit(BridgeEventKind::WorkerReady {
                    instance_id: record.instance_id,
                    plugin_id: record.plugin_id.clone(),
                });
                response_result(id, json!(record))
            }
            Err(error) => response_instance_error(id, error),
        },
        Err(error) => {
            context.metrics.increment_worker_failures();
            let _ = context.instances.mark_worker_failed(record.instance_id);
            let _ = context.instances.destroy(InstanceDestroyParams {
                instance_id: record.instance_id,
            });
            emit_worker_start_policy_rejection(
                &context,
                record.instance_id,
                &record.plugin_id,
                &error,
            );
            emit_worker_error(&context, record.instance_id, &record.plugin_id, &error);
            if let Some(status) = context.workers.quarantine_status(&record.plugin_id).await {
                context.events.emit(BridgeEventKind::WorkerQuarantined {
                    plugin_id: record.plugin_id.clone(),
                    failures: status.failures,
                    release_after_ms: status.release_after_ms,
                });
            }
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

    let existing = context.instances.get(params.instance_id).ok();
    let Some(record) = existing else {
        return response_instance_error(id, InstanceError::InstanceNotFound(params.instance_id));
    };

    emit_worker_destroying(&context, &record);
    match destroy_instance_record(&record, &context).await {
        Ok(result) => {
            emit_worker_destroyed(&context, record);
            response_result(id, json!(result))
        }
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
    let was_processing = record.state == InstanceState::Processing;
    let _ = context.instances.mark_worker_recovering(record.instance_id);
    context
        .component_handler_events
        .reset_instance(record.instance_id);
    context.events.emit(BridgeEventKind::WorkerRecovering {
        instance_id: record.instance_id,
        plugin_id: record.plugin_id.clone(),
        mode: WorkerRecoveryMode::ManualRestart,
        reason: "manual-restart".to_string(),
        error_data: None,
    });
    emit_policy_decision(
        &context,
        Some(record.instance_id),
        Some(&record.plugin_id),
        "worker-recovery",
        "restart",
        "manual-restart",
        Some(json!({ "wasProcessing": was_processing })),
    );

    match context.workers.restart_instance(&record).await {
        Ok(worker) => {
            context.metrics.increment_worker_restarts();
            mark_restarted_instance(id, record, worker, was_processing, context).await
        }
        Err(error) => {
            context.metrics.increment_worker_failures();
            let _ = context.instances.mark_worker_failed(record.instance_id);
            emit_recovery_failed(
                &context,
                record.instance_id,
                &record.plugin_id,
                WorkerRecoveryMode::ManualRestart,
                "restart-failed",
                &error,
            );
            response_worker_supervisor_error(id, error)
        }
    }
}

pub async fn handle_instance_start(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<InstanceProcessingParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance start params: {error}"),
            );
        }
    };

    let record = match context.instances.get(params.instance_id) {
        Ok(record) => record,
        Err(error) => return response_instance_error(id, error),
    };
    if let Err(error) = context.instances.mark_worker_starting(params.instance_id) {
        return response_instance_error(id, error);
    }
    context
        .events
        .emit(BridgeEventKind::WorkerProcessingStarting {
            instance_id: record.instance_id,
            plugin_id: record.plugin_id.clone(),
        });

    match context.workers.start_processing(params.instance_id).await {
        Ok(worker) => match context.instances.mark_processing(params.instance_id) {
            Ok(instance) => {
                context.events.emit(BridgeEventKind::WorkerProcessing {
                    instance_id: instance.instance_id,
                });
                response_result(id, json!({ "instance": instance, "worker": worker }))
            }
            Err(error) => response_instance_error(id, error),
        },
        Err(error) => {
            context.metrics.increment_worker_failures();
            let _ = context.instances.mark_worker_failed(params.instance_id);
            emit_worker_error(&context, params.instance_id, &record.plugin_id, &error);
            response_worker_supervisor_error(id, error)
        }
    }
}

pub async fn handle_instance_stop(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = match serde_json::from_value::<InstanceProcessingParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid instance stop params: {error}"));
        }
    };

    let record = match context.instances.get(params.instance_id) {
        Ok(record) => record,
        Err(error) => return response_instance_error(id, error),
    };
    if let Err(error) = context.instances.mark_stopping(params.instance_id) {
        return response_instance_error(id, error);
    }
    context
        .events
        .emit(BridgeEventKind::WorkerProcessingStopping {
            instance_id: record.instance_id,
            plugin_id: record.plugin_id.clone(),
        });

    match context.workers.stop_processing(params.instance_id).await {
        Ok(worker) => match context.instances.mark_stopped(params.instance_id) {
            Ok(instance) => {
                context.events.emit(BridgeEventKind::WorkerStopped {
                    instance_id: instance.instance_id,
                });
                response_result(id, json!({ "instance": instance, "worker": worker }))
            }
            Err(error) => response_instance_error(id, error),
        },
        Err(error) => {
            context.metrics.increment_worker_failures();
            let _ = context.instances.mark_worker_failed(params.instance_id);
            emit_worker_error(&context, params.instance_id, &record.plugin_id, &error);
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

    let record = match context.instances.get(params.instance_id) {
        Ok(record) => record,
        Err(error) => return response_instance_error(id, error),
    };

    match context.workers.heartbeat_instance(params.instance_id).await {
        Ok(worker) => match context.instances.mark_worker_ready(params.instance_id) {
            Ok(instance) => {
                context
                    .component_handler_events
                    .publish_from_worker_metrics(context.events, &instance, &worker);
                response_result(id, json!({ "instance": instance, "worker": worker }))
            }
            Err(error) => response_instance_error(id, error),
        },
        Err(error) => handle_status_worker_failure(id, record, error, context).await,
    }
}
