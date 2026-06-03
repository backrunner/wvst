use super::{
    expectations::RuntimeProbeExpectations,
    report::{RuntimeProbeResult, u64_field},
};

impl RuntimeProbeExpectations {
    pub const fn max_process_time_p50_micros(mut self, micros: u64) -> Self {
        self.max_process_time_p50_micros = Some(micros);
        self
    }

    pub const fn max_process_time_p95_micros(mut self, micros: u64) -> Self {
        self.max_process_time_p95_micros = Some(micros);
        self
    }

    pub const fn max_process_time_p99_micros(mut self, micros: u64) -> Self {
        self.max_process_time_p99_micros = Some(micros);
        self
    }

    pub const fn max_process_time_max_micros(mut self, micros: u64) -> Self {
        self.max_process_time_max_micros = Some(micros);
        self
    }
}

pub(super) fn evaluate_process_timing_expectations(
    expectations: &RuntimeProbeExpectations,
    result: &RuntimeProbeResult,
    failures: &mut Vec<String>,
) {
    if !has_process_timing_expectations(expectations) {
        return;
    }

    let Some(timing) = result
        .probe_report
        .as_ref()
        .and_then(|report| report.get("process"))
        .and_then(|process| process.get("processTimeMicros"))
    else {
        failures.push("missing runtime-probe process timing diagnostics".to_string());
        return;
    };

    push_max(
        failures,
        "processTimeMicros.p50",
        u64_field(timing, "p50"),
        expectations.max_process_time_p50_micros,
    );
    push_max(
        failures,
        "processTimeMicros.p95",
        u64_field(timing, "p95"),
        expectations.max_process_time_p95_micros,
    );
    push_max(
        failures,
        "processTimeMicros.p99",
        u64_field(timing, "p99"),
        expectations.max_process_time_p99_micros,
    );
    push_max(
        failures,
        "processTimeMicros.max",
        u64_field(timing, "max"),
        expectations.max_process_time_max_micros,
    );
}

fn has_process_timing_expectations(expectations: &RuntimeProbeExpectations) -> bool {
    expectations.max_process_time_p50_micros.is_some()
        || expectations.max_process_time_p95_micros.is_some()
        || expectations.max_process_time_p99_micros.is_some()
        || expectations.max_process_time_max_micros.is_some()
}

fn push_max(failures: &mut Vec<String>, label: &'static str, actual: u64, expected: Option<u64>) {
    if let Some(expected) = expected
        && actual > expected
    {
        failures.push(format!(
            "expected {label} <= {expected} us, got {actual} us"
        ));
    }
}
