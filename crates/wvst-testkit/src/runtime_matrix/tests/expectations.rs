use super::*;
use serde_json::json;

#[test]
fn marks_audio_expectation_failures() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "totalBlocks": 2,
                    "silentOutputBlocks": 2,
                    "nonZeroOutputBlocks": 0,
                    "nonFiniteOutputSamples": 1,
                    "clippedOutputSamples": 2,
                    "maxOutputPeak": 1.25,
                    "outputRms": 0.0
                }
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("strict-synth", "/tmp/Synth.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .require_non_zero_output()
                .max_non_finite_output_samples(0)
                .max_clipped_output_samples(0)
                .max_output_peak_milli(1_000),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.expectation_failed, 1);
    assert_eq!(
        report.results[0].status,
        RuntimeProbeStatus::ExpectationFailed
    );
    assert_eq!(
        report.results[0].probe_status,
        Some(RuntimeProbeStatus::Passed)
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("non-zero output"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("clipped output samples"))
    );
}

#[test]
fn marks_note_timing_expectation_failures() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "totalBlocks": 4,
                    "silentOutputBlocks": 1,
                    "nonZeroOutputBlocks": 3,
                    "nonFiniteOutputSamples": 0,
                    "clippedOutputSamples": 0,
                    "maxOutputPeak": 0.75,
                    "outputRms": 0.25,
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
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("slow-synth", "/tmp/Synth.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .require_note_response()
                .max_note_to_audio_frames(512)
                .max_note_to_audio_micros(10_000),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.expectation_failed, 1);
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("512 frames"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("10000 us"))
    );
}

