use super::*;
use crate::runtime_matrix::{RuntimeProbeExecutor, RuntimeProbeInvocation, RuntimeProbeResult};
use serde_json::json;
use std::path::PathBuf;

#[derive(Default)]
struct RecordingExecutor {
    invocations: Vec<RuntimeProbeInvocation>,
}

impl RuntimeProbeExecutor for RecordingExecutor {
    fn run(&mut self, invocation: RuntimeProbeInvocation) -> RuntimeProbeResult {
        self.invocations.push(invocation);
        RuntimeProbeResult {
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
                "processContextRequirements": 6,
                "process": {
                    "totalBlocks": 1,
                    "silentOutputBlocks": 0,
                    "nonZeroOutputBlocks": 1,
                    "nonFiniteOutputSamples": 0,
                    "clippedOutputSamples": 0,
                    "maxOutputPeak": 0.75,
                    "outputRms": 0.25,
                    "outputEvents": 1,
                    "outputParameterChanges": 0,
                    "processTimeMicros": {
                        "min": 20,
                        "p50": 30,
                        "p95": 80,
                        "p99": 100,
                        "max": 120
                    },
                    "diagnostics": {
                        "outputEvents": {
                            "rawEvents": 1,
                            "normalizedEvents": 1,
                            "filteredEvents": 0,
                            "advancedEvents": 1,
                            "advancedDataEvents": 0,
                            "advancedNoteExpressionEvents": 1,
                            "advancedChordEvents": 0,
                            "advancedScaleEvents": 0,
                            "advancedPayloadEvents": 1,
                            "advancedPayloadBytes": 4,
                            "advancedRawPayloadEvents": 0,
                            "advancedTextPayloadEvents": 1,
                            "advancedTruncatedPayloadEvents": 0,
                            "advancedUnavailablePayloadEvents": 0,
                            "advancedInvalidTextPayloadEvents": 0,
                            "invalidSampleOffsetEvents": 0,
                            "invalidPayloadEvents": 0,
                            "unknownTypeEvents": 0
                        },
                        "outputParameterChanges": {
                            "rawPoints": 0,
                            "normalizedPoints": 0,
                            "filteredPoints": 0
                        }
                    },
                    "noteTiming": {
                        "sampleRateHz": 48000,
                        "notePresent": true,
                        "noteOnAbsoluteFrame": 0,
                        "firstNonZeroOutputAbsoluteFrame": 128,
                        "framesFromNoteOnToFirstNonZeroOutput": 128,
                        "microsFromNoteOnToFirstNonZeroOutput": 2666
                    }
                },
                "controller": {
                    "parameters": {
                        "count": 3,
                        "automatable": 2
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
                        "unitCount": 1,
                        "programListCount": 1,
                        "totalPrograms": 4
                    },
                    "programListData": {
                        "available": true,
                        "checked": 1,
                        "supported": 1,
                        "unsupported": 0
                    },
                    "unitData": {
                        "available": true,
                        "checked": 1,
                        "supported": 1,
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
                },
                "selectedAudioBuses": {
                    "input": null,
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
        }
    }
}

#[test]
fn parses_manifest_and_builds_matrix() {
    let manifest = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"description":"local third-party VST3 smoke matrix","fixtureRoot":"${CARGO_MANIFEST_DIR}/fixtures","cases":[{"name":"instrument-note","pluginPath":"Synth.vst3","classId":"class-a","evidence":{"pluginName":"Example Synth","vendor":"Example Audio","pluginVersion":"1.2.3","pluginKind":"instrument","thirdParty":true,"tags":["instrument","note-response"],"validationNotes":"local notarized fixture"},"inputChannels":0,"outputChannels":2,"frames":128,"blocks":8,"timeoutMillis":25000,"controllerEditProbeParameterId":99,"note":{"pitch":60,"velocityMilli":750,"channel":1},"parameterChanges":[{"parameterId":42,"valueMilli":500,"sampleOffset":64}],"expectations":{"requireNonZeroOutput":true,"requireNoteResponse":true,"maxNoteToAudioFrames":512,"maxNonFiniteOutputSamples":0,"maxClippedOutputSamples":0,"minOutputRmsMilli":1,"maxOutputPeakMilli":1000,"minParameterCount":2,"minAutomatableParameters":1,"requireComponentState":true,"minComponentStateBytes":8,"requireComponentStateRoundtrip":true,"requireControllerState":true,"minControllerStateBytes":8,"requireControllerStateRoundtrip":true,"requireControllerComponentStateSync":true,"minControllerComponentStateSyncBytes":8,"requireUnitInfo":true,"minUnitCount":1,"minProgramListCount":1,"minTotalPrograms":1,"requireProgramListData":true,"minProgramListDataSupported":1,"requireUnitData":true,"minUnitDataSupported":1,"requireComponentHandler":true,"requireComponentHandlerEditProbe":true,"requireConnectionPoints":true,"requireConnectionNotifyProbe":true,"minComponentHandlerEvents":3,"requireNoInputBus":true,"expectedOutputBusIndex":0,"expectedOutputBusChannels":2,"minAdvancedOutputEvents":1,"minAdvancedNoteExpressionOutputEvents":1,"minAdvancedPayloadEvents":1,"minAdvancedTextPayloadEvents":1,"minAdvancedPayloadBytes":1,"maxAdvancedTruncatedPayloadEvents":0,"maxAdvancedUnavailablePayloadEvents":0,"maxAdvancedInvalidTextPayloadEvents":0,"maxFilteredOutputEvents":0,"maxFilteredOutputParameterChanges":0,"maxProcessTimeP95Micros":100,"maxProcessTimeP99Micros":150,"maxProcessTimeMaxMicros":200,"maxLatencySamples":128,"maxTailSamples":256,"requireTailInfo":true,"expectedTailKind":"finite","expectedProcessContextRequirements":6,"requiredProcessContextRequirements":2}}]}"#,
    )
    .expect("manifest");

