use serde::{Deserialize, Serialize};

use crate::latency::{LatencyPercentiles, LatencySnapshot};
use crate::stability::StabilityRunReport;

mod bridge;
mod webaudio;

pub use bridge::{BridgeLatencyPercentiles, BridgeStabilityMetrics};
pub use webaudio::WebAudioLoopbackMetrics;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StabilityBudget {
    pub min_observations: Option<usize>,
    pub max_dropped_frames: Option<u64>,
    pub max_timeout_frames: Option<u64>,
    pub max_late_frames: Option<u64>,
    pub max_silence_frames: Option<u64>,
    pub max_process_error_frames: Option<u64>,
    pub max_sequence_gap_events: Option<u64>,
    pub max_sequence_gap_frames: Option<u64>,
    pub max_duplicate_frames: Option<u64>,
    pub max_out_of_order_frames: Option<u64>,
    pub max_route_latency_p95_us: Option<u64>,
    pub max_route_latency_p99_us: Option<u64>,
    pub max_round_trip_p95_us: Option<u64>,
    pub max_round_trip_p99_us: Option<u64>,
    #[serde(rename = "minWebAudioInputFrames")]
    pub min_webaudio_input_frames: Option<u64>,
    #[serde(rename = "minWebAudioOutputFrames")]
    pub min_webaudio_output_frames: Option<u64>,
    #[serde(rename = "maxWebAudioUnderflows")]
    pub max_webaudio_underflows: Option<u64>,
    #[serde(rename = "maxWebAudioOverflows")]
    pub max_webaudio_overflows: Option<u64>,
    #[serde(rename = "maxWebAudioDroppedInputQuanta")]
    pub max_webaudio_dropped_input_quanta: Option<u64>,
    #[serde(rename = "maxWebAudioDroppedOutputQuanta")]
    pub max_webaudio_dropped_output_quanta: Option<u64>,
    #[serde(rename = "maxWebAudioDroppedMidiEvents")]
    pub max_webaudio_dropped_midi_events: Option<u64>,
    #[serde(rename = "maxWebAudioDroppedParameterEvents")]
    pub max_webaudio_dropped_parameter_events: Option<u64>,
    #[serde(rename = "maxWebAudioLateMidiEvents")]
    pub max_webaudio_late_midi_events: Option<u64>,
    #[serde(rename = "maxWebAudioLateParameterEvents")]
    pub max_webaudio_late_parameter_events: Option<u64>,
    #[serde(rename = "maxWebAudioTransportFailures")]
    pub max_webaudio_transport_failures: Option<u64>,
    #[serde(rename = "maxWebAudioEndToEndRoundTripP95Us")]
    pub max_webaudio_end_to_end_round_trip_p95_us: Option<u64>,
    #[serde(rename = "maxWebAudioEndToEndRoundTripP99Us")]
    pub max_webaudio_end_to_end_round_trip_p99_us: Option<u64>,
    pub max_pending_input_quanta: Option<u64>,
    pub max_pending_output_quanta: Option<u64>,
    pub max_shared_memory_pump_preflight_skips: Option<u64>,
    pub max_shared_memory_pump_overruns: Option<u64>,
    pub max_shared_memory_pump_input_underruns: Option<u64>,
    pub max_shared_memory_pump_output_backpressure: Option<u64>,
    pub max_shared_memory_pump_worker_errors: Option<u64>,
    pub min_shared_memory_pump_events_enqueued: Option<u64>,
    pub min_shared_memory_pump_events_drained: Option<u64>,
    pub max_shared_memory_pump_events_late: Option<u64>,
    pub max_shared_memory_pump_events_dropped: Option<u64>,
    pub max_shared_memory_pump_events_cleared: Option<u64>,
    pub max_shared_memory_process_latency_p95_us: Option<u64>,
    pub max_shared_memory_process_latency_p99_us: Option<u64>,
    pub max_shared_memory_pump_last_process_micros: Option<u64>,
    pub max_shared_memory_pump_max_process_micros: Option<u64>,
    pub max_shared_memory_pump_last_overrun_micros: Option<u64>,
}

impl StabilityBudget {
    pub const fn min_observations(mut self, observations: usize) -> Self {
        self.min_observations = Some(observations);
        self
    }

