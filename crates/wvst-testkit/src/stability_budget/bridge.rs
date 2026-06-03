use serde::{Deserialize, Serialize};

use super::{StabilityBudget, StabilityBudgetViolation};

mod input;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStabilityMetrics {
    #[serde(default)]
    pub audio_frames_routed: u64,
    #[serde(default)]
    pub audio_backpressure_drops: u64,
    #[serde(default)]
    pub audio_frame_route_failures: u64,
    #[serde(default)]
    pub audio_frame_invalid_headers: u64,
    #[serde(default)]
    pub audio_frame_invalid_lengths: u64,
    #[serde(default)]
    pub audio_frame_unmatched_streams: u64,
    #[serde(default)]
    pub audio_sequence_gap_events: u64,
    #[serde(default)]
    pub audio_sequence_gap_frames: u64,
    #[serde(default)]
    pub audio_frames_duplicate: u64,
    #[serde(default)]
    pub audio_frames_out_of_order: u64,
    #[serde(default)]
    pub audio_frames_late: u64,
    #[serde(default)]
    pub audio_route_latency: BridgeLatencyPercentiles,
    #[serde(default)]
    pub audio_interarrival_jitter: BridgeLatencyPercentiles,
    #[serde(default)]
    pub shared_memory_pump_preflight_skips: u64,
    #[serde(default)]
    pub shared_memory_pump_overruns: u64,
    #[serde(default)]
    pub shared_memory_pump_input_underruns: u64,
    #[serde(default)]
    pub shared_memory_pump_output_backpressure: u64,
    #[serde(default)]
    pub shared_memory_pump_worker_errors: u64,
    #[serde(default)]
    pub shared_memory_pump_events_enqueued: u64,
    #[serde(default)]
    pub shared_memory_pump_events_drained: u64,
    #[serde(default)]
    pub shared_memory_pump_events_late: u64,
    #[serde(default)]
    pub shared_memory_pump_events_dropped: u64,
    #[serde(default)]
    pub shared_memory_pump_events_cleared: u64,
    #[serde(default)]
    pub shared_memory_process_latency: BridgeLatencyPercentiles,
    #[serde(default)]
    pub shared_memory_pump_last_process_micros: Option<u64>,
    #[serde(default)]
    pub shared_memory_pump_max_process_micros: Option<u64>,
    #[serde(default)]
    pub shared_memory_pump_last_overrun_micros: Option<u64>,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeLatencyPercentiles {
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub p50_us: Option<u64>,
    #[serde(default)]
    pub p95_us: Option<u64>,
    #[serde(default)]
    pub p99_us: Option<u64>,
}

impl StabilityBudget {
    pub const fn min_bridge_audio_frames_routed(mut self, frames: u64) -> Self {
        self.min_bridge_audio_frames_routed = Some(frames);
        self
    }

    pub const fn max_bridge_audio_backpressure_drops(mut self, drops: u64) -> Self {
        self.max_bridge_audio_backpressure_drops = Some(drops);
        self
    }

    pub const fn max_bridge_audio_frame_route_failures(mut self, failures: u64) -> Self {
        self.max_bridge_audio_frame_route_failures = Some(failures);
        self
    }

    pub const fn max_bridge_audio_frame_invalid_headers(mut self, frames: u64) -> Self {
        self.max_bridge_audio_frame_invalid_headers = Some(frames);
        self
    }

    pub const fn max_bridge_audio_frame_invalid_lengths(mut self, frames: u64) -> Self {
        self.max_bridge_audio_frame_invalid_lengths = Some(frames);
        self
    }

    pub const fn max_bridge_audio_frame_unmatched_streams(mut self, frames: u64) -> Self {
        self.max_bridge_audio_frame_unmatched_streams = Some(frames);
        self
    }

    pub const fn max_bridge_audio_sequence_gap_events(mut self, events: u64) -> Self {
        self.max_bridge_audio_sequence_gap_events = Some(events);
        self
    }

    pub const fn max_bridge_audio_sequence_gap_frames(mut self, frames: u64) -> Self {
        self.max_bridge_audio_sequence_gap_frames = Some(frames);
        self
    }

    pub const fn max_bridge_audio_frames_duplicate(mut self, frames: u64) -> Self {
        self.max_bridge_audio_frames_duplicate = Some(frames);
        self
    }

