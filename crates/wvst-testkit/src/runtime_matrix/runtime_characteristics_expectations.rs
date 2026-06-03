use super::{
    expectations::RuntimeProbeExpectations,
    report::{RuntimeProbeResult, u64_field},
};

impl RuntimeProbeExpectations {
    pub const fn max_latency_samples(mut self, samples: u64) -> Self {
        self.max_latency_samples = Some(samples);
        self
    }

    pub const fn max_tail_samples(mut self, samples: u64) -> Self {
        self.max_tail_samples = Some(samples);
        self
    }

    pub const fn require_tail_info(mut self) -> Self {
        self.require_tail_info = true;
        self
    }

    pub fn expected_tail_kind(mut self, kind: impl Into<String>) -> Self {
        self.expected_tail_kind = Some(kind.into());
        self
    }

    pub const fn expected_process_context_requirements(mut self, requirements: u64) -> Self {
        self.expected_process_context_requirements = Some(requirements);
        self
    }

    pub const fn required_process_context_requirements(mut self, requirements: u64) -> Self {
        self.required_process_context_requirements = Some(requirements);
        self
    }
}

pub(super) fn evaluate_runtime_characteristics_expectations(
    expectations: &RuntimeProbeExpectations,
    result: &RuntimeProbeResult,
    failures: &mut Vec<String>,
) {
    if !has_runtime_characteristics_expectations(expectations) {
        return;
    }

    let Some(report) = result.probe_report.as_ref() else {
        failures.push("missing runtime-probe report".to_string());
        return;
    };

    push_max(
        failures,
        "latencySamples",
        u64_field(report, "latencySamples"),
        expectations.max_latency_samples,
    );
    push_max(
        failures,
        "tailSamples",
        u64_field(report, "tailSamples"),
        expectations.max_tail_samples,
    );
    if expectations.require_tail_info && !has_tail_info(report) {
        failures.push("expected tailInfo with kind and samples".to_string());
    }
    if let Some(expected) = &expectations.expected_tail_kind {
        let actual = tail_kind(report);
        if actual.as_deref() != Some(expected.as_str()) {
            failures.push(format!(
                "expected tailInfo.kind == {expected:?}, got {:?}",
                actual
            ));
        }
    }

    let context_requirements = u64_field(report, "processContextRequirements");
    if let Some(expected) = expectations.expected_process_context_requirements
        && context_requirements != expected
    {
        failures.push(format!(
            "expected processContextRequirements == {expected}, got {context_requirements}"
        ));
    }
    if let Some(required) = expectations.required_process_context_requirements
        && (context_requirements & required) != required
    {
        failures.push(format!(
            "expected processContextRequirements to include bitmask {required}, got {context_requirements}"
        ));
    }
}

fn has_runtime_characteristics_expectations(expectations: &RuntimeProbeExpectations) -> bool {
    expectations.max_latency_samples.is_some()
        || expectations.max_tail_samples.is_some()
        || expectations.require_tail_info
        || expectations.expected_tail_kind.is_some()
        || expectations.expected_process_context_requirements.is_some()
        || expectations.required_process_context_requirements.is_some()
}

fn has_tail_info(report: &serde_json::Value) -> bool {
    report.get("tailInfo").is_some_and(|tail| {
        tail.get("kind")
            .and_then(serde_json::Value::as_str)
            .is_some()
            && tail
                .get("samples")
                .and_then(serde_json::Value::as_u64)
                .is_some()
    })
}

fn tail_kind(report: &serde_json::Value) -> Option<String> {
    report
        .get("tailInfo")
        .and_then(|tail| tail.get("kind"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn push_max(failures: &mut Vec<String>, label: &'static str, actual: u64, expected: Option<u64>) {
    if let Some(expected) = expected
        && actual > expected
    {
        failures.push(format!("expected {label} <= {expected}, got {actual}"));
    }
}