    pub const fn max_dropped_frames(mut self, frames: u64) -> Self {
        self.max_dropped_frames = Some(frames);
        self
    }

    pub const fn max_timeout_frames(mut self, frames: u64) -> Self {
        self.max_timeout_frames = Some(frames);
        self
    }

    pub const fn max_late_frames(mut self, frames: u64) -> Self {
        self.max_late_frames = Some(frames);
        self
    }

    pub const fn max_silence_frames(mut self, frames: u64) -> Self {
        self.max_silence_frames = Some(frames);
        self
    }

    pub const fn max_process_error_frames(mut self, frames: u64) -> Self {
        self.max_process_error_frames = Some(frames);
        self
    }

    pub const fn max_sequence_gap_events(mut self, events: u64) -> Self {
        self.max_sequence_gap_events = Some(events);
        self
    }

    pub const fn max_sequence_gap_frames(mut self, frames: u64) -> Self {
        self.max_sequence_gap_frames = Some(frames);
        self
    }

    pub const fn max_duplicate_frames(mut self, frames: u64) -> Self {
        self.max_duplicate_frames = Some(frames);
        self
    }

    pub const fn max_out_of_order_frames(mut self, frames: u64) -> Self {
        self.max_out_of_order_frames = Some(frames);
        self
    }

    pub const fn max_route_latency_p95_us(mut self, micros: u64) -> Self {
        self.max_route_latency_p95_us = Some(micros);
        self
    }

    pub const fn max_route_latency_p99_us(mut self, micros: u64) -> Self {
        self.max_route_latency_p99_us = Some(micros);
        self
    }

    pub const fn max_round_trip_p95_us(mut self, micros: u64) -> Self {
        self.max_round_trip_p95_us = Some(micros);
        self
    }

    pub const fn max_round_trip_p99_us(mut self, micros: u64) -> Self {
        self.max_round_trip_p99_us = Some(micros);
        self
    }

    pub const fn min_webaudio_input_frames(mut self, frames: u64) -> Self {
        self.min_webaudio_input_frames = Some(frames);
        self
    }

    pub const fn min_webaudio_output_frames(mut self, frames: u64) -> Self {
        self.min_webaudio_output_frames = Some(frames);
        self
    }

    pub const fn max_webaudio_underflows(mut self, underflows: u64) -> Self {
        self.max_webaudio_underflows = Some(underflows);
        self
    }

    pub const fn max_webaudio_overflows(mut self, overflows: u64) -> Self {
        self.max_webaudio_overflows = Some(overflows);
        self
    }

    pub const fn max_pending_input_quanta(mut self, quanta: u64) -> Self {
        self.max_pending_input_quanta = Some(quanta);
        self
    }

