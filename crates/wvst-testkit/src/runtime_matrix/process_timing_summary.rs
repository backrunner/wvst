use serde::Serialize;

use super::report::{RuntimeProbeResult, u64_field};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeProcessTimingSummary {
    pub reported_cases: usize,
    pub max_p50_micros: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_p50_micros_case: Option<String>,
    pub max_p95_micros: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_p95_micros_case: Option<String>,
    pub max_p99_micros: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_p99_micros_case: Option<String>,
    pub max_process_micros: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_process_micros_case: Option<String>,
}

impl RuntimeProbeProcessTimingSummary {
    pub(crate) fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(timing) = result
                .probe_report
                .as_ref()
                .and_then(|report| report.get("process"))
                .and_then(|process| process.get("processTimeMicros"))
            else {
                continue;
            };

            summary.reported_cases += 1;
            observe_max(
                &mut summary.max_p50_micros,
                &mut summary.max_p50_micros_case,
                u64_field(timing, "p50"),
                result,
            );
            observe_max(
                &mut summary.max_p95_micros,
                &mut summary.max_p95_micros_case,
                u64_field(timing, "p95"),
                result,
            );
            observe_max(
                &mut summary.max_p99_micros,
                &mut summary.max_p99_micros_case,
                u64_field(timing, "p99"),
                result,
            );
            observe_max(
                &mut summary.max_process_micros,
                &mut summary.max_process_micros_case,
                u64_field(timing, "max"),
                result,
            );
        }

        summary
    }
}

fn observe_max(
    current: &mut u64,
    current_case: &mut Option<String>,
    value: u64,
    result: &RuntimeProbeResult,
) {
    if value > *current {
        *current = value;
        *current_case = Some(result.case_name.clone());
    }
}
