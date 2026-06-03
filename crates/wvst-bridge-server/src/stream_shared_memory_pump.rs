use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{self, MissedTickBehavior};

use crate::metrics::BridgeMetrics;
use crate::worker_supervisor::{WorkerSupervisor, WorkerSupervisorError};

#[derive(Debug, Default)]
pub struct SharedMemoryPumpRegistry {
    records: Mutex<BTreeMap<u64, SharedMemoryPumpRecord>>,
}

#[derive(Debug)]
struct SharedMemoryPumpRecord {
    config: SharedMemoryPumpConfig,
    started_at: Instant,
    runtime: Arc<SharedMemoryPumpRuntime>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpConfig {
    pub instance_id: u64,
    pub frames: u16,
    pub interval_micros: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpStatus {
    pub running: bool,
    pub task_finished: bool,
    pub uptime_ms: u64,
    pub config: SharedMemoryPumpConfig,
    pub iterations: u64,
    pub successes: u64,
    pub failures: u64,
    pub last_success_frames: Option<u64>,
    pub last_error: Option<SharedMemoryPumpLastError>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpLastError {
    pub code: i64,
    pub message: String,
    pub data: Value,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpStartParams {
    pub instance_id: u64,
    #[serde(default)]
    pub frames: Option<u16>,
    #[serde(default)]
    pub interval_micros: Option<u64>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpInstanceParams {
    pub instance_id: u64,
}

#[derive(Debug)]
pub enum SharedMemoryPumpError {
    AlreadyRunning { instance_id: u64 },
    NotAttached { instance_id: u64 },
    StreamClosed { instance_id: u64 },
    InstanceNotProcessing { instance_id: u64 },
    InvalidFrames { frames: u16, max_frames: u16 },
    InvalidInterval,
    RegistryUnavailable,
}

impl SharedMemoryPumpRegistry {
    pub fn start(
        &self,
        config: SharedMemoryPumpConfig,
        workers: Arc<WorkerSupervisor>,
        metrics: Arc<BridgeMetrics>,
    ) -> Result<SharedMemoryPumpStatus, SharedMemoryPumpError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| SharedMemoryPumpError::RegistryUnavailable)?;
        if records.contains_key(&config.instance_id) {
            return Err(SharedMemoryPumpError::AlreadyRunning {
                instance_id: config.instance_id,
            });
        }

        let runtime = Arc::new(SharedMemoryPumpRuntime::default());
        let (stop, stop_rx) = watch::channel(false);
        let task = tokio::spawn(run_pump(
            config,
            Arc::clone(&workers),
            Arc::clone(&metrics),
            Arc::clone(&runtime),
            stop_rx,
        ));
        let record = SharedMemoryPumpRecord {
            config,
            started_at: Instant::now(),
            runtime,
            stop,
            task,
        };
        let status = status_from_record(&record);
        records.insert(config.instance_id, record);
        Ok(status)
    }

    pub fn stop_by_instance(
        &self,
        instance_id: u64,
    ) -> Result<Option<SharedMemoryPumpStatus>, SharedMemoryPumpError> {
        let record = self
            .records
            .lock()
            .map_err(|_| SharedMemoryPumpError::RegistryUnavailable)?
            .remove(&instance_id);
        Ok(record.map(stop_record))
    }

    pub fn status_by_instance(
        &self,
        instance_id: u64,
    ) -> Result<Option<SharedMemoryPumpStatus>, SharedMemoryPumpError> {
        let records = self
            .records
            .lock()
            .map_err(|_| SharedMemoryPumpError::RegistryUnavailable)?;
        Ok(records.get(&instance_id).map(status_from_record))
    }
}

impl Drop for SharedMemoryPumpRegistry {
    fn drop(&mut self) {
        let records = match self.records.get_mut() {
            Ok(records) => records,
            Err(poisoned) => poisoned.into_inner(),
        };
        for record in std::mem::take(records).into_values() {
            let _ = record.stop.send(true);
            record.task.abort();
        }
    }
}

impl SharedMemoryPumpError {
    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::AlreadyRunning { .. }
            | Self::NotAttached { .. }
            | Self::StreamClosed { .. }
            | Self::InstanceNotProcessing { .. } => 4096,
            Self::InvalidFrames { .. } | Self::InvalidInterval => 4222,
            Self::RegistryUnavailable => 5037,
        }
    }

    pub fn rpc_message(&self) -> String {
        match self {
            Self::AlreadyRunning { instance_id } => {
                format!("shared memory pump already running for instance {instance_id}")
            }
            Self::NotAttached { instance_id } => {
                format!("shared memory stream is not attached for instance {instance_id}")
            }
            Self::StreamClosed { instance_id } => {
                format!("stream is closed for instance {instance_id}")
            }
            Self::InstanceNotProcessing { instance_id } => {
                format!("instance {instance_id} is not processing")
            }
            Self::InvalidFrames { frames, max_frames } => {
                format!("invalid pump frames {frames}; maxBlockFrames is {max_frames}")
            }
            Self::InvalidInterval => "intervalMicros must be non-zero".to_string(),
            Self::RegistryUnavailable => "shared memory pump registry unavailable".to_string(),
        }
    }

    pub fn rpc_data(&self) -> Value {
        match self {
            Self::AlreadyRunning { instance_id } => {
                json!({ "kind": "shared-memory-pump-already-running", "instanceId": instance_id })
            }
            Self::NotAttached { instance_id } => {
                json!({ "kind": "shared-memory-not-attached", "instanceId": instance_id })
            }
            Self::StreamClosed { instance_id } => {
                json!({ "kind": "stream-closed", "instanceId": instance_id })
            }
            Self::InstanceNotProcessing { instance_id } => {
                json!({ "kind": "instance-not-processing", "instanceId": instance_id })
            }
            Self::InvalidFrames { frames, max_frames } => {
                json!({ "kind": "invalid-pump-frames", "frames": frames, "maxFrames": max_frames })
            }
            Self::InvalidInterval => {
                json!({ "kind": "invalid-pump-interval" })
            }
            Self::RegistryUnavailable => {
                json!({ "kind": "shared-memory-pump-registry-unavailable" })
            }
        }
    }
}

#[derive(Debug, Default)]
struct SharedMemoryPumpRuntime {
    iterations: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    last_success_frames: AtomicU64,
    last_error: Mutex<Option<SharedMemoryPumpLastError>>,
}

async fn run_pump(
    config: SharedMemoryPumpConfig,
    workers: Arc<WorkerSupervisor>,
    metrics: Arc<BridgeMetrics>,
    runtime: Arc<SharedMemoryPumpRuntime>,
    mut stop: watch::Receiver<bool>,
) {
    let mut interval = time::interval(Duration::from_micros(config.interval_micros));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return;
                }
            }
            _ = interval.tick() => {
                process_once(config, &workers, &metrics, &runtime).await;
            }
        }
    }
}

