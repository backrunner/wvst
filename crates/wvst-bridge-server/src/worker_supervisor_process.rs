use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::{ChildStderr, Command};
use tokio::time::timeout;
use wvst_process_supervision::{WorkerResourceLimits, WorkerTerminationTarget};
use wvst_protocol::{
    WORKER_CONTROL_IPC_HEADER_LEN, WORKER_CONTROL_IPC_MAX_BODY_LEN, WorkerControlIpcHeader,
    WorkerControlIpcMessage, WorkerControlMessageKind,
};

use super::{
    DEFAULT_IPC_TIMEOUT, STDERR_TAIL_BYTES, StderrTail, WorkerAudioConnection, WorkerProcess,
    WorkerResponse, WorkerSupervisorError, record_worker_shutdown,
};
use crate::metrics::{BridgeMetrics, WorkerShutdownAudit};

impl WorkerProcess {
    pub(super) async fn spawn(
        executable: PathBuf,
        timeout_duration: Duration,
        use_audio_ipc: bool,
        resource_limits: WorkerResourceLimits,
        metrics: Option<Arc<BridgeMetrics>>,
    ) -> Result<Self, WorkerSupervisorError> {
        let audio_listener = if use_audio_ipc {
            Some(
                TcpListener::bind("127.0.0.1:0")
                    .await
                    .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?,
            )
        } else {
            None
        };

        let mut command = Command::new(&executable);
        command.arg("serve-framed");
        WorkerTerminationTarget::configure_command(&mut command, resource_limits.clone());
        if let Some(listener) = audio_listener.as_ref() {
            let address = listener
                .local_addr()
                .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;
            command.arg("--audio-connect").arg(address.to_string());
        }
        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| WorkerSupervisorError::Spawn {
                executable,
                message: error.to_string(),
            })?;
        let termination_target =
            match WorkerTerminationTarget::try_from_child_with_limits(&child, resource_limits) {
                Ok(target) => target,
                Err(error) => {
                    let _ = child.start_kill();
                    let _ = timeout(DEFAULT_IPC_TIMEOUT, child.wait()).await;
                    return Err(WorkerSupervisorError::SupervisionSetup {
                        message: error.to_string(),
                    });
                }
            };
        let stdin = child
            .stdin
            .take()
            .ok_or(WorkerSupervisorError::MissingPipe("stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(WorkerSupervisorError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(WorkerSupervisorError::MissingPipe("stderr"))?;

        let mut process = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            stderr: StderrTail::spawn(stderr),
            audio: None,
            termination_target,
            next_request_id: 1,
        };

        if let Some(listener) = audio_listener {
            let accepted = match timeout(timeout_duration, listener.accept()).await {
                Ok(Ok((stream, _))) => stream,
                Ok(Err(error)) => {
                    record_worker_shutdown(metrics.as_ref(), process.shutdown().await);
                    return Err(WorkerSupervisorError::Io(error.to_string()));
                }
                Err(_) => {
                    let error = process
                        .timeout_error("audio.connect", timeout_duration)
                        .await;
                    record_worker_shutdown(metrics.as_ref(), process.shutdown().await);
                    return Err(error);
                }
            };
            process.audio = Some(WorkerAudioConnection::new(accepted));
        }

        Ok(process)
    }

