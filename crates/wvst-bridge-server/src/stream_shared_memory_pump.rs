use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time;

use crate::metrics::BridgeMetrics;
use crate::stream_shared_memory::SharedMemoryStreamRegistry;
use crate::worker_supervisor::WorkerSupervisor;

mod config;
mod error;
mod event_queue;
mod runtime;
mod scheduler;

pub use config::{
    SharedMemoryPumpConfig, SharedMemoryPumpInstanceParams, SharedMemoryPumpStartParams,
    default_interval_micros, pump_config_from_params,
};
pub use error::SharedMemoryPumpError;
pub use event_queue::{
    SharedMemoryPumpClearEventsParams, SharedMemoryPumpClearEventsResult,
    SharedMemoryPumpEnqueueEventsParams, SharedMemoryPumpEnqueueEventsResult,
    SharedMemoryPumpEventOverflowPolicy, SharedMemoryPumpEventQueueStatus,
};
use runtime::{SharedMemoryPumpRuntime, outcome_from_code, process_once};
#[cfg(test)]
use runtime::{
    classify_worker_error, record_preflight_skip, record_process_timing, record_tick_outcome,
};
pub use scheduler::{
    SharedMemoryPumpPreflightSkip, SharedMemoryPumpRingStatus, SharedMemoryPumpScheduleStatus,
    SharedMemoryPumpSchedulingConfig, SharedMemoryPumpSchedulingMode,
};

#[derive(Debug, Default)]
pub struct SharedMemoryPumpRegistry {
    records: Mutex<BTreeMap<u64, SharedMemoryPumpRecord>>,
}

