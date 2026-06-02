use std::time::Duration;

use serde_json::{Value, json};

use super::{
    ControlContext, response_error, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::events::{BridgeEventKind, WorkerRecoveryMode};
use crate::instance_registry::{
    InstanceConnectionNotifyParams, InstanceCreateParams, InstanceDestroyParams, InstanceError,
    InstanceParameterInfoParams, InstanceParameterNormalizedByPlainParams, InstanceParameterParams,
    InstanceParameterSetParams, InstanceParameterValueByStringParams, InstanceProcessingParams,
    InstanceRestartParams, InstanceSetStateParams, InstanceState, InstanceStatusParams,
    StreamLifecycleParams, WorkerRuntimeInfo,
};
use crate::worker_supervisor::WorkerSupervisorError;

const STREAM_CLOSE_DRAIN_TIMEOUT: Duration = Duration::from_millis(500);

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

    match context.workers.start_instance(&record).await {
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
    if let Some(record) = &existing {
        context.events.emit(BridgeEventKind::WorkerDestroying {
            instance_id: record.instance_id,
            plugin_id: record.plugin_id.clone(),
            stream_id: record.stream_id,
        });
    }
    context
        .component_handler_events
        .reset_instance(params.instance_id);
    let _ = context.workers.destroy_instance(params.instance_id).await;

    match context.instances.destroy(params) {
        Ok(result) => {
            if let Some(record) = existing {
                let stream_id = record.stream_id;
                context.stream_tracker.reset(stream_id);
                context.events.emit(BridgeEventKind::WorkerDestroyed {
                    instance_id: record.instance_id,
                    plugin_id: record.plugin_id,
                    stream_id,
                });
            }
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

    match context.workers.restart_instance(&record).await {
        Ok(worker) => {
            context.metrics.increment_worker_restarts();
            mark_restarted_instance(id, record, worker, was_processing, context).await
        }
        Err(error) => {
            context.metrics.increment_worker_failures();
            let _ = context.instances.mark_worker_failed(record.instance_id);
            emit_worker_error(&context, record.instance_id, &record.plugin_id, &error);
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

async fn handle_status_worker_failure(
    id: Value,
    record: crate::instance_registry::InstanceRecord,
    error: WorkerSupervisorError,
    context: ControlContext<'_>,
) -> String {
    context.metrics.increment_worker_failures();
    if !context.config.worker_auto_restart_enabled() {
        let _ = context.instances.mark_worker_failed(record.instance_id);
        emit_worker_error(&context, record.instance_id, &record.plugin_id, &error);
        return response_worker_supervisor_error(id, error);
    }

    let _ = context.instances.mark_worker_recovering(record.instance_id);
    context
        .component_handler_events
        .reset_instance(record.instance_id);
    context.events.emit(BridgeEventKind::WorkerRecovering {
        instance_id: record.instance_id,
        plugin_id: record.plugin_id.clone(),
        mode: WorkerRecoveryMode::AutoHeartbeat,
        reason: "heartbeat-failed".to_string(),
        error_data: Some(error.rpc_data()),
    });

    match context.workers.restart_instance(&record).await {
        Ok(worker) => {
            context.metrics.increment_worker_restarts();
            context.metrics.increment_worker_auto_restarts();
            mark_auto_recovered_instance(id, record, worker, context).await
        }
        Err(restart_error) => {
            let _ = context.instances.mark_worker_failed(record.instance_id);
            emit_worker_error(
                &context,
                record.instance_id,
                &record.plugin_id,
                &restart_error,
            );
            response_worker_supervisor_error(id, restart_error)
        }
    }
}

async fn mark_restarted_instance(
    id: Value,
    record: crate::instance_registry::InstanceRecord,
    worker: Value,
    restore_processing: bool,
    context: ControlContext<'_>,
) -> String {
    let runtime = WorkerRuntimeInfo::from_worker_result(&worker);
    if let Err(error) = context
        .instances
        .mark_worker_ready_with_runtime(record.instance_id, runtime)
    {
        return response_instance_error(id, error);
    }

    if restore_processing {
        return match context.workers.start_processing(record.instance_id).await {
            Ok(worker) => match context.instances.mark_processing(record.instance_id) {
                Ok(instance) => {
                    context.events.emit(BridgeEventKind::WorkerRecovered {
                        instance_id: record.instance_id,
                        plugin_id: record.plugin_id,
                        processing_restored: true,
                        mode: WorkerRecoveryMode::ManualRestart,
                    });
                    response_result(
                        id,
                        json!({ "instance": instance, "worker": worker, "restarted": true }),
                    )
                }
                Err(error) => response_instance_error(id, error),
            },
            Err(error) => {
                let _ = context.instances.mark_worker_failed(record.instance_id);
                emit_worker_error(&context, record.instance_id, &record.plugin_id, &error);
                response_worker_supervisor_error(id, error)
            }
        };
    }

    match context.instances.get(record.instance_id) {
        Ok(instance) => {
            context.events.emit(BridgeEventKind::WorkerRecovered {
                instance_id: record.instance_id,
                plugin_id: record.plugin_id,
                processing_restored: false,
                mode: WorkerRecoveryMode::ManualRestart,
            });
            response_result(
                id,
                json!({ "instance": instance, "worker": worker, "restarted": true }),
            )
        }
        Err(error) => response_instance_error(id, error),
    }
}

async fn mark_auto_recovered_instance(
    id: Value,
    record: crate::instance_registry::InstanceRecord,
    worker: Value,
    context: ControlContext<'_>,
) -> String {
    let runtime = WorkerRuntimeInfo::from_worker_result(&worker);
    if let Err(error) = context
        .instances
        .mark_worker_ready_with_runtime(record.instance_id, runtime)
    {
        return response_instance_error(id, error);
    }

    if record.state == InstanceState::Processing {
        return match context.workers.start_processing(record.instance_id).await {
            Ok(worker) => match context.instances.mark_processing(record.instance_id) {
                Ok(instance) => {
                    context.events.emit(BridgeEventKind::WorkerRecovered {
                        instance_id: record.instance_id,
                        plugin_id: record.plugin_id,
                        processing_restored: true,
                        mode: WorkerRecoveryMode::AutoHeartbeat,
                    });
                    response_result(
                        id,
                        json!({ "instance": instance, "worker": worker, "recovered": true }),
                    )
                }
                Err(error) => response_instance_error(id, error),
            },
            Err(error) => {
                let _ = context.instances.mark_worker_failed(record.instance_id);
                emit_worker_error(&context, record.instance_id, &record.plugin_id, &error);
                response_worker_supervisor_error(id, error)
            }
        };
    }

    match context.instances.get(record.instance_id) {
        Ok(instance) => {
            context.events.emit(BridgeEventKind::WorkerRecovered {
                instance_id: record.instance_id,
                plugin_id: record.plugin_id,
                processing_restored: false,
                mode: WorkerRecoveryMode::AutoHeartbeat,
            });
            response_result(
                id,
                json!({ "instance": instance, "worker": worker, "recovered": true }),
            )
        }
        Err(error) => response_instance_error(id, error),
    }
}

fn emit_worker_error(
    context: &ControlContext<'_>,
    instance_id: u64,
    plugin_id: &str,
    error: &WorkerSupervisorError,
) {
    if let WorkerSupervisorError::Quarantined {
        plugin_id,
        failures,
        release_after_ms,
    } = error
    {
        context.events.emit(BridgeEventKind::WorkerQuarantined {
            plugin_id: plugin_id.clone(),
            failures: *failures,
            release_after_ms: *release_after_ms,
        });
    }

    context.events.emit(BridgeEventKind::WorkerFailed {
        instance_id,
        plugin_id: plugin_id.to_string(),
        code: error.rpc_code(),
        message: error.rpc_message(),
        error_data: Some(error.rpc_data()),
    });
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

fn is_normalized_value(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
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
            params.state_base64,
            params.component_state_base64,
            params.controller_state_base64,
        )
        .await
    {
        Ok(result) => response_result(id, result),
        Err(error) => response_worker_supervisor_error(id, error),
    }
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

fn normalize_connection_notify_attributes(attributes: Value) -> Result<Value, &'static str> {
    if attributes.is_object() {
        Ok(attributes)
    } else if attributes.is_null() {
        Ok(serde_json::json!({}))
    } else {
        Err("attributes must be an object")
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
        Ok(record) => {
            context.stream_tracker.reset(record.stream_id);
            context.events.emit(BridgeEventKind::StreamOpened {
                instance_id: record.instance_id,
                plugin_id: record.plugin_id.clone(),
                stream_id: record.stream_id,
            });
            response_result(id, json!(record))
        }
        Err(error) => response_instance_error(id, error),
    }
}

pub async fn handle_stream_close(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = match serde_json::from_value::<StreamLifecycleParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid stream close params: {error}"));
        }
    };

    match context.instances.close_stream(params) {
        Ok(record) => {
            context.events.emit(BridgeEventKind::StreamClosing {
                instance_id: record.instance_id,
                plugin_id: record.plugin_id.clone(),
                stream_id: record.stream_id,
            });
            let drained = context
                .audio_in_flight
                .wait_until_idle(record.stream_id, STREAM_CLOSE_DRAIN_TIMEOUT)
                .await;
            context.stream_tracker.reset(record.stream_id);
            context.events.emit(BridgeEventKind::StreamClosed {
                instance_id: record.instance_id,
                plugin_id: record.plugin_id.clone(),
                stream_id: record.stream_id,
                drain_timed_out: !drained,
            });
            response_result(id, json!(record))
        }
        Err(error) => response_instance_error(id, error),
    }
}
