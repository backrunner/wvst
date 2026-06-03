use super::*;
use serde_json::json;

#[derive(Default)]
struct RecordingExecutor {
    invocations: Vec<RuntimeProbeInvocation>,
    results: Vec<RuntimeProbeResult>,
}

impl RuntimeProbeExecutor for RecordingExecutor {
    fn run(&mut self, invocation: RuntimeProbeInvocation) -> RuntimeProbeResult {
        self.invocations.push(invocation);
        self.results.pop().unwrap_or_default()
    }
}

#[test]
fn builds_runtime_probe_invocation_with_note_and_parameter_change() {
    let mut executor = RecordingExecutor::default();
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new(
            "synth",
            "/Library/Audio/Plug-Ins/VST3/Synth.vst3",
            "class-a",
        )
        .with_processing(48_000, 256, 0, 2, 128, 8)
        .with_note(RuntimeProbeNote::new(60, 750, 1))
        .with_parameter_change(RuntimeProbeParameterChange::new(42, 500, 64)),
    );

    let report = matrix.run_with(&mut executor);

    assert_eq!(
        report.schema_version,
        RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION
    );
    assert!(report.all_passed());
    let invocation = &executor.invocations[0];
    assert_eq!(
        invocation.worker_executable,
        PathBuf::from("/tmp/wvst-host-worker")
    );
    assert_eq!(invocation.args[0], "runtime-probe");
    assert_eq!(
        invocation.args[1],
        "/Library/Audio/Plug-Ins/VST3/Synth.vst3"
    );
    assert_eq!(invocation.args[2], "class-a");
    assert!(invocation.args.contains(&"--input-channels".to_string()));
    assert!(invocation.args.contains(&"0".to_string()));
    assert!(invocation.args.contains(&"--note".to_string()));
    assert!(invocation.args.contains(&"60:0.750:1".to_string()));
    assert!(invocation.args.contains(&"--parameter-change".to_string()));
    assert!(invocation.args.contains(&"42=0.500:64".to_string()));
}

#[test]
fn summarizes_failed_and_launch_failed_cases() {
    let mut executor = RecordingExecutor {
        results: vec![
            RuntimeProbeResult {
                status: RuntimeProbeStatus::LaunchFailed,
                ..RuntimeProbeResult::default()
            },
            RuntimeProbeResult {
                status: RuntimeProbeStatus::Failed,
                exit_code: Some(2),
                ..RuntimeProbeResult::default()
            },
        ],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker")
        .with_case(RuntimeProbeCase::new(
            "broken",
            "/tmp/Broken.vst3",
            "class-a",
        ))
        .with_case(RuntimeProbeCase::new(
            "missing",
            "/tmp/Missing.vst3",
            "class-b",
        ));

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(
        report.schema_version,
        RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.passed, 0);
    assert_eq!(report.failed, 1);
    assert_eq!(report.launch_failed, 1);
    assert_eq!(report.results[0].case_name, "broken");
    assert_eq!(report.results[1].class_id, "class-b");
}

#[test]
fn summarizes_probe_audio_health() {
    let report = RuntimeProbeMatrixReport::new(vec![
        RuntimeProbeResult {
            case_name: "silent-effect".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "totalBlocks": 2,
                    "silentOutputBlocks": 2,
                    "nonZeroOutputBlocks": 0,
                    "nonFiniteOutputSamples": 0,
                    "clippedOutputSamples": 0,
                    "maxOutputPeak": 0.0,
                    "outputRms": 0.0
                }
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "hot-synth".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "totalBlocks": 3,
                    "silentOutputBlocks": 1,
                    "nonZeroOutputBlocks": 2,
                    "nonFiniteOutputSamples": 4,
                    "clippedOutputSamples": 2,
                    "maxOutputPeak": 1.25,
                    "outputRms": 0.5
                }
            })),
            ..RuntimeProbeResult::default()
        },
    ]);

    assert_eq!(report.audio_health.reported_cases, 2);
    assert_eq!(report.audio_health.fully_silent_cases, 1);
    assert_eq!(report.audio_health.non_zero_cases, 1);
    assert_eq!(report.audio_health.non_finite_cases, 1);
    assert_eq!(report.audio_health.clipped_cases, 1);
    assert_eq!(report.audio_health.total_silent_output_blocks, 3);
    assert_eq!(report.audio_health.total_non_finite_output_samples, 4);
    assert_eq!(report.audio_health.total_clipped_output_samples, 2);
    assert_eq!(report.audio_health.max_output_peak, 1.25);
    assert_eq!(
        report.audio_health.max_output_peak_case.as_deref(),
        Some("hot-synth")
    );
    assert_eq!(report.audio_health.max_output_rms, 0.5);
    assert_eq!(
        report.audio_health.max_output_rms_case.as_deref(),
        Some("hot-synth")
    );

    let value = serde_json::to_value(&report).expect("report json");
    assert_eq!(value["audioHealth"]["reportedCases"], 2);
    assert_eq!(value["audioHealth"]["fullySilentCases"], 1);
    assert_eq!(value["audioHealth"]["maxOutputPeakCase"], "hot-synth");
}

#[test]
fn parses_runtime_probe_json_report_from_output() {
    let report = parse_probe_report(
        "worker banner\n{\"schemaVersion\":1,\"ok\":false,\"data\":{\"kind\":\"vst3-runtime-init\"}}\n",
    )
    .expect("probe report");

    assert_eq!(report["ok"], false);
    assert_eq!(report["data"]["kind"], "vst3-runtime-init");
    assert!(parse_probe_report("{\"schemaVersion\":2,\"ok\":false}").is_none());
    assert!(parse_probe_report("plain stderr").is_none());
}
