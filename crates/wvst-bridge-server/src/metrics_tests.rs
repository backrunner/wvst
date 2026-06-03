use super::*;

#[test]
fn reports_audio_route_latency_percentiles() {
    let metrics = BridgeMetrics::new();

    metrics.record_audio_route_latency_us(80);
    metrics.record_audio_route_latency_us(700);
    metrics.record_audio_route_latency_us(60_000);

    let latency = metrics.snapshot().audio_route_latency;
    assert_eq!(latency.count, 3);
    assert_eq!(latency.p50_us, Some(1_000));
    assert_eq!(latency.p95_us, Some(100_000));
    assert_eq!(latency.p99_us, Some(100_000));
    assert_eq!(latency.buckets[0].count, 1);
}

#[test]
fn reports_audio_stream_observation_counters() {
    let metrics = BridgeMetrics::new();

    metrics.record_audio_stream_observation(AudioStreamObservation {
        sequence_gap: 3,
        duplicate: true,
        out_of_order: true,
        late: true,
        interarrival_jitter_us: Some(1_200),
    });

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.audio_sequence_gap_events, 1);
    assert_eq!(snapshot.audio_sequence_gap_frames, 3);
    assert_eq!(snapshot.audio_frames_duplicate, 1);
    assert_eq!(snapshot.audio_frames_out_of_order, 1);
    assert_eq!(snapshot.audio_frames_late, 1);
    assert_eq!(snapshot.audio_interarrival_jitter.count, 1);
    assert_eq!(snapshot.audio_interarrival_jitter.p50_us, Some(2_000));

    metrics.increment_audio_backpressure_drops();
    assert_eq!(metrics.snapshot().audio_backpressure_drops, 1);
}

#[test]
fn reports_worker_shutdown_audit_counters() {
    let metrics = BridgeMetrics::new();

    metrics.record_worker_shutdown(WorkerShutdownAudit {
        kill_requested: true,
        tree_kill_requested: true,
        forced_kill_requested: false,
        wait_succeeded: true,
        wait_timed_out: false,
    });
    metrics.record_worker_shutdown(WorkerShutdownAudit {
        kill_requested: true,
        tree_kill_requested: true,
        forced_kill_requested: true,
        wait_succeeded: false,
        wait_timed_out: true,
    });

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.worker_shutdowns, 2);
    assert_eq!(snapshot.worker_kill_requests, 2);
    assert_eq!(snapshot.worker_tree_kill_requests, 2);
    assert_eq!(snapshot.worker_forced_kill_requests, 1);
    assert_eq!(snapshot.worker_wait_successes, 1);
    assert_eq!(snapshot.worker_wait_timeouts, 1);
}

#[test]
fn reports_shared_memory_process_counters() {
    let metrics = BridgeMetrics::new();

    metrics.record_shared_memory_process_success(128, 250);
    metrics.record_shared_memory_process_success(64, 700);
    metrics.record_shared_memory_process_failure(1_200);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.shared_memory_process_blocks, 2);
    assert_eq!(snapshot.shared_memory_process_frames, 192);
    assert_eq!(snapshot.shared_memory_process_failures, 1);
    assert_eq!(snapshot.shared_memory_process_latency.count, 3);
    assert_eq!(snapshot.shared_memory_process_latency.p50_us, Some(1_000));

    metrics.increment_shared_memory_pump_overruns();
    assert_eq!(metrics.snapshot().shared_memory_pump_overruns, 1);
    metrics.increment_shared_memory_pump_preflight_skips();
    metrics.increment_shared_memory_pump_input_underruns();
    metrics.increment_shared_memory_pump_output_backpressure();
    metrics.increment_shared_memory_pump_worker_errors();
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.shared_memory_pump_preflight_skips, 1);
    assert_eq!(snapshot.shared_memory_pump_input_underruns, 1);
    assert_eq!(snapshot.shared_memory_pump_output_backpressure, 1);
    assert_eq!(snapshot.shared_memory_pump_worker_errors, 1);
}

#[test]
fn reports_shared_memory_pump_event_counters() {
    let metrics = BridgeMetrics::new();

    metrics.record_shared_memory_pump_events_enqueued(5);
    metrics.record_shared_memory_pump_events_drained(3, 1);
    metrics.record_shared_memory_pump_events_dropped(2);
    metrics.record_shared_memory_pump_events_cleared(4);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.shared_memory_pump_events_enqueued, 5);
    assert_eq!(snapshot.shared_memory_pump_events_drained, 3);
    assert_eq!(snapshot.shared_memory_pump_events_late, 1);
    assert_eq!(snapshot.shared_memory_pump_events_dropped, 2);
    assert_eq!(snapshot.shared_memory_pump_events_cleared, 4);
}
