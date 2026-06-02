use serde::Deserialize;
use serde_json::Value;
use wvst_protocol::WORKER_CONTROL_IPC_SCHEMA_VERSION;

use super::{EXPECTED_WORKER_IPC_VERSION, StderrTail, WorkerSupervisorError};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerHello {
    ipc_version: u16,
    #[serde(default)]
    capabilities: WorkerCapabilities,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerCapabilities {
    #[serde(default)]
    instance_lifecycle: bool,
    #[serde(default)]
    binary_audio_process: bool,
    #[serde(default)]
    framed_control_ipc: bool,
    #[serde(default)]
    framed_control_ipc_version: Option<u16>,
}

pub(super) async fn validate_worker_hello(
    stderr: &StderrTail,
    hello: Value,
    require_framed_control_ipc: bool,
) -> Result<(), WorkerSupervisorError> {
    let parsed = match serde_json::from_value::<WorkerHello>(hello.clone()) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Err(incompatible_worker(
                stderr,
                format!("invalid worker hello: {error}"),
                actual_ipc_version(&hello),
                hello,
            )
            .await);
        }
    };

    if parsed.ipc_version != EXPECTED_WORKER_IPC_VERSION {
        return Err(incompatible_worker(
            stderr,
            format!(
                "unsupported ipcVersion {}, expected {}",
                parsed.ipc_version, EXPECTED_WORKER_IPC_VERSION
            ),
            Some(u64::from(parsed.ipc_version)),
            hello,
        )
        .await);
    }

    if !parsed.capabilities.instance_lifecycle {
        return Err(incompatible_worker(
            stderr,
            "missing instanceLifecycle capability".to_string(),
            Some(u64::from(parsed.ipc_version)),
            hello,
        )
        .await);
    }

    if !parsed.capabilities.binary_audio_process {
        return Err(incompatible_worker(
            stderr,
            "missing binaryAudioProcess capability".to_string(),
            Some(u64::from(parsed.ipc_version)),
            hello,
        )
        .await);
    }

    if require_framed_control_ipc && !parsed.capabilities.framed_control_ipc {
        return Err(incompatible_worker(
            stderr,
            "missing framedControlIpc capability".to_string(),
            Some(u64::from(parsed.ipc_version)),
            hello,
        )
        .await);
    }

    if require_framed_control_ipc
        && parsed.capabilities.framed_control_ipc_version != Some(WORKER_CONTROL_IPC_SCHEMA_VERSION)
    {
        return Err(incompatible_worker(
            stderr,
            format!(
                "unsupported framedControlIpcVersion {:?}, expected {}",
                parsed.capabilities.framed_control_ipc_version, WORKER_CONTROL_IPC_SCHEMA_VERSION
            ),
            Some(u64::from(parsed.ipc_version)),
            hello,
        )
        .await);
    }

    Ok(())
}

async fn incompatible_worker(
    stderr: &StderrTail,
    reason: String,
    actual_ipc_version: Option<u64>,
    hello: Value,
) -> WorkerSupervisorError {
    WorkerSupervisorError::IncompatibleWorker {
        reason,
        expected_ipc_version: EXPECTED_WORKER_IPC_VERSION,
        actual_ipc_version,
        hello,
        stderr: stderr.snapshot().await,
    }
}

fn actual_ipc_version(hello: &Value) -> Option<u64> {
    hello.get("ipcVersion").and_then(Value::as_u64)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn accepts_matching_framed_control_ipc_version() {
        let hello = json!({
            "ipcVersion": EXPECTED_WORKER_IPC_VERSION,
            "capabilities": {
                "instanceLifecycle": true,
                "binaryAudioProcess": true,
                "framedControlIpc": true,
                "framedControlIpcVersion": WORKER_CONTROL_IPC_SCHEMA_VERSION
            }
        });

        validate_worker_hello(&StderrTail::default(), hello, true)
            .await
            .expect("compatible hello");
    }

    #[tokio::test]
    async fn rejects_missing_framed_control_ipc_version_when_required() {
        let hello = json!({
            "ipcVersion": EXPECTED_WORKER_IPC_VERSION,
            "capabilities": {
                "instanceLifecycle": true,
                "binaryAudioProcess": true,
                "framedControlIpc": true
            }
        });

        let error = validate_worker_hello(&StderrTail::default(), hello, true)
            .await
            .expect_err("incompatible hello");

        assert!(matches!(
            error,
            WorkerSupervisorError::IncompatibleWorker { reason, .. }
                if reason.contains("framedControlIpcVersion")
        ));
    }

    #[tokio::test]
    async fn allows_json_line_test_workers_without_framed_control_version() {
        let hello = json!({
            "ipcVersion": EXPECTED_WORKER_IPC_VERSION,
            "capabilities": {
                "instanceLifecycle": true,
                "binaryAudioProcess": true
            }
        });

        validate_worker_hello(&StderrTail::default(), hello, false)
            .await
            .expect("json-line worker compatibility");
    }
}
