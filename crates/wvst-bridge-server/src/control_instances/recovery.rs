use serde_json::{Value, json};

use super::super::{
    ControlContext, response_instance_error, response_result, response_worker_supervisor_error,
};
use super::worker_events::{
    destroy_instance_record, emit_policy_decision, emit_recovery_failed, emit_worker_destroyed,
    emit_worker_destroying, emit_worker_error,
};
use crate::events::{BridgeEventKind, WorkerRecoveryMode};
use crate::instance_registry::{InstanceRecord, InstanceState, WorkerRuntimeInfo};
use crate::worker_supervisor::WorkerSupervisorError;

pub(super) async fn handle_status_worker_failure(
    id: Value,
    record: crate::instance_registry::InstanceRecord,
    error: WorkerSupervisorError,
    context: ControlContext<'_>,
) -> String {
    context.metrics.increment_worker_failures();
    if !context.config.worker_auto_restart_enabled() {
        let _ = context.instances.mark_worker_failed(record.instance_id);
        emit_policy_decision(
            &context,
            Some(record.instance_id),
            Some(&record.plugin_id),
            "worker-recovery",
            "reject",
            "auto-restart-disabled",
            Some(error.rpc_data()),
        );
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
    emit_policy_decision(
        &context,
        Some(record.instance_id),
        Some(&record.plugin_id),
        "worker-recovery",
        "restart",
        "heartbeat-failed",
        Some(error.rpc_data()),
    );

    match context.workers.restart_instance(&record).await {
        Ok(worker) => {
            context.metrics.increment_worker_restarts();
            context.metrics.increment_worker_auto_restarts();
            mark_auto_recovered_instance(id, record, worker, context).await
        }
        Err(restart_error) => {
            let _ = context.instances.mark_worker_failed(record.instance_id);
            emit_recovery_failed(
                &context,
                record.instance_id,
                &record.plugin_id,
                WorkerRecoveryMode::AutoHeartbeat,
                "restart-failed",
                &restart_error,
            );
            response_worker_supervisor_error(id, restart_error)
        }
    }
}

pub(super) async fn mark_restarted_instance(
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
                emit_recovery_failed(
                    &context,
                    record.instance_id,
                    &record.plugin_id,
                    WorkerRecoveryMode::ManualRestart,
                    "restore-processing-failed",
                    &error,
                );
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
                emit_recovery_failed(
                    &context,
                    record.instance_id,
                    &record.plugin_id,
                    WorkerRecoveryMode::AutoHeartbeat,
                    "restore-processing-failed",
                    &error,
                );
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

pub(super) async fn start_instance_with_idle_reclaim(
    record: &InstanceRecord,
    context: &ControlContext<'_>,
) -> Result<Value, WorkerSupervisorError> {
    loop {
        match context.workers.start_instance(record).await {
            Ok(worker) => return Ok(worker),
            Err(error) => {
                let WorkerSupervisorError::ResourceLimitExceeded { limit, active } = &error else {
                    return Err(error);
                };

                let Some(candidate) = context
                    .instances
                    .idle_worker_reclaim_candidate(&record.plugin_id, record.instance_id)
                else {
                    return Err(error);
                };

                emit_policy_decision(
                    context,
                    Some(record.instance_id),
                    Some(&record.plugin_id),
                    "resource-limit",
                    "reclaim",
                    "idle-worker",
                    Some(json!({
                        "kind": "idle-worker-reclaimed",
                        "resource": "worker-instances",
                        "limit": limit,
                        "active": active,
                        "targetInstanceId": record.instance_id,
                        "reclaimedInstanceId": candidate.instance_id,
                        "reclaimedStreamId": candidate.stream_id,
                    })),
                );
                emit_worker_destroying(context, &candidate);

                if let Err(instance_error) = destroy_instance_record(&candidate, context).await {
                    emit_policy_decision(
                        context,
                        Some(record.instance_id),
                        Some(&record.plugin_id),
                        "resource-limit",
                        "reject",
                        "idle-worker-reclaim-failed",
                        Some(json!({
                            "kind": "idle-worker-reclaim-failed",
                            "targetInstanceId": record.instance_id,
                            "reclaimedInstanceId": candidate.instance_id,
                            "error": instance_error.rpc_data(),
                        })),
                    );
                    return Err(error);
                }

                emit_worker_destroyed(context, candidate);
            }
        }
    }
}
