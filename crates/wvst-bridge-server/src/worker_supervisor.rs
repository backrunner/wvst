use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use wvst_process_supervision::{WorkerResourceLimits, WorkerTerminationTarget};

use crate::instance_registry::InstanceRecord;
use crate::metrics::{BridgeMetrics, WorkerShutdownAudit};

const DEFAULT_LOAD_TIMEOUT: Duration = Duration::from_secs(120);
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
#[path = "worker_supervisor_policy.rs"]
mod worker_supervisor_policy;
use worker_supervisor_policy::FailureRecord;
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
    load_timeout: Duration,
    use_audio_ipc: bool,
    max_instances: usize,
    resource_limits: WorkerResourceLimits,
    metrics: Option<Arc<BridgeMetrics>>,
    failures: Mutex<BTreeMap<String, FailureRecord>>,
    instance_slots: Arc<Semaphore>,
    processes: Mutex<BTreeMap<u64, Arc<Mutex<WorkerProcess>>>>,
    quarantine_duration: Duration,
    quarantine_failure_threshold: u32,
}

#[derive(Debug, Clone)]
pub struct WorkerSupervisorOptions {
    executable: PathBuf,
    timeout: Duration,
    load_timeout: Duration,
    quarantine_duration: Duration,
    quarantine_failure_threshold: u32,
    use_audio_ipc: bool,
    max_instances: usize,
    resource_limits: WorkerResourceLimits,
    metrics: Option<Arc<BridgeMetrics>>,
}

#[derive(Debug)]
struct WorkerProcess {
    plugin_id: String,
    _instance_slot: Option<OwnedSemaphorePermit>,
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr: StderrTail,
    audio: Option<WorkerAudioConnection>,
    termination_target: WorkerTerminationTarget,
    next_request_id: u64,
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
            load_timeout: options.load_timeout,
            use_audio_ipc: options.use_audio_ipc,
            max_instances: options.max_instances,
            resource_limits: options.resource_limits,
            metrics: options.metrics,
            failures: Mutex::new(BTreeMap::new()),
            instance_slots: Arc::new(Semaphore::new(options.max_instances)),
            processes: Mutex::new(BTreeMap::new()),
            quarantine_duration: options.quarantine_duration,
            quarantine_failure_threshold: options.quarantine_failure_threshold,
        }
    }

    pub async fn start_instance(
        &self,
        record: &InstanceRecord,
    ) -> Result<Value, WorkerSupervisorError> {
        self.reject_if_quarantined(&record.plugin_id).await?;
        // Reserve before spawning: simultaneous slow loads count toward the limit.
        let slot = Arc::clone(&self.instance_slots)
            .try_acquire_owned()
            .map_err(|_| WorkerSupervisorError::ResourceLimitExceeded {
                limit: self.max_instances,
                active: self.max_instances,
            })?;

        let result = self.start_instance_inner(record).await;
        match result {
            Ok((ready, mut process)) => {
                process._instance_slot = Some(slot);
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
                self.remove_failed_process(instance_id, &process).await;
                Err(error)
            }
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
            self.resource_limits.clone(),
            self.metrics.clone(),
        )
        .await?;
        process.plugin_id = record.plugin_id.clone();
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
                self.load_timeout,
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

    fn record_shutdown(&self, audit: WorkerShutdownAudit) {
        record_worker_shutdown(self.metrics.as_ref(), audit);
    }

    async fn remove_failed_process(&self, instance_id: u64, process: &Arc<Mutex<WorkerProcess>>) {
        let plugin_id = process.lock().await.plugin_id.clone();
        let mut processes = self.processes.lock().await;
        if processes
            .get(&instance_id)
            .is_some_and(|current| Arc::ptr_eq(current, process))
        {
            processes.remove(&instance_id);
            drop(processes);
            self.record_failure(&plugin_id).await;
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

fn instance_create_params(record: &InstanceRecord) -> Value {
    let mut params = json!({
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
    });
    if let Some(input_bus_index) = record.input_bus_index {
        params["inputBusIndex"] = json!(input_bus_index);
    }
    if let Some(output_bus_index) = record.output_bus_index {
        params["outputBusIndex"] = json!(output_bus_index);
    }
    params
}

#[cfg(test)]
impl WorkerSupervisor {
    pub fn new_for_test(executable: PathBuf, timeout: Duration) -> Self {
        Self::with_options(
            WorkerSupervisorOptions::new(executable)
                .with_timeout(timeout)
                .with_load_timeout(timeout)
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
                .with_load_timeout(timeout)
                .with_quarantine_duration(quarantine_duration)
                .with_audio_ipc(false),
        )
    }
}

#[cfg(test)]
#[path = "worker_supervisor_tests.rs"]
mod tests;
