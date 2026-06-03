use std::sync::atomic::Ordering;

use serde_json::json;
use tokio::sync::watch;

use crate::worker_supervisor::WorkerSupervisorError;

use super::*;

#[test]
fn derives_interval_from_frame_duration() {
    assert_eq!(default_interval_micros(128, 48_000), 2_666);
    assert_eq!(default_interval_micros(1, 192_000), 5);
}

#[test]
fn validates_pump_config() {
    let config = pump_config_from_params(start_params(None, None), 128, 48_000).expect("config");
    assert_eq!(config.frames, 128);
    assert_eq!(config.interval_micros, 2_666);
    assert_eq!(config.max_queued_events, 1024);
    assert_eq!(
        config.scheduling.mode,
        SharedMemoryPumpSchedulingMode::Fixed
    );

    assert!(matches!(
        pump_config_from_params(
            SharedMemoryPumpStartParams {
                instance_id: 7,
                frames: Some(129),
                ..start_params(None, None)
            },
            128,
            48_000,
        ),
        Err(SharedMemoryPumpError::InvalidFrames { .. })
    ));
    assert!(matches!(
        pump_config_from_params(start_params(Some(64), Some(0)), 128, 48_000),
        Err(SharedMemoryPumpError::InvalidInterval)
    ));

    let invalid_queue = SharedMemoryPumpStartParams {
        max_queued_events: Some(0),
        ..start_params(None, None)
    };
    assert!(matches!(
        pump_config_from_params(invalid_queue, 128, 48_000),
        Err(SharedMemoryPumpError::InvalidMaxQueuedEvents { .. })
    ));
}

#[test]
fn validates_adaptive_pump_config() {
    let mut params = start_params(Some(64), Some(1_000));
    params.adaptive = true;
    params.min_interval_micros = Some(250);
    params.max_interval_micros = Some(4_000);
    params.idle_backoff_micros = Some(2_000);

    let config = pump_config_from_params(params, 128, 48_000).expect("config");

    assert_eq!(
        config.scheduling,
        SharedMemoryPumpSchedulingConfig::adaptive(1_000, 250, 4_000, 2_000, 64)
    );

    let mut invalid = start_params(Some(64), Some(1_000));
    invalid.adaptive = true;
    invalid.min_interval_micros = Some(4_001);
    invalid.max_interval_micros = Some(4_000);
    assert!(matches!(
        pump_config_from_params(invalid, 128, 48_000),
        Err(SharedMemoryPumpError::InvalidSchedulingWindow { .. })
    ));
}

#[test]
fn adaptive_scheduler_skips_input_underrun_and_output_backpressure() {
    let scheduling = SharedMemoryPumpSchedulingConfig::adaptive(1_000, 250, 4_000, 2_000, 64);
    let input_underrun = ring_status(2, 2, 32, 128, 0, 128);
    let output_backpressure = ring_status(2, 2, 128, 0, 0, 32);

    assert_eq!(
        scheduling.preflight_skip(&input_underrun),
        Some(SharedMemoryPumpPreflightSkip {
            outcome: SharedMemoryPumpTickOutcome::InputUnderrun,
            requested_frames: 64,
            available_frames: 32,
        })
    );
    assert_eq!(
        scheduling.next_delay_micros(
            SharedMemoryPumpTickOutcome::InputUnderrun,
            Some(&input_underrun),
        ),
        2_000
    );
    assert_eq!(
        scheduling.preflight_skip(&output_backpressure),
        Some(SharedMemoryPumpPreflightSkip {
            outcome: SharedMemoryPumpTickOutcome::OutputBackpressure,
            requested_frames: 64,
            available_frames: 32,
        })
    );
    assert_eq!(
        scheduling.next_delay_micros(
            SharedMemoryPumpTickOutcome::OutputBackpressure,
            Some(&output_backpressure),
        ),
        4_000
    );
}

#[test]
fn adaptive_scheduler_catches_up_only_when_input_has_backlog() {
    let scheduling = SharedMemoryPumpSchedulingConfig::adaptive(1_000, 250, 4_000, 2_000, 64);
    let backlog = ring_status(2, 2, 128, 0, 0, 128);
    let zero_input = ring_status(0, 2, 0, 128, 0, 128);

    assert_eq!(
        scheduling.next_delay_micros(SharedMemoryPumpTickOutcome::Success, Some(&backlog)),
        250
    );
    assert_eq!(
        scheduling.preflight_skip(&zero_input),
        None,
        "zero-input instruments must not require input ring frames"
    );
    assert_eq!(
        scheduling.next_delay_micros(SharedMemoryPumpTickOutcome::Success, Some(&zero_input)),
        1_000
    );
}

