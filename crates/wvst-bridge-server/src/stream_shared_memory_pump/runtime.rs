use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde_json::{Value, json};

use crate::metrics::BridgeMetrics;
use crate::stream_shared_memory::SharedMemoryStreamRegistry;
use crate::worker_supervisor::{WorkerSupervisor, WorkerSupervisorError};

use super::{
    SharedMemoryPumpConfig, SharedMemoryPumpError, SharedMemoryPumpLastError,
    SharedMemoryPumpPreflightSkip, SharedMemoryPumpRingStatus, SharedMemoryPumpTickOutcome,
    event_queue::{
        SharedMemoryPumpEventDrain, SharedMemoryPumpEventOverflowPolicy,
        SharedMemoryPumpEventQueue, SharedMemoryPumpEventQueueStatus,
    },
};

#[derive(Debug)]
pub(super) struct SharedMemoryPumpRuntime {
    pub(super) iterations: AtomicU64,
    pub(super) successes: AtomicU64,
    pub(super) failures: AtomicU64,
    pub(super) preflight_skips: AtomicU64,
    pub(super) overruns: AtomicU64,
    pub(super) input_underruns: AtomicU64,
    pub(super) output_backpressure: AtomicU64,
    pub(super) worker_errors: AtomicU64,
    pub(super) last_process_micros: AtomicU64,
    pub(super) max_process_micros: AtomicU64,
    pub(super) last_overrun_micros: AtomicU64,
    pub(super) last_outcome: AtomicU64,
    pub(super) last_success_frames: AtomicU64,
    pub(super) current_interval_micros: AtomicU64,
    pub(super) last_delay_micros: AtomicU64,
    pub(super) last_ring_status: Mutex<Option<SharedMemoryPumpRingStatus>>,
    pub(super) last_error: Mutex<Option<SharedMemoryPumpLastError>>,
    event_queue: Mutex<SharedMemoryPumpEventQueue>,
}

impl SharedMemoryPumpRuntime {
    pub(super) fn new(initial_interval_micros: u64, max_queued_events: u32) -> Self {
        Self {
            current_interval_micros: AtomicU64::new(initial_interval_micros),
            last_delay_micros: AtomicU64::new(initial_interval_micros),
            event_queue: Mutex::new(SharedMemoryPumpEventQueue::new(max_queued_events)),
            ..Self::default()
        }
    }

    pub(super) fn record_schedule(
        &self,
        delay_micros: u64,
        ring_status: Option<SharedMemoryPumpRingStatus>,
    ) {
        self.current_interval_micros
            .store(delay_micros, Ordering::Relaxed);
        self.last_delay_micros
            .store(delay_micros, Ordering::Relaxed);
        if let Ok(mut last_ring_status) = self.last_ring_status.lock() {
            *last_ring_status = ring_status;
        }
    }

    pub(super) fn enqueue_events(
        &self,
        target_iteration: u64,
        midi_events: Vec<Value>,
        parameter_events: Vec<Value>,
        overflow_policy: SharedMemoryPumpEventOverflowPolicy,
    ) -> Result<super::SharedMemoryPumpEnqueueEventsResult, SharedMemoryPumpError> {
        self.event_queue
            .lock()
            .map_err(|_| SharedMemoryPumpError::RegistryUnavailable)?
            .enqueue(
                target_iteration,
                midi_events,
                parameter_events,
                overflow_policy,
            )
            .map_err(SharedMemoryPumpError::from)
    }

    pub(super) fn drain_events_for_iteration(&self, iteration: u64) -> SharedMemoryPumpEventDrain {
        self.event_queue
            .lock()
            .map(|mut queue| queue.drain_ready(iteration))
            .unwrap_or_default()
    }

    pub(super) fn clear_events(&self) -> (u64, u64) {
        self.event_queue
            .lock()
            .map(|mut queue| queue.clear())
            .unwrap_or_default()
    }

    pub(super) fn event_queue_status(&self) -> SharedMemoryPumpEventQueueStatus {
        self.event_queue
            .lock()
            .map(|queue| queue.status())
            .unwrap_or_else(|_| SharedMemoryPumpEventQueue::new(0).status())
    }
}

impl Default for SharedMemoryPumpRuntime {
    fn default() -> Self {
        Self {
            iterations: AtomicU64::new(0),
            successes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            preflight_skips: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
            input_underruns: AtomicU64::new(0),
            output_backpressure: AtomicU64::new(0),
            worker_errors: AtomicU64::new(0),
            last_process_micros: AtomicU64::new(0),
            max_process_micros: AtomicU64::new(0),
            last_overrun_micros: AtomicU64::new(0),
            last_outcome: AtomicU64::new(0),
            last_success_frames: AtomicU64::new(0),
            current_interval_micros: AtomicU64::new(0),
            last_delay_micros: AtomicU64::new(0),
            last_ring_status: Mutex::new(None),
            last_error: Mutex::new(None),
            event_queue: Mutex::new(SharedMemoryPumpEventQueue::new(0)),
        }
    }
}

