use super::*;
use serde_json::json;
use std::time::Duration;
use wvst_protocol::AudioFrameFlags;

use crate::latency::LatencyHarnessConfig;
use crate::stability::{
    StabilityObservation, StabilityRunConfig, StabilityRunner, StabilityStepOutcome,
};

#[test]
fn passes_when_snapshot_stays_inside_budget() {
    let report = passing_report();
    let budget = StabilityBudget::default()
        .min_observations(3)
        .max_dropped_frames(0)
        .max_timeout_frames(0)
        .max_late_frames(0)
        .max_sequence_gap_events(0)
        .max_route_latency_p95_us(750)
        .max_round_trip_p99_us(6_000);

    let budget_report = report.evaluate_budget(budget);

    assert!(budget_report.passed);
    assert!(budget_report.violations.is_empty());
}

#[test]
fn reports_count_and_percentile_violations() {
    let config = StabilityRunConfig::new(LatencyHarnessConfig::new(48_000, 128))
        .with_duration(Duration::from_secs(1))
        .with_max_blocks(3);
    let report = StabilityRunner::new(config).run(|step| match step.sequence {
        0 => StabilityStepOutcome::Dropped,
        1 => StabilityStepOutcome::Observed(StabilityObservation::new(
            Some(step.sent_frame_time + 512),
            Some(2_000),
            AudioFrameFlags::LATE,
        )),
        _ => StabilityStepOutcome::Observed(StabilityObservation::new(
            Some(step.sent_frame_time + 128),
            Some(500),
            AudioFrameFlags::empty(),
        )),
    });
    let budget = StabilityBudget::default()
        .min_observations(3)
        .max_dropped_frames(0)
        .max_late_frames(0)
        .max_route_latency_p95_us(1_000)
        .max_round_trip_p95_us(5_000);

    let budget_report = report.evaluate_budget(budget);

    assert!(!budget_report.passed);
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MinimumNotMet {
                metric: "observations",
                min: 3,
                actual: 2,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "droppedFrames",
                max: 0,
                actual: 1,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "routeLatencyUs.p95",
                max: 1_000,
                actual: 2_000,
            })
    );
}

#[test]
fn reports_missing_percentile_when_no_observations_exist() {
    let config = StabilityRunConfig::new(LatencyHarnessConfig::new(48_000, 128)).with_max_blocks(1);
    let report = StabilityRunner::new(config).run(|_| StabilityStepOutcome::Dropped);

    let budget_report = report.evaluate_budget(
        StabilityBudget::default()
            .max_route_latency_p99_us(1_000)
            .max_round_trip_p99_us(4_000),
    );

    assert_eq!(
        budget_report.violations,
        vec![
            StabilityBudgetViolation::PercentileMissing {
                metric: "routeLatencyUs.p99",
            },
            StabilityBudgetViolation::PercentileMissing {
                metric: "roundTripUs.p99",
            },
        ]
    );
}

#[test]
fn serializes_budget_report_for_ci_artifacts() {
    let report = passing_report().evaluate_budget(
        StabilityBudget::default()
            .max_route_latency_p95_us(100)
            .max_round_trip_p99_us(100),
    );

    let value = serde_json::to_value(report).expect("budget report json");

    assert_eq!(value["passed"], false);
    assert_eq!(value["violations"][0]["kind"], "maximum-exceeded");
    assert_eq!(value["violations"][0]["metric"], "routeLatencyUs.p95");
}

#[test]
fn evaluates_webaudio_loopback_metrics_with_native_budget() {
    let report = passing_report();
    let webaudio = WebAudioLoopbackMetrics {
        input_frames: 384,
        output_frames: 256,
        underflows: 2,
        overflows: 1,
        dropped_input_quanta: 3,
        dropped_output_quanta: 4,
        dropped_midi_events: 5,
        dropped_parameter_events: 6,
        late_midi_events: 7,
        late_parameter_events: 8,
        transport_failures: 9,
        end_to_end_round_trip_us: LatencyPercentiles {
            count: 3,
            p50: Some(3_000),
            p95: Some(5_500),
            p99: Some(7_000),
        },
        input_sequence: 3,
        input_consumed_sequence: 1,
        output_sequence: 2,
        output_consumed_sequence: 2,
        pending_input_quanta: 2,
        pending_output_quanta: 0,
    };
    let budget = StabilityBudget::default()
        .min_observations(3)
        .min_webaudio_input_frames(384)
        .min_webaudio_output_frames(384)
        .max_webaudio_underflows(0)
        .max_webaudio_overflows(0)
        .max_webaudio_dropped_input_quanta(0)
        .max_webaudio_dropped_output_quanta(0)
        .max_webaudio_dropped_midi_events(0)
        .max_webaudio_dropped_parameter_events(0)
        .max_webaudio_late_midi_events(0)
        .max_webaudio_late_parameter_events(0)
        .max_webaudio_transport_failures(0)
        .max_webaudio_end_to_end_round_trip_p95_us(5_000)
        .max_webaudio_end_to_end_round_trip_p99_us(6_000)
        .max_pending_input_quanta(1)
        .max_pending_output_quanta(0);

    let budget_report = report.evaluate_budget_with_webaudio(budget, webaudio);

    assert!(!budget_report.passed);
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MinimumNotMet {
                metric: "webAudio.outputFrames",
                min: 384,
                actual: 256,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.underflows",
                max: 0,
                actual: 2,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.pendingInputQuanta",
                max: 1,
                actual: 2,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.droppedInputQuanta",
                max: 0,
                actual: 3,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.droppedOutputQuanta",
                max: 0,
                actual: 4,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.droppedMidiEvents",
                max: 0,
                actual: 5,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.droppedParameterEvents",
                max: 0,
                actual: 6,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.lateMidiEvents",
                max: 0,
                actual: 7,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.lateParameterEvents",
                max: 0,
                actual: 8,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.transportFailures",
                max: 0,
                actual: 9,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.endToEndRoundTripUs.p95",
                max: 5_000,
                actual: 5_500,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.endToEndRoundTripUs.p99",
                max: 6_000,
                actual: 7_000,
            })
    );
}

