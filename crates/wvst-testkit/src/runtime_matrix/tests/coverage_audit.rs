use std::collections::BTreeMap;

use crate::runtime_matrix_manifest::RuntimeProbeEvidenceRequirements;

use super::*;

#[test]
fn coverage_audit_passes_observed_runtime_requirements() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(controller_rich_note_response_report()),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker")
        .with_coverage_requirements(strict_runtime_coverage_requirements())
        .with_case(controller_rich_instrument_case());

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert!(report.coverage_audit.checked);
    assert!(report.coverage_audit.passed);
    assert!(report.coverage_audit.violations.is_empty());
    assert_eq!(report.coverage_audit.observed.third_party_cases, 1);
    assert_eq!(
        report.coverage_audit.observed.third_party_non_silent_cases,
        1
    );
    assert_eq!(
        report
            .coverage_audit
            .observed
            .third_party_note_response_cases,
        1
    );
    assert_eq!(
        report
            .coverage_audit
            .observed
            .third_party_controller_rich_cases,
        1
    );
    assert_eq!(
        report
            .coverage_audit
            .observed
            .third_party_output_event_cases,
        1
    );
}

#[test]
fn coverage_audit_fails_missing_runtime_requirements() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(silent_minimal_report()),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker")
        .with_coverage_requirements(strict_runtime_coverage_requirements())
        .with_case(controller_rich_instrument_case());

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert!(report.coverage_audit.checked);
    assert!(!report.coverage_audit.passed);
    assert!(
        report
            .coverage_audit
            .violations
            .iter()
            .any(|violation| violation.contains("minThirdPartyNonSilentCases"))
    );
    assert!(
        report
            .coverage_audit
            .violations
            .iter()
            .any(|violation| violation.contains("minThirdPartyNoteResponseCases"))
    );
    assert!(
        report
            .coverage_audit
            .violations
            .iter()
            .any(|violation| violation.contains("minThirdPartyControllerRichCases"))
    );
    assert!(
        report
            .coverage_audit
            .violations
            .iter()
            .any(|violation| violation.contains("minThirdPartyOutputEventCases"))
    );
    assert!(
        report
            .coverage_audit
            .violations
            .iter()
            .any(|violation| violation.contains("minThirdPartyBlocks"))
    );
}

fn strict_runtime_coverage_requirements() -> RuntimeProbeEvidenceRequirements {
    RuntimeProbeEvidenceRequirements {
        require_all_cases_evidence: true,
        min_reported_cases: Some(1),
        min_third_party_cases: Some(1),
        min_third_party_instrument_cases: Some(1),
        min_third_party_non_silent_cases: Some(1),
        min_third_party_note_response_cases: Some(1),
        min_third_party_controller_rich_cases: Some(1),
        min_third_party_output_event_cases: Some(1),
        min_third_party_blocks: Some(4),
        min_third_party_process_frames: Some(512),
        required_third_party_tag_counts: BTreeMap::from([
            ("controller-rich".to_string(), 1),
            ("note-response".to_string(), 1),
        ]),
        ..RuntimeProbeEvidenceRequirements::default()
    }
}

fn controller_rich_instrument_case() -> RuntimeProbeCase {
    RuntimeProbeCase::new("controller-rich-synth", "/tmp/Synth.vst3", "class-a").with_evidence(
        RuntimeProbeCaseEvidence::third_party_plugin(
            "Example Audio",
            "Example Synth",
            RuntimeProbePluginKind::Instrument,
        )
        .with_tag("controller-rich")
        .with_tag("note-response"),
    )
}

fn controller_rich_note_response_report() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
        "ok": true,
        "process": {
            "totalBlocks": 4,
            "totalFrames": 512,
            "nonZeroOutputBlocks": 4,
            "outputEvents": 1,
            "outputParameterChanges": 0,
            "noteTiming": {
                "notePresent": true,
                "framesFromNoteOnToFirstNonZeroOutput": 128,
                "microsFromNoteOnToFirstNonZeroOutput": 2666
            },
            "diagnostics": {
                "outputEvents": {
                    "normalizedEvents": 1,
                    "advancedEvents": 1
                },
                "outputParameterChanges": {
                    "normalizedPoints": 0
                }
            }
        },
        "controller": {
            "parameters": {
                "count": 4,
                "automatable": 2
            },
            "componentState": {
                "roundtrip": {
                    "success": true
                }
            },
            "controllerState": {
                "roundtrip": {
                    "success": true
                }
            },
            "componentHandler": {
                "editProbe": {
                    "success": true
                }
            },
            "connectionPoints": {
                "notifyProbe": {
                    "success": true
                }
            }
        }
    })
}

fn silent_minimal_report() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
        "ok": true,
        "process": {
            "totalBlocks": 1,
            "totalFrames": 128,
            "nonZeroOutputBlocks": 0,
            "outputEvents": 0,
            "outputParameterChanges": 0,
            "noteTiming": {
                "notePresent": true,
                "framesFromNoteOnToFirstNonZeroOutput": null
            }
        },
        "controller": {
            "parameters": {
                "count": 0,
                "automatable": 0
            }
        }
    })
}
