use super::*;
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
