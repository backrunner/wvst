use super::*;
use serde_json::json;

#[test]
fn summarizes_probe_process_timing() {
    let report = RuntimeProbeMatrixReport::new(vec![
        RuntimeProbeResult {
            case_name: "fast-effect".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "processTimeMicros": {
                        "min": 10,
                        "p50": 20,
                        "p95": 50,
                        "p99": 80,
                        "max": 100
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
        RuntimeProbeResult {
            case_name: "slow-instrument".to_string(),
            probe_report: Some(json!({
                "schemaVersion": 1,
                "ok": true,
                "process": {
                    "processTimeMicros": {
                        "min": 30,
                        "p50": 60,
                        "p95": 300,
                        "p99": 450,
                        "max": 700
                    }
                }
            })),
            ..RuntimeProbeResult::default()
        },
    ]);

    assert_eq!(report.process_timing.reported_cases, 2);
    assert_eq!(report.process_timing.max_p50_micros, 60);
    assert_eq!(
        report.process_timing.max_p50_micros_case.as_deref(),
        Some("slow-instrument")
    );
    assert_eq!(report.process_timing.max_p95_micros, 300);
    assert_eq!(
        report.process_timing.max_p95_micros_case.as_deref(),
        Some("slow-instrument")
    );
    assert_eq!(report.process_timing.max_p99_micros, 450);
    assert_eq!(report.process_timing.max_process_micros, 700);

    let value = serde_json::to_value(&report).expect("report json");
    assert_eq!(value["processTiming"]["reportedCases"], 2);
    assert_eq!(value["processTiming"]["maxP99Micros"], 450);
    assert_eq!(
        value["processTiming"]["maxProcessMicrosCase"],
        "slow-instrument"
    );
}