#[test]
fn reports_missing_webaudio_metrics_when_budget_requires_them() {
    let report = passing_report();
    let budget = StabilityBudget::default().max_webaudio_underflows(0);

    let budget_report = report.evaluate_budget(budget);

    assert_eq!(
        budget_report.violations,
        vec![StabilityBudgetViolation::MetricMissing {
            metric: "webAudioLoopbackMetrics",
        }]
    );
}

#[test]
fn reports_missing_webaudio_end_to_end_percentiles() {
    let report = passing_report();
    let webaudio = WebAudioLoopbackMetrics {
        input_frames: 384,
        output_frames: 384,
        ..WebAudioLoopbackMetrics::default()
    };
    let budget = StabilityBudget::default().max_webaudio_end_to_end_round_trip_p95_us(4_000);

    let budget_report = report.evaluate_budget_with_webaudio(budget, webaudio);

    assert_eq!(
        budget_report.violations,
        vec![StabilityBudgetViolation::PercentileMissing {
            metric: "webAudio.endToEndRoundTripUs.p95",
        }]
    );
}

#[test]
fn evaluates_bridge_metrics_with_native_budget() {
    let report = passing_report();
    let bridge = BridgeStabilityMetrics {
        shared_memory_pump_preflight_skips: 1,
        shared_memory_pump_overruns: 2,
        shared_memory_pump_input_underruns: 3,
        shared_memory_pump_output_backpressure: 4,
        shared_memory_pump_worker_errors: 5,
        shared_memory_pump_events_enqueued: 6,
        shared_memory_pump_events_drained: 4,
        shared_memory_pump_events_late: 2,
        shared_memory_pump_events_dropped: 1,
        shared_memory_pump_events_cleared: 1,
        shared_memory_process_latency: BridgeLatencyPercentiles {
            count: 3,
            p50_us: Some(500),
            p95_us: Some(2_000),
            p99_us: Some(5_000),
        },
        shared_memory_pump_last_process_micros: Some(1_500),
        shared_memory_pump_max_process_micros: Some(5_000),
        shared_memory_pump_last_overrun_micros: Some(750),
    };
    let budget = StabilityBudget::default()
        .max_shared_memory_pump_preflight_skips(0)
        .max_shared_memory_pump_overruns(0)
        .max_shared_memory_pump_input_underruns(0)
        .max_shared_memory_pump_output_backpressure(0)
        .max_shared_memory_pump_worker_errors(0)
        .min_shared_memory_pump_events_enqueued(7)
        .min_shared_memory_pump_events_drained(5)
        .max_shared_memory_pump_events_late(0)
        .max_shared_memory_pump_events_dropped(0)
        .max_shared_memory_pump_events_cleared(0)
        .max_shared_memory_process_latency_p95_us(1_000)
        .max_shared_memory_process_latency_p99_us(4_000)
        .max_shared_memory_pump_last_process_micros(1_000)
        .max_shared_memory_pump_max_process_micros(4_000)
        .max_shared_memory_pump_last_overrun_micros(500);

    let budget_report = report.evaluate_budget_with_metrics(budget, None, Some(bridge));

    assert!(!budget_report.passed);
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "bridge.sharedMemoryPumpInputUnderruns",
                max: 0,
                actual: 3,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MinimumNotMet {
                metric: "bridge.sharedMemoryPumpEventsDrained",
                min: 5,
                actual: 4,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "bridge.sharedMemoryPumpEventsDropped",
                max: 0,
                actual: 1,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "bridge.sharedMemoryProcessLatency.p95",
                max: 1_000,
                actual: 2_000,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "bridge.sharedMemoryPumpMaxProcessMicros",
                max: 4_000,
                actual: 5_000,
            })
    );
    assert!(
        budget_report
            .violations
            .contains(&StabilityBudgetViolation::MaximumExceeded {
                metric: "bridge.sharedMemoryPumpLastOverrunMicros",
                max: 500,
                actual: 750,
            })
    );
}