async fn process_once(
    config: SharedMemoryPumpConfig,
    workers: &WorkerSupervisor,
    metrics: &BridgeMetrics,
    runtime: &SharedMemoryPumpRuntime,
) {
    runtime.iterations.fetch_add(1, Ordering::Relaxed);
    let started_at = Instant::now();
    match workers
        .process_shared_memory(config.instance_id, Some(config.frames))
        .await
    {
        Ok(result) => {
            let frames = result.get("frames").and_then(Value::as_u64).unwrap_or(0);
            runtime.successes.fetch_add(1, Ordering::Relaxed);
            runtime.last_success_frames.store(frames, Ordering::Relaxed);
            if let Ok(mut last_error) = runtime.last_error.lock() {
                *last_error = None;
            }
            metrics.record_shared_memory_process_success(
                frames,
                started_at.elapsed().as_micros() as u64,
            );
        }
        Err(error) => {
            runtime.failures.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last_error) = runtime.last_error.lock() {
                *last_error = Some(last_error_from_worker(error));
            }
            metrics.record_shared_memory_process_failure(started_at.elapsed().as_micros() as u64);
        }
    }
}

fn stop_record(record: SharedMemoryPumpRecord) -> SharedMemoryPumpStatus {
    let status = status_from_record(&record);
    let _ = record.stop.send(true);
    status
}

fn status_from_record(record: &SharedMemoryPumpRecord) -> SharedMemoryPumpStatus {
    let last_success_frames = record.runtime.last_success_frames.load(Ordering::Relaxed);
    SharedMemoryPumpStatus {
        running: true,
        task_finished: record.task.is_finished(),
        uptime_ms: record.started_at.elapsed().as_millis() as u64,
        config: record.config,
        iterations: record.runtime.iterations.load(Ordering::Relaxed),
        successes: record.runtime.successes.load(Ordering::Relaxed),
        failures: record.runtime.failures.load(Ordering::Relaxed),
        last_success_frames: (last_success_frames > 0).then_some(last_success_frames),
        last_error: record
            .runtime
            .last_error
            .lock()
            .ok()
            .and_then(|error| error.clone()),
    }
}

fn last_error_from_worker(error: WorkerSupervisorError) -> SharedMemoryPumpLastError {
    SharedMemoryPumpLastError {
        code: error.rpc_code(),
        message: error.rpc_message(),
        data: error.rpc_data(),
    }
}

pub fn default_interval_micros(frames: u16, sample_rate: u32) -> u64 {
    let micros = u64::from(frames)
        .saturating_mul(1_000_000)
        .checked_div(u64::from(sample_rate))
        .unwrap_or(0);
    micros.max(1)
}

pub fn pump_config_from_params(
    params: SharedMemoryPumpStartParams,
    max_block_frames: u16,
    sample_rate: u32,
) -> Result<SharedMemoryPumpConfig, SharedMemoryPumpError> {
    let frames = params.frames.unwrap_or(max_block_frames);
    if frames == 0 || frames > max_block_frames {
        return Err(SharedMemoryPumpError::InvalidFrames {
            frames,
            max_frames: max_block_frames,
        });
    }
    let interval_micros = match params.interval_micros {
        Some(0) => return Err(SharedMemoryPumpError::InvalidInterval),
        Some(value) => value,
        None => default_interval_micros(frames, sample_rate),
    };
    Ok(SharedMemoryPumpConfig {
        instance_id: params.instance_id,
        frames,
        interval_micros,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn derives_interval_from_frame_duration() {
        assert_eq!(default_interval_micros(128, 48_000), 2_666);
        assert_eq!(default_interval_micros(1, 192_000), 5);
    }
    #[test]
    fn validates_pump_config() {
        let params = SharedMemoryPumpStartParams {
            instance_id: 7,
            frames: None,
            interval_micros: None,
        };
        let config = pump_config_from_params(params, 128, 48_000).expect("config");
        assert_eq!(config.frames, 128);
        assert_eq!(config.interval_micros, 2_666);
        assert!(matches!(
            pump_config_from_params(
                SharedMemoryPumpStartParams {
                    instance_id: 7,
                    frames: Some(129),
                    interval_micros: None,
                },
                128,
                48_000,
            ),
            Err(SharedMemoryPumpError::InvalidFrames { .. })
        ));
        assert!(matches!(
            pump_config_from_params(
                SharedMemoryPumpStartParams {
                    instance_id: 7,
                    frames: Some(64),
                    interval_micros: Some(0),
                },
                128,
                48_000,
            ),
            Err(SharedMemoryPumpError::InvalidInterval)
        ));
    }
}
