use super::*;
use serde_json::json;

#[test]
fn summarizes_runtime_characteristics() {
    let report = RuntimeProbeMatrixReport::new(vec![
        RuntimeProbeResult {
            case_name: "linear-effect".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "latencySamples": 64,
                "tailSamples": 0,
                "tailInfo": {
                    "samples": 0,
                    "kind": "none"
                },
                "processContextRequirements": 2
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "large-reverb".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "latencySamples": 512,
                "tailSamples": 96000,
                "tailInfo": {
                    "samples": 96000,
                    "kind": "finite"
                },
                "processContextRequirements": 6
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "infinite-pad".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "latencySamples": 0,
                "tailSamples": u32::MAX,
                "tailInfo": {
                    "samples": u32::MAX,
                    "kind": "infinite"
                },
                "processContextRequirements": 0
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "tail-samples-only".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "latencySamples": 0,
                "tailSamples": 2048,
                "processContextRequirements": 0
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "timing-less-failure".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": false
            })),
            ..RuntimeProbeResult::default()
        },
    ]);

    assert_eq!(report.runtime_characteristics.reported_cases, 4);
    assert_eq!(report.runtime_characteristics.latency_cases, 2);
    assert_eq!(report.runtime_characteristics.tail_cases, 3);
    assert_eq!(report.runtime_characteristics.no_tail_cases, 1);
    assert_eq!(report.runtime_characteristics.finite_tail_cases, 1);
    assert_eq!(report.runtime_characteristics.infinite_tail_cases, 1);
    assert_eq!(
        report
            .runtime_characteristics
            .process_context_requirement_cases,
        2
    );
    assert_eq!(report.runtime_characteristics.max_latency_samples, 512);
    assert_eq!(
        report
            .runtime_characteristics
            .max_latency_samples_case
            .as_deref(),
        Some("large-reverb")
    );
    assert_eq!(
        report.runtime_characteristics.max_tail_samples,
        u64::from(u32::MAX)
    );
    assert_eq!(
        report
            .runtime_characteristics
            .max_tail_samples_case
            .as_deref(),
        Some("infinite-pad")
    );
    assert_eq!(
        report.runtime_characteristics.max_finite_tail_samples,
        96000
    );
    assert_eq!(
        report
            .runtime_characteristics
            .max_finite_tail_samples_case
            .as_deref(),
        Some("large-reverb")
    );
    assert_eq!(
        report
            .runtime_characteristics
            .process_context_requirements
            .get(&2)
            .copied(),
        Some(1)
    );
    assert_eq!(
        report
            .runtime_characteristics
            .process_context_requirements
            .get(&6)
            .copied(),
        Some(1)
    );

    let value = serde_json::to_value(&report).expect("report json");
    assert_eq!(value["runtimeCharacteristics"]["reportedCases"], 4);
    assert_eq!(
        value["runtimeCharacteristics"]["maxLatencySamplesCase"],
        "large-reverb"
    );
    assert_eq!(
        value["runtimeCharacteristics"]["maxTailSamples"],
        u64::from(u32::MAX)
    );
    assert_eq!(value["runtimeCharacteristics"]["infiniteTailCases"], 1);
    assert_eq!(
        value["runtimeCharacteristics"]["maxFiniteTailSamples"],
        96000
    );
}
