use serde_json::{Value, json};

use super::WorkerSupervisorError;

impl WorkerSupervisorError {
    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::WorkerRejected { code, .. } => *code,
            Self::Timeout { .. } => 5035,
            Self::Spawn { .. } | Self::MissingPipe(_) | Self::Io(_) => 5036,
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
            Self::Quarantined {
                plugin_id,
                failures,
            } => {
                format!("worker quarantined for plugin {plugin_id} after {failures} failures")
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
        match self {
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
            Self::Quarantined {
                plugin_id,
                failures,
            } => {
                json!({ "kind": "quarantined", "pluginId": plugin_id, "failures": failures })
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
                stderr,
            } => {
                json!({ "kind": "worker-rejected", "code": code, "message": message, "stderr": stderr })
            }
            Self::ResourceLimitExceeded { limit, active } => {
                json!({ "kind": "resource-limit-exceeded", "resource": "worker-instances", "limit": limit, "active": active })
            }
        }
    }
}