    pub const fn max_bridge_audio_frames_out_of_order(mut self, frames: u64) -> Self {
        self.max_bridge_audio_frames_out_of_order = Some(frames);
        self
    }

    pub const fn max_bridge_audio_frames_late(mut self, frames: u64) -> Self {
        self.max_bridge_audio_frames_late = Some(frames);
        self
    }

    pub const fn max_bridge_audio_route_latency_p95_us(mut self, micros: u64) -> Self {
        self.max_bridge_audio_route_latency_p95_us = Some(micros);
        self
    }

    pub const fn max_bridge_audio_route_latency_p99_us(mut self, micros: u64) -> Self {
        self.max_bridge_audio_route_latency_p99_us = Some(micros);
        self
    }

    pub const fn max_bridge_audio_interarrival_jitter_p95_us(mut self, micros: u64) -> Self {
        self.max_bridge_audio_interarrival_jitter_p95_us = Some(micros);
        self
    }

    pub const fn max_bridge_audio_interarrival_jitter_p99_us(mut self, micros: u64) -> Self {
        self.max_bridge_audio_interarrival_jitter_p99_us = Some(micros);
        self
    }

    pub const fn max_shared_memory_pump_preflight_skips(mut self, skips: u64) -> Self {
        self.max_shared_memory_pump_preflight_skips = Some(skips);
        self
    }

    pub const fn max_shared_memory_pump_overruns(mut self, overruns: u64) -> Self {
        self.max_shared_memory_pump_overruns = Some(overruns);
        self
    }

    pub const fn max_shared_memory_pump_input_underruns(mut self, underruns: u64) -> Self {
        self.max_shared_memory_pump_input_underruns = Some(underruns);
        self
    }

    pub const fn max_shared_memory_pump_output_backpressure(mut self, backpressure: u64) -> Self {
        self.max_shared_memory_pump_output_backpressure = Some(backpressure);
        self
    }

    pub const fn max_shared_memory_pump_worker_errors(mut self, errors: u64) -> Self {
        self.max_shared_memory_pump_worker_errors = Some(errors);
        self
    }

    pub const fn min_shared_memory_pump_events_enqueued(mut self, events: u64) -> Self {
        self.min_shared_memory_pump_events_enqueued = Some(events);
        self
    }

    pub const fn min_shared_memory_pump_events_drained(mut self, events: u64) -> Self {
        self.min_shared_memory_pump_events_drained = Some(events);
        self
    }

    pub const fn max_shared_memory_pump_events_late(mut self, events: u64) -> Self {
        self.max_shared_memory_pump_events_late = Some(events);
        self
    }

    pub const fn max_shared_memory_pump_events_dropped(mut self, events: u64) -> Self {
        self.max_shared_memory_pump_events_dropped = Some(events);
        self
    }

    pub const fn max_shared_memory_pump_events_cleared(mut self, events: u64) -> Self {
        self.max_shared_memory_pump_events_cleared = Some(events);
        self
    }

    pub const fn max_shared_memory_process_latency_p95_us(mut self, micros: u64) -> Self {
        self.max_shared_memory_process_latency_p95_us = Some(micros);
        self
    }

    pub const fn max_shared_memory_process_latency_p99_us(mut self, micros: u64) -> Self {
        self.max_shared_memory_process_latency_p99_us = Some(micros);
        self
    }

    pub const fn max_shared_memory_pump_last_process_micros(mut self, micros: u64) -> Self {
        self.max_shared_memory_pump_last_process_micros = Some(micros);
        self
    }

    pub const fn max_shared_memory_pump_max_process_micros(mut self, micros: u64) -> Self {
        self.max_shared_memory_pump_max_process_micros = Some(micros);
        self
    }

    pub const fn max_shared_memory_pump_last_overrun_micros(mut self, micros: u64) -> Self {
        self.max_shared_memory_pump_last_overrun_micros = Some(micros);
        self
    }

