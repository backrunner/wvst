use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::TcpListener;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::instance_registry::InstanceRecord;

const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_QUARANTINE_DURATION: Duration = Duration::from_secs(60);
const EXPECTED_WORKER_IPC_VERSION: u16 = 1;
const QUARANTINE_FAILURES: u32 = 3;
const STDERR_TAIL_BYTES: usize = 4096;

#[path = "worker_supervisor_audio.rs"]
mod worker_supervisor_audio;
#[path = "worker_supervisor_error.rs"]
mod worker_supervisor_error;
#[path = "worker_supervisor_hello.rs"]
mod worker_supervisor_hello;
#[path = "worker_supervisor_lifecycle.rs"]
mod worker_supervisor_lifecycle;

use worker_supervisor_audio::WorkerAudioConnection;

#[derive(Debug)]
pub struct WorkerSupervisor {
    executable: PathBuf,
    timeout: Duration,
    use_audio_ipc: bool,
    failures: Mutex<BTreeMap<String, u32>>,
    processes: Mutex<BTreeMap<u64, Arc<Mutex<WorkerProcess>>>>,
    quarantine_duration: Duration,
    quarantined: Mutex<BTreeMap<String, QuarantineRecord>>,
}

#[derive(Debug, Clone)]
pub struct WorkerSupervisorOptions {
    executable: PathBuf,
    timeout: Duration,
    quarantine_duration: Duration,
    use_audio_ipc: bool,
}

#[derive(Debug)]
struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr: StderrTail,
    audio: Option<WorkerAudioConnection>,
    next_request_id: u64,
}

#[derive(Debug, Clone)]
struct QuarantineRecord {
    failures: u32,
    release_at: Instant,
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
    IncompatibleWorker {
        reason: String,
        expected_ipc_version: u16,
        actual_ipc_version: Option<u64>,
        hello: Value,
        stderr: String,
    },
    WorkerMissing {
        instance_id: u64,
    },
    AudioIpcUnavailable {
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
        Self::with_options(WorkerSupervisorOptions::new(executable))
    }

    pub fn with_options(options: WorkerSupervisorOptions) -> Self {
        Self {
            executable: options.executable,
            timeout: options.timeout,
            use_audio_ipc: options.use_audio_ipc,
            failures: Mutex::new(BTreeMap::new()),
            processes: Mutex::new(BTreeMap::new()),
            quarantine_duration: options.quarantine_duration,
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
                let _ = self.record_failure(&record.plugin_id).await;
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

    pub async fn restart_instance(
        &self,
        record: &InstanceRecord,
    ) -> Result<Value, WorkerSupervisorError> {
        self.kill_instance(record.instance_id).await;
        self.start_instance(record).await
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

    pub async fn quarantine_failures(&self, plugin_id: &str) -> Option<u32> {
        self.quarantined
            .lock()
            .await
            .get(plugin_id)
            .map(|record| record.failures)
    }

    pub async fn release_expired_quarantine(&self, plugin_id: &str) -> Option<u32> {
        let mut quarantined = self.quarantined.lock().await;
        let record = quarantined.get(plugin_id)?;
        if Instant::now() < record.release_at {
            return None;
        }

        let failures = record.failures;
        quarantined.remove(plugin_id);
        self.failures.lock().await.remove(plugin_id);
        Some(failures)
    }
}

impl WorkerSupervisorOptions {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            timeout: DEFAULT_IPC_TIMEOUT,
            quarantine_duration: DEFAULT_QUARANTINE_DURATION,
            use_audio_ipc: true,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_quarantine_duration(mut self, duration: Duration) -> Self {
        self.quarantine_duration = duration;
        self
    }

    pub fn with_audio_ipc(mut self, enabled: bool) -> Self {
        self.use_audio_ipc = enabled;
        self
    }
}

impl WorkerProcess {
    async fn spawn(
        executable: PathBuf,
        timeout_duration: Duration,
        use_audio_ipc: bool,
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
        command.arg("serve");
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
            next_request_id: 1,
        };

        if let Some(listener) = audio_listener {
            let accepted = match timeout(timeout_duration, listener.accept()).await {
                Ok(Ok((stream, _))) => stream,
                Ok(Err(error)) => {
                    process.shutdown().await;
                    return Err(WorkerSupervisorError::Io(error.to_string()));
                }
                Err(_) => {
                    let error = process
                        .timeout_error("audio.connect", timeout_duration)
                        .await;
                    process.shutdown().await;
                    return Err(error);
                }
            };
            process.audio = Some(WorkerAudioConnection::new(accepted));
        }

        Ok(process)
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
        let mut process =
            WorkerProcess::spawn(self.executable.clone(), self.timeout, self.use_audio_ipc).await?;
        let hello = match process
            .request("worker.hello", json!({}), self.timeout)
            .await
        {
            Ok(hello) => hello,
            Err(error) => {
                process.shutdown().await;
                return Err(error);
            }
        };

        if let Err(error) =
            worker_supervisor_hello::validate_worker_hello(&process.stderr, hello).await
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
        let mut quarantined = self.quarantined.lock().await;
        if let Some(record) = quarantined.get(plugin_id) {
            if Instant::now() < record.release_at {
                return Err(WorkerSupervisorError::Quarantined {
                    plugin_id: plugin_id.to_string(),
                    failures: record.failures,
                });
            }
        }
        if quarantined.remove(plugin_id).is_some() {
            drop(quarantined);
            self.failures.lock().await.remove(plugin_id);
        }

        Ok(())
    }

    async fn record_failure(&self, plugin_id: &str) -> Option<u32> {
        let mut failures = self.failures.lock().await;
        let count = failures
            .entry(plugin_id.to_string())
            .and_modify(|count| *count += 1)
            .or_insert(1);

        if *count >= QUARANTINE_FAILURES {
            self.quarantined.lock().await.insert(
                plugin_id.to_string(),
                QuarantineRecord {
                    failures: *count,
                    release_at: Instant::now() + self.quarantine_duration,
                },
            );
            return Some(*count);
        }

        None
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

    async fn kill_instance(&self, instance_id: u64) {
        let Some(process) = self.processes.lock().await.remove(&instance_id) else {
            return;
        };

        process.lock().await.shutdown().await;
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

fn instance_create_params(record: &InstanceRecord) -> Value {
    json!({
        "instanceId": record.instance_id,
        "streamId": record.stream_id,
        "pluginId": record.plugin_id,
        "pluginPath": record.plugin_path,
        "classId": record.class_id,
        "className": record.class_name,
        "sampleRate": record.sample_rate,
        "maxBlockFrames": record.max_block_frames,
        "inputChannels": record.input_channels,
        "outputChannels": record.output_channels,
    })
}

#[cfg(test)]
impl WorkerSupervisor {
    pub fn new_for_test(executable: PathBuf, timeout: Duration) -> Self {
        Self::with_options(
            WorkerSupervisorOptions::new(executable)
                .with_timeout(timeout)
                .with_audio_ipc(false),
        )
    }

    pub fn new_for_test_with_audio(executable: PathBuf, timeout: Duration) -> Self {
        Self::with_options(WorkerSupervisorOptions::new(executable).with_timeout(timeout))
    }

    pub fn new_for_test_with_quarantine(
        executable: PathBuf,
        timeout: Duration,
        quarantine_duration: Duration,
    ) -> Self {
        Self::with_options(
            WorkerSupervisorOptions::new(executable)
                .with_timeout(timeout)
                .with_quarantine_duration(quarantine_duration)
                .with_audio_ipc(false),
        )
    }
}

#[cfg(test)]
#[path = "worker_supervisor_tests.rs"]
mod tests;