    let matrix = manifest.to_matrix("/tmp/wvst-host-worker").expect("matrix");
    let expected_fixture_root =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir")).join("fixtures");

    assert_eq!(manifest.cases[0].sample_rate_hz, DEFAULT_SAMPLE_RATE_HZ);
    assert_eq!(matrix.cases().len(), 1);
    assert_eq!(matrix.cases()[0].name, "instrument-note");
    assert_eq!(
        matrix.cases()[0].plugin_path,
        expected_fixture_root.join("Synth.vst3")
    );
    assert_eq!(matrix.cases()[0].input_channels, 0);
    assert_eq!(matrix.cases()[0].timeout_millis, 25_000);
    assert!(matrix.cases()[0].controller_edit_probe);
    assert!(matrix.cases()[0].connection_notify_probe);
    assert!(matrix.cases()[0].state_roundtrip_probe);
    assert_eq!(
        matrix.cases()[0].controller_edit_probe_parameter_id,
        Some(99)
    );
    assert_eq!(
        matrix.cases()[0]
            .evidence
            .as_ref()
            .expect("evidence")
            .vendor
            .as_deref(),
        Some("Example Audio")
    );
    assert_eq!(
        matrix.cases()[0]
            .evidence
            .as_ref()
            .expect("evidence")
            .plugin_kind,
        crate::runtime_matrix::RuntimeProbePluginKind::Instrument
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_non_zero_output
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_note_response
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_parameter_count,
        Some(2)
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_component_state
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_component_state_bytes,
        Some(8)
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_component_state_roundtrip
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_controller_state_bytes,
        Some(8)
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_controller_state_roundtrip
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_controller_component_state_sync
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_controller_component_state_sync_bytes,
        Some(8)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_unit_count,
        Some(1)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_total_programs,
        Some(1)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_program_list_data_supported,
        Some(1)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_unit_data_supported,
        Some(1)
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_component_handler_edit_probe
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_connection_notify_probe
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_component_handler_events,
        Some(3)
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_no_input_bus
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .expected_output_bus_channels,
        Some(2)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .max_filtered_output_events,
        Some(0)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_advanced_note_expression_output_events,
        Some(1)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .min_advanced_payload_events,
        Some(1)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .max_advanced_unavailable_payload_events,
        Some(0)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .max_process_time_p95_micros,
        Some(100)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .max_latency_samples,
        Some(128)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .expected_process_context_requirements,
        Some(6)
    );
    assert_eq!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .expected_tail_kind
            .as_deref(),
        Some("finite")
    );
    assert!(
        matrix.cases()[0]
            .expectations
            .as_ref()
            .expect("expectations")
            .require_tail_info
    );
    let mut executor = RecordingExecutor::default();
    let report = matrix.run_with(&mut executor);
    assert_eq!(
        report.schema_version,
        crate::runtime_matrix::RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION
    );
    assert!(report.all_passed());
    assert_eq!(report.evidence.reported_cases, 1);
    assert_eq!(report.evidence.third_party_cases, 1);
    assert_eq!(report.evidence.instrument_cases, 1);
    assert_eq!(
        report.evidence.vendors.get("Example Audio").copied(),
        Some(1)
    );
    assert!(
        executor.invocations[0]
            .args
            .contains(&"60:0.750:1".to_string())
    );
    assert!(
        !executor.invocations[0]
            .args
            .contains(&"--skip-controller-edit-probe".to_string())
    );
    assert!(
        !executor.invocations[0]
            .args
            .contains(&"--skip-connection-notify-probe".to_string())
    );
    assert!(
        !executor.invocations[0]
            .args
            .contains(&"--skip-state-roundtrip-probe".to_string())
    );
    assert!(
        executor.invocations[0]
            .args
            .contains(&"--controller-edit-probe-parameter".to_string())
    );
    assert!(executor.invocations[0].args.contains(&"99".to_string()));
    assert!(
        executor.invocations[0]
            .args
            .contains(&"42=0.500:64".to_string())
    );
    assert_eq!(executor.invocations[0].timeout_millis, 25_000);
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireNoteResponse\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireComponentState\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"minComponentStateBytes\": 8")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireComponentStateRoundtrip\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireControllerStateRoundtrip\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireControllerComponentStateSync\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"minControllerComponentStateSyncBytes\": 8")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"minProgramListDataSupported\": 1")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"minUnitDataSupported\": 1")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireComponentHandlerEditProbe\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireConnectionNotifyProbe\": true")
    );
    assert!(
        !manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"controllerEditProbe\"")
    );
    assert!(
        !manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"connectionNotifyProbe\"")
    );
    assert!(
        !manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"stateRoundtripProbe\"")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"controllerEditProbeParameterId\": 99")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireNoInputBus\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"maxFilteredOutputEvents\": 0")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"minAdvancedNoteExpressionOutputEvents\": 1")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"minAdvancedPayloadEvents\": 1")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"maxAdvancedUnavailablePayloadEvents\": 0")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"maxProcessTimeP99Micros\": 150")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"expectedProcessContextRequirements\": 6")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireTailInfo\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"expectedTailKind\": \"finite\"")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"pluginKind\": \"instrument\"")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"timeoutMillis\": 25000")
    );
}

