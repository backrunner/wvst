use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::TcpListener;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;
use wvst_process_supervision::{WorkerResourceLimits, WorkerTerminationTarget};

use crate::instance_registry::InstanceRecord;
use crate::metrics::{BridgeMetrics, WorkerShutdownAudit};
use wvst_protocol::{
    WORKER_CONTROL_IPC_HEADER_LEN, WORKER_CONTROL_IPC_MAX_BODY_LEN, WorkerControlIpcHeader,
    WorkerControlIpcMessage, WorkerControlMessageKind,
};

const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_QUARANTINE_DURATION: Duration = Duration::from_secs(60);
const DEFAULT_MAX_WORKER_INSTANCES: usize = 64;
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
    use_framed_control_ipc: bool,
    max_instances: usize,
    resource_limits: WorkerResourceLimits,
    metrics: Option<Arc<BridgeMetrics>>,
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
    use_framed_control_ipc: bool,
    max_instances: usize,
    resource_limits: WorkerResourceLimits,
    metrics: Option<Arc<BridgeMetrics>>,
}

#[derive(Debug)]
struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr: StderrTail,
    audio: Option<WorkerAudioConnection>,
    termination_target: WorkerTerminationTarget,
    use_framed_control_ipc: bool,
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
    #[serde(default)]
    data: Option<Value>,
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
        data: Option<Value>,
        stderr: String,
    },
    ResourceLimitExceeded {
        limit: usize,
        active: usize,
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
            use_framed_control_ipc: options.use_framed_control_ipc,
            max_instances: options.max_instances,
            resource_limits: options.resource_limits,
            metrics: options.metrics,
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
        self.reject_if_instance_limit_reached().await?;

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
        self.record_shutdown(process.shutdown().await);

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
                let audit = process.lock().await.shutdown().await;
                self.record_shutdown(audit);
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
            use_framed_control_ipc: true,
            max_instances: DEFAULT_MAX_WORKER_INSTANCES,
            resource_limits: WorkerResourceLimits::none(),
            metrics: None,
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

    pub fn with_framed_control_ipc(mut self, enabled: bool) -> Self {
        self.use_framed_control_ipc = enabled;
        self
    }

    pub fn with_max_instances(mut self, max_instances: usize) -> Self {
        self.max_instances = max_instances.max(1);
        self
    }

    pub fn with_resource_limits(mut self, limits: WorkerResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    pub fn with_metrics(mut self, metrics: Arc<BridgeMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }
}

impl WorkerProcess {
    async fn spawn(
        executable: PathBuf,
        timeout_duration: Duration,
        use_audio_ipc: bool,
        use_framed_control_ipc: bool,
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
        command.arg(if use_framed_control_ipc {
            "serve-framed"
        } else {
            "serve"
        });
        WorkerTerminationTarget::configure_command(&mut command, resource_limits);
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
            WorkerTerminationTarget::from_child_with_limits(&child, resource_limits);
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
            use_framed_control_ipc,
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
        let body = if self.use_framed_control_ipc {
            WorkerControlIpcMessage::request(id, request.as_bytes().to_vec())
                .map_err(|error| self.protocol_error(error.to_string()))?
                .encode()
                .map_err(|error| self.protocol_error(error.to_string()))?
        } else {
            format!("{request}\n").into_bytes()
        };

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
        if !self.use_framed_control_ipc {
            return match timeout(timeout_duration, self.stdout.next_line()).await {
                Ok(line) => line
                    .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?
                    .ok_or_else(|| self.protocol_error("worker stdout closed".to_string())),
                Err(_) => {
                    let error = self.timeout_error(method, timeout_duration).await;
                    Err(error)
                }
            };
        }

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

    async fn shutdown(&mut self) -> WorkerShutdownAudit {
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
        let mut process = WorkerProcess::spawn(
            self.executable.clone(),
            self.timeout,
            self.use_audio_ipc,
            self.use_framed_control_ipc,
            self.resource_limits,
            self.metrics.clone(),
        )
        .await?;
        let hello = match process
            .request("worker.hello", json!({}), self.timeout)
            .await
        {
            Ok(hello) => hello,
            Err(error) => {
                self.record_shutdown(process.shutdown().await);
                return Err(error);
            }
        };

        if let Err(error) = worker_supervisor_hello::validate_worker_hello(
            &process.stderr,
            hello,
            self.use_framed_control_ipc,
        )
        .await
        {
            self.record_shutdown(process.shutdown().await);
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
                self.record_shutdown(process.shutdown().await);
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

    async fn reject_if_instance_limit_reached(&self) -> Result<(), WorkerSupervisorError> {
        let active = self.processes.lock().await.len();
        if active >= self.max_instances {
            return Err(WorkerSupervisorError::ResourceLimitExceeded {
                limit: self.max_instances,
                active,
            });
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

    fn record_shutdown(&self, audit: WorkerShutdownAudit) {
        record_worker_shutdown(self.metrics.as_ref(), audit);
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

        let audit = process.lock().await.shutdown().await;
        self.record_shutdown(audit);
    }
}

fn record_worker_shutdown(metrics: Option<&Arc<BridgeMetrics>>, audit: WorkerShutdownAudit) {
    if let Some(metrics) = metrics {
        metrics.record_worker_shutdown(audit);
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

fn validate_framed_response_header(
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
    }
}

fn validate_framed_response_body_len(body_len: u32) -> Result<(), String> {
    if body_len > WORKER_CONTROL_IPC_MAX_BODY_LEN {
        return Err(format!(
            "framed response body too large: max {}, got {}",
            WORKER_CONTROL_IPC_MAX_BODY_LEN, body_len
        ));
    }

    Ok(())
}

#[cfg(test)]
impl WorkerSupervisor {
    pub fn new_for_test(executable: PathBuf, timeout: Duration) -> Self {
        Self::with_options(
            WorkerSupervisorOptions::new(executable)
                .with_timeout(timeout)
                .with_audio_ipc(false)
                .with_framed_control_ipc(false),
        )
    }

    pub fn new_for_test_with_audio(executable: PathBuf, timeout: Duration) -> Self {
        Self::with_options(
            WorkerSupervisorOptions::new(executable)
                .with_timeout(timeout)
                .with_framed_control_ipc(false),
        )
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
                .with_audio_ipc(false)
                .with_framed_control_ipc(false),
        )
    }
}

#[cfg(test)]
#[path = "worker_supervisor_tests.rs"]
mod tests;