    pub(super) async fn request(
        &mut self,
        method: &'static str,
        params: Value,
        timeout_duration: Duration,
    ) -> Result<Value, WorkerSupervisorError> {
        let id = self.next_request_id;
        self.next_request_id += 1;
        let request = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))
        .map_err(|error| self.protocol_error(error.to_string()))?;
        self.write_request(id, &request, timeout_duration).await?;
        let line = self
            .read_response_body(id, method, timeout_duration)
            .await?;
        let response = serde_json::from_str::<WorkerResponse>(&line)
            .map_err(|error| WorkerSupervisorError::InvalidJson(error.to_string()))?;

        if response.id != json!(id) {
            return Err(self.protocol_error(format!("response id mismatch for {method}")));
        }

        if let Some(error) = response.error {
            return Err(WorkerSupervisorError::WorkerRejected {
                code: error.code,
                message: error.message,
                data: error.data,
                stderr: self.stderr.snapshot().await,
            });
        }

        response
            .result
            .ok_or_else(|| self.protocol_error("missing result".to_string()))
    }

    async fn write_request(
        &mut self,
        id: u64,
        request: &str,
        timeout_duration: Duration,
    ) -> Result<(), WorkerSupervisorError> {
        let body = WorkerControlIpcMessage::request(id, request.as_bytes().to_vec())
            .map_err(|error| self.protocol_error(error.to_string()))?
            .encode()
            .map_err(|error| self.protocol_error(error.to_string()))?;

        match timeout(timeout_duration, self.stdin.write_all(&body)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self.timeout_error("control.write", timeout_duration).await;
                return Err(error);
            }
        }
        match timeout(timeout_duration, self.stdin.flush()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self.timeout_error("control.flush", timeout_duration).await;
                return Err(error);
            }
        }

        Ok(())
    }

    async fn read_response_body(
        &mut self,
        id: u64,
        method: &'static str,
        timeout_duration: Duration,
    ) -> Result<String, WorkerSupervisorError> {
        let mut header_bytes = [0; WORKER_CONTROL_IPC_HEADER_LEN];
        match timeout(
            timeout_duration,
            self.stdout.get_mut().read_exact(&mut header_bytes),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self.timeout_error(method, timeout_duration).await;
                return Err(error);
            }
        }
        let header = WorkerControlIpcHeader::decode(&header_bytes)
            .map_err(|error| self.protocol_error(error.to_string()))?;
        validate_framed_response_header(&header, id, method)
            .map_err(|message| self.protocol_error(message))?;
        validate_framed_response_body_len(header.body_len)
            .map_err(|message| self.protocol_error(message))?;
        let body_len = usize::try_from(header.body_len)
            .map_err(|_| self.protocol_error("framed response body too large".to_string()))?;
        let mut body = vec![0; body_len];
        match timeout(
            timeout_duration,
            self.stdout.get_mut().read_exact(&mut body),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self.timeout_error(method, timeout_duration).await;
                return Err(error);
            }
        }

        String::from_utf8(body)
            .map_err(|error| self.protocol_error(format!("invalid framed response utf8: {error}")))
    }

    pub(super) async fn shutdown(&mut self) -> WorkerShutdownAudit {
        let mut audit = WorkerShutdownAudit {
            tree_kill_requested: self.termination_target.terminate_tree(),
            ..WorkerShutdownAudit::default()
        };
        if audit.tree_kill_requested {
            audit.kill_requested = true;
        } else if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            audit.kill_requested = true;
            let _ = self.child.start_kill();
        }

        match timeout(DEFAULT_IPC_TIMEOUT, self.child.wait()).await {
            Ok(Ok(_)) => audit.wait_succeeded = true,
            Ok(Err(_)) => {}
            Err(_) => {
                audit.wait_timed_out = true;
                audit.forced_kill_requested = true;
                if !self.termination_target.kill_tree() {
                    let _ = self.child.start_kill();
                }
                if timeout(DEFAULT_IPC_TIMEOUT, self.child.wait())
                    .await
                    .is_ok_and(|result| result.is_ok())
                {
                    audit.wait_succeeded = true;
                }
            }
        }
        audit
    }

    pub(super) async fn timeout_error(
        &self,
        method: &'static str,
        timeout_duration: Duration,
    ) -> WorkerSupervisorError {
        WorkerSupervisorError::Timeout {
            method,
            timeout_ms: timeout_duration.as_millis(),
            stderr: self.stderr.snapshot().await,
        }
    }

    pub(super) fn protocol_error(&self, message: String) -> WorkerSupervisorError {
        WorkerSupervisorError::Protocol {
            message,
            stderr: String::new(),
        }
    }
}

impl StderrTail {
    fn spawn(stderr: ChildStderr) -> Self {
        let tail = Self::default();
        let buffer = Arc::clone(&tail.buffer);

        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut buffer = buffer.lock().await;
                if !buffer.is_empty() {
                    buffer.push('\n');
                }
                buffer.push_str(&line);

                while buffer.len() > STDERR_TAIL_BYTES {
                    buffer.remove(0);
                }
            }
        });

        tail
    }

    pub(super) async fn snapshot(&self) -> String {
        self.buffer.lock().await.clone()
    }
}

pub(super) fn validate_framed_response_header(
    header: &WorkerControlIpcHeader,
    id: u64,
    method: &'static str,
) -> Result<(), String> {
    if header.sequence != id {
        return Err(format!("framed response sequence mismatch for {method}"));
    }

    match header.kind {
        WorkerControlMessageKind::Response => {
            if header.status_code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "framed response for {method} used nonzero status {}",
                    header.status_code
                ))
            }
        }
        WorkerControlMessageKind::ErrorResponse => {
            if header.status_code == 0 {
                Err(format!(
                    "framed error response for {method} used zero status"
                ))
            } else {
                Ok(())
            }
        }
        WorkerControlMessageKind::Request => Err(format!(
            "worker sent request frame while responding to {method}"
        )),
        WorkerControlMessageKind::BatchRequest => Err(format!(
            "worker sent batch request frame while responding to {method}"
        )),
        WorkerControlMessageKind::BatchResponse => Err(format!(
            "worker sent batch response frame to non-batch request {method}"
        )),
    }
}

pub(super) fn validate_framed_response_body_len(body_len: u32) -> Result<(), String> {
    if body_len > WORKER_CONTROL_IPC_MAX_BODY_LEN {
        return Err(format!(
            "framed response body too large: max {}, got {}",
            WORKER_CONTROL_IPC_MAX_BODY_LEN, body_len
        ));
    }

    Ok(())
}
