use std::array;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;

use crate::audio_stream_tracker::AudioStreamObservation;

#[derive(Debug)]
pub struct BridgeMetrics {
    started_at: Instant,
    websocket_connections: AtomicU64,
    control_messages: AtomicU64,
    binary_frames: AtomicU64,
    audio_frames_routed: AtomicU64,
    audio_frame_fallbacks: AtomicU64,
    audio_frame_route_failures: AtomicU64,
    hello_requests: AtomicU64,
    worker_failures: AtomicU64,
    worker_restarts: AtomicU64,
    worker_auto_restarts: AtomicU64,
    worker_shutdowns: AtomicU64,
    worker_kill_requests: AtomicU64,
    worker_tree_kill_requests: AtomicU64,
    worker_forced_kill_requests: AtomicU64,
    worker_wait_successes: AtomicU64,
    worker_wait_timeouts: AtomicU64,
    audio_sequence_gap_events: AtomicU64,
    audio_sequence_gap_frames: AtomicU64,
    audio_frames_duplicate: AtomicU64,
    audio_frames_out_of_order: AtomicU64,
    audio_frames_late: AtomicU64,
    audio_backpressure_drops: AtomicU64,
    shared_memory_process_blocks: AtomicU64,
    shared_memory_process_frames: AtomicU64,
    shared_memory_process_failures: AtomicU64,
    audio_route_latency: LatencyHistogram,
    audio_interarrival_jitter: LatencyHistogram,
    shared_memory_process_latency: LatencyHistogram,
}

const LATENCY_BUCKETS_US: [u64; 13] = [
    100,
    250,
    500,
    1_000,
    2_000,
    5_000,
    10_000,
    20_000,
    50_000,
    100_000,
    250_000,
    500_000,
    u64::MAX,
];

