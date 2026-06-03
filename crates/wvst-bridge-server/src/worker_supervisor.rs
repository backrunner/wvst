use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use wvst_process_supervision::{WorkerResourceLimits, WorkerTerminationTarget};

use crate::instance_registry::InstanceRecord;
use crate::metrics::{BridgeMetrics, WorkerShutdownAudit};

const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_QUARANTINE_DURATION: Duration = Duration::from_secs(60);
const DEFAULT_MAX_WORKER_INSTANCES: usize = 64;
const EXPECTED_WORKER_IPC_VERSION: u16 = 1;
const DEFAULT_QUARANTINE_FAILURE_THRESHOLD: u32 = 3;
const STDERR_TAIL_BYTES: usize = 4096;

#[path = "worker_supervisor_audio.rs"]
mod worker_supervisor_audio;
#[path = "worker_supervisor_batch.rs"]
mod worker_supervisor_batch;
#[path = "worker_supervisor_error.rs"]
mod worker_supervisor_error;
#[path = "worker_supervisor_hello.rs"]
mod worker_supervisor_hello;
#[path = "worker_supervisor_lifecycle.rs"]
mod worker_supervisor_lifecycle;
#[path = "worker_supervisor_options.rs"]
mod worker_supervisor_options;
#[path = "worker_supervisor_process.rs"]
mod worker_supervisor_process;

use worker_supervisor_audio::WorkerAudioConnection;
use worker_supervisor_process::{
    validate_framed_response_body_len, validate_framed_response_header,
};

#[cfg(test)]
use wvst_protocol::{
    WORKER_CONTROL_IPC_MAX_BODY_LEN, WorkerControlIpcHeader, WorkerControlMessageKind,
};

#[derive(Debug)]
pub struct WorkerSupervisor {
    executable: PathBuf,
    timeout: Duration,
    use_audio_ipc: bool,
    max_instances: usize,
    resource_limits: WorkerResourceLimits,
    metrics: Option<Arc<BridgeMetrics>>,
    failures: Mutex<BTreeMap<String, u32>>,
    processes: Mutex<BTreeMap<u64, Arc<Mutex<WorkerProcess>>>>,
    quarantine_duration: Duration,
    quarantine_failure_threshold: u32,
    quarantined: Mutex<BTreeMap<String, QuarantineRecord>>,
}

#[derive(Debug, Clone)]
pub struct WorkerSupervisorOptions {
    executable: PathBuf,
    timeout: Duration,
    quarantine_duration: Duration,
    quarantine_failure_threshold: u32,
    use_audio_ipc: bool,
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
    next_request_id: u64,
}

#[derive(Debug, Clone)]
struct QuarantineRecord {
    failures: u32,
    release_at: Instant,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WorkerQuarantineStatus {
    pub failures: u32,
    pub release_after_ms: u128,
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
    SupervisionSetup {
        message: String,
    },
    Quarantined {
        plugin_id: String,
        failures: u32,
        release_after_ms: u128,
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
            max_instances: options.max_instances,
            resource_limits: options.resource_limits,
            metrics: options.metrics,
            failures: Mutex::new(BTreeMap::new()),
            processes: Mutex::new(BTreeMap::new()),
            quarantine_duration: options.quarantine_duration,
            quarantine_failure_threshold: options.quarantine_failure_threshold,
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
        self.quarantine_status(plugin_id)
            .await
            .map(|status| status.failures)
    }

    pub async fn quarantine_status(&self, plugin_id: &str) -> Option<WorkerQuarantineStatus> {
        self.quarantined
            .lock()
            .await
            .get(plugin_id)
            .map(quarantine_status)
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

impl WorkerSupervisor {
    async fn start_instance_inner(
        &self,
        record: &InstanceRecord,
    ) -> Result<(Value, WorkerProcess), WorkerSupervisorError> {
        let mut process = WorkerProcess::spawn(
            self.executable.clone(),
            self.timeout,
            self.use_audio_ipc,
            self.resource_limits.clone(),
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

        if let Err(error) =
            worker_supervisor_hello::validate_worker_hello(&process.stderr, hello).await
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
                    release_after_ms: remaining_ms(record.release_at),
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

        if *count >= self.quarantine_failure_threshold {
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

fn quarantine_status(record: &QuarantineRecord) -> WorkerQuarantineStatus {
    WorkerQuarantineStatus {
        failures: record.failures,
        release_after_ms: remaining_ms(record.release_at),
    }
}

fn remaining_ms(release_at: Instant) -> u128 {
    release_at
        .saturating_duration_since(Instant::now())
        .as_millis()
}

fn record_worker_shutdown(metrics: Option<&Arc<BridgeMetrics>>, audit: WorkerShutdownAudit) {
    if let Some(metrics) = metrics {
        metrics.record_worker_shutdown(audit);
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
