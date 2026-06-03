use super::*;
use serde_json::json;

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
fn summarizes_probe_note_timing_health() {
    let report = RuntimeProbeMatrixReport::new(vec![
        RuntimeProbeResult {
            case_name: "fast-synth".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "noteTiming": {
                        "sampleRateHz": 48000,
                        "notePresent": true,
                        "noteOnAbsoluteFrame": 0,
                        "firstNonZeroOutputAbsoluteFrame": 128,
                        "framesFromNoteOnToFirstNonZeroOutput": 128,
                        "microsFromNoteOnToFirstNonZeroOutput": 2666
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "slow-synth".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "noteTiming": {
                        "sampleRateHz": 48000,
                        "notePresent": true,
                        "noteOnAbsoluteFrame": 0,
                        "firstNonZeroOutputAbsoluteFrame": 1024,
                        "framesFromNoteOnToFirstNonZeroOutput": 1024,
                        "microsFromNoteOnToFirstNonZeroOutput": 21333
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "silent-synth".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "noteTiming": {
                        "sampleRateHz": 48000,
                        "notePresent": true,
                        "noteOnAbsoluteFrame": 0,
                        "firstNonZeroOutputAbsoluteFrame": null,
                        "framesFromNoteOnToFirstNonZeroOutput": null,
                        "microsFromNoteOnToFirstNonZeroOutput": null
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "effect-no-note".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "noteTiming": {
                        "sampleRateHz": 48000,
                        "notePresent": false
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
    ]);

    assert_eq!(report.note_timing.reported_cases, 4);
    assert_eq!(report.note_timing.note_input_cases, 3);
    assert_eq!(report.note_timing.note_response_cases, 2);
    assert_eq!(report.note_timing.missing_note_response_cases, 1);
    assert_eq!(report.note_timing.max_note_to_audio_frames, Some(1024));
    assert_eq!(
        report.note_timing.max_note_to_audio_frames_case.as_deref(),
        Some("slow-synth")
    );
    assert_eq!(report.note_timing.max_note_to_audio_micros, Some(21333));

    let value = serde_json::to_value(&report).expect("report json");
    assert_eq!(value["noteTiming"]["reportedCases"], 4);
    assert_eq!(value["noteTiming"]["noteResponseCases"], 2);
    assert_eq!(
        value["noteTiming"]["maxNoteToAudioFramesCase"],
        "slow-synth"
    );
}

#[test]
fn summarizes_probe_controller_health() {
    let report = RuntimeProbeMatrixReport::new(vec![
        RuntimeProbeResult {
            case_name: "simple-effect".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "controller": {
                    "parameters": {
                        "count": 2,
                        "automatable": 1
                    },
                    "componentState": {
                        "available": true,
                        "bytes": 4,
                        "roundtrip": {
                            "attempted": true,
                            "success": false,
                            "bytesBefore": 4,
                            "bytesAfter": 0
                        }
                    },
                    "controllerState": {
                        "available": false
                    },
                    "controllerComponentStateSync": {
                        "attempted": true,
                        "success": false,
                        "componentStateBytes": 4,
                        "error": "sync failed"
                    },
                    "units": {
                        "available": false
                    },
                    "programListData": {
                        "available": false
                    },
                    "unitData": {
                        "available": false
                    },
                    "componentHandler": {
                        "available": true,
                        "totalEvents": 1,
                        "recentEventCount": 1,
                        "editProbe": {
                            "attempted": false,
                            "success": false,
                            "eventDelta": 0
                        }
                    },
                    "connectionPoints": {
                        "connected": false,
                        "notifyProbe": {
                            "attempted": true,
                            "success": false,
                            "component": {
                                "attempted": true,
                                "success": false,
                                "notified": false
                            },
                            "controller": {
                                "attempted": true,
                                "success": false,
                                "notified": false
                            }
                        }
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "workstation".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "controller": {
                    "parameters": {
                        "count": 8,
                        "automatable": 6
                    },
                    "componentState": {
                        "available": true,
                        "bytes": 64,
                        "roundtrip": {
                            "attempted": true,
                            "success": true,
                            "bytesBefore": 64,
                            "bytesAfter": 64
                        }
                    },
                    "controllerState": {
                        "available": true,
                        "bytes": 48,
                        "roundtrip": {
                            "attempted": true,
                            "success": true,
                            "bytesBefore": 48,
                            "bytesAfter": 48
                        }
                    },
                    "controllerComponentStateSync": {
                        "attempted": true,
                        "success": true,
                        "componentStateBytes": 64
                    },
                    "units": {
                        "available": true,
                        "unitCount": 3,
                        "programListCount": 2,
                        "totalPrograms": 16
                    },
                    "programListData": {
                        "available": true,
                        "checked": 2,
                        "supported": 1,
                        "unsupported": 1
                    },
                    "unitData": {
                        "available": true,
                        "checked": 3,
                        "supported": 2,
                        "unsupported": 1
                    },
                    "componentHandler": {
                        "available": true,
                        "totalEvents": 3,
                        "recentEventCount": 3,
                        "editProbe": {
                            "attempted": true,
                            "success": true,
                            "eventDelta": 3
                        }
                    },
                    "connectionPoints": {
                        "connected": true,
                        "notifyProbe": {
                            "attempted": true,
                            "success": true,
                            "component": {
                                "attempted": true,
                                "success": true,
                                "notified": true
                            },
                            "controller": {
                                "attempted": true,
                                "success": true,
                                "notified": true
                            }
                        }
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
    ]);

    assert_eq!(report.controller_health.reported_cases, 2);
    assert_eq!(report.controller_health.parameter_cases, 2);
    assert_eq!(report.controller_health.total_parameters, 10);
    assert_eq!(report.controller_health.total_automatable_parameters, 7);
    assert_eq!(report.controller_health.max_parameter_count, 8);
    assert_eq!(
        report.controller_health.max_parameter_count_case.as_deref(),
        Some("workstation")
    );
    assert_eq!(report.controller_health.max_automatable_parameters, 6);
    assert_eq!(
        report
            .controller_health
            .max_automatable_parameters_case
            .as_deref(),
        Some("workstation")
    );
    assert_eq!(report.controller_health.component_state_cases, 2);
    assert_eq!(report.controller_health.component_state_roundtrip_cases, 1);
    assert_eq!(report.controller_health.total_component_state_bytes, 68);
    assert_eq!(report.controller_health.max_component_state_bytes, 64);
    assert_eq!(
        report
            .controller_health
            .max_component_state_bytes_case
            .as_deref(),
        Some("workstation")
    );
    assert_eq!(report.controller_health.controller_state_cases, 1);
    assert_eq!(report.controller_health.controller_state_roundtrip_cases, 1);
    assert_eq!(
        report
            .controller_health
            .controller_component_state_sync_attempted_cases,
        2
    );
    assert_eq!(
        report
            .controller_health
            .controller_component_state_sync_cases,
        1
    );
    assert_eq!(
        report
            .controller_health
            .controller_component_state_sync_failed_cases,
        1
    );
    assert_eq!(
        report
            .controller_health
            .total_controller_component_state_sync_bytes,
        68
    );
    assert_eq!(
        report
            .controller_health
            .max_controller_component_state_sync_bytes,
        64
    );
    assert_eq!(
        report
            .controller_health
            .max_controller_component_state_sync_bytes_case
            .as_deref(),
        Some("workstation")
    );
    assert_eq!(report.controller_health.total_controller_state_bytes, 48);
    assert_eq!(report.controller_health.max_controller_state_bytes, 48);
    assert_eq!(
        report
            .controller_health
            .max_controller_state_bytes_case
            .as_deref(),
        Some("workstation")
    );
    assert_eq!(report.controller_health.unit_info_cases, 1);
    assert_eq!(report.controller_health.total_units, 3);
    assert_eq!(report.controller_health.total_program_lists, 2);
    assert_eq!(report.controller_health.total_programs, 16);
    assert_eq!(report.controller_health.max_total_programs, 16);
    assert_eq!(
        report.controller_health.max_total_programs_case.as_deref(),
        Some("workstation")
    );
    assert_eq!(report.controller_health.program_list_data_cases, 1);
    assert_eq!(report.controller_health.total_program_list_data_checked, 2);
    assert_eq!(
        report.controller_health.total_program_list_data_supported,
        1
    );
    assert_eq!(
        report.controller_health.total_program_list_data_unsupported,
        1
    );
    assert_eq!(report.controller_health.unit_data_cases, 1);
    assert_eq!(report.controller_health.total_unit_data_checked, 3);
    assert_eq!(report.controller_health.total_unit_data_supported, 2);
    assert_eq!(report.controller_health.total_unit_data_unsupported, 1);
    assert_eq!(report.controller_health.component_handler_cases, 2);
    assert_eq!(
        report.controller_health.component_handler_edit_probe_cases,
        1
    );
    assert_eq!(report.controller_health.connection_point_cases, 1);
    assert_eq!(report.controller_health.connection_notify_probe_cases, 1);
    assert_eq!(
        report
            .controller_health
            .component_connection_notify_probe_cases,
        1
    );
    assert_eq!(
        report
            .controller_health
            .controller_connection_notify_probe_cases,
        1
    );
    assert_eq!(report.controller_health.total_component_handler_events, 4);
    assert_eq!(
        report
            .controller_health
            .total_component_handler_edit_probe_event_delta,
        3
    );

    let value = serde_json::to_value(&report).expect("report json");
    assert_eq!(value["controllerHealth"]["reportedCases"], 2);
    assert_eq!(value["controllerHealth"]["totalParameters"], 10);
    assert_eq!(value["controllerHealth"]["componentHandlerCases"], 2);
    assert_eq!(
        value["controllerHealth"]["componentHandlerEditProbeCases"],
        1
    );
    assert_eq!(value["controllerHealth"]["connectionNotifyProbeCases"], 1);
    assert_eq!(
        value["controllerHealth"]["componentConnectionNotifyProbeCases"],
        1
    );
    assert_eq!(
        value["controllerHealth"]["controllerConnectionNotifyProbeCases"],
        1
    );
    assert_eq!(value["controllerHealth"]["totalComponentHandlerEvents"], 4);
    assert_eq!(
        value["controllerHealth"]["totalComponentHandlerEditProbeEventDelta"],
        3
    );
    assert_eq!(value["controllerHealth"]["totalComponentStateBytes"], 68);
    assert_eq!(value["controllerHealth"]["componentStateRoundtripCases"], 1);
    assert_eq!(value["controllerHealth"]["maxComponentStateBytes"], 64);
    assert_eq!(value["controllerHealth"]["totalControllerStateBytes"], 48);
    assert_eq!(
        value["controllerHealth"]["controllerStateRoundtripCases"],
        1
    );
    assert_eq!(
        value["controllerHealth"]["controllerComponentStateSyncAttemptedCases"],
        2
    );
    assert_eq!(
        value["controllerHealth"]["controllerComponentStateSyncCases"],
        1
    );
    assert_eq!(
        value["controllerHealth"]["controllerComponentStateSyncFailedCases"],
        1
    );
    assert_eq!(
        value["controllerHealth"]["totalControllerComponentStateSyncBytes"],
        68
    );
    assert_eq!(
        value["controllerHealth"]["maxControllerComponentStateSyncBytes"],
        64
    );
    assert_eq!(value["controllerHealth"]["totalPrograms"], 16);
    assert_eq!(
        value["controllerHealth"]["totalProgramListDataSupported"],
        1
    );
    assert_eq!(value["controllerHealth"]["totalUnitDataSupported"], 2);
    assert_eq!(
        value["controllerHealth"]["maxAutomatableParametersCase"],
        "workstation"
    );
}

#[test]
fn summarizes_probe_evidence() {
    let report = RuntimeProbeMatrixReport::new(vec![
        RuntimeProbeResult {
            case_name: "free-effect".to_string(),
            evidence: Some(
                RuntimeProbeCaseEvidence::third_party_plugin(
                    "Free DSP",
                    "Free Verb",
                    RuntimeProbePluginKind::Effect,
                )
                .with_plugin_version("2.0.0")
                .with_tag("effect")
                .with_tag("automation"),
            ),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "hybrid-workstation".to_string(),
            evidence: Some(
                RuntimeProbeCaseEvidence::third_party_plugin(
                    "Acme Audio",
                    "Workstation",
                    RuntimeProbePluginKind::Hybrid,
                )
                .with_tag("instrument"),
            ),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "undocumented".to_string(),
            evidence: Some(RuntimeProbeCaseEvidence {
                plugin_kind: RuntimeProbePluginKind::Unknown,
                tags: vec!["smoke".to_string()],
                ..RuntimeProbeCaseEvidence::default()
            }),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "missing-evidence".to_string(),
            ..RuntimeProbeResult::default()
        },
    ]);

    assert_eq!(report.evidence.reported_cases, 3);
    assert_eq!(report.evidence.missing_evidence_cases, 1);
    assert_eq!(report.evidence.third_party_cases, 2);
    assert_eq!(report.evidence.effect_cases, 1);
    assert_eq!(report.evidence.hybrid_cases, 1);
    assert_eq!(report.evidence.unknown_kind_cases, 1);
    assert_eq!(report.evidence.missing_vendor_cases, 1);
    assert_eq!(report.evidence.missing_plugin_name_cases, 1);
    assert_eq!(report.evidence.vendors.get("Free DSP").copied(), Some(1));
    assert_eq!(report.evidence.tags.get("automation").copied(), Some(1));

    let value = serde_json::to_value(&report).expect("report json");
    assert_eq!(value["evidence"]["reportedCases"], 3);
    assert_eq!(value["evidence"]["thirdPartyCases"], 2);
    assert_eq!(value["evidence"]["vendors"]["Acme Audio"], 1);
}
