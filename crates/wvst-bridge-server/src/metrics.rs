use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;

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
}

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
        }
    }
}

impl Default for BridgeMetrics {
    fn default() -> Self {
        Self::new()
    }
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
}