pub(super) async fn process_once(
    config: SharedMemoryPumpConfig,
    workers: &WorkerSupervisor,
    metrics: &BridgeMetrics,
    shared_memory: &SharedMemoryStreamRegistry,
    runtime: &SharedMemoryPumpRuntime,
) -> SharedMemoryPumpTickReport {
    let iteration = runtime.iterations.fetch_add(1, Ordering::Relaxed) + 1;
    let mut ring_status = capture_ring_status(config.instance_id, shared_memory);
    if let Some(skip) = ring_status
        .as_ref()
        .and_then(|ring| config.scheduling.preflight_skip(ring))
    {
        record_preflight_skip(config, metrics, runtime, skip);
        return SharedMemoryPumpTickReport {
            outcome: skip.outcome,
            ring_status,
        };
    }

    let started_at = Instant::now();
    let input_events = runtime.drain_events_for_iteration(iteration);
    if input_events.events > 0 {
        metrics.record_shared_memory_pump_events_drained(
            input_events.events,
            input_events.late_events,
        );
    }
    let outcome = match workers
        .process_shared_memory(
            config.instance_id,
            Some(config.frames),
            input_events.midi_events,
            input_events.parameter_events,
        )
        .await
    {
        Ok(result) => {
            let frames = result.get("frames").and_then(Value::as_u64).unwrap_or(0);
            runtime.successes.fetch_add(1, Ordering::Relaxed);
            record_tick_outcome(runtime, metrics, SharedMemoryPumpTickOutcome::Success);
            runtime.last_success_frames.store(frames, Ordering::Relaxed);
            if let Ok(mut last_error) = runtime.last_error.lock() {
                *last_error = None;
            }
            let latency_us = started_at.elapsed().as_micros() as u64;
            record_process_timing(config, metrics, runtime, latency_us);
            metrics.record_shared_memory_process_success(frames, latency_us);
            ring_status = capture_ring_status(config.instance_id, shared_memory).or(ring_status);
            SharedMemoryPumpTickOutcome::Success
        }
        Err(error) => {
            let outcome = classify_worker_error(&error);
            runtime.failures.fetch_add(1, Ordering::Relaxed);
            record_tick_outcome(runtime, metrics, outcome);
            if let Ok(mut last_error) = runtime.last_error.lock() {
                *last_error = Some(last_error_from_worker(error));
            }
            let latency_us = started_at.elapsed().as_micros() as u64;
            record_process_timing(config, metrics, runtime, latency_us);
            metrics.record_shared_memory_process_failure(latency_us);
            outcome
        }
    };

    SharedMemoryPumpTickReport {
        outcome,
        ring_status,
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SharedMemoryPumpTickReport {
    pub(super) outcome: SharedMemoryPumpTickOutcome,
    pub(super) ring_status: Option<SharedMemoryPumpRingStatus>,
}

fn capture_ring_status(
    instance_id: u64,
    shared_memory: &SharedMemoryStreamRegistry,
) -> Option<SharedMemoryPumpRingStatus> {
    shared_memory
        .status_by_instance(instance_id)
        .ok()
        .flatten()
        .as_ref()
        .map(SharedMemoryPumpRingStatus::from)
}

pub(super) fn record_preflight_skip(
    config: SharedMemoryPumpConfig,
    metrics: &BridgeMetrics,
    runtime: &SharedMemoryPumpRuntime,
    skip: SharedMemoryPumpPreflightSkip,
) {
    runtime.failures.fetch_add(1, Ordering::Relaxed);
    runtime.preflight_skips.fetch_add(1, Ordering::Relaxed);
    metrics.increment_shared_memory_pump_preflight_skips();
    record_tick_outcome(runtime, metrics, skip.outcome);
    if let Ok(mut last_error) = runtime.last_error.lock() {
        *last_error = Some(preflight_error(config, skip));
    }
}

fn preflight_error(
    config: SharedMemoryPumpConfig,
    skip: SharedMemoryPumpPreflightSkip,
) -> SharedMemoryPumpLastError {
    let reason = match skip.outcome {
        SharedMemoryPumpTickOutcome::InputUnderrun => "input-underrun",
        SharedMemoryPumpTickOutcome::OutputBackpressure => "output-backpressure",
        SharedMemoryPumpTickOutcome::Success | SharedMemoryPumpTickOutcome::WorkerError => {
            "preflight-skip"
        }
    };
    SharedMemoryPumpLastError {
        code: 4094,
        message: format!(
            "shared memory pump preflight skipped {reason}: requested {}, available {}",
            skip.requested_frames, skip.available_frames
        ),
        data: json!({
            "kind": "shared-memory-pump-preflight",
            "reason": reason,
            "instanceId": config.instance_id,
            "frames": config.frames,
            "requestedFrames": skip.requested_frames,
            "availableFrames": skip.available_frames,
        }),
    }
}

pub(super) fn record_process_timing(
    config: SharedMemoryPumpConfig,
    metrics: &BridgeMetrics,
    runtime: &SharedMemoryPumpRuntime,
    latency_us: u64,
) {
    runtime
        .last_process_micros
        .store(latency_us, Ordering::Relaxed);
    update_max(&runtime.max_process_micros, latency_us);

    if latency_us > config.interval_micros {
        runtime.overruns.fetch_add(1, Ordering::Relaxed);
        runtime
            .last_overrun_micros
            .store(latency_us - config.interval_micros, Ordering::Relaxed);
        metrics.increment_shared_memory_pump_overruns();
    }
}

pub(super) fn record_tick_outcome(
    runtime: &SharedMemoryPumpRuntime,
    metrics: &BridgeMetrics,
    outcome: SharedMemoryPumpTickOutcome,
) {
    runtime
        .last_outcome
        .store(outcome_code(outcome), Ordering::Relaxed);
    match outcome {
        SharedMemoryPumpTickOutcome::Success => {}
        SharedMemoryPumpTickOutcome::InputUnderrun => {
            runtime.input_underruns.fetch_add(1, Ordering::Relaxed);
            metrics.increment_shared_memory_pump_input_underruns();
        }
        SharedMemoryPumpTickOutcome::OutputBackpressure => {
            runtime.output_backpressure.fetch_add(1, Ordering::Relaxed);
            metrics.increment_shared_memory_pump_output_backpressure();
        }
        SharedMemoryPumpTickOutcome::WorkerError => {
            runtime.worker_errors.fetch_add(1, Ordering::Relaxed);
            metrics.increment_shared_memory_pump_worker_errors();
        }
    }
}

pub(super) fn classify_worker_error(error: &WorkerSupervisorError) -> SharedMemoryPumpTickOutcome {
    let WorkerSupervisorError::WorkerRejected {
        data: Some(data), ..
    } = error
    else {
        return SharedMemoryPumpTickOutcome::WorkerError;
    };
    match shared_memory_error_reason(data) {
        Some("input-underrun") => SharedMemoryPumpTickOutcome::InputUnderrun,
        Some("output-backpressure") => SharedMemoryPumpTickOutcome::OutputBackpressure,
        _ => SharedMemoryPumpTickOutcome::WorkerError,
    }
}

fn shared_memory_error_reason(data: &Value) -> Option<&str> {
    data.get("reason")
        .and_then(Value::as_str)
        .or_else(|| data.pointer("/workerData/reason").and_then(Value::as_str))
}

fn update_max(value: &AtomicU64, candidate: u64) {
    let mut current = value.load(Ordering::Relaxed);
    while candidate > current {
        match value.compare_exchange_weak(current, candidate, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return,
            Err(actual) => current = actual,
        }
    }
}

fn outcome_code(outcome: SharedMemoryPumpTickOutcome) -> u64 {
    match outcome {
        SharedMemoryPumpTickOutcome::Success => 1,
        SharedMemoryPumpTickOutcome::InputUnderrun => 2,
        SharedMemoryPumpTickOutcome::OutputBackpressure => 3,
        SharedMemoryPumpTickOutcome::WorkerError => 4,
    }
}

pub(super) fn outcome_from_code(code: u64) -> Option<SharedMemoryPumpTickOutcome> {
    match code {
        1 => Some(SharedMemoryPumpTickOutcome::Success),
        2 => Some(SharedMemoryPumpTickOutcome::InputUnderrun),
        3 => Some(SharedMemoryPumpTickOutcome::OutputBackpressure),
        4 => Some(SharedMemoryPumpTickOutcome::WorkerError),
        _ => None,
    }
}

fn last_error_from_worker(error: WorkerSupervisorError) -> SharedMemoryPumpLastError {
    SharedMemoryPumpLastError {
        code: error.rpc_code(),
        message: error.rpc_message(),
        data: error.rpc_data(),
    }
}
