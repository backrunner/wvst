use super::*;
use serde_json::json;

#[test]
fn passes_process_timing_expectations_when_probe_reports_fast_process() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "processTimeMicros": {
                        "min": 20,
                        "p50": 30,
                        "p95": 80,
                        "p99": 100,
                        "max": 120
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("fast-effect", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .max_process_time_p50_micros(40)
                .max_process_time_p95_micros(100)
                .max_process_time_p99_micros(120)
                .max_process_time_max_micros(150),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert_eq!(report.expectation_failed, 0);
    assert_eq!(report.results[0].expectation_failures, Vec::<String>::new());
}

#[test]
fn marks_process_timing_expectation_failures() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "processTimeMicros": {
                        "min": 20,
                        "p50": 80,
                        "p95": 250,
                        "p99": 300,
                        "max": 500
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("slow-effect", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .max_process_time_p50_micros(50)
                .max_process_time_p95_micros(100)
                .max_process_time_p99_micros(200)
                .max_process_time_max_micros(400),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.expectation_failed, 1);
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("processTimeMicros.p50 <= 50 us"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("processTimeMicros.p95 <= 100 us"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("processTimeMicros.p99 <= 200 us"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("processTimeMicros.max <= 400 us"))
    );
}
