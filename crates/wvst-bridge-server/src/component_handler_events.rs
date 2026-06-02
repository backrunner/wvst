use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::events::{BridgeEventBus, BridgeEventKind, Vst3ComponentHandlerEventKind};
use crate::instance_registry::InstanceRecord;

#[derive(Debug, Default)]
pub struct ComponentHandlerEventPublisher {
    last_sequences: Mutex<BTreeMap<u64, u64>>,
}

#[derive(Debug, Clone, PartialEq)]
struct ComponentHandlerSnapshot {
    total_events: u64,
    recent_events: Vec<ComponentHandlerEvent>,
}

#[derive(Debug, Clone, PartialEq)]
struct ComponentHandlerEvent {
    sequence: u64,
    kind: String,
    parameter_id: Option<u32>,
    value_normalized: Option<f64>,
    flags: Option<i32>,
    dirty: Option<bool>,
    editor_name: Option<String>,
}

impl ComponentHandlerEventPublisher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset_instance(&self, instance_id: u64) {
        if let Ok(mut last_sequences) = self.last_sequences.lock() {
            last_sequences.remove(&instance_id);
        }
    }

    pub fn publish_from_worker_metrics(
        &self,
        events: &BridgeEventBus,
        instance: &InstanceRecord,
        worker_metrics: &Value,
    ) {
        let Some(snapshot) = component_handler_snapshot(worker_metrics, instance.stream_id) else {
            return;
        };
        if snapshot.total_events == 0 {
            self.reset_if_worker_sequence_restarted(instance.instance_id, 0);
            return;
        }

        let Some((last_seen, new_events)) = self.new_events(
            instance.instance_id,
            snapshot.total_events,
            &snapshot.recent_events,
        ) else {
            return;
        };

        if let Some(first_event) = new_events.first()
            && first_event.sequence > last_seen.saturating_add(1)
        {
            let from_sequence = last_seen.saturating_add(1);
            let to_sequence = first_event.sequence.saturating_sub(1);
            events.emit(BridgeEventKind::Vst3ComponentHandlerEventsLost {
                instance_id: instance.instance_id,
                plugin_id: instance.plugin_id.clone(),
                stream_id: instance.stream_id,
                from_sequence,
                to_sequence,
                lost_count: to_sequence.saturating_sub(from_sequence).saturating_add(1),
            });
        }

        for event in new_events {
            events.emit(BridgeEventKind::Vst3ComponentHandlerEvent {
                instance_id: instance.instance_id,
                plugin_id: instance.plugin_id.clone(),
                stream_id: instance.stream_id,
                handler_sequence: event.sequence,
                handler_kind: Vst3ComponentHandlerEventKind(event.kind.clone()),
                parameter_id: event.parameter_id,
                value_normalized: event.value_normalized,
                flags: event.flags,
                dirty: event.dirty,
                editor_name: event.editor_name.clone(),
            });
        }
    }

    fn new_events(
        &self,
        instance_id: u64,
        total_events: u64,
        recent_events: &[ComponentHandlerEvent],
    ) -> Option<(u64, Vec<ComponentHandlerEvent>)> {
        let mut last_sequences = self.last_sequences.lock().ok()?;
        let last_seen = last_sequences.get(&instance_id).copied().unwrap_or(0);
        let last_seen = if total_events < last_seen {
            0
        } else {
            last_seen
        };
        let new_events = recent_events
            .iter()
            .filter(|event| event.sequence > last_seen)
            .cloned()
            .collect::<Vec<_>>();
        last_sequences.insert(instance_id, total_events);
        if new_events.is_empty() {
            None
        } else {
            Some((last_seen, new_events))
        }
    }

    fn reset_if_worker_sequence_restarted(&self, instance_id: u64, total_events: u64) {
        if let Ok(mut last_sequences) = self.last_sequences.lock()
            && last_sequences
                .get(&instance_id)
                .is_some_and(|last_seen| total_events < *last_seen)
        {
            last_sequences.remove(&instance_id);
        }
    }
}

