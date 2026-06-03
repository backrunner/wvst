use serde::{Deserialize, Serialize};
use std::fmt;

use crate::latency::LatencyPercentiles;

use super::{PercentileKind, StabilityBudget, StabilityBudgetViolation, push_percentile};

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAudioLoopbackMetrics {
    pub input_frames: u64,
    pub output_frames: u64,
    pub underflows: u64,
    pub overflows: u64,
    #[serde(default)]
    pub dropped_input_quanta: u64,
    #[serde(default)]
    pub dropped_output_quanta: u64,
    #[serde(default)]
    pub dropped_midi_events: u64,
    #[serde(default)]
    pub dropped_parameter_events: u64,
    #[serde(default)]
    pub late_midi_events: u64,
    #[serde(default)]
    pub late_parameter_events: u64,
    #[serde(default)]
    pub transport_failures: u64,
    #[serde(default, rename = "endToEndRoundTripUs")]
    pub end_to_end_round_trip_us: LatencyPercentiles,
    pub input_sequence: u64,
    pub input_consumed_sequence: u64,
    pub output_sequence: u64,
    pub output_consumed_sequence: u64,
    pub pending_input_quanta: u64,
    pub pending_output_quanta: u64,
}

impl WebAudioLoopbackMetrics {
    pub fn from_json_str(text: &str) -> Result<Self, WebAudioLoopbackMetricsParseError> {
        serde_json::from_str::<WebAudioLoopbackMetricsInput>(text)
            .map_err(|error| WebAudioLoopbackMetricsParseError::InvalidJson(error.to_string()))?
            .into_metrics()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WebAudioLoopbackMetricsParseError {
    InvalidJson(String),
    BrowserSmokeFailed { error: Option<String> },
    MissingBrowserSmokeMetrics,
}

impl fmt::Display for WebAudioLoopbackMetricsParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(formatter, "{error}"),
            Self::BrowserSmokeFailed { error } => {
                write!(formatter, "browser smoke report did not pass")?;
                if let Some(error) = error {
                    write!(formatter, ": {error}")?;
                }
                Ok(())
            }
            Self::MissingBrowserSmokeMetrics => {
                write!(formatter, "browser smoke report did not include metrics")
            }
        }
    }
}

impl std::error::Error for WebAudioLoopbackMetricsParseError {}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WebAudioLoopbackMetricsInput {
    Metrics(WebAudioLoopbackMetrics),
    BrowserSmoke(BrowserSmokeReport),
}

impl WebAudioLoopbackMetricsInput {
    fn into_metrics(self) -> Result<WebAudioLoopbackMetrics, WebAudioLoopbackMetricsParseError> {
        match self {
            Self::Metrics(metrics) => Ok(metrics),
            Self::BrowserSmoke(report) => report.into_metrics(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserSmokeReport {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    metrics: Option<WebAudioLoopbackMetrics>,
}

impl BrowserSmokeReport {
    fn into_metrics(self) -> Result<WebAudioLoopbackMetrics, WebAudioLoopbackMetricsParseError> {
        if !self.ok {
            return Err(WebAudioLoopbackMetricsParseError::BrowserSmokeFailed {
                error: self.error,
            });
        }
        self.metrics
            .ok_or(WebAudioLoopbackMetricsParseError::MissingBrowserSmokeMetrics)
    }
}

impl StabilityBudget {
    pub const fn max_webaudio_dropped_input_quanta(mut self, quanta: u64) -> Self {
        self.max_webaudio_dropped_input_quanta = Some(quanta);
        self
    }

    pub const fn max_webaudio_dropped_output_quanta(mut self, quanta: u64) -> Self {
        self.max_webaudio_dropped_output_quanta = Some(quanta);
        self
    }

    pub const fn max_webaudio_dropped_midi_events(mut self, events: u64) -> Self {
        self.max_webaudio_dropped_midi_events = Some(events);
        self
    }

    pub const fn max_webaudio_dropped_parameter_events(mut self, events: u64) -> Self {
        self.max_webaudio_dropped_parameter_events = Some(events);
        self
    }

    pub const fn max_webaudio_late_midi_events(mut self, events: u64) -> Self {
        self.max_webaudio_late_midi_events = Some(events);
        self
    }

    pub const fn max_webaudio_late_parameter_events(mut self, events: u64) -> Self {
        self.max_webaudio_late_parameter_events = Some(events);
        self
    }

    pub const fn max_webaudio_transport_failures(mut self, failures: u64) -> Self {
        self.max_webaudio_transport_failures = Some(failures);
        self
    }

    pub const fn max_webaudio_end_to_end_round_trip_p95_us(mut self, micros: u64) -> Self {
        self.max_webaudio_end_to_end_round_trip_p95_us = Some(micros);
        self
    }

    pub const fn max_webaudio_end_to_end_round_trip_p99_us(mut self, micros: u64) -> Self {
        self.max_webaudio_end_to_end_round_trip_p99_us = Some(micros);
        self
    }

    fn has_webaudio_expectations(self) -> bool {
        self.min_webaudio_input_frames.is_some()
            || self.min_webaudio_output_frames.is_some()
            || self.max_webaudio_underflows.is_some()
            || self.max_webaudio_overflows.is_some()
            || self.max_webaudio_dropped_input_quanta.is_some()
            || self.max_webaudio_dropped_output_quanta.is_some()
            || self.max_webaudio_dropped_midi_events.is_some()
            || self.max_webaudio_dropped_parameter_events.is_some()
            || self.max_webaudio_late_midi_events.is_some()
            || self.max_webaudio_late_parameter_events.is_some()
            || self.max_webaudio_transport_failures.is_some()
            || self.max_webaudio_end_to_end_round_trip_p95_us.is_some()
            || self.max_webaudio_end_to_end_round_trip_p99_us.is_some()
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
        "webAudio.droppedInputQuanta",
        webaudio.dropped_input_quanta,
        budget.max_webaudio_dropped_input_quanta,
    );
    push_max(
        violations,
        "webAudio.droppedOutputQuanta",
        webaudio.dropped_output_quanta,
        budget.max_webaudio_dropped_output_quanta,
    );
    push_max(
        violations,
        "webAudio.droppedMidiEvents",
        webaudio.dropped_midi_events,
        budget.max_webaudio_dropped_midi_events,
    );
    push_max(
        violations,
        "webAudio.droppedParameterEvents",
        webaudio.dropped_parameter_events,
        budget.max_webaudio_dropped_parameter_events,
    );
    push_max(
        violations,
        "webAudio.lateMidiEvents",
        webaudio.late_midi_events,
        budget.max_webaudio_late_midi_events,
    );
    push_max(
        violations,
        "webAudio.lateParameterEvents",
        webaudio.late_parameter_events,
        budget.max_webaudio_late_parameter_events,
    );
    push_max(
        violations,
        "webAudio.transportFailures",
        webaudio.transport_failures,
        budget.max_webaudio_transport_failures,
    );
    push_percentile(
        violations,
        "webAudio.endToEndRoundTripUs.p95",
        webaudio.end_to_end_round_trip_us,
        PercentileKind::P95,
        budget.max_webaudio_end_to_end_round_trip_p95_us,
    );
    push_percentile(
        violations,
        "webAudio.endToEndRoundTripUs.p99",
        webaudio.end_to_end_round_trip_us,
        PercentileKind::P99,
        budget.max_webaudio_end_to_end_round_trip_p99_us,
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
