use super::*;
use serde_json::json;

#[test]
fn passes_process_output_expectations_when_probe_reports_output_activity() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "outputEvents": 3,
                    "outputParameterChanges": 2,
                    "diagnostics": {
                        "outputEvents": {
                            "rawEvents": 4,
                            "normalizedEvents": 3,
                            "filteredEvents": 1,
                            "advancedEvents": 3,
                            "advancedDataEvents": 1,
                            "advancedNoteExpressionEvents": 1,
                            "advancedChordEvents": 1,
                            "advancedScaleEvents": 0,
                            "advancedPayloadEvents": 2,
                            "advancedPayloadBytes": 76,
                            "advancedRawPayloadEvents": 1,
                            "advancedTextPayloadEvents": 1,
                            "advancedTruncatedPayloadEvents": 1,
                            "advancedUnavailablePayloadEvents": 0,
                            "advancedInvalidTextPayloadEvents": 0,
                            "invalidSampleOffsetEvents": 0,
                            "invalidPayloadEvents": 0,
                            "unknownTypeEvents": 1
                        },
                        "outputParameterChanges": {
                            "rawPoints": 2,
                            "normalizedPoints": 2,
                            "filteredPoints": 0
                        }
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("midi-output", "/tmp/Instrument.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .min_output_events(3)
                .min_output_parameter_changes(2)
                .min_normalized_output_events(3)
                .min_advanced_output_events(3)
                .min_advanced_data_output_events(1)
                .min_advanced_note_expression_output_events(1)
                .min_advanced_chord_output_events(1)
                .min_advanced_scale_output_events(0)
                .min_advanced_payload_events(2)
                .min_advanced_payload_bytes(64)
                .min_advanced_raw_payload_events(1)
                .min_advanced_text_payload_events(1)
                .min_normalized_output_parameter_changes(2)
                .max_advanced_truncated_payload_events(1)
                .max_advanced_unavailable_payload_events(0)
                .max_advanced_invalid_text_payload_events(0)
                .max_filtered_output_events(1)
                .max_filtered_output_parameter_changes(0),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert_eq!(report.expectation_failed, 0);
    assert_eq!(report.results[0].expectation_failures, Vec::<String>::new());
}

#[test]
fn marks_process_output_expectation_failures() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "outputEvents": 1,
                    "outputParameterChanges": 0,
                    "diagnostics": {
                        "outputEvents": {
                            "rawEvents": 4,
                            "normalizedEvents": 1,
                            "filteredEvents": 3,
                            "advancedEvents": 1,
                            "advancedDataEvents": 0,
                            "advancedNoteExpressionEvents": 1,
                            "advancedChordEvents": 0,
                            "advancedScaleEvents": 0,
                            "advancedPayloadEvents": 1,
                            "advancedPayloadBytes": 16,
                            "advancedRawPayloadEvents": 0,
                            "advancedTextPayloadEvents": 1,
                            "advancedTruncatedPayloadEvents": 2,
                            "advancedUnavailablePayloadEvents": 1,
                            "advancedInvalidTextPayloadEvents": 1,
                            "invalidSampleOffsetEvents": 1,
                            "invalidPayloadEvents": 1,
                            "unknownTypeEvents": 1
                        },
                        "outputParameterChanges": {
                            "rawPoints": 2,
                            "normalizedPoints": 0,
                            "filteredPoints": 2
                        }
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("missing-output", "/tmp/Instrument.vst3", "class-a")
            .with_expectations(
                RuntimeProbeExpectations::default()
                    .min_output_events(2)
                    .min_output_parameter_changes(1)
                    .min_normalized_output_events(2)
                    .min_advanced_output_events(2)
                    .min_advanced_data_output_events(1)
                    .min_advanced_note_expression_output_events(2)
                    .min_advanced_chord_output_events(1)
                    .min_advanced_scale_output_events(1)
                    .min_advanced_payload_events(2)
                    .min_advanced_payload_bytes(64)
                    .min_advanced_raw_payload_events(1)
                    .min_advanced_text_payload_events(2)
                    .min_normalized_output_parameter_changes(1)
                    .max_advanced_truncated_payload_events(0)
                    .max_advanced_unavailable_payload_events(0)
                    .max_advanced_invalid_text_payload_events(0)
                    .max_filtered_output_events(0)
                    .max_filtered_output_parameter_changes(0),
            ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.expectation_failed, 1);
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("at least 2 output events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("output parameter changes"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("normalized output events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced output events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced data output events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced note-expression output events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced chord output events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced scale output events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced output payload events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced output payload bytes"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced raw output payload events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced text output payload events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced truncated output payload events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced unavailable output payload events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("advanced invalid-text output payload events"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("filtered output events"))
    );
}
