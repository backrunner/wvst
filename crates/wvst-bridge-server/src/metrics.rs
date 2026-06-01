use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;

#[derive(Debug)]
pub struct BridgeMetrics {
    started_at: Instant,
    websocket_connections: AtomicU64,
    control_messages: AtomicU64,
    binary_frames: AtomicU64,
    hello_requests: AtomicU64,
}

impl BridgeMetrics {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            websocket_connections: AtomicU64::new(0),
            control_messages: AtomicU64::new(0),
            binary_frames: AtomicU64::new(0),
            hello_requests: AtomicU64::new(0),
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

    pub fn increment_hello_requests(&self) {
        self.hello_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> BridgeMetricsSnapshot {
        BridgeMetricsSnapshot {
            uptime_ms: self.started_at.elapsed().as_millis() as u64,
            websocket_connections: self.websocket_connections.load(Ordering::Relaxed),
            control_messages: self.control_messages.load(Ordering::Relaxed),
            binary_frames: self.binary_frames.load(Ordering::Relaxed),
            hello_requests: self.hello_requests.load(Ordering::Relaxed),
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
    pub hello_requests: u64,
}