#[test]
fn records_process_timing_and_overruns() {
    let runtime = SharedMemoryPumpRuntime::default();
    let metrics = BridgeMetrics::new();
    let config = pump_config(64, 1_000);

    record_process_timing(config, &metrics, &runtime, 900);
    assert_eq!(runtime.last_process_micros.load(Ordering::Relaxed), 900);
    assert_eq!(runtime.max_process_micros.load(Ordering::Relaxed), 900);
    assert_eq!(runtime.overruns.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.snapshot().shared_memory_pump_overruns, 0);

    record_process_timing(config, &metrics, &runtime, 1_250);
    assert_eq!(runtime.last_process_micros.load(Ordering::Relaxed), 1_250);
    assert_eq!(runtime.max_process_micros.load(Ordering::Relaxed), 1_250);
    assert_eq!(runtime.overruns.load(Ordering::Relaxed), 1);
    assert_eq!(runtime.last_overrun_micros.load(Ordering::Relaxed), 250);
    assert_eq!(metrics.snapshot().shared_memory_pump_overruns, 1);
}

#[test]
fn classifies_shared_memory_tick_outcomes() {
    let input = WorkerSupervisorError::WorkerRejected {
        code: 4094,
        message: "shared audio read requested 2 frames but only 0 are available".to_string(),
        data: Some(json!({
            "kind": "shared-memory-audio",
            "reason": "input-underrun",
        })),
        stderr: String::new(),
    };
    assert_eq!(
        classify_worker_error(&input),
        SharedMemoryPumpTickOutcome::InputUnderrun
    );

    let output = WorkerSupervisorError::WorkerRejected {
        code: 4094,
        message: "shared memory output ring is full".to_string(),
        data: Some(json!({
            "kind": "shared-memory-audio",
            "workerData": { "reason": "output-backpressure" },
        })),
        stderr: String::new(),
    };
    assert_eq!(
        classify_worker_error(&output),
        SharedMemoryPumpTickOutcome::OutputBackpressure
    );

    let worker = WorkerSupervisorError::Timeout {
        method: "stream.sharedMemory.process",
        timeout_ms: 50,
        stderr: String::new(),
    };
    assert_eq!(
        classify_worker_error(&worker),
        SharedMemoryPumpTickOutcome::WorkerError
    );
}

#[test]
fn records_tick_outcome_counters() {
    let runtime = SharedMemoryPumpRuntime::default();
    let metrics = BridgeMetrics::new();

    record_tick_outcome(
        &runtime,
        &metrics,
        SharedMemoryPumpTickOutcome::InputUnderrun,
    );
    record_tick_outcome(
        &runtime,
        &metrics,
        SharedMemoryPumpTickOutcome::OutputBackpressure,
    );
    record_tick_outcome(&runtime, &metrics, SharedMemoryPumpTickOutcome::WorkerError);

    assert_eq!(runtime.input_underruns.load(Ordering::Relaxed), 1);
    assert_eq!(runtime.output_backpressure.load(Ordering::Relaxed), 1);
    assert_eq!(runtime.worker_errors.load(Ordering::Relaxed), 1);
    assert_eq!(
        outcome_from_code(runtime.last_outcome.load(Ordering::Relaxed)),
        Some(SharedMemoryPumpTickOutcome::WorkerError)
    );
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.shared_memory_pump_input_underruns, 1);
    assert_eq!(snapshot.shared_memory_pump_output_backpressure, 1);
    assert_eq!(snapshot.shared_memory_pump_worker_errors, 1);
}

#[test]
fn records_preflight_skip_diagnostics() {
    let runtime = SharedMemoryPumpRuntime::default();
    let metrics = BridgeMetrics::new();

    record_preflight_skip(
        pump_config(64, 1_000),
        &metrics,
        &runtime,
        SharedMemoryPumpPreflightSkip {
            outcome: SharedMemoryPumpTickOutcome::InputUnderrun,
            requested_frames: 64,
            available_frames: 0,
        },
    );

    assert_eq!(runtime.failures.load(Ordering::Relaxed), 1);
    assert_eq!(runtime.preflight_skips.load(Ordering::Relaxed), 1);
    assert_eq!(runtime.input_underruns.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.snapshot().shared_memory_pump_preflight_skips, 1);
    let last_error = runtime
        .last_error
        .lock()
        .expect("last error")
        .clone()
        .expect("diagnostic");
    assert_eq!(last_error.code, 4094);
    assert_eq!(last_error.data["reason"], "input-underrun");
}

