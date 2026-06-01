use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::instance_registry::InstanceRecord;

const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct WorkerSupervisor {
    executable: PathBuf,
    timeout: Duration,
    processes: Mutex<BTreeMap<u64, WorkerProcess>>,
}

#[derive(Debug)]
struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_request_id: u64,
}

#[derive(Debug, Deserialize)]
struct WorkerResponse {
    id: Value,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<WorkerResponseError>,
}

#[derive(Debug, Deserialize)]
struct WorkerResponseError {
    code: i64,
    message: String,
}

#[derive(Debug)]
pub enum WorkerSupervisorError {
    Spawn {
        executable: PathBuf,
        message: String,
    },
    MissingPipe(&'static str),
    Io(String),
    Timeout {
        method: &'static str,
        timeout_ms: u128,
    },
    InvalidJson(String),
    Protocol(String),
    WorkerRejected {
        code: i64,
        message: String,
    },
}

impl WorkerSupervisor {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            timeout: DEFAULT_IPC_TIMEOUT,
            processes: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn start_instance(
        &self,
        record: &InstanceRecord,
    ) -> Result<Value, WorkerSupervisorError> {
        let mut process = WorkerProcess::spawn(self.executable.clone()).await?;
        process
            .request("worker.hello", json!({}), self.timeout)
            .await?;
        let ready = process
            .request(
                "instance.create",
                instance_create_params(record),
                self.timeout,
            )
            .await?;

        self.processes
            .lock()
            .await
            .insert(record.instance_id, process);

        Ok(ready)
    }

    pub async fn destroy_instance(
        &self,
        instance_id: u64,
    ) -> Result<Option<Value>, WorkerSupervisorError> {
        let Some(mut process) = self.processes.lock().await.remove(&instance_id) else {
            return Ok(None);
        };

        let result = process
            .request(
                "instance.destroy",
                json!({ "instanceId": instance_id }),
                self.timeout,
            )
            .await?;
        process.shutdown().await;

        Ok(Some(result))
    }
}

impl WorkerProcess {
    async fn spawn(executable: PathBuf) -> Result<Self, WorkerSupervisorError> {
        let mut command = Command::new(&executable);
        command
            .arg("serve")
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
        let stdin = child
            .stdin
            .take()
            .ok_or(WorkerSupervisorError::MissingPipe("stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(WorkerSupervisorError::MissingPipe("stdout"))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            next_request_id: 1,
        })
    }

    async fn request(
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
        .map_err(|error| WorkerSupervisorError::Protocol(error.to_string()))?;
        let line = format!("{request}\n");

        timeout(timeout_duration, self.stdin.write_all(line.as_bytes()))
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method,
                timeout_ms: timeout_duration.as_millis(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;
        timeout(timeout_duration, self.stdin.flush())
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method,
                timeout_ms: timeout_duration.as_millis(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;

        let line = timeout(timeout_duration, self.stdout.next_line())
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method,
                timeout_ms: timeout_duration.as_millis(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?
            .ok_or_else(|| WorkerSupervisorError::Protocol("worker stdout closed".to_string()))?;
        let response = serde_json::from_str::<WorkerResponse>(&line)
            .map_err(|error| WorkerSupervisorError::InvalidJson(error.to_string()))?;

        if response.id != json!(id) {
            return Err(WorkerSupervisorError::Protocol(format!(
                "response id mismatch for {method}"
            )));
        }

        if let Some(error) = response.error {
            return Err(WorkerSupervisorError::WorkerRejected {
                code: error.code,
                message: error.message,
            });
        }

        response
            .result
            .ok_or_else(|| WorkerSupervisorError::Protocol("missing result".to_string()))
    }

    async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

impl WorkerSupervisorError {
    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::WorkerRejected { code, .. } => *code,
            Self::Timeout { .. } => 5035,
            Self::Spawn { .. } | Self::MissingPipe(_) | Self::Io(_) => 5036,
            Self::InvalidJson(_) | Self::Protocol(_) => 5037,
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
            Self::Timeout { method, timeout_ms } => {
                format!("worker method {method} timed out after {timeout_ms}ms")
            }
            Self::InvalidJson(message) => format!("worker returned invalid JSON: {message}"),
            Self::Protocol(message) => format!("worker protocol error: {message}"),
            Self::WorkerRejected { message, .. } => message.clone(),
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
            Self::Timeout { method, timeout_ms } => {
                json!({ "kind": "timeout", "method": method, "timeoutMs": timeout_ms })
            }
            Self::InvalidJson(message) => json!({ "kind": "invalid-json", "message": message }),
            Self::Protocol(message) => json!({ "kind": "protocol", "message": message }),
            Self::WorkerRejected { code, message } => {
                json!({ "kind": "worker-rejected", "code": code, "message": message })
            }
        }
    }
}

fn instance_create_params(record: &InstanceRecord) -> Value {
    json!({
        "instanceId": record.instance_id,
        "streamId": record.stream_id,
        "pluginId": record.plugin_id,
        "pluginPath": record.plugin_path,
        "classId": record.class_id,
        "className": record.class_name,
        "inputChannels": record.input_channels,
        "outputChannels": record.output_channels,
    })
}

#[cfg(test)]
impl WorkerSupervisor {
    pub fn new_for_test(executable: PathBuf, timeout: Duration) -> Self {
        Self {
            executable,
            timeout,
            processes: Mutex::new(BTreeMap::new()),
        }
    }
}