#[test]
fn parses_manifest_with_controller_edit_probe_disabled() {
    let manifest = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"sensitive-fx","pluginPath":"/tmp/Sensitive.vst3","classId":"class-a","controllerEditProbe":false,"controllerEditProbeParameterId":99,"connectionNotifyProbe":false,"stateRoundtripProbe":false}]}"#,
    )
    .expect("manifest");

    let matrix = manifest.to_matrix("/tmp/wvst-host-worker").expect("matrix");

    assert!(!matrix.cases()[0].controller_edit_probe);
    assert!(!matrix.cases()[0].connection_notify_probe);
    assert!(!matrix.cases()[0].state_roundtrip_probe);
    assert_eq!(
        matrix.cases()[0].controller_edit_probe_parameter_id,
        Some(99)
    );

    let mut executor = RecordingExecutor::default();
    let report = matrix.run_with(&mut executor);

    assert!(report.all_passed());
    assert!(
        executor.invocations[0]
            .args
            .contains(&"--skip-controller-edit-probe".to_string())
    );
    assert!(
        executor.invocations[0]
            .args
            .contains(&"--skip-connection-notify-probe".to_string())
    );
    assert!(
        executor.invocations[0]
            .args
            .contains(&"--skip-state-roundtrip-probe".to_string())
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"controllerEditProbe\": false")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"connectionNotifyProbe\": false")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"stateRoundtripProbe\": false")
    );
}

