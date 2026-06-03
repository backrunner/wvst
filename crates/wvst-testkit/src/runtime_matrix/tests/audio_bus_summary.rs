use super::*;
use serde_json::json;

#[test]
fn summarizes_selected_audio_buses() {
    let report = RuntimeProbeMatrixReport::new(vec![
        RuntimeProbeResult {
            case_name: "stereo-effect".to_string(),
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
                        "available": [
                            { "index": 0 },
                            { "index": 1 }
                        ]
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
                        "available": [
                            { "index": 0 }
                        ]
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "zero-input-instrument".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "selectedAudioBuses": {
                    "input": null,
                    "output": {
                        "direction": "output",
                        "requestedChannels": 2,
                        "selectedIndex": 1,
                        "selected": {
                            "index": 1,
                            "direction": "output",
                            "channelCount": 1,
                            "busType": "aux"
                        },
                        "available": [
                            { "index": 0 },
                            { "index": 1 },
                            { "index": 2 }
                        ]
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "missing-output".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "selectedAudioBuses": {
                    "input": null
                }
            })),
            ..RuntimeProbeResult::default()
        },
    ]);

    assert_eq!(report.audio_buses.reported_cases, 3);
    assert_eq!(report.audio_buses.input_bus_cases, 1);
    assert_eq!(report.audio_buses.no_input_bus_cases, 2);
    assert_eq!(report.audio_buses.output_bus_cases, 2);
    assert_eq!(report.audio_buses.missing_output_bus_cases, 1);
    assert_eq!(report.audio_buses.output_channel_mismatch_cases, 1);
    assert_eq!(
        report.audio_buses.requested_input_channels.get(&2).copied(),
        Some(1)
    );
    assert_eq!(
        report.audio_buses.selected_output_channels.get(&1).copied(),
        Some(1)
    );
    assert_eq!(
        report
            .audio_buses
            .selected_output_bus_indexes
            .get(&1)
            .copied(),
        Some(1)
    );
    assert_eq!(
        report
            .audio_buses
            .selected_output_bus_types
            .get("aux")
            .copied(),
        Some(1)
    );
    assert_eq!(report.audio_buses.max_available_input_buses, 2);
    assert_eq!(
        report.audio_buses.max_available_input_buses_case.as_deref(),
        Some("stereo-effect")
    );
    assert_eq!(report.audio_buses.max_available_output_buses, 3);
    assert_eq!(
        report
            .audio_buses
            .max_available_output_buses_case
            .as_deref(),
        Some("zero-input-instrument")
    );

    let value = serde_json::to_value(&report).expect("report json");
    assert_eq!(value["audioBuses"]["reportedCases"], 3);
    assert_eq!(value["audioBuses"]["noInputBusCases"], 2);
    assert_eq!(value["audioBuses"]["missingOutputBusCases"], 1);
    assert_eq!(
        value["audioBuses"]["maxAvailableOutputBusesCase"],
        "zero-input-instrument"
    );
}