    fn has_bridge_expectations(self) -> bool {
        self.min_bridge_audio_frames_routed.is_some()
            || self.max_bridge_audio_backpressure_drops.is_some()
            || self.max_bridge_audio_frame_route_failures.is_some()
            || self.max_bridge_audio_frame_invalid_headers.is_some()
            || self.max_bridge_audio_frame_invalid_lengths.is_some()
            || self.max_bridge_audio_frame_unmatched_streams.is_some()
            || self.max_bridge_audio_sequence_gap_events.is_some()
            || self.max_bridge_audio_sequence_gap_frames.is_some()
            || self.max_bridge_audio_frames_duplicate.is_some()
            || self.max_bridge_audio_frames_out_of_order.is_some()
            || self.max_bridge_audio_frames_late.is_some()
            || self.max_bridge_audio_route_latency_p95_us.is_some()
            || self.max_bridge_audio_route_latency_p99_us.is_some()
            || self.max_bridge_audio_interarrival_jitter_p95_us.is_some()
            || self.max_bridge_audio_interarrival_jitter_p99_us.is_some()
            || self.max_shared_memory_pump_preflight_skips.is_some()
            || self.max_shared_memory_pump_overruns.is_some()
            || self.max_shared_memory_pump_input_underruns.is_some()
            || self.max_shared_memory_pump_output_backpressure.is_some()
            || self.max_shared_memory_pump_worker_errors.is_some()
            || self.min_shared_memory_pump_events_enqueued.is_some()
            || self.min_shared_memory_pump_events_drained.is_some()
            || self.max_shared_memory_pump_events_late.is_some()
            || self.max_shared_memory_pump_events_dropped.is_some()
            || self.max_shared_memory_pump_events_cleared.is_some()
            || self.max_shared_memory_process_latency_p95_us.is_some()
            || self.max_shared_memory_process_latency_p99_us.is_some()
            || self.max_shared_memory_pump_last_process_micros.is_some()
            || self.max_shared_memory_pump_max_process_micros.is_some()
            || self.max_shared_memory_pump_last_overrun_micros.is_some()
    }
}

pub(super) fn push_bridge_violations(
    violations: &mut Vec<StabilityBudgetViolation>,
    budget: StabilityBudget,
    bridge: Option<BridgeStabilityMetrics>,
) {
    if !budget.has_bridge_expectations() {
        return;
    }
    let Some(bridge) = bridge else {
        violations.push(StabilityBudgetViolation::MetricMissing {
            metric: "bridgeMetrics",
        });
        return;
    };

    push_min(
        violations,
        "bridge.audioFramesRouted",
        bridge.audio_frames_routed,
        budget.min_bridge_audio_frames_routed,
    );
    push_max(
        violations,
        "bridge.audioBackpressureDrops",
        bridge.audio_backpressure_drops,
        budget.max_bridge_audio_backpressure_drops,
    );
    push_max(
        violations,
        "bridge.audioFrameRouteFailures",
        bridge.audio_frame_route_failures,
        budget.max_bridge_audio_frame_route_failures,
    );
    push_max(
        violations,
        "bridge.audioFrameInvalidHeaders",
        bridge.audio_frame_invalid_headers,
        budget.max_bridge_audio_frame_invalid_headers,
    );
    push_max(
        violations,
        "bridge.audioFrameInvalidLengths",
        bridge.audio_frame_invalid_lengths,
        budget.max_bridge_audio_frame_invalid_lengths,
    );
    push_max(
        violations,
        "bridge.audioFrameUnmatchedStreams",
        bridge.audio_frame_unmatched_streams,
        budget.max_bridge_audio_frame_unmatched_streams,
    );
    push_max(
        violations,
        "bridge.audioSequenceGapEvents",
        bridge.audio_sequence_gap_events,
        budget.max_bridge_audio_sequence_gap_events,
    );
    push_max(
        violations,
        "bridge.audioSequenceGapFrames",
        bridge.audio_sequence_gap_frames,
        budget.max_bridge_audio_sequence_gap_frames,
    );
    push_max(
        violations,
        "bridge.audioFramesDuplicate",
        bridge.audio_frames_duplicate,
        budget.max_bridge_audio_frames_duplicate,
    );
    push_max(
        violations,
        "bridge.audioFramesOutOfOrder",
        bridge.audio_frames_out_of_order,
        budget.max_bridge_audio_frames_out_of_order,
    );
    push_max(
        violations,
        "bridge.audioFramesLate",
        bridge.audio_frames_late,
        budget.max_bridge_audio_frames_late,
    );
    push_optional_percentile_max(
        violations,
        "bridge.audioRouteLatency.p95",
        bridge.audio_route_latency.p95_us,
        budget.max_bridge_audio_route_latency_p95_us,
    );
    push_optional_percentile_max(
        violations,
        "bridge.audioRouteLatency.p99",
        bridge.audio_route_latency.p99_us,
        budget.max_bridge_audio_route_latency_p99_us,
    );
    push_optional_percentile_max(
        violations,
        "bridge.audioInterarrivalJitter.p95",
        bridge.audio_interarrival_jitter.p95_us,
        budget.max_bridge_audio_interarrival_jitter_p95_us,
    );
    push_optional_percentile_max(
        violations,
        "bridge.audioInterarrivalJitter.p99",
        bridge.audio_interarrival_jitter.p99_us,
        budget.max_bridge_audio_interarrival_jitter_p99_us,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpPreflightSkips",
        bridge.shared_memory_pump_preflight_skips,
        budget.max_shared_memory_pump_preflight_skips,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpOverruns",
        bridge.shared_memory_pump_overruns,
        budget.max_shared_memory_pump_overruns,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpInputUnderruns",
        bridge.shared_memory_pump_input_underruns,
        budget.max_shared_memory_pump_input_underruns,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpOutputBackpressure",
        bridge.shared_memory_pump_output_backpressure,
        budget.max_shared_memory_pump_output_backpressure,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpWorkerErrors",
        bridge.shared_memory_pump_worker_errors,
        budget.max_shared_memory_pump_worker_errors,
    );
    push_min(
        violations,
        "bridge.sharedMemoryPumpEventsEnqueued",
        bridge.shared_memory_pump_events_enqueued,
        budget.min_shared_memory_pump_events_enqueued,
    );
    push_min(
        violations,
        "bridge.sharedMemoryPumpEventsDrained",
        bridge.shared_memory_pump_events_drained,
        budget.min_shared_memory_pump_events_drained,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpEventsLate",
        bridge.shared_memory_pump_events_late,
        budget.max_shared_memory_pump_events_late,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpEventsDropped",
        bridge.shared_memory_pump_events_dropped,
        budget.max_shared_memory_pump_events_dropped,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpEventsCleared",
        bridge.shared_memory_pump_events_cleared,
        budget.max_shared_memory_pump_events_cleared,
    );
    push_optional_percentile_max(
        violations,
        "bridge.sharedMemoryProcessLatency.p95",
        bridge.shared_memory_process_latency.p95_us,
        budget.max_shared_memory_process_latency_p95_us,
    );
    push_optional_percentile_max(
        violations,
        "bridge.sharedMemoryProcessLatency.p99",
        bridge.shared_memory_process_latency.p99_us,
        budget.max_shared_memory_process_latency_p99_us,
    );
    push_optional_max(
        violations,
        "bridge.sharedMemoryPumpLastProcessMicros",
        bridge.shared_memory_pump_last_process_micros,
        budget.max_shared_memory_pump_last_process_micros,
    );
    push_optional_max(
        violations,
        "bridge.sharedMemoryPumpMaxProcessMicros",
        bridge.shared_memory_pump_max_process_micros,
        budget.max_shared_memory_pump_max_process_micros,
    );
    push_max(
        violations,
        "bridge.sharedMemoryPumpLastOverrunMicros",
        bridge.shared_memory_pump_last_overrun_micros.unwrap_or(0),
        budget.max_shared_memory_pump_last_overrun_micros,
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

fn push_optional_max(
    violations: &mut Vec<StabilityBudgetViolation>,
    metric: &'static str,
    actual: Option<u64>,
    max: Option<u64>,
) {
    let Some(max) = max else {
        return;
    };
    match actual {
        Some(actual) => push_max(violations, metric, actual, Some(max)),
        None => violations.push(StabilityBudgetViolation::MetricMissing { metric }),
    }
}

fn push_optional_percentile_max(
    violations: &mut Vec<StabilityBudgetViolation>,
    metric: &'static str,
    actual: Option<u64>,
    max: Option<u64>,
) {
    let Some(max) = max else {
        return;
    };
    match actual {
        Some(actual) => push_max(violations, metric, actual, Some(max)),
        None => violations.push(StabilityBudgetViolation::PercentileMissing { metric }),
    }
}