#[derive(Debug)]
struct SharedMemoryPumpRecord {
    config: SharedMemoryPumpConfig,
    started_at: Instant,
    runtime: Arc<SharedMemoryPumpRuntime>,
    metrics: Arc<BridgeMetrics>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
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
    pub overruns: u64,
    pub input_underruns: u64,
    pub output_backpressure: u64,
    pub worker_errors: u64,
    pub last_process_micros: u64,
    pub max_process_micros: u64,
    pub last_overrun_micros: Option<u64>,
    pub last_outcome: Option<SharedMemoryPumpTickOutcome>,
    pub last_success_frames: Option<u64>,
    pub preflight_skips: u64,
    pub schedule: SharedMemoryPumpScheduleStatus,
    pub events: SharedMemoryPumpEventQueueStatus,
    pub last_error: Option<SharedMemoryPumpLastError>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SharedMemoryPumpTickOutcome {
    Success,
    InputUnderrun,
    OutputBackpressure,
    WorkerError,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpLastError {
    pub code: i64,
    pub message: String,
    pub data: Value,
}

impl SharedMemoryPumpRegistry {
    pub fn start(
        &self,
        config: SharedMemoryPumpConfig,
        workers: Arc<WorkerSupervisor>,
        metrics: Arc<BridgeMetrics>,
        shared_memory: Arc<SharedMemoryStreamRegistry>,
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

        let runtime = Arc::new(SharedMemoryPumpRuntime::new(
            config.interval_micros,
            config.max_queued_events,
        ));
        let (stop, stop_rx) = watch::channel(false);
        let task = tokio::spawn(run_pump(
            config,
            Arc::clone(&workers),
            Arc::clone(&metrics),
            Arc::clone(&shared_memory),
            Arc::clone(&runtime),
            stop_rx,
        ));
        let record = SharedMemoryPumpRecord {
            config,
            started_at: Instant::now(),
            runtime,
            metrics,
            stop,
            task,
        };
        let status = status_from_record(&record, true);
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
        Ok(records
            .get(&instance_id)
            .map(|record| status_from_record(record, true)))
    }

    pub fn enqueue_events(
        &self,
        params: SharedMemoryPumpEnqueueEventsParams,
    ) -> Result<SharedMemoryPumpEnqueueEventsResult, SharedMemoryPumpError> {
        let records = self
            .records
            .lock()
            .map_err(|_| SharedMemoryPumpError::RegistryUnavailable)?;
        let Some(record) = records.get(&params.instance_id) else {
            return Err(SharedMemoryPumpError::NotRunning {
                instance_id: params.instance_id,
            });
        };
        let target_iteration = event_target_iteration(
            record.runtime.iterations.load(Ordering::Relaxed),
            params.target_iteration,
            params.delay_iterations,
        )?;
        let mut result = record.runtime.enqueue_events(
            target_iteration,
            params.midi_events,
            params.parameter_events,
            params.overflow_policy,
        )?;
        result.instance_id = params.instance_id;
        record
            .metrics
            .record_shared_memory_pump_events_enqueued(result.queued_events);
        if result.dropped_events > 0 {
            record
                .metrics
                .record_shared_memory_pump_events_dropped(result.dropped_events);
        }
        Ok(result)
    }

    pub fn clear_events(
        &self,
        params: SharedMemoryPumpClearEventsParams,
    ) -> Result<SharedMemoryPumpClearEventsResult, SharedMemoryPumpError> {
        let records = self
            .records
            .lock()
            .map_err(|_| SharedMemoryPumpError::RegistryUnavailable)?;
        let Some(record) = records.get(&params.instance_id) else {
            return Err(SharedMemoryPumpError::NotRunning {
                instance_id: params.instance_id,
            });
        };
        let (cleared_batches, cleared_events) = record.runtime.clear_events();
        if cleared_events > 0 {
            record
                .metrics
                .record_shared_memory_pump_events_cleared(cleared_events);
        }
        Ok(SharedMemoryPumpClearEventsResult {
            instance_id: params.instance_id,
            cleared_events,
            cleared_batches,
            status: record.runtime.event_queue_status(),
        })
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

async fn run_pump(
    config: SharedMemoryPumpConfig,
    workers: Arc<WorkerSupervisor>,
    metrics: Arc<BridgeMetrics>,
    shared_memory: Arc<SharedMemoryStreamRegistry>,
    runtime: Arc<SharedMemoryPumpRuntime>,
    mut stop: watch::Receiver<bool>,
) {
    let mut next_delay = Duration::ZERO;
    loop {
        if wait_for_next_tick(next_delay, &mut stop).await {
            return;
        }
        let report = process_once(config, &workers, &metrics, &shared_memory, &runtime).await;
        let delay_micros = config
            .scheduling
            .next_delay_micros(report.outcome, report.ring_status.as_ref());
        runtime.record_schedule(delay_micros, report.ring_status);
        next_delay = Duration::from_micros(delay_micros);
    }
}

async fn wait_for_next_tick(delay: Duration, stop: &mut watch::Receiver<bool>) -> bool {
    if delay.is_zero() {
        return false;
    }
    tokio::select! {
        changed = stop.changed() => changed.is_err() || *stop.borrow(),
        _ = time::sleep(delay) => false,
    }
}

fn stop_record(record: SharedMemoryPumpRecord) -> SharedMemoryPumpStatus {
    let _ = record.stop.send(true);
    status_from_record(&record, false)
}

fn status_from_record(record: &SharedMemoryPumpRecord, running: bool) -> SharedMemoryPumpStatus {
    let last_success_frames = record.runtime.last_success_frames.load(Ordering::Relaxed);
    let last_overrun_micros = record.runtime.last_overrun_micros.load(Ordering::Relaxed);
    let last_outcome = outcome_from_code(record.runtime.last_outcome.load(Ordering::Relaxed));
    SharedMemoryPumpStatus {
        running,
        task_finished: record.task.is_finished(),
        uptime_ms: record.started_at.elapsed().as_millis() as u64,
        config: record.config,
        iterations: record.runtime.iterations.load(Ordering::Relaxed),
        successes: record.runtime.successes.load(Ordering::Relaxed),
        failures: record.runtime.failures.load(Ordering::Relaxed),
        preflight_skips: record.runtime.preflight_skips.load(Ordering::Relaxed),
        overruns: record.runtime.overruns.load(Ordering::Relaxed),
        input_underruns: record.runtime.input_underruns.load(Ordering::Relaxed),
        output_backpressure: record.runtime.output_backpressure.load(Ordering::Relaxed),
        worker_errors: record.runtime.worker_errors.load(Ordering::Relaxed),
        last_process_micros: record.runtime.last_process_micros.load(Ordering::Relaxed),
        max_process_micros: record.runtime.max_process_micros.load(Ordering::Relaxed),
        last_overrun_micros: (last_overrun_micros > 0).then_some(last_overrun_micros),
        last_outcome,
        last_success_frames: (last_success_frames > 0).then_some(last_success_frames),
        schedule: SharedMemoryPumpScheduleStatus {
            config: record.config.scheduling,
            current_interval_micros: record
                .runtime
                .current_interval_micros
                .load(Ordering::Relaxed),
            last_delay_micros: record.runtime.last_delay_micros.load(Ordering::Relaxed),
            last_ring_status: record
                .runtime
                .last_ring_status
                .lock()
                .ok()
                .and_then(|status| *status),
        },
        events: record.runtime.event_queue_status(),
        last_error: record
            .runtime
            .last_error
            .lock()
            .ok()
            .and_then(|error| error.clone()),
    }
}

fn event_target_iteration(
    current_iteration: u64,
    target_iteration: Option<u64>,
    delay_iterations: Option<u64>,
) -> Result<u64, SharedMemoryPumpError> {
    match (target_iteration, delay_iterations) {
        (Some(_), Some(_)) => Err(SharedMemoryPumpError::InvalidEventTarget),
        (Some(target), None) => Ok(target),
        (None, Some(delay)) => Ok(current_iteration.saturating_add(delay)),
        (None, None) => Ok(current_iteration.saturating_add(1)),
    }
}

#[cfg(test)]
#[path = "stream_shared_memory_pump_tests.rs"]
mod tests;
