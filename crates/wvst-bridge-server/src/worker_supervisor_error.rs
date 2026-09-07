use serde_json::{Value, json};

use super::WorkerSupervisorError;
use crate::error_classification::{ErrorClassification, attach_error_classification};

impl WorkerSupervisorError {
    pub(crate) fn is_control_request_rejection(&self) -> bool {
        matches!(self, Self::WorkerRejected { data, .. }
            if data.as_ref().and_then(|value| value.get("kind")).and_then(Value::as_str)
                != Some("vst3-runtime-process"))
    }

    pub(crate) fn is_audio_request_rejection(&self) -> bool {
        matches!(
            self,
            Self::WorkerRejected { data: Some(data), .. }
                if matches!(
                    data.get("kind").and_then(Value::as_str),
                    Some("audio-request-invalid" | "audio-stream-not-found")
                )
        )
    }

    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::WorkerRejected { code, .. } => *code,
            Self::Timeout { .. } => 5035,
            Self::Spawn { .. }
            | Self::MissingPipe(_)
            | Self::Io(_)
            | Self::SupervisionSetup { .. } => 5036,
            Self::InvalidJson(_) | Self::Protocol { .. } => 5037,
            Self::Quarantined { .. } => 4093,
            Self::IncompatibleWorker { .. } => 4094,
            Self::WorkerMissing { .. } | Self::AudioIpcUnavailable { .. } => 4041,
            Self::ResourceLimitExceeded { .. } => 4290,
        }
    }

    pub fn rpc_message(&self) -> String {
        match self {
            Self::Spawn {
                executable,
                message,
            } => format!("failed to start worker {}: {message}", executable.display()),
            Self::MissingPipe(pipe) => format!("worker missing {pipe} pipe"),
            Self::Io(message) => format!("worker io failed: {message}"),
            Self::Timeout {
                method, timeout_ms, ..
            } => {
                format!("worker method {method} timed out after {timeout_ms}ms")
            }
            Self::InvalidJson(message) => format!("worker returned invalid JSON: {message}"),
            Self::Protocol { message, .. } => format!("worker protocol error: {message}"),
            Self::SupervisionSetup { message } => {
                format!("worker supervision setup failed: {message}")
            }
            Self::Quarantined {
                plugin_id,
                failures,
                release_after_ms,
            } => {
                format!(
                    "worker quarantined for plugin {plugin_id} after {failures} failures; retry after {release_after_ms}ms"
                )
            }
            Self::IncompatibleWorker { reason, .. } => {
                format!("worker incompatible: {reason}")
            }
            Self::WorkerMissing { instance_id } => {
                format!("worker not found for instance {instance_id}")
            }
            Self::AudioIpcUnavailable { instance_id } => {
                format!("worker audio IPC unavailable for instance {instance_id}")
            }
            Self::WorkerRejected { message, .. } => message.clone(),
            Self::ResourceLimitExceeded { limit, active } => {
                format!("worker instance limit reached: active {active}, limit {limit}")
            }
        }
    }

    pub fn rpc_data(&self) -> Value {
        let data = match self {
            Self::Spawn {
                executable,
                message,
            } => json!({
                "kind": "spawn",
                "executable": executable.display().to_string(),
                "message": message,
            }),
            Self::MissingPipe(pipe) => json!({ "kind": "missing-pipe", "pipe": pipe }),
            Self::Io(message) => json!({ "kind": "io", "message": message }),
            Self::Timeout {
                method,
                timeout_ms,
                stderr,
            } => {
                json!({ "kind": "timeout", "method": method, "timeoutMs": timeout_ms, "stderr": stderr })
            }
            Self::InvalidJson(message) => json!({ "kind": "invalid-json", "message": message }),
            Self::Protocol { message, stderr } => {
                json!({ "kind": "protocol", "message": message, "stderr": stderr })
            }
            Self::SupervisionSetup { message } => {
                json!({ "kind": "supervision-setup-failed", "message": message })
            }
            Self::Quarantined {
                plugin_id,
                failures,
                release_after_ms,
            } => {
                json!({ "kind": "quarantined", "pluginId": plugin_id, "failures": failures, "releaseAfterMs": release_after_ms })
            }
            Self::IncompatibleWorker {
                reason,
                expected_ipc_version,
                actual_ipc_version,
                hello,
                stderr,
            } => {
                json!({
                    "kind": "worker-incompatible",
                    "reason": reason,
                    "expectedIpcVersion": expected_ipc_version,
                    "actualIpcVersion": actual_ipc_version,
                    "hello": hello,
                    "stderr": stderr,
                })
            }
            Self::WorkerMissing { instance_id } => {
                json!({ "kind": "worker-missing", "instanceId": instance_id })
            }
            Self::AudioIpcUnavailable { instance_id } => {
                json!({ "kind": "audio-ipc-unavailable", "instanceId": instance_id })
            }
            Self::WorkerRejected {
                code,
                message,
                data,
                stderr,
            } => {
                json!({ "kind": "worker-rejected", "code": code, "message": message, "workerData": data, "stderr": stderr })
            }
            Self::ResourceLimitExceeded { limit, active } => {
                json!({ "kind": "resource-limit-exceeded", "resource": "worker-instances", "limit": limit, "active": active })
            }
        };

        attach_error_classification(data, worker_supervisor_classification(self))
    }
}

