use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::instance_registry::InstanceRecord;

const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(5);
const QUARANTINE_FAILURES: u32 = 3;
const STDERR_TAIL_BYTES: usize = 4096;

#[derive(Debug)]
pub struct WorkerSupervisor {
    executable: PathBuf,
    timeout: Duration,
    failures: Mutex<BTreeMap<String, u32>>,
    processes: Mutex<BTreeMap<u64, Arc<Mutex<WorkerProcess>>>>,
    quarantined: Mutex<BTreeMap<String, u32>>,
}

#[derive(Debug)]
struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr: StderrTail,
    next_request_id: u64,
}

#[derive(Debug, Clone, Default)]
struct StderrTail {
    buffer: Arc<Mutex<String>>,
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
        stderr: String,
    },
    InvalidJson(String),
    Protocol {
        message: String,
        stderr: String,
    },
    Quarantined {
        plugin_id: String,
        failures: u32,
    },
    WorkerMissing {
        instance_id: u64,
    },
    WorkerRejected {
        code: i64,
        message: String,
        stderr: String,
    },
}

impl WorkerSupervisor {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            timeout: DEFAULT_IPC_TIMEOUT,
            failures: Mutex::new(BTreeMap::new()),
            processes: Mutex::new(BTreeMap::new()),
            quarantined: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn start_instance(
        &self,
        record: &InstanceRecord,
    ) -> Result<Value, WorkerSupervisorError> {
        self.reject_if_quarantined(&record.plugin_id).await?;

        let result = self.start_instance_inner(record).await;
        match result {
            Ok((ready, process)) => {
                self.clear_failures(&record.plugin_id).await;
                self.processes
                    .lock()
                    .await
                    .insert(record.instance_id, Arc::new(Mutex::new(process)));
                Ok(ready)
            }
            Err(error) => {
                self.record_failure(&record.plugin_id).await;
                Err(error)
            }
        }
    }

    pub async fn destroy_instance(
        &self,
        instance_id: u64,
    ) -> Result<Option<Value>, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.remove(&instance_id) else {
            return Ok(None);
        };
        let mut process = process.lock().await;

        let result = process
            .request(
                "instance.destroy",
                json!({ "instanceId": instance_id }),
                self.timeout,
            )
            .await;
        process.shutdown().await;

        result.map(Some)
    }

    pub async fn heartbeat_instance(
        &self,
        instance_id: u64,
    ) -> Result<Value, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.get(&instance_id).cloned() else {
            return Err(WorkerSupervisorError::WorkerMissing { instance_id });
        };

        let result = process
            .lock()
            .await
            .request("worker.metrics", json!({}), self.timeout)
            .await;

        match result {
            Ok(result) => Ok(result),
            Err(error) => {
                process.lock().await.shutdown().await;
                self.remove_process_if_same(instance_id, &process).await;
                Err(error)
            }
        }
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
        let stderr = child
            .stderr
            .take()
            .ok_or(WorkerSupervisorError::MissingPipe("stderr"))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            stderr: StderrTail::spawn(stderr),
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
        .map_err(|error| self.protocol_error(error.to_string()))?;
        let line = format!("{request}\n");

        match timeout(timeout_duration, self.stdin.write_all(line.as_bytes())).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self.timeout_error(method, timeout_duration).await;
                self.shutdown().await;
                return Err(error);
            }
        }
        match timeout(timeout_duration, self.stdin.flush()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self.timeout_error(method, timeout_duration).await;
                self.shutdown().await;
                return Err(error);
            }
        }

        let line = match timeout(timeout_duration, self.stdout.next_line()).await {
            Ok(line) => line
                .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?
                .ok_or_else(|| self.protocol_error("worker stdout closed".to_string()))?,
            Err(_) => {
                let error = self.timeout_error(method, timeout_duration).await;
                self.shutdown().await;
                return Err(error);
            }
        };
        let response = serde_json::from_str::<WorkerResponse>(&line)
            .map_err(|error| WorkerSupervisorError::InvalidJson(error.to_string()))?;

        if response.id != json!(id) {
            return Err(self.protocol_error(format!("response id mismatch for {method}")));
        }

        if let Some(error) = response.error {
            return Err(WorkerSupervisorError::WorkerRejected {
                code: error.code,
                message: error.message,
                stderr: self.stderr.snapshot().await,
            });
        }

        response
            .result
            .ok_or_else(|| self.protocol_error("missing result".to_string()))
    }

    async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }

    async fn timeout_error(
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

    fn protocol_error(&self, message: String) -> WorkerSupervisorError {
        WorkerSupervisorError::Protocol {
            message,
            stderr: String::new(),
        }
    }
}