#[test]
fn normalizes_bridge_metrics_from_json_rpc_and_pump_status() {
    let bridge = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "result": {
            "bridgeMetrics": {
                "sharedMemoryPumpEventsEnqueued": 2,
                "sharedMemoryProcessLatency": {
                    "count": 3,
                    "p50Us": 500,
                    "p95Us": 1000,
                    "p99Us": 2000
                }
            },
            "dataPlane": {
                "sharedMemoryPump": {
                    "running": true,
                    "status": {
                        "preflightSkips": 1,
                        "overruns": 2,
                        "inputUnderruns": 3,
                        "outputBackpressure": 4,
                        "workerErrors": 5,
                        "lastProcessMicros": 700,
                        "maxProcessMicros": 900,
                        "lastOverrunMicros": 25,
                        "events": {
                            "enqueuedEvents": 6,
                            "drainedEvents": 7,
                            "lateEvents": 8,
                            "droppedEvents": 9,
                            "clearedEvents": 10
                        }
                    }
                }
            }
        }
    });

    let metrics = BridgeStabilityMetrics::from_json_str(&bridge.to_string()).expect("bridge json");

    assert_eq!(metrics.shared_memory_pump_preflight_skips, 1);
    assert_eq!(metrics.shared_memory_pump_events_enqueued, 2);
    assert_eq!(metrics.shared_memory_pump_events_drained, 7);
    assert_eq!(metrics.shared_memory_process_latency.p95_us, Some(1_000));
    assert_eq!(metrics.shared_memory_pump_last_process_micros, Some(700));
    assert_eq!(metrics.shared_memory_pump_max_process_micros, Some(900));
    assert_eq!(metrics.shared_memory_pump_last_overrun_micros, Some(25));
}

#[test]
fn normalizes_bridge_metrics_from_pump_status_result() {
    let bridge = json!({
        "jsonrpc": "2.0",
        "id": 8,
        "result": {
            "instanceId": 42,
            "status": {
                "preflightSkips": 2,
                "overruns": 3,
                "inputUnderruns": 4,
                "outputBackpressure": 5,
                "workerErrors": 6,
                "lastProcessMicros": 1100,
                "maxProcessMicros": 1700,
                "lastOverrunMicros": null,
                "events": {
                    "enqueuedEvents": 11,
                    "drainedEvents": 10,
                    "lateEvents": 1,
                    "droppedEvents": 0,
                    "clearedEvents": 2
                }
            }
        }
    });

    let metrics = BridgeStabilityMetrics::from_json_str(&bridge.to_string()).expect("bridge json");

    assert_eq!(metrics.shared_memory_pump_preflight_skips, 2);
    assert_eq!(metrics.shared_memory_pump_overruns, 3);
    assert_eq!(metrics.shared_memory_pump_worker_errors, 6);
    assert_eq!(metrics.shared_memory_pump_events_enqueued, 11);
    assert_eq!(metrics.shared_memory_pump_events_drained, 10);
    assert_eq!(metrics.shared_memory_pump_events_cleared, 2);
    assert_eq!(metrics.shared_memory_pump_last_process_micros, Some(1_100));
    assert_eq!(metrics.shared_memory_pump_max_process_micros, Some(1_700));
    assert_eq!(metrics.shared_memory_pump_last_overrun_micros, None);
}

#[test]
fn reports_missing_bridge_metrics_when_budget_requires_them() {
    let report = passing_report();
    let budget = StabilityBudget::default().max_shared_memory_pump_worker_errors(0);

    let budget_report = report.evaluate_budget(budget);

    assert_eq!(
        budget_report.violations,
        vec![StabilityBudgetViolation::MetricMissing {
            metric: "bridgeMetrics",
        }]
    );
}

#[test]
fn reports_missing_bridge_pump_timing_when_budget_requires_it() {
    let report = passing_report();
    let budget = StabilityBudget::default()
        .max_shared_memory_process_latency_p95_us(1_000)
        .max_shared_memory_pump_last_process_micros(1_000);
    let bridge = BridgeStabilityMetrics::default();

    let budget_report = report.evaluate_budget_with_metrics(budget, None, Some(bridge));

    assert_eq!(
        budget_report.violations,
        vec![
            StabilityBudgetViolation::PercentileMissing {
                metric: "bridge.sharedMemoryProcessLatency.p95",
            },
            StabilityBudgetViolation::MetricMissing {
                metric: "bridge.sharedMemoryPumpLastProcessMicros",
            },
        ]
    );
}

fn passing_report() -> StabilityRunReport {
    let config = StabilityRunConfig::new(LatencyHarnessConfig::new(48_000, 128))
        .with_duration(Duration::from_secs(1))
        .with_max_blocks(3);

    StabilityRunner::new(config).run(|step| {
        StabilityStepOutcome::Observed(StabilityObservation::new(
            Some(step.sent_frame_time + 128),
            Some(500 + step.sequence),
            AudioFrameFlags::empty(),
        ))
    })
}
