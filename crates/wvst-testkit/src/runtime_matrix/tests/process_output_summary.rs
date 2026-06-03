use super::*;
use serde_json::json;

#[test]
fn summarizes_probe_process_output_health() {
    let report = RuntimeProbeMatrixReport::new(vec![
        RuntimeProbeResult {
            case_name: "arp-synth".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "outputEvents": 4,
                    "outputParameterChanges": 2,
                    "diagnostics": {
                        "outputEvents": {
                            "rawEvents": 5,
                            "normalizedEvents": 4,
                            "filteredEvents": 1,
                            "advancedEvents": 1,
                            "advancedDataEvents": 0,
                            "advancedNoteExpressionEvents": 1,
                            "advancedChordEvents": 0,
                            "advancedScaleEvents": 0,
                            "advancedPayloadEvents": 1,
                            "advancedPayloadBytes": 12,
                            "advancedRawPayloadEvents": 0,
                            "advancedTextPayloadEvents": 1,
                            "advancedTruncatedPayloadEvents": 0,
                            "advancedUnavailablePayloadEvents": 0,
                            "advancedInvalidTextPayloadEvents": 0
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
        },
        RuntimeProbeResult {
            case_name: "mod-effect".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "outputEvents": 1,
                    "outputParameterChanges": 5,
                    "diagnostics": {
                        "outputEvents": {
                            "rawEvents": 1,
                            "normalizedEvents": 1,
                            "filteredEvents": 0,
                            "advancedEvents": 0,
                            "advancedDataEvents": 0,
                            "advancedNoteExpressionEvents": 0,
                            "advancedChordEvents": 0,
                            "advancedScaleEvents": 0,
                            "advancedPayloadEvents": 1,
                            "advancedPayloadBytes": 64,
                            "advancedRawPayloadEvents": 1,
                            "advancedTextPayloadEvents": 0,
                            "advancedTruncatedPayloadEvents": 1,
                            "advancedUnavailablePayloadEvents": 0,
                            "advancedInvalidTextPayloadEvents": 0
                        },
                        "outputParameterChanges": {
                            "rawPoints": 8,
                            "normalizedPoints": 5,
                            "filteredPoints": 3
                        }
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
    ]);

    assert_eq!(report.process_output.reported_cases, 2);
    assert_eq!(report.process_output.output_event_cases, 2);
    assert_eq!(report.process_output.output_parameter_change_cases, 2);
    assert_eq!(report.process_output.filtered_output_event_cases, 1);
    assert_eq!(
        report.process_output.filtered_output_parameter_change_cases,
        1
    );
    assert_eq!(report.process_output.total_output_events, 5);
    assert_eq!(report.process_output.total_output_parameter_changes, 7);
    assert_eq!(report.process_output.total_raw_output_events, 6);
    assert_eq!(report.process_output.total_normalized_output_events, 5);
    assert_eq!(report.process_output.total_filtered_output_events, 1);
    assert_eq!(report.process_output.total_advanced_output_events, 1);
    assert_eq!(report.process_output.advanced_payload_cases, 2);
    assert_eq!(report.process_output.advanced_payload_problem_cases, 1);
    assert_eq!(report.process_output.total_advanced_payload_events, 2);
    assert_eq!(report.process_output.total_advanced_payload_bytes, 76);
    assert_eq!(report.process_output.total_advanced_raw_payload_events, 1);
    assert_eq!(report.process_output.total_advanced_text_payload_events, 1);
    assert_eq!(
        report
            .process_output
            .total_advanced_truncated_payload_events,
        1
    );
    assert_eq!(
        report
            .process_output
            .total_advanced_note_expression_output_events,
        1
    );
    assert_eq!(report.process_output.total_raw_output_parameter_changes, 10);
    assert_eq!(
        report
            .process_output
            .total_normalized_output_parameter_changes,
        7
    );
    assert_eq!(
        report
            .process_output
            .total_filtered_output_parameter_changes,
        3
    );
    assert_eq!(report.process_output.max_output_events, 4);
    assert_eq!(
        report.process_output.max_output_events_case.as_deref(),
        Some("arp-synth")
    );
    assert_eq!(report.process_output.max_output_parameter_changes, 5);
    assert_eq!(
        report
            .process_output
            .max_output_parameter_changes_case
            .as_deref(),
        Some("mod-effect")
    );

    let value = serde_json::to_value(&report).expect("report json");
    assert_eq!(value["processOutput"]["reportedCases"], 2);
    assert_eq!(value["processOutput"]["totalOutputEvents"], 5);
    assert_eq!(value["processOutput"]["totalAdvancedOutputEvents"], 1);
    assert_eq!(value["processOutput"]["advancedPayloadProblemCases"], 1);
    assert_eq!(value["processOutput"]["totalAdvancedPayloadBytes"], 76);
    assert_eq!(
        value["processOutput"]["maxOutputParameterChangesCase"],
        "mod-effect"
    );
}
