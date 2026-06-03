use super::*;
use serde_json::json;

#[test]
fn passes_runtime_characteristics_expectations() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "latencySamples": 64,
                "tailSamples": 128,
                "tailInfo": {
                    "samples": 128,
                    "kind": "finite",
                    "finiteSamples": 128
                },
                "processContextRequirements": 6
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("runtime-info", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .max_latency_samples(128)
                .max_tail_samples(256)
                .require_tail_info()
                .expected_tail_kind("finite")
                .expected_process_context_requirements(6)
                .required_process_context_requirements(2),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert_eq!(report.expectation_failed, 0);
    assert_eq!(report.results[0].expectation_failures, Vec::<String>::new());
}

#[test]
fn require_tail_info_fails_when_tail_samples_only() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "tailSamples": u32::MAX,
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("tail-samples-only", "/tmp/Effect.vst3", "class-a")
            .with_expectations(
                RuntimeProbeExpectations::default()
                    .require_tail_info()
                    .expected_tail_kind("infinite"),
            ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.expectation_failed, 1);
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("expected tailInfo with kind and samples"))
    );
}

#[test]
fn marks_runtime_characteristics_expectation_failures() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "latencySamples": 512,
                "tailSamples": 1024,
                "tailInfo": {
                    "samples": 1024,
                    "kind": "finite",
                    "finiteSamples": 1024
                },
                "processContextRequirements": 2
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("bad-runtime-info", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .max_latency_samples(128)
                .max_tail_samples(256)
                .expected_tail_kind("infinite")
                .expected_process_context_requirements(3)
                .required_process_context_requirements(4),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.expectation_failed, 1);
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("latencySamples <= 128"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("tailSamples <= 256"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("tailInfo.kind"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("processContextRequirements == 3"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("include bitmask 4"))
    );
}