#[test]
fn validates_manifest_evidence_requirements() {
    let manifest = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireAllCasesEvidence":true,"minReportedCases":1,"minThirdPartyCases":1,"minThirdPartyInstrumentCases":1,"minThirdPartyNonSilentCases":1,"minThirdPartyNoteResponseCases":1,"minThirdPartyBlocks":1,"minThirdPartyProcessFrames":128,"minThirdPartyTimeoutMillis":1000,"requireThirdPartyExpectations":true,"requiredTags":["instrument","note-response"],"requiredTagCounts":{"instrument":1,"note-response":1},"requiredThirdPartyTagCounts":{"note-response":1}},"cases":[{"name":"instrument-note","pluginPath":"/tmp/Synth.vst3","classId":"class-a","note":{"pitch":60},"expectations":{"requireNonZeroOutput":true,"requireNoteResponse":true},"evidence":{"pluginName":"Example Synth","vendor":"Example Audio","pluginKind":"instrument","thirdParty":true,"tags":["instrument","note-response"]}}]}"#,
    )
    .expect("manifest");

    let requirements = manifest
        .evidence_requirements
        .as_ref()
        .expect("evidence requirements");
    assert!(requirements.require_all_cases_evidence);
    assert_eq!(requirements.min_third_party_instrument_cases, Some(1));
    assert_eq!(requirements.min_third_party_non_silent_cases, Some(1));
    assert_eq!(requirements.min_third_party_note_response_cases, Some(1));
    assert_eq!(requirements.min_third_party_blocks, Some(1));
    assert_eq!(requirements.min_third_party_process_frames, Some(128));
    assert_eq!(requirements.min_third_party_timeout_millis, Some(1_000));
    assert!(requirements.require_third_party_expectations);
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"minThirdPartyInstrumentCases\": 1")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"minThirdPartyNonSilentCases\": 1")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireThirdPartyExpectations\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requiredThirdPartyTagCounts\"")
    );

    let missing_instrument = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"minThirdPartyInstrumentCases":1},"cases":[{"name":"effect","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true}}]}"#,
    )
    .expect_err("missing instrument evidence");
    assert!(matches!(
        missing_instrument,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("minThirdPartyInstrumentCases")
    ));

    let duplicate_tag = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requiredTags":["smoke","smoke"]},"cases":[{"name":"effect","pluginPath":"/tmp/Fx.vst3","classId":"class-a"}]}"#,
    )
    .expect_err("duplicate required tag");
    assert!(matches!(
        duplicate_tag,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("duplicate tag")
    ));

    let missing_third_party_tag = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requiredThirdPartyTagCounts":{"controller-rich":1}},"cases":[{"name":"local-controller","pluginPath":"/tmp/Fx.vst3","classId":"class-a","expectations":{"minParameterCount":1,"minAutomatableParameters":1,"requireComponentState":true},"evidence":{"pluginName":"Local FX","vendor":"WVST","pluginKind":"effect","thirdParty":false,"tags":["controller-rich"]}}]}"#,
    )
    .expect_err("missing third-party tag count");
    assert!(matches!(
        missing_third_party_tag,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("requiredThirdPartyTagCounts")
                && message.contains("controller-rich")
    ));

    let zero_tag_count = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requiredTagCounts":{"smoke":0}},"cases":[{"name":"effect","pluginPath":"/tmp/Fx.vst3","classId":"class-a"}]}"#,
    )
    .expect_err("zero tag count");
    assert!(matches!(
        zero_tag_count,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("requiredTagCounts") && message.contains("greater than 0")
    ));

    let missing_expectations = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireThirdPartyExpectations":true},"cases":[{"name":"third-party-no-expectations","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true}}]}"#,
    )
    .expect_err("missing third-party expectations");
    assert!(matches!(
        missing_expectations,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("requireThirdPartyExpectations")
                && message.contains("third-party-no-expectations")
    ));

    let too_few_blocks = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"minThirdPartyBlocks":2},"cases":[{"name":"short-third-party","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true}}]}"#,
    )
    .expect_err("too few third-party blocks");
    assert!(matches!(
        too_few_blocks,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("minThirdPartyBlocks") && message.contains("short-third-party")
    ));

    let too_few_process_frames = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"minThirdPartyProcessFrames":129},"cases":[{"name":"short-third-party","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true}}]}"#,
    )
    .expect_err("too few third-party process frames");
    assert!(matches!(
        too_few_process_frames,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("minThirdPartyProcessFrames")
                && message.contains("short-third-party")
    ));

    let too_short_timeout = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"minThirdPartyTimeoutMillis":30001},"cases":[{"name":"short-timeout","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true}}]}"#,
    )
    .expect_err("too short third-party timeout");
    assert!(matches!(
        too_short_timeout,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("minThirdPartyTimeoutMillis") && message.contains("short-timeout")
    ));

    let impossible_runtime_coverage = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"minThirdPartyControllerRichCases":2},"cases":[{"name":"effect","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true,"tags":["effect"]}}]}"#,
    )
    .expect_err("impossible runtime coverage");
    assert!(matches!(
        impossible_runtime_coverage,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("minThirdPartyControllerRichCases")
    ));

    let zero_quality_gate = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"minThirdPartyBlocks":0},"cases":[{"name":"effect","pluginPath":"/tmp/Fx.vst3","classId":"class-a"}]}"#,
    )
    .expect_err("zero quality gate");
    assert!(matches!(
        zero_quality_gate,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains("minThirdPartyBlocks") && message.contains("greater than 0")
    ));
}