impl WorkerSupervisor {
    async fn start_instance_inner(
        &self,
        record: &InstanceRecord,
    ) -> Result<(Value, WorkerProcess), WorkerSupervisorError> {
        let mut process = WorkerProcess::spawn(self.executable.clone()).await?;
        if let Err(error) = process
            .request("worker.hello", json!({}), self.timeout)
            .await
        {
            process.shutdown().await;
            return Err(error);
        }

        match process
            .request(
                "instance.create",
                instance_create_params(record),
                self.timeout,
            )
            .await
        {
            Ok(ready) => Ok((ready, process)),
            Err(error) => {
                process.shutdown().await;
                Err(error)
            }
        }
    }

    async fn reject_if_quarantined(&self, plugin_id: &str) -> Result<(), WorkerSupervisorError> {
        if let Some(failures) = self.quarantined.lock().await.get(plugin_id).copied() {
            return Err(WorkerSupervisorError::Quarantined {
                plugin_id: plugin_id.to_string(),
                failures,
            });
        }

        Ok(())
    }

    async fn record_failure(&self, plugin_id: &str) {
        let mut failures = self.failures.lock().await;
        let count = failures
            .entry(plugin_id.to_string())
            .and_modify(|count| *count += 1)
            .or_insert(1);

        if *count >= QUARANTINE_FAILURES {
            self.quarantined
                .lock()
                .await
                .insert(plugin_id.to_string(), *count);
        }
    }

    async fn clear_failures(&self, plugin_id: &str) {
        self.failures.lock().await.remove(plugin_id);
        self.quarantined.lock().await.remove(plugin_id);
    }

    async fn remove_process_if_same(&self, instance_id: u64, process: &Arc<Mutex<WorkerProcess>>) {
        let mut processes = self.processes.lock().await;
        if processes
            .get(&instance_id)
            .is_some_and(|current| Arc::ptr_eq(current, process))
        {
            processes.remove(&instance_id);
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

    async fn snapshot(&self) -> String {
        self.buffer.lock().await.clone()
    }
}

impl WorkerSupervisorError {
    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::WorkerRejected { code, .. } => *code,
            Self::Timeout { .. } => 5035,
            Self::Spawn { .. } | Self::MissingPipe(_) | Self::Io(_) => 5036,
            Self::InvalidJson(_) | Self::Protocol { .. } => 5037,
            Self::Quarantined { .. } => 4093,
            Self::WorkerMissing { .. } => 4041,
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
            Self::WorkerMissing { instance_id } => {
                format!("worker not found for instance {instance_id}")
            }
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
            Self::WorkerMissing { instance_id } => {
                json!({ "kind": "worker-missing", "instanceId": instance_id })
            }
            Self::WorkerRejected {
                code,
                message,
                stderr,
            } => {
                json!({ "kind": "worker-rejected", "code": code, "message": message, "stderr": stderr })
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
            failures: Mutex::new(BTreeMap::new()),
            processes: Mutex::new(BTreeMap::new()),
            quarantined: Mutex::new(BTreeMap::new()),
        }
    }
}

#[cfg(test)]
#[path = "worker_supervisor_tests.rs"]
mod tests;