const fn worker_supervisor_classification(error: &WorkerSupervisorError) -> ErrorClassification {
    match error {
        WorkerSupervisorError::Spawn { .. }
        | WorkerSupervisorError::MissingPipe(_)
        | WorkerSupervisorError::SupervisionSetup { .. } => ErrorClassification::new(
            "worker-launch",
            "verify the host worker executable path, permissions, platform supervision setup, and resource-limit configuration",
        ),
        WorkerSupervisorError::Io(_) => ErrorClassification::new(
            "worker-ipc-io",
            "the bridge lost an IPC read/write path to the worker; restart the instance and inspect worker stderr",
        ),
        WorkerSupervisorError::Timeout { .. } => ErrorClassification::new(
            "worker-timeout",
            "the worker did not answer within the configured timeout; treat the instance as unhealthy and inspect plugin CPU or deadlock behavior",
        ),
        WorkerSupervisorError::InvalidJson(_) | WorkerSupervisorError::Protocol { .. } => {
            ErrorClassification::new(
                "worker-ipc-protocol",
                "the worker produced malformed control/audio IPC data; verify worker version compatibility and captured stderr",
            )
        }
        WorkerSupervisorError::Quarantined { .. } => ErrorClassification::new(
            "worker-quarantine",
            "the plugin exceeded the configured worker failure threshold; wait for release or explicitly clear quarantine after inspection",
        ),
        WorkerSupervisorError::IncompatibleWorker { .. } => ErrorClassification::new(
            "worker-compatibility",
            "the worker IPC capabilities do not match the bridge requirements; rebuild or replace the host worker binary",
        ),
        WorkerSupervisorError::WorkerMissing { .. }
        | WorkerSupervisorError::AudioIpcUnavailable { .. } => ErrorClassification::new(
            "worker-lifecycle",
            "the requested instance no longer has a usable worker process; refresh instance status before sending more work",
        ),
        WorkerSupervisorError::WorkerRejected { .. } => ErrorClassification::new(
            "worker-rejection",
            "the worker rejected the request; inspect workerData for VST3 runtime/control/process details when present",
        ),
        WorkerSupervisorError::ResourceLimitExceeded { .. } => ErrorClassification::new(
            "worker-resource-limit",
            "the bridge refused to start another worker because the configured resource policy is exhausted",
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    #[test]
    fn classifies_worker_supervisor_errors() {
        let timeout = WorkerSupervisorError::Timeout {
            method: "worker.metrics",
            timeout_ms: 5000,
            stderr: "busy".to_string(),
        }
        .rpc_data();

        assert_eq!(timeout["schemaVersion"], 1);
        assert_eq!(timeout["kind"], "timeout");
        assert_eq!(timeout["classification"]["schemaVersion"], 1);
        assert_eq!(timeout["classification"]["category"], "worker-timeout");

        let launch = WorkerSupervisorError::Spawn {
            executable: PathBuf::from("/tmp/wvst-host-worker"),
            message: "permission denied".to_string(),
        }
        .rpc_data();
        assert_eq!(launch["classification"]["category"], "worker-launch");

        let rejected = WorkerSupervisorError::WorkerRejected {
            code: 4220,
            message: "VST3 init failed".to_string(),
            data: Some(json!({ "kind": "vst3-runtime-init" })),
            stderr: String::new(),
        }
        .rpc_data();
        assert_eq!(rejected["classification"]["category"], "worker-rejection");
        assert_eq!(rejected["workerData"]["kind"], "vst3-runtime-init");
    }
}