#[test]
fn validates_third_party_expectation_quality_gates() {
    let manifest = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireThirdPartyAudioHealthExpectations":true,"requireThirdPartyProcessTimingExpectations":true,"requireThirdPartyRuntimeHealthExpectations":true,"requireThirdPartyNoteResponseExpectations":true,"requireThirdPartyControllerRichExpectations":true,"requireThirdPartyOutputEventExpectations":true},"cases":[{"name":"controller-rich-fx","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true,"tags":["controller-rich","output-events"]},"expectations":{"maxNonFiniteOutputSamples":0,"maxClippedOutputSamples":0,"maxProcessTimeP95Micros":2000,"maxProcessTimeP99Micros":4000,"maxProcessTimeMaxMicros":8000,"requireTailInfo":true,"maxLatencySamples":4096,"minParameterCount":1,"minAutomatableParameters":1,"requireComponentState":true,"requireControllerState":true,"requireComponentHandler":true,"requireConnectionPoints":true,"requireComponentStateRoundtrip":true,"requireControllerStateRoundtrip":true,"requireComponentHandlerEditProbe":true,"requireConnectionNotifyProbe":true,"minAdvancedOutputEvents":1,"maxFilteredOutputEvents":0,"maxFilteredOutputParameterChanges":0}},{"name":"note-instrument","pluginPath":"/tmp/Synth.vst3","classId":"class-b","inputChannels":0,"evidence":{"pluginName":"Example Synth","vendor":"Example Audio","pluginKind":"instrument","thirdParty":true,"tags":["note-response"]},"note":{"pitch":60},"expectations":{"requireNonZeroOutput":true,"maxNonFiniteOutputSamples":0,"maxClippedOutputSamples":0,"maxProcessTimeP95Micros":3000,"maxProcessTimeP99Micros":6000,"maxProcessTimeMaxMicros":12000,"requireTailInfo":true,"maxLatencySamples":4096,"requireNoteResponse":true,"maxNoteToAudioFrames":2048,"maxNoteToAudioMicros":50000}}]}"#,
    )
    .expect("quality gates manifest");

    let requirements = manifest
        .evidence_requirements
        .as_ref()
        .expect("evidence requirements");
    assert!(requirements.require_third_party_audio_health_expectations);
    assert!(requirements.require_third_party_process_timing_expectations);
    assert!(requirements.require_third_party_runtime_health_expectations);
    assert!(requirements.require_third_party_note_response_expectations);
    assert!(requirements.require_third_party_controller_rich_expectations);
    assert!(requirements.require_third_party_output_event_expectations);
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireThirdPartyAudioHealthExpectations\": true")
    );
    assert!(
        manifest
            .to_json_string_pretty()
            .expect("pretty json")
            .contains("\"requireThirdPartyOutputEventExpectations\": true")
    );

    assert_quality_gate_error(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireThirdPartyAudioHealthExpectations":true},"cases":[{"name":"weak-audio-health","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true},"expectations":{"maxNonFiniteOutputSamples":0}}]}"#,
        "requireThirdPartyAudioHealthExpectations",
    );
    assert_quality_gate_error(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireThirdPartyProcessTimingExpectations":true},"cases":[{"name":"weak-process-timing","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true},"expectations":{"maxProcessTimeP95Micros":2000,"maxProcessTimeMaxMicros":8000}}]}"#,
        "requireThirdPartyProcessTimingExpectations",
    );
    assert_quality_gate_error(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireThirdPartyRuntimeHealthExpectations":true},"cases":[{"name":"weak-runtime-health","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true},"expectations":{"maxLatencySamples":4096}}]}"#,
        "requireThirdPartyRuntimeHealthExpectations",
    );
    assert_quality_gate_error(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireThirdPartyNoteResponseExpectations":true},"cases":[{"name":"weak-note-response","pluginPath":"/tmp/Synth.vst3","classId":"class-a","inputChannels":0,"evidence":{"pluginName":"Example Synth","vendor":"Example Audio","pluginKind":"instrument","thirdParty":true,"tags":["note-response"]},"note":{"pitch":60},"expectations":{"requireNonZeroOutput":true,"requireNoteResponse":true,"maxNoteToAudioFrames":2048}}]}"#,
        "requireThirdPartyNoteResponseExpectations",
    );
    assert_quality_gate_error(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireThirdPartyControllerRichExpectations":true},"cases":[{"name":"weak-controller","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true,"tags":["controller-rich"]},"expectations":{"minParameterCount":1,"minAutomatableParameters":1,"requireComponentState":true}}]}"#,
        "requireThirdPartyControllerRichExpectations",
    );
    assert_quality_gate_error(
        r#"{"schemaVersion":1,"evidenceRequirements":{"requireThirdPartyOutputEventExpectations":true},"cases":[{"name":"weak-output-events","pluginPath":"/tmp/Fx.vst3","classId":"class-a","evidence":{"pluginName":"Example FX","vendor":"Example Audio","pluginKind":"effect","thirdParty":true,"tags":["output-events"]},"expectations":{"minAdvancedOutputEvents":1}}]}"#,
        "requireThirdPartyOutputEventExpectations",
    );
}

