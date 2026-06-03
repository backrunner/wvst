use super::*;

mod audio_bus_summary;
mod coverage_audit;
mod expectations;
mod output_expectations;
mod process_output_summary;
mod process_timing_expectations;
mod process_timing_summary;
mod runtime_characteristics_expectations;
mod runtime_characteristics_summary;
mod summaries;

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
        .with_evidence(
            RuntimeProbeCaseEvidence::third_party_plugin(
                "Example Audio",
                "Example Synth",
                RuntimeProbePluginKind::Instrument,
            )
            .with_plugin_version("1.0.0")
            .with_tag("instrument")
            .with_validation_notes("local fixture"),
        )
        .with_processing(48_000, 256, 0, 2, 128, 8)
        .with_controller_edit_probe_parameter_id(99)
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
        invocation.timeout_millis,
        DEFAULT_RUNTIME_PROBE_TIMEOUT_MILLIS
    );
    assert_eq!(
        invocation.args[1],
        "/Library/Audio/Plug-Ins/VST3/Synth.vst3"
    );
    assert_eq!(invocation.args[2], "class-a");
    assert_eq!(
        report.results[0]
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.plugin_name.as_deref()),
        Some("Example Synth")
    );
    assert_eq!(report.evidence.reported_cases, 1);
    assert_eq!(report.evidence.third_party_cases, 1);
    assert_eq!(report.evidence.instrument_cases, 1);
    assert_eq!(report.evidence.tags.get("instrument").copied(), Some(1));
    assert!(invocation.args.contains(&"--input-channels".to_string()));
    assert!(invocation.args.contains(&"0".to_string()));
    assert!(invocation.args.contains(&"--note".to_string()));
    assert!(invocation.args.contains(&"60:0.750:1".to_string()));
    assert!(
        !invocation
            .args
            .contains(&"--skip-controller-edit-probe".to_string())
    );
    assert!(
        !invocation
            .args
            .contains(&"--skip-connection-notify-probe".to_string())
    );
    assert!(
        !invocation
            .args
            .contains(&"--skip-state-roundtrip-probe".to_string())
    );
    assert!(
        invocation
            .args
            .contains(&"--controller-edit-probe-parameter".to_string())
    );
    assert!(invocation.args.contains(&"99".to_string()));
    assert!(invocation.args.contains(&"--parameter-change".to_string()));
    assert!(invocation.args.contains(&"42=0.500:64".to_string()));
}

#[test]
fn builds_runtime_probe_invocation_with_controller_edit_probe_disabled() {
    let mut executor = RecordingExecutor::default();
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("sensitive-fx", "/tmp/Sensitive.vst3", "class-a")
            .with_controller_edit_probe(false)
            .with_controller_edit_probe_parameter_id(99),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    let invocation = &executor.invocations[0];
    assert!(
        invocation
            .args
            .contains(&"--skip-controller-edit-probe".to_string())
    );
    assert!(
        invocation
            .args
            .contains(&"--controller-edit-probe-parameter".to_string())
    );
    assert!(invocation.args.contains(&"99".to_string()));
}

#[test]
fn builds_runtime_probe_invocation_with_connection_notify_probe_disabled() {
    let mut executor = RecordingExecutor::default();
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("sensitive-fx", "/tmp/Sensitive.vst3", "class-a")
            .with_connection_notify_probe(false),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert!(
        executor.invocations[0]
            .args
            .contains(&"--skip-connection-notify-probe".to_string())
    );
}

#[test]
fn builds_runtime_probe_invocation_with_state_roundtrip_probe_disabled() {
    let mut executor = RecordingExecutor::default();
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("sensitive-fx", "/tmp/Sensitive.vst3", "class-a")
            .with_state_roundtrip_probe(false),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert!(
        executor.invocations[0]
            .args
            .contains(&"--skip-state-roundtrip-probe".to_string())
    );
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
    assert_eq!(report.timed_out, 0);
    assert_eq!(report.expectation_failed, 0);
    assert_eq!(report.results[0].case_name, "broken");
    assert_eq!(report.results[1].class_id, "class-b");
}

#[test]
fn timed_out_cases_fail_matrix() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            status: RuntimeProbeStatus::TimedOut,
            stderr: "runtime probe timed out after 25 ms".to_string(),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("hung", "/tmp/Hung.vst3", "class-a").with_timeout_millis(25),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.timed_out, 1);
    assert_eq!(executor.invocations[0].timeout_millis, 25);
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
