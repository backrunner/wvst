use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{Value, json};
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
pub struct BridgeEventNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: BridgeEventNotificationParams,
}

#[derive(Debug, Clone, Serialize)]
pub struct BridgeEventNotificationParams {
    pub event: BridgeEvent,
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
    WorkerProcessingStarting {
        instance_id: u64,
        plugin_id: String,
    },
    WorkerProcessingStopping {
        instance_id: u64,
        plugin_id: String,
    },
    WorkerStopped {
        instance_id: u64,
    },
    WorkerDestroying {
        instance_id: u64,
        plugin_id: String,
        stream_id: u64,
    },
    WorkerDestroyed {
        instance_id: u64,
        plugin_id: String,
        stream_id: u64,
    },
    StreamOpened {
        instance_id: u64,
        plugin_id: String,
        stream_id: u64,
    },
    StreamClosing {
        instance_id: u64,
        plugin_id: String,
        stream_id: u64,
    },
    StreamClosed {
        instance_id: u64,
        plugin_id: String,
        stream_id: u64,
        drain_timed_out: bool,
    },
    WorkerFailed {
        instance_id: u64,
        plugin_id: String,
        code: i64,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_data: Option<Value>,
    },
    WorkerRecovering {
        instance_id: u64,
        plugin_id: String,
        mode: WorkerRecoveryMode,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_data: Option<Value>,
    },
    WorkerRecovered {
        instance_id: u64,
        plugin_id: String,
        processing_restored: bool,
        mode: WorkerRecoveryMode,
    },
    WorkerRecoveryFailed {
        instance_id: u64,
        plugin_id: String,
        mode: WorkerRecoveryMode,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_data: Option<Value>,
    },
    WorkerQuarantined {
        plugin_id: String,
        failures: u32,
        release_after_ms: u128,
    },
    WorkerQuarantineReleased {
        plugin_id: String,
    },
    WorkerPolicyDecision {
        #[serde(skip_serializing_if = "Option::is_none")]
        instance_id: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin_id: Option<String>,
        policy: String,
        decision: String,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Value>,
    },
    Vst3ComponentHandlerEvent {
        instance_id: u64,
        plugin_id: String,
        stream_id: u64,
        handler_sequence: u64,
        handler_kind: Vst3ComponentHandlerEventKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        parameter_id: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        value_normalized: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        flags: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        restart_flags: Option<Vst3RestartFlags>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dirty: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        editor_name: Option<String>,
    },
    Vst3MetadataInvalidated {
        instance_id: u64,
        plugin_id: String,
        stream_id: u64,
        handler_sequence: u64,
        reasons: Vec<Vst3MetadataInvalidationReason>,
        refresh_policy: Vst3MetadataRefreshPolicy,
        restart_flags: Vst3RestartFlags,
    },
    Vst3ComponentHandlerEventsLost {
        instance_id: u64,
        plugin_id: String,
        stream_id: u64,
        from_sequence: u64,
        to_sequence: u64,
        lost_count: u64,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Vst3ComponentHandlerEventKind(pub String);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Vst3MetadataInvalidationReason {
    ReloadComponent,
    AudioIo,
    ParameterValues,
    ParameterInfo,
    Latency,
    MidiMapping,
    NoteExpression,
    RoutingInfo,
    PrefetchableSupport,
    Keyswitches,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Vst3MetadataRefreshPolicy {
    RefreshMetadata,
    RebuildAudioGraph,
    ReloadComponent,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerRecoveryMode {
    ManualRestart,
    AutoHeartbeat,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3RestartFlags {
    pub raw: i32,
    pub reload_component: bool,
    pub io_changed: bool,
    pub param_values_changed: bool,
    pub latency_changed: bool,
    pub param_titles_changed: bool,
    pub midi_cc_assignment_changed: bool,
    pub note_expression_changed: bool,
    pub io_titles_changed: bool,
    pub prefetchable_support_changed: bool,
    pub routing_info_changed: bool,
    pub keyswitch_changed: bool,
    pub param_id_mapping_changed: bool,
    pub unknown_bits: i32,
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

pub fn bridge_event_notification(event: BridgeEvent) -> Value {
    json!(BridgeEventNotification {
        jsonrpc: "2.0",
        method: "bridge.event",
        params: BridgeEventNotificationParams { event },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_worker_quarantine_release_after_ms() {
        let value = json!(BridgeEventKind::WorkerQuarantined {
            plugin_id: "vst3:test".to_string(),
            failures: 3,
            release_after_ms: 1_500,
        });

        assert_eq!(value["type"], "worker-quarantined");
        assert_eq!(value["pluginId"], "vst3:test");
        assert_eq!(value["failures"], 3);
        assert_eq!(value["releaseAfterMs"], 1_500);
    }

    #[test]
    fn serializes_worker_recovery_diagnostics() {
        let recovering = json!(BridgeEventKind::WorkerRecovering {
            instance_id: 7,
            plugin_id: "vst3:test".to_string(),
            mode: WorkerRecoveryMode::AutoHeartbeat,
            reason: "heartbeat-failed".to_string(),
            error_data: Some(json!({ "kind": "timeout" })),
        });
        let recovered = json!(BridgeEventKind::WorkerRecovered {
            instance_id: 7,
            plugin_id: "vst3:test".to_string(),
            processing_restored: true,
            mode: WorkerRecoveryMode::ManualRestart,
        });

        assert_eq!(recovering["type"], "worker-recovering");
        assert_eq!(recovering["mode"], "auto-heartbeat");
        assert_eq!(recovering["reason"], "heartbeat-failed");
        assert_eq!(recovering["errorData"]["kind"], "timeout");
        assert_eq!(recovered["type"], "worker-recovered");
        assert_eq!(recovered["mode"], "manual-restart");
        assert_eq!(recovered["processingRestored"], true);
    }

    #[test]
    fn serializes_worker_recovery_failed_diagnostics() {
        let value = json!(BridgeEventKind::WorkerRecoveryFailed {
            instance_id: 7,
            plugin_id: "vst3:test".to_string(),
            mode: WorkerRecoveryMode::AutoHeartbeat,
            reason: "restart-failed".to_string(),
            error_data: Some(json!({ "kind": "spawn" })),
        });

        assert_eq!(value["type"], "worker-recovery-failed");
        assert_eq!(value["mode"], "auto-heartbeat");
        assert_eq!(value["reason"], "restart-failed");
        assert_eq!(value["errorData"]["kind"], "spawn");
    }

    #[test]
    fn serializes_worker_policy_decision() {
        let value = json!(BridgeEventKind::WorkerPolicyDecision {
            instance_id: Some(7),
            plugin_id: Some("vst3:test".to_string()),
            policy: "resource-limit".to_string(),
            decision: "reject".to_string(),
            reason: "worker-instance-limit".to_string(),
            data: Some(json!({ "resource": "worker-instances" })),
        });

        assert_eq!(value["type"], "worker-policy-decision");
        assert_eq!(value["instanceId"], 7);
        assert_eq!(value["pluginId"], "vst3:test");
        assert_eq!(value["policy"], "resource-limit");
        assert_eq!(value["decision"], "reject");
        assert_eq!(value["reason"], "worker-instance-limit");
        assert_eq!(value["data"]["resource"], "worker-instances");
    }
}
