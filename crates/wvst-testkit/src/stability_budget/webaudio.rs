use serde::{Deserialize, Serialize};

use super::{StabilityBudget, StabilityBudgetViolation};

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAudioLoopbackMetrics {
    pub input_frames: u64,
    pub output_frames: u64,
    pub underflows: u64,
    pub overflows: u64,
    pub input_sequence: u64,
    pub input_consumed_sequence: u64,
    pub output_sequence: u64,
    pub output_consumed_sequence: u64,
    pub pending_input_quanta: u64,
    pub pending_output_quanta: u64,
}

impl StabilityBudget {
    fn has_webaudio_expectations(self) -> bool {
        self.min_webaudio_input_frames.is_some()
            || self.min_webaudio_output_frames.is_some()
            || self.max_webaudio_underflows.is_some()
            || self.max_webaudio_overflows.is_some()
            || self.max_pending_input_quanta.is_some()
            || self.max_pending_output_quanta.is_some()
    }
}

pub(super) fn push_webaudio_violations(
    violations: &mut Vec<StabilityBudgetViolation>,
    budget: StabilityBudget,
    webaudio: Option<WebAudioLoopbackMetrics>,
) {
    if !budget.has_webaudio_expectations() {
        return;
    }
    let Some(webaudio) = webaudio else {
        violations.push(StabilityBudgetViolation::MetricMissing {
            metric: "webAudioLoopbackMetrics",
        });
        return;
    };

    push_min(
        violations,
        "webAudio.inputFrames",
        webaudio.input_frames,
        budget.min_webaudio_input_frames,
    );
    push_min(
        violations,
        "webAudio.outputFrames",
        webaudio.output_frames,
        budget.min_webaudio_output_frames,
    );
    push_max(
        violations,
        "webAudio.underflows",
        webaudio.underflows,
        budget.max_webaudio_underflows,
    );
    push_max(
        violations,
        "webAudio.overflows",
        webaudio.overflows,
        budget.max_webaudio_overflows,
    );
    push_max(
        violations,
        "webAudio.pendingInputQuanta",
        webaudio.pending_input_quanta,
        budget.max_pending_input_quanta,
    );
    push_max(
        violations,
        "webAudio.pendingOutputQuanta",
        webaudio.pending_output_quanta,
        budget.max_pending_output_quanta,
    );
}

fn push_min(
    violations: &mut Vec<StabilityBudgetViolation>,
    metric: &'static str,
    actual: u64,
    min: Option<u64>,
) {
    if let Some(min) = min
        && actual < min
    {
        violations.push(StabilityBudgetViolation::MinimumNotMet {
            metric,
            min,
            actual,
        });
    }
}

fn push_max(
    violations: &mut Vec<StabilityBudgetViolation>,
    metric: &'static str,
    actual: u64,
    max: Option<u64>,
) {
    if let Some(max) = max
        && actual > max
    {
        violations.push(StabilityBudgetViolation::MaximumExceeded {
            metric,
            max,
            actual,
        });
    }
}
