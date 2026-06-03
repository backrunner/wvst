use serde_json::Value;

use super::super::ControlContext;
use crate::events::{BridgeEventKind, WorkerRecoveryMode};
use crate::instance_registry::{InstanceDestroyParams, InstanceError, InstanceRecord};
use crate::worker_supervisor::WorkerSupervisorError;

pub(super) async fn destroy_instance_record(
    record: &InstanceRecord,
    context: &ControlContext<'_>,
) -> Result<crate::instance_registry::InstanceDestroyResult, InstanceError> {
    context
        .component_handler_events
        .reset_instance(record.instance_id);
    let _ = context
        .shared_memory_pumps
        .stop_by_instance(record.instance_id);
    let _ = context
        .workers
        .detach_shared_memory(record.instance_id)
        .await;
    let _ = context.workers.destroy_instance(record.instance_id).await;
    let _ = context
        .shared_memory
        .destroy_by_instance(record.instance_id);
    let result = context.instances.destroy(InstanceDestroyParams {
        instance_id: record.instance_id,
    })?;
    context.stream_tracker.reset(record.stream_id);
    Ok(result)
}

pub(super) fn emit_worker_destroying(context: &ControlContext<'_>, record: &InstanceRecord) {
    context.events.emit(BridgeEventKind::WorkerDestroying {
        instance_id: record.instance_id,
        plugin_id: record.plugin_id.clone(),
        stream_id: record.stream_id,
    });
}

pub(super) fn emit_worker_destroyed(context: &ControlContext<'_>, record: InstanceRecord) {
    context.events.emit(BridgeEventKind::WorkerDestroyed {
        instance_id: record.instance_id,
        plugin_id: record.plugin_id,
        stream_id: record.stream_id,
    });
}

pub(super) fn emit_worker_error(
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

pub(super) fn emit_worker_start_policy_rejection(
    context: &ControlContext<'_>,
    instance_id: u64,
    plugin_id: &str,
    error: &WorkerSupervisorError,
) {
    match error {
        WorkerSupervisorError::Quarantined { .. } => emit_policy_decision(
            context,
            Some(instance_id),
            Some(plugin_id),
            "plugin-quarantine",
            "reject",
            "active-quarantine",
            Some(error.rpc_data()),
        ),
        WorkerSupervisorError::ResourceLimitExceeded { .. } => emit_policy_decision(
            context,
            Some(instance_id),
            Some(plugin_id),
            "resource-limit",
            "reject",
            "worker-instance-limit",
            Some(error.rpc_data()),
        ),
        _ => {}
    }
}

pub(super) fn emit_policy_decision(
    context: &ControlContext<'_>,
    instance_id: Option<u64>,
    plugin_id: Option<&str>,
    policy: &str,
    decision: &str,
    reason: &str,
    data: Option<Value>,
) {
    context.events.emit(BridgeEventKind::WorkerPolicyDecision {
        instance_id,
        plugin_id: plugin_id.map(ToOwned::to_owned),
        policy: policy.to_string(),
        decision: decision.to_string(),
        reason: reason.to_string(),
        data,
    });
}

pub(super) fn emit_recovery_failed(
    context: &ControlContext<'_>,
    instance_id: u64,
    plugin_id: &str,
    mode: WorkerRecoveryMode,
    reason: &str,
    error: &WorkerSupervisorError,
) {
    context.events.emit(BridgeEventKind::WorkerRecoveryFailed {
        instance_id,
        plugin_id: plugin_id.to_string(),
        mode,
        reason: reason.to_string(),
        error_data: Some(error.rpc_data()),
    });
    emit_worker_error(context, instance_id, plugin_id, error);
}
