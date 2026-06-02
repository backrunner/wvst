use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 256;
const RECENT_EVENT_LIMIT: usize = 512;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEvent {
    pub sequence: u64,
    pub kind: BridgeEventKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum BridgeEventKind {
    ServerStarting,
    ServerStarted {
        local_addr: SocketAddr,
    },
    ServerStopping,
    ServerStopped,
    WorkerStarting {
        instance_id: u64,
        plugin_id: String,
    },
    WorkerReady {
        instance_id: u64,
        plugin_id: String,
    },
    WorkerProcessing {
        instance_id: u64,
    },
    WorkerStopped {
        instance_id: u64,
    },
    WorkerFailed {
        instance_id: u64,
        plugin_id: String,
        code: i64,
        message: String,
    },
    WorkerRecovering {
        instance_id: u64,
        plugin_id: String,
    },
    WorkerRecovered {
        instance_id: u64,
        plugin_id: String,
        processing_restored: bool,
    },
    WorkerQuarantined {
        plugin_id: String,
        failures: u32,
    },
    WorkerQuarantineReleased {
        plugin_id: String,
    },
}

#[derive(Debug, Clone)]
pub struct BridgeEventBus {
    next_sequence: Arc<AtomicU64>,
    recent: Arc<Mutex<VecDeque<BridgeEvent>>>,
    sender: broadcast::Sender<BridgeEvent>,
}

impl BridgeEventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            next_sequence: Arc::new(AtomicU64::new(1)),
            recent: Arc::new(Mutex::new(VecDeque::with_capacity(RECENT_EVENT_LIMIT))),
            sender,
        }
    }

    pub fn emit(&self, kind: BridgeEventKind) -> BridgeEvent {
        let event = BridgeEvent {
            sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
            kind,
        };

        if let Ok(mut recent) = self.recent.lock() {
            if recent.len() >= RECENT_EVENT_LIMIT {
                recent.pop_front();
            }
            recent.push_back(event.clone());
        }
        let _ = self.sender.send(event.clone());

        event
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BridgeEvent> {
        self.sender.subscribe()
    }

    pub fn recent_since(&self, after_sequence: Option<u64>) -> Vec<BridgeEvent> {
        let Some(recent) = self.recent.lock().ok() else {
            return Vec::new();
        };
        recent
            .iter()
            .filter(|event| after_sequence.is_none_or(|after| event.sequence > after))
            .cloned()
            .collect()
    }
}

impl Default for BridgeEventBus {
    fn default() -> Self {
        Self::new()
    }
}