    pub const fn max_pending_output_quanta(mut self, quanta: u64) -> Self {
        self.max_pending_output_quanta = Some(quanta);
        self
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StabilityBudgetReport {
    pub passed: bool,
    pub violations: Vec<StabilityBudgetViolation>,
}

impl StabilityBudgetReport {
    pub fn from_snapshot(snapshot: &LatencySnapshot, budget: StabilityBudget) -> Self {
        Self::from_snapshot_and_webaudio(snapshot, budget, None)
    }

    pub fn from_snapshot_and_webaudio(
        snapshot: &LatencySnapshot,
        budget: StabilityBudget,
        webaudio: Option<WebAudioLoopbackMetrics>,
    ) -> Self {
        Self::from_snapshot_and_metrics(snapshot, budget, webaudio, None)
    }

    pub fn from_snapshot_and_metrics(
        snapshot: &LatencySnapshot,
        budget: StabilityBudget,
        webaudio: Option<WebAudioLoopbackMetrics>,
        bridge: Option<BridgeStabilityMetrics>,
    ) -> Self {
        let mut violations = Vec::new();

        if let Some(min) = budget.min_observations
            && snapshot.observations < min
        {
            violations.push(StabilityBudgetViolation::MinimumNotMet {
                metric: "observations",
                min: min as u64,
                actual: snapshot.observations as u64,
            });
        }

        push_max(
            &mut violations,
            "droppedFrames",
            snapshot.dropped_frames,
            budget.max_dropped_frames,
        );
        push_max(
            &mut violations,
            "timeoutFrames",
            snapshot.timeout_frames,
            budget.max_timeout_frames,
        );
        push_max(
            &mut violations,
            "lateFrames",
            snapshot.late_frames,
            budget.max_late_frames,
        );
        push_max(
            &mut violations,
            "silenceFrames",
            snapshot.silence_frames,
            budget.max_silence_frames,
        );
        push_max(
            &mut violations,
            "processErrorFrames",
            snapshot.process_error_frames,
            budget.max_process_error_frames,
        );
        push_max(
            &mut violations,
            "sequenceGapEvents",
            snapshot.sequence_gap_events,
            budget.max_sequence_gap_events,
        );
        push_max(
            &mut violations,
            "sequenceGapFrames",
            snapshot.sequence_gap_frames,
            budget.max_sequence_gap_frames,
        );
        push_max(
            &mut violations,
            "duplicateFrames",
            snapshot.duplicate_frames,
            budget.max_duplicate_frames,
        );
        push_max(
            &mut violations,
            "outOfOrderFrames",
            snapshot.out_of_order_frames,
            budget.max_out_of_order_frames,
        );
        push_percentile(
            &mut violations,
            "routeLatencyUs.p95",
            snapshot.route_latency_us,
            PercentileKind::P95,
            budget.max_route_latency_p95_us,
        );
        push_percentile(
            &mut violations,
            "routeLatencyUs.p99",
            snapshot.route_latency_us,
            PercentileKind::P99,
            budget.max_route_latency_p99_us,
        );
        push_percentile(
            &mut violations,
            "roundTripUs.p95",
            snapshot.round_trip_us,
            PercentileKind::P95,
            budget.max_round_trip_p95_us,
        );
        push_percentile(
            &mut violations,
            "roundTripUs.p99",
            snapshot.round_trip_us,
            PercentileKind::P99,
            budget.max_round_trip_p99_us,
        );
        webaudio::push_webaudio_violations(&mut violations, budget, webaudio);
        bridge::push_bridge_violations(&mut violations, budget, bridge);

        Self {
            passed: violations.is_empty(),
            violations,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum StabilityBudgetViolation {
    MaximumExceeded {
        metric: &'static str,
        max: u64,
        actual: u64,
    },
    MinimumNotMet {
        metric: &'static str,
        min: u64,
        actual: u64,
    },
    PercentileMissing {
        metric: &'static str,
    },
    MetricMissing {
        metric: &'static str,
    },
}

impl StabilityRunReport {
    pub fn evaluate_budget(&self, budget: StabilityBudget) -> StabilityBudgetReport {
        StabilityBudgetReport::from_snapshot(&self.snapshot, budget)
    }

    pub fn evaluate_budget_with_webaudio(
        &self,
        budget: StabilityBudget,
        webaudio: WebAudioLoopbackMetrics,
    ) -> StabilityBudgetReport {
        StabilityBudgetReport::from_snapshot_and_webaudio(&self.snapshot, budget, Some(webaudio))
    }

    pub fn evaluate_budget_with_metrics(
        &self,
        budget: StabilityBudget,
        webaudio: Option<WebAudioLoopbackMetrics>,
        bridge: Option<BridgeStabilityMetrics>,
    ) -> StabilityBudgetReport {
        StabilityBudgetReport::from_snapshot_and_metrics(&self.snapshot, budget, webaudio, bridge)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PercentileKind {
    P95,
    P99,
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

pub(super) fn push_percentile(
    violations: &mut Vec<StabilityBudgetViolation>,
    metric: &'static str,
    percentiles: LatencyPercentiles,
    kind: PercentileKind,
    max: Option<u64>,
) {
    let Some(max) = max else {
        return;
    };
    let actual = match kind {
        PercentileKind::P95 => percentiles.p95,
        PercentileKind::P99 => percentiles.p99,
    };
    match actual {
        Some(actual) if actual > max => {
            violations.push(StabilityBudgetViolation::MaximumExceeded {
                metric,
                max,
                actual,
            })
        }
        Some(_) => {}
        None => violations.push(StabilityBudgetViolation::PercentileMissing { metric }),
    }
}

#[cfg(test)]
mod tests;
