use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::report::RuntimeProbeResult;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeRuntimeCharacteristicsSummary {
    pub reported_cases: usize,
    pub latency_cases: usize,
    pub tail_cases: usize,
    pub no_tail_cases: usize,
    pub finite_tail_cases: usize,
    pub infinite_tail_cases: usize,
    pub process_context_requirement_cases: usize,
    pub max_latency_samples: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_latency_samples_case: Option<String>,
    pub max_tail_samples: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tail_samples_case: Option<String>,
    pub max_finite_tail_samples: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_finite_tail_samples_case: Option<String>,
    pub process_context_requirements: BTreeMap<u64, usize>,
}

impl RuntimeProbeRuntimeCharacteristicsSummary {
    pub(crate) fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(report) = result.probe_report.as_ref() else {
                continue;
            };
            if !has_runtime_characteristics(report) {
                continue;
            }

            summary.reported_cases += 1;
            let latency_samples = u64_field(report, "latencySamples");
            let tail_samples = u64_field(report, "tailSamples");
            let process_context_requirements = u64_field(report, "processContextRequirements");

            if latency_samples > 0 {
                summary.latency_cases += 1;
            }
            if tail_samples > 0 {
                summary.tail_cases += 1;
            }
            if let Some(kind) = tail_kind(report).as_deref() {
                match kind {
                    "none" => summary.no_tail_cases += 1,
                    "finite" => {
                        summary.finite_tail_cases += 1;
                        observe_max(
                            &mut summary.max_finite_tail_samples,
                            &mut summary.max_finite_tail_samples_case,
                            tail_samples,
                            result,
                        );
                    }
                    "infinite" => summary.infinite_tail_cases += 1,
                    _ => {}
                }
            }
            if process_context_requirements > 0 {
                summary.process_context_requirement_cases += 1;
            }
            observe_max(
                &mut summary.max_latency_samples,
                &mut summary.max_latency_samples_case,
                latency_samples,
                result,
            );
            observe_max(
                &mut summary.max_tail_samples,
                &mut summary.max_tail_samples_case,
                tail_samples,
                result,
            );
            increment(
                &mut summary.process_context_requirements,
                process_context_requirements,
            );
        }

        summary
    }
}

fn has_runtime_characteristics(report: &Value) -> bool {
    report.get("latencySamples").is_some()
        || report.get("tailSamples").is_some()
        || report.get("processContextRequirements").is_some()
}

fn u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn tail_kind(report: &Value) -> Option<String> {
    report
        .get("tailInfo")
        .and_then(|tail| tail.get("kind"))
        .and_then(Value::as_str)
        .map(str::to_string)
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

fn increment(map: &mut BTreeMap<u64, usize>, key: u64) {
    *map.entry(key).or_default() += 1;
}