fn assert_quality_gate_error(text: &str, expected: &str) {
    let error = RuntimeProbeMatrixManifest::from_json_str(text).expect_err("quality gate error");
    assert!(matches!(
        error,
        RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements { message }
            if message.contains(expected)
    ));
}

#[test]
fn rejects_invalid_manifest() {
    let version_error = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":2,"cases":[{"name":"x","pluginPath":"/tmp/X.vst3","classId":"a"}]}"#,
    )
    .expect_err("unsupported schema");
    assert!(matches!(
        version_error,
        RuntimeProbeMatrixManifestError::UnsupportedSchemaVersion {
            expected: RUNTIME_PROBE_MATRIX_SCHEMA_VERSION,
            actual: 2,
        }
    ));

    let case_error = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-offset","pluginPath":"/tmp/X.vst3","classId":"a","frames":32,"parameterChanges":[{"parameterId":1,"valueMilli":1001,"sampleOffset":0}]}]}"#,
    )
    .expect_err("invalid case");
    assert!(matches!(
        case_error,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-offset" && message.contains("valueMilli")
    ));

    let expectation_error = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-expectations","pluginPath":"/tmp/X.vst3","classId":"a","expectations":{"requireNonZeroOutput":true,"requireSilentOutput":true}}]}"#,
    )
    .expect_err("invalid expectations");
    assert!(matches!(
        expectation_error,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-expectations" && message.contains("non-zero and silent")
    ));

    let input_bus_conflict = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-bus-expectations","pluginPath":"/tmp/X.vst3","classId":"a","expectations":{"requireNoInputBus":true,"expectedInputBusIndex":0}}]}"#,
    )
    .expect_err("invalid bus expectations");
    assert!(matches!(
        input_bus_conflict,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-bus-expectations" && message.contains("no input bus")
    ));

    let invalid_bus_channels = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-bus-channels","pluginPath":"/tmp/X.vst3","classId":"a","expectations":{"expectedOutputBusChannels":0}}]}"#,
    )
    .expect_err("invalid bus channels");
    assert!(matches!(
        invalid_bus_channels,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-bus-channels" && message.contains("greater than 0")
    ));

    let invalid_context_requirements = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-context","pluginPath":"/tmp/X.vst3","classId":"a","expectations":{"expectedProcessContextRequirements":2,"requiredProcessContextRequirements":4}}]}"#,
    )
    .expect_err("invalid process context requirements");
    assert!(matches!(
        invalid_context_requirements,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-context" && message.contains("expectedProcessContextRequirements")
    ));

    let invalid_tail_kind = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-tail-kind","pluginPath":"/tmp/X.vst3","classId":"a","expectations":{"expectedTailKind":"forever-ish"}}]}"#,
    )
    .expect_err("invalid tail kind");
    assert!(matches!(
        invalid_tail_kind,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-tail-kind" && message.contains("expectedTailKind")
    ));

    let invalid_coverage_tag = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-note-response-tag","pluginPath":"/tmp/X.vst3","classId":"a","evidence":{"pluginName":"X","vendor":"Example","pluginKind":"instrument","thirdParty":true,"tags":["note-response"]}}]}"#,
    )
    .expect_err("invalid coverage tag");
    assert!(matches!(
        invalid_coverage_tag,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-note-response-tag" && message.contains("note-response")
    ));

    let invalid_component_handler_events = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-handler-events","pluginPath":"/tmp/X.vst3","classId":"a","expectations":{"minComponentHandlerEvents":0}}]}"#,
    )
    .expect_err("invalid component handler events");
    assert!(matches!(
        invalid_component_handler_events,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-handler-events" && message.contains("minComponentHandlerEvents")
    ));

    let invalid_timeout = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-timeout","pluginPath":"/tmp/X.vst3","classId":"a","timeoutMillis":0}]}"#,
    )
    .expect_err("invalid timeout");
    assert!(matches!(
        invalid_timeout,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-timeout" && message.contains("timeoutMillis")
    ));

    let missing_env = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"missing-env","pluginPath":"${WVST_TESTKIT_UNSET_FIXTURE_ROOT}/X.vst3","classId":"a"}]}"#,
    )
    .expect("manifest parses")
    .to_matrix("/tmp/wvst-host-worker")
    .expect_err("missing env");
    assert!(matches!(
        missing_env,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "missing-env" && message.contains("unset environment variable")
    ));

    let evidence_error = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-evidence","pluginPath":"/tmp/X.vst3","classId":"a","evidence":{"thirdParty":true,"pluginName":"X","pluginKind":"unknown"}}]}"#,
    )
    .expect_err("invalid evidence");
    assert!(matches!(
        evidence_error,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-evidence" && message.contains("evidence.vendor")
    ));

    let tag_error = RuntimeProbeMatrixManifest::from_json_str(
        r#"{"schemaVersion":1,"cases":[{"name":"bad-tag","pluginPath":"/tmp/X.vst3","classId":"a","evidence":{"tags":["smoke","smoke"]}}]}"#,
    )
    .expect_err("duplicate tag");
    assert!(matches!(
        tag_error,
        RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
            if case_name == "bad-tag" && message.contains("duplicate tag")
    ));
}