fn component_handler_snapshot(
    worker_metrics: &Value,
    stream_id: u64,
) -> Option<ComponentHandlerSnapshot> {
    let runtime = worker_metrics
        .get("runtime")?
        .as_array()?
        .iter()
        .find(|entry| json_u64(entry, "streamId") == Some(stream_id))?;
    let component_handler = runtime
        .get("diagnostics")?
        .get("componentHandler")?
        .as_object()?;
    let total_events = component_handler.get("totalEvents")?.as_u64()?;
    let recent_events = component_handler
        .get("recentEvents")?
        .as_array()?
        .iter()
        .filter_map(component_handler_event)
        .collect::<Vec<_>>();
    Some(ComponentHandlerSnapshot {
        total_events,
        recent_events,
    })
}

fn component_handler_event(value: &Value) -> Option<ComponentHandlerEvent> {
    Some(ComponentHandlerEvent {
        sequence: json_u64(value, "sequence")?,
        kind: value.get("kind")?.as_str()?.to_string(),
        parameter_id: json_u32(value, "parameterId"),
        value_normalized: json_finite_f64(value, "valueNormalized"),
        flags: json_i32(value, "flags"),
        dirty: value.get("dirty").and_then(Value::as_bool),
        editor_name: value
            .get("editorName")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn json_u32(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
}

fn json_i32(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)?
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
}

fn json_finite_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key)?.as_f64().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::instance_registry::{InstanceState, StreamState, WorkerState};
    use crate::runtime_capabilities::RuntimeCapabilities;

    #[test]
    fn publishes_component_handler_events_once() {
        let publisher = ComponentHandlerEventPublisher::new();
        let events = BridgeEventBus::new();
        let instance = instance_record();
        let metrics = json!({
            "runtime": [{
                "streamId": 9,
                "diagnostics": {
                    "componentHandler": {
                        "totalEvents": 2,
                        "recentEvents": [
                            { "sequence": 1, "kind": "begin-edit", "parameterId": 42 },
                            { "sequence": 2, "kind": "perform-edit", "parameterId": 42, "valueNormalized": 0.75 }
                        ]
                    }
                }
            }]
        });

        publisher.publish_from_worker_metrics(&events, &instance, &metrics);
        publisher.publish_from_worker_metrics(&events, &instance, &metrics);

        let recent = events.recent_since(None);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].kind.kind_type(), "vst3-component-handler-event");
        assert_eq!(recent[1].kind.kind_type(), "vst3-component-handler-event");
    }

    #[test]
    fn reports_lost_component_handler_events() {
        let publisher = ComponentHandlerEventPublisher::new();
        let events = BridgeEventBus::new();
        let instance = instance_record();
        let metrics = json!({
            "runtime": [{
                "streamId": 9,
                "diagnostics": {
                    "componentHandler": {
                        "totalEvents": 70,
                        "recentEvents": [
                            { "sequence": 7, "kind": "restart-component", "flags": 1 },
                            { "sequence": 70, "kind": "set-dirty", "dirty": true }
                        ]
                    }
                }
            }]
        });

        publisher.publish_from_worker_metrics(&events, &instance, &metrics);

        let recent = events.recent_since(None);
        assert_eq!(recent.len(), 3);
        assert_eq!(
            recent[0].kind.kind_type(),
            "vst3-component-handler-events-lost"
        );
    }

    fn instance_record() -> InstanceRecord {
        InstanceRecord {
            instance_id: 7,
            stream_id: 9,
            plugin_id: "vst3:test".to_string(),
            plugin_path: "/tmp/Test.vst3".to_string(),
            class_id: Some("class-a".to_string()),
            class_name: Some("Test".to_string()),
            sample_rate: 48_000,
            max_block_frames: 128,
            input_channels: 2,
            output_channels: 2,
            state: InstanceState::Ready,
            worker_state: WorkerState::Ready,
            stream_state: StreamState::Open,
            backend: Some("vst3-runtime".to_string()),
            controller_class_id: None,
            runtime_capabilities: RuntimeCapabilities::default(),
            latency_samples: 0,
            tail_samples: 0,
        }
    }

    trait EventKindExt {
        fn kind_type(&self) -> &'static str;
    }

    impl EventKindExt for BridgeEventKind {
        fn kind_type(&self) -> &'static str {
            match self {
                BridgeEventKind::Vst3ComponentHandlerEvent { .. } => "vst3-component-handler-event",
                BridgeEventKind::Vst3ComponentHandlerEventsLost { .. } => {
                    "vst3-component-handler-events-lost"
                }
                _ => "other",
            }
        }
    }
}