impl BridgeMetrics {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            websocket_connections: AtomicU64::new(0),
            control_messages: AtomicU64::new(0),
            binary_frames: AtomicU64::new(0),
            audio_frames_routed: AtomicU64::new(0),
            audio_frame_fallbacks: AtomicU64::new(0),
            audio_frame_route_failures: AtomicU64::new(0),
            hello_requests: AtomicU64::new(0),
            worker_failures: AtomicU64::new(0),
            worker_restarts: AtomicU64::new(0),
            worker_auto_restarts: AtomicU64::new(0),
            worker_shutdowns: AtomicU64::new(0),
            worker_kill_requests: AtomicU64::new(0),
            worker_tree_kill_requests: AtomicU64::new(0),
            worker_forced_kill_requests: AtomicU64::new(0),
            worker_wait_successes: AtomicU64::new(0),
            worker_wait_timeouts: AtomicU64::new(0),
            audio_sequence_gap_events: AtomicU64::new(0),
            audio_sequence_gap_frames: AtomicU64::new(0),
            audio_frames_duplicate: AtomicU64::new(0),
            audio_frames_out_of_order: AtomicU64::new(0),
            audio_frames_late: AtomicU64::new(0),
            audio_backpressure_drops: AtomicU64::new(0),
            shared_memory_process_blocks: AtomicU64::new(0),
            shared_memory_process_frames: AtomicU64::new(0),
            shared_memory_process_failures: AtomicU64::new(0),
            audio_route_latency: LatencyHistogram::new(),
            audio_interarrival_jitter: LatencyHistogram::new(),
            shared_memory_process_latency: LatencyHistogram::new(),
        }
    }

    pub fn increment_websocket_connections(&self) {
        self.websocket_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_control_messages(&self) {
        self.control_messages.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_binary_frames(&self) {
        self.binary_frames.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_audio_frames_routed(&self) {
        self.audio_frames_routed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_audio_frame_fallbacks(&self) {
        self.audio_frame_fallbacks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_audio_frame_route_failures(&self) {
        self.audio_frame_route_failures
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_hello_requests(&self) {
        self.hello_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_worker_failures(&self) {
        self.worker_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_worker_restarts(&self) {
        self.worker_restarts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_worker_auto_restarts(&self) {
        self.worker_auto_restarts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_worker_shutdown(&self, audit: WorkerShutdownAudit) {
        self.worker_shutdowns.fetch_add(1, Ordering::Relaxed);
        if audit.kill_requested {
            self.worker_kill_requests.fetch_add(1, Ordering::Relaxed);
        }
        if audit.tree_kill_requested {
            self.worker_tree_kill_requests
                .fetch_add(1, Ordering::Relaxed);
        }
        if audit.forced_kill_requested {
            self.worker_forced_kill_requests
                .fetch_add(1, Ordering::Relaxed);
        }
        if audit.wait_succeeded {
            self.worker_wait_successes.fetch_add(1, Ordering::Relaxed);
        }
        if audit.wait_timed_out {
            self.worker_wait_timeouts.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_audio_route_latency_us(&self, value: u64) {
        self.audio_route_latency.record(value);
    }

    pub fn increment_audio_backpressure_drops(&self) {
        self.audio_backpressure_drops
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_shared_memory_process_success(&self, frames: u64, latency_us: u64) {
        self.shared_memory_process_blocks
            .fetch_add(1, Ordering::Relaxed);
        self.shared_memory_process_frames
            .fetch_add(frames, Ordering::Relaxed);
        self.shared_memory_process_latency.record(latency_us);
    }

    pub fn record_shared_memory_process_failure(&self, latency_us: u64) {
        self.shared_memory_process_failures
            .fetch_add(1, Ordering::Relaxed);
        self.shared_memory_process_latency.record(latency_us);
    }

    pub fn record_audio_stream_observation(&self, observation: AudioStreamObservation) {
        if observation.sequence_gap > 0 {
            self.audio_sequence_gap_events
                .fetch_add(1, Ordering::Relaxed);
            self.audio_sequence_gap_frames
                .fetch_add(observation.sequence_gap, Ordering::Relaxed);
        }
        if observation.duplicate {
            self.audio_frames_duplicate.fetch_add(1, Ordering::Relaxed);
        }
        if observation.out_of_order {
            self.audio_frames_out_of_order
                .fetch_add(1, Ordering::Relaxed);
        }
        if observation.late {
            self.audio_frames_late.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(jitter_us) = observation.interarrival_jitter_us {
            self.audio_interarrival_jitter.record(jitter_us);
        }
    }

    pub fn snapshot(&self) -> BridgeMetricsSnapshot {
        BridgeMetricsSnapshot {
            uptime_ms: self.started_at.elapsed().as_millis() as u64,
            websocket_connections: self.websocket_connections.load(Ordering::Relaxed),
            control_messages: self.control_messages.load(Ordering::Relaxed),
            binary_frames: self.binary_frames.load(Ordering::Relaxed),
            audio_frames_routed: self.audio_frames_routed.load(Ordering::Relaxed),
            audio_frame_fallbacks: self.audio_frame_fallbacks.load(Ordering::Relaxed),
            audio_frame_route_failures: self.audio_frame_route_failures.load(Ordering::Relaxed),
            hello_requests: self.hello_requests.load(Ordering::Relaxed),
            worker_failures: self.worker_failures.load(Ordering::Relaxed),
            worker_restarts: self.worker_restarts.load(Ordering::Relaxed),
            worker_auto_restarts: self.worker_auto_restarts.load(Ordering::Relaxed),
            worker_shutdowns: self.worker_shutdowns.load(Ordering::Relaxed),
            worker_kill_requests: self.worker_kill_requests.load(Ordering::Relaxed),
            worker_tree_kill_requests: self.worker_tree_kill_requests.load(Ordering::Relaxed),
            worker_forced_kill_requests: self.worker_forced_kill_requests.load(Ordering::Relaxed),
            worker_wait_successes: self.worker_wait_successes.load(Ordering::Relaxed),
            worker_wait_timeouts: self.worker_wait_timeouts.load(Ordering::Relaxed),
            audio_sequence_gap_events: self.audio_sequence_gap_events.load(Ordering::Relaxed),
            audio_sequence_gap_frames: self.audio_sequence_gap_frames.load(Ordering::Relaxed),
            audio_frames_duplicate: self.audio_frames_duplicate.load(Ordering::Relaxed),
            audio_frames_out_of_order: self.audio_frames_out_of_order.load(Ordering::Relaxed),
            audio_frames_late: self.audio_frames_late.load(Ordering::Relaxed),
            audio_backpressure_drops: self.audio_backpressure_drops.load(Ordering::Relaxed),
            shared_memory_process_blocks: self.shared_memory_process_blocks.load(Ordering::Relaxed),
            shared_memory_process_frames: self.shared_memory_process_frames.load(Ordering::Relaxed),
            shared_memory_process_failures: self
                .shared_memory_process_failures
                .load(Ordering::Relaxed),
            audio_route_latency: self.audio_route_latency.snapshot(),
            audio_interarrival_jitter: self.audio_interarrival_jitter.snapshot(),
            shared_memory_process_latency: self.shared_memory_process_latency.snapshot(),
        }
    }
}

impl Default for BridgeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct BridgeMetricsHandle {
    metrics: Arc<BridgeMetrics>,
}

impl BridgeMetricsHandle {
    pub(crate) fn new(metrics: Arc<BridgeMetrics>) -> Self {
        Self { metrics }
    }

    pub fn snapshot(&self) -> BridgeMetricsSnapshot {
        self.metrics.snapshot()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorkerShutdownAudit {
    pub kill_requested: bool,
    pub tree_kill_requested: bool,
    pub forced_kill_requested: bool,
    pub wait_succeeded: bool,
    pub wait_timed_out: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeMetricsSnapshot {
    pub uptime_ms: u64,
    pub websocket_connections: u64,
    pub control_messages: u64,
    pub binary_frames: u64,
    pub audio_frames_routed: u64,
    pub audio_frame_fallbacks: u64,
    pub audio_frame_route_failures: u64,
    pub hello_requests: u64,
    pub worker_failures: u64,
    pub worker_restarts: u64,
    pub worker_auto_restarts: u64,
    pub worker_shutdowns: u64,
    pub worker_kill_requests: u64,
    pub worker_tree_kill_requests: u64,
    pub worker_forced_kill_requests: u64,
    pub worker_wait_successes: u64,
    pub worker_wait_timeouts: u64,
    pub audio_sequence_gap_events: u64,
    pub audio_sequence_gap_frames: u64,
    pub audio_frames_duplicate: u64,
    pub audio_frames_out_of_order: u64,
    pub audio_frames_late: u64,
    pub audio_backpressure_drops: u64,
    pub shared_memory_process_blocks: u64,
    pub shared_memory_process_frames: u64,
    pub shared_memory_process_failures: u64,
    pub audio_route_latency: LatencySnapshot,
    pub audio_interarrival_jitter: LatencySnapshot,
    pub shared_memory_process_latency: LatencySnapshot,
}

#[derive(Debug)]
struct LatencyHistogram {
    buckets: [AtomicU64; LATENCY_BUCKETS_US.len()],
}

impl LatencyHistogram {
    fn new() -> Self {
        Self {
            buckets: array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    fn record(&self, value_us: u64) {
        let bucket = LATENCY_BUCKETS_US
            .iter()
            .position(|limit| value_us <= *limit)
            .unwrap_or(LATENCY_BUCKETS_US.len() - 1);
        self.buckets[bucket].fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> LatencySnapshot {
        let buckets = self
            .buckets
            .iter()
            .enumerate()
            .map(|(index, bucket)| LatencyBucketSnapshot {
                le_us: bucket_upper_bound(index),
                count: bucket.load(Ordering::Relaxed),
            })
            .collect::<Vec<_>>();
        let count = buckets.iter().map(|bucket| bucket.count).sum();

        LatencySnapshot {
            count,
            p50_us: percentile_from_buckets(&buckets, count, 50),
            p95_us: percentile_from_buckets(&buckets, count, 95),
            p99_us: percentile_from_buckets(&buckets, count, 99),
            buckets,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencySnapshot {
    pub count: u64,
    pub p50_us: Option<u64>,
    pub p95_us: Option<u64>,
    pub p99_us: Option<u64>,
    pub buckets: Vec<LatencyBucketSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyBucketSnapshot {
    pub le_us: Option<u64>,
    pub count: u64,
}

fn percentile_from_buckets(
    buckets: &[LatencyBucketSnapshot],
    total: u64,
    percentile: u64,
) -> Option<u64> {
    if total == 0 {
        return None;
    }
    let target = total.saturating_mul(percentile).div_ceil(100).max(1);
    let mut cumulative = 0_u64;
    for bucket in buckets {
        cumulative = cumulative.saturating_add(bucket.count);
        if cumulative >= target {
            return bucket.le_us;
        }
    }

    None
}

fn bucket_upper_bound(index: usize) -> Option<u64> {
    let value = LATENCY_BUCKETS_US[index];
    (value != u64::MAX).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_audio_route_latency_percentiles() {
        let metrics = BridgeMetrics::new();

        metrics.record_audio_route_latency_us(80);
        metrics.record_audio_route_latency_us(700);
        metrics.record_audio_route_latency_us(60_000);

        let latency = metrics.snapshot().audio_route_latency;
        assert_eq!(latency.count, 3);
        assert_eq!(latency.p50_us, Some(1_000));
        assert_eq!(latency.p95_us, Some(100_000));
        assert_eq!(latency.p99_us, Some(100_000));
        assert_eq!(latency.buckets[0].count, 1);
    }

    #[test]
    fn reports_audio_stream_observation_counters() {
        let metrics = BridgeMetrics::new();

        metrics.record_audio_stream_observation(AudioStreamObservation {
            sequence_gap: 3,
            duplicate: true,
            out_of_order: true,
            late: true,
            interarrival_jitter_us: Some(1_200),
        });

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.audio_sequence_gap_events, 1);
        assert_eq!(snapshot.audio_sequence_gap_frames, 3);
        assert_eq!(snapshot.audio_frames_duplicate, 1);
        assert_eq!(snapshot.audio_frames_out_of_order, 1);
        assert_eq!(snapshot.audio_frames_late, 1);
        assert_eq!(snapshot.audio_interarrival_jitter.count, 1);
        assert_eq!(snapshot.audio_interarrival_jitter.p50_us, Some(2_000));

        metrics.increment_audio_backpressure_drops();
        assert_eq!(metrics.snapshot().audio_backpressure_drops, 1);
    }

    #[test]
    fn reports_worker_shutdown_audit_counters() {
        let metrics = BridgeMetrics::new();

        metrics.record_worker_shutdown(WorkerShutdownAudit {
            kill_requested: true,
            tree_kill_requested: true,
            forced_kill_requested: false,
            wait_succeeded: true,
            wait_timed_out: false,
        });
        metrics.record_worker_shutdown(WorkerShutdownAudit {
            kill_requested: true,
            tree_kill_requested: true,
            forced_kill_requested: true,
            wait_succeeded: false,
            wait_timed_out: true,
        });

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.worker_shutdowns, 2);
        assert_eq!(snapshot.worker_kill_requests, 2);
        assert_eq!(snapshot.worker_tree_kill_requests, 2);
        assert_eq!(snapshot.worker_forced_kill_requests, 1);
        assert_eq!(snapshot.worker_wait_successes, 1);
        assert_eq!(snapshot.worker_wait_timeouts, 1);
    }

    #[test]
    fn reports_shared_memory_process_counters() {
        let metrics = BridgeMetrics::new();

        metrics.record_shared_memory_process_success(128, 250);
        metrics.record_shared_memory_process_success(64, 700);
        metrics.record_shared_memory_process_failure(1_200);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.shared_memory_process_blocks, 2);
        assert_eq!(snapshot.shared_memory_process_frames, 192);
        assert_eq!(snapshot.shared_memory_process_failures, 1);
        assert_eq!(snapshot.shared_memory_process_latency.count, 3);
        assert_eq!(snapshot.shared_memory_process_latency.p50_us, Some(1_000));
    }
}