#[test]
fn marks_controller_expectation_failures() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "controller": {
                    "parameters": {
                        "count": 1,
                        "automatable": 0
                    },
                    "componentState": {
                        "available": false
                    },
                    "controllerState": {
                        "available": true
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
                        "available": false,
                        "totalEvents": 0,
                        "recentEventCount": 0,
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
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("controller-rich", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .min_parameter_count(2)
                .min_automatable_parameters(1)
                .require_component_state()
                .min_component_state_bytes(8)
                .require_component_state_roundtrip()
                .require_controller_state()
                .min_controller_state_bytes(8)
                .require_controller_state_roundtrip()
                .require_controller_component_state_sync()
                .min_controller_component_state_sync_bytes(8)
                .require_unit_info()
                .min_unit_count(2)
                .min_program_list_count(1)
                .min_total_programs(1)
                .require_program_list_data()
                .min_program_list_data_supported(1)
                .require_unit_data()
                .min_unit_data_supported(1)
                .require_component_handler()
                .require_component_handler_edit_probe()
                .require_connection_points()
                .require_connection_notify_probe()
                .min_component_handler_events(1),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.expectation_failed, 1);
    assert_eq!(
        report.results[0].status,
        RuntimeProbeStatus::ExpectationFailed
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("at least 2 parameters"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("automatable parameters"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("component state"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("component state to contain"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("component state sync"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("sync to carry at least 8 bytes"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("component state roundtrip"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("controller state to contain"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("controller state roundtrip"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("unit info"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("at least 2 units"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("program lists"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("total programs"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("program list data"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("program list data entries"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("unit data"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("unit data entries"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("component handler"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("edit probe"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("connection points"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("connection notify probe"))
    );
}

#[test]
fn passes_controller_expectations_when_probe_reports_support() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "controller": {
                    "parameters": {
                        "count": 4,
                        "automatable": 3
                    },
                    "componentState": {
                        "available": true,
                        "bytes": 16,
                        "roundtrip": {
                            "attempted": true,
                            "success": true,
                            "bytesBefore": 16,
                            "bytesAfter": 16
                        }
                    },
                    "controllerState": {
                        "available": true,
                        "bytes": 12,
                        "roundtrip": {
                            "attempted": true,
                            "success": true,
                            "bytesBefore": 12,
                            "bytesAfter": 12
                        }
                    },
                    "controllerComponentStateSync": {
                        "attempted": true,
                        "success": true,
                        "componentStateBytes": 16
                    },
                    "units": {
                        "available": true,
                        "unitCount": 2,
                        "programListCount": 1,
                        "totalPrograms": 8
                    },
                    "programListData": {
                        "available": true,
                        "checked": 1,
                        "supported": 1,
                        "unsupported": 0
                    },
                    "unitData": {
                        "available": true,
                        "checked": 2,
                        "supported": 2,
                        "unsupported": 0
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
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("controller-rich", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .expected_status(RuntimeProbeStatus::Passed)
                .min_parameter_count(2)
                .min_automatable_parameters(1)
                .require_component_state()
                .min_component_state_bytes(8)
                .require_component_state_roundtrip()
                .require_controller_state()
                .min_controller_state_bytes(8)
                .require_controller_state_roundtrip()
                .require_controller_component_state_sync()
                .min_controller_component_state_sync_bytes(8)
                .require_unit_info()
                .min_unit_count(2)
                .min_program_list_count(1)
                .min_total_programs(8)
                .require_program_list_data()
                .min_program_list_data_supported(1)
                .require_unit_data()
                .min_unit_data_supported(2)
                .require_component_handler()
                .require_component_handler_edit_probe()
                .require_connection_points()
                .require_connection_notify_probe()
                .min_component_handler_events(3),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert_eq!(report.passed, 1);
    assert_eq!(report.expectation_failed, 0);
    assert_eq!(report.results[0].expectation_failures, Vec::<String>::new());
    assert_eq!(
        report.results[0].probe_status,
        Some(RuntimeProbeStatus::Passed)
    );
}

#[test]
fn passes_audio_bus_expectations_when_probe_reports_selected_buses() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "selectedAudioBuses": {
                    "input": {
                        "direction": "input",
                        "requestedChannels": 2,
                        "selectedIndex": 0,
                        "selected": {
                            "index": 0,
                            "direction": "input",
                            "channelCount": 2,
                            "busType": "main"
                        },
                        "available": []
                    },
                    "output": {
                        "direction": "output",
                        "requestedChannels": 2,
                        "selectedIndex": 0,
                        "selected": {
                            "index": 0,
                            "direction": "output",
                            "channelCount": 2,
                            "busType": "main"
                        },
                        "available": []
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("stereo-effect", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .expected_input_bus_index(0)
                .expected_input_bus_channels(2)
                .expected_output_bus_index(0)
                .expected_output_bus_channels(2),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert_eq!(report.passed, 1);
    assert_eq!(report.expectation_failed, 0);
    assert_eq!(report.results[0].expectation_failures, Vec::<String>::new());
}

#[test]
fn marks_audio_bus_expectation_failures() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "selectedAudioBuses": {
                    "input": {
                        "direction": "input",
                        "requestedChannels": 2,
                        "selectedIndex": 1,
                        "selected": {
                            "index": 1,
                            "direction": "input",
                            "channelCount": 1,
                            "busType": "main"
                        },
                        "available": []
                    },
                    "output": {
                        "direction": "output",
                        "requestedChannels": 2,
                        "selectedIndex": 1,
                        "selected": {
                            "index": 1,
                            "direction": "output",
                            "channelCount": 1,
                            "busType": "main"
                        },
                        "available": []
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("bad-bus", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .require_no_input_bus()
                .expected_output_bus_index(0)
                .expected_output_bus_channels(2),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(!report.all_passed());
    assert_eq!(report.expectation_failed, 1);
    assert_eq!(
        report.results[0].status,
        RuntimeProbeStatus::ExpectationFailed
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("no selected input audio bus"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("output audio bus index 0"))
    );
    assert!(
        report.results[0]
            .expectation_failures
            .iter()
            .any(|failure| failure.contains("output audio bus channels 2"))
    );
}

#[test]
fn expected_failed_probe_can_pass_with_matching_diagnostics() {
    let mut executor = RecordingExecutor {
        results: vec![RuntimeProbeResult {
            status: RuntimeProbeStatus::Failed,
            exit_code: Some(1),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": false,
                "data": {
                    "kind": "vst3-runtime-init",
                    "compatibility": {
                        "schemaVersion": 1,
                        "category": "processing-configuration"
                    },
                    "workerData": {
                        "classification": {
                            "schemaVersion": 1,
                            "category": "worker-rejection"
                        }
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        }],
        ..RecordingExecutor::default()
    };
    let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
        RuntimeProbeCase::new("bad-config", "/tmp/Effect.vst3", "class-a").with_expectations(
            RuntimeProbeExpectations::default()
                .expected_status(RuntimeProbeStatus::Failed)
                .expected_compatibility_category("processing-configuration")
                .expected_classification_category("worker-rejection"),
        ),
    );

    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert_eq!(report.passed, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(report.expectation_failed, 0);
    assert_eq!(report.results[0].status, RuntimeProbeStatus::Passed);
    assert_eq!(
        report.results[0].probe_status,
        Some(RuntimeProbeStatus::Failed)
    );
    assert_eq!(
        report
            .diagnostics
            .compatibility_categories
            .get("processing-configuration"),
        Some(&1)
    );
    assert_eq!(
        report
            .diagnostics
            .classification_categories
            .get("worker-rejection"),
        Some(&1)
    );
    assert_eq!(
        report.diagnostics.failure_kinds.get("vst3-runtime-init"),
        Some(&1)
    );
}