#[test]
fn pump_runtime_queues_and_drains_ready_events() {
    let runtime = SharedMemoryPumpRuntime::new(1_000, 8);

    let result = runtime
        .enqueue_events(
            2,
            vec![json!({ "sampleOffset": 0, "kind": 1 })],
            vec![json!({ "sampleOffset": 1, "parameterId": 42 })],
            SharedMemoryPumpEventOverflowPolicy::Reject,
        )
        .expect("enqueue");
    assert_eq!(result.queued_events, 2);
    assert_eq!(result.status.pending_events, 2);

    let early = runtime.drain_events_for_iteration(1);
    assert_eq!(early.events, 0);
    assert_eq!(runtime.event_queue_status().pending_events, 2);

    let drain = runtime.drain_events_for_iteration(3);
    assert_eq!(drain.events, 2);
    assert_eq!(drain.late_events, 2);
    assert_eq!(drain.midi_events.len(), 1);
    assert_eq!(drain.parameter_events.len(), 1);

    let status = runtime.event_queue_status();
    assert_eq!(status.pending_events, 0);
    assert_eq!(status.drained_events, 2);
    assert_eq!(status.late_events, 2);
}

#[test]
fn pump_event_queue_rejects_or_drops_when_full() {
    let runtime = SharedMemoryPumpRuntime::new(1_000, 4);
    runtime
        .enqueue_events(
            2,
            vec![json!({}); 3],
            Vec::new(),
            SharedMemoryPumpEventOverflowPolicy::Reject,
        )
        .expect("first batch");

    assert!(matches!(
        runtime.enqueue_events(
            3,
            vec![json!({}); 2],
            Vec::new(),
            SharedMemoryPumpEventOverflowPolicy::Reject,
        ),
        Err(SharedMemoryPumpError::EventQueueCapacityExceeded { .. })
    ));

    let result = runtime
        .enqueue_events(
            3,
            vec![json!({}); 2],
            Vec::new(),
            SharedMemoryPumpEventOverflowPolicy::DropOldest,
        )
        .expect("drop oldest");
    assert_eq!(result.dropped_events, 3);
    assert_eq!(result.status.pending_events, 2);
    assert_eq!(result.status.dropped_events, 3);
}

#[tokio::test]
async fn stopped_record_reports_not_running() {
    let runtime = std::sync::Arc::new(SharedMemoryPumpRuntime::new(1_000, 1024));
    let (stop, _stop_rx) = watch::channel(false);
    let record = SharedMemoryPumpRecord {
        config: pump_config(64, 1_000),
        started_at: std::time::Instant::now(),
        runtime,
        metrics: std::sync::Arc::new(BridgeMetrics::new()),
        stop,
        task: tokio::spawn(async {}),
    };

    let status = stop_record(record);

    assert!(!status.running);
    assert_eq!(status.config.instance_id, 7);
}

fn start_params(frames: Option<u16>, interval_micros: Option<u64>) -> SharedMemoryPumpStartParams {
    SharedMemoryPumpStartParams {
        instance_id: 7,
        frames,
        interval_micros,
        adaptive: false,
        min_interval_micros: None,
        max_interval_micros: None,
        idle_backoff_micros: None,
        max_queued_events: None,
    }
}

fn pump_config(frames: u16, interval_micros: u64) -> SharedMemoryPumpConfig {
    SharedMemoryPumpConfig {
        instance_id: 7,
        frames,
        interval_micros,
        scheduling: SharedMemoryPumpSchedulingConfig::fixed(interval_micros, frames),
        max_queued_events: 1024,
    }
}

fn ring_status(
    input_channels: u16,
    output_channels: u16,
    input_readable_frames: u64,
    input_writable_frames: u64,
    output_readable_frames: u64,
    output_writable_frames: u64,
) -> SharedMemoryPumpRingStatus {
    SharedMemoryPumpRingStatus {
        input_channels,
        output_channels,
        input_readable_frames,
        input_writable_frames,
        output_readable_frames,
        output_writable_frames,
    }
}
