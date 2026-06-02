use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::events::{
    BridgeEventBus, BridgeEventKind, Vst3ComponentHandlerEventKind, Vst3MetadataInvalidationReason,
    Vst3MetadataRefreshPolicy, Vst3RestartFlags,
};
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
    restart_flags: Option<Vst3RestartFlags>,
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
                restart_flags: event.restart_flags,
                dirty: event.dirty,
                editor_name: event.editor_name.clone(),
            });
            if let Some(restart_flags) = event.restart_flags {
                let reasons = metadata_invalidation_reasons(restart_flags);
                if !reasons.is_empty() {
                    let refresh_policy = metadata_refresh_policy(restart_flags);
                    events.emit(BridgeEventKind::Vst3MetadataInvalidated {
                        instance_id: instance.instance_id,
                        plugin_id: instance.plugin_id.clone(),
                        stream_id: instance.stream_id,
                        handler_sequence: event.sequence,
                        reasons,
                        refresh_policy,
                        restart_flags,
                    });
                }
            }
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
        restart_flags: value.get("restartFlags").and_then(restart_flags),
        dirty: value.get("dirty").and_then(Value::as_bool),
        editor_name: value
            .get("editorName")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn restart_flags(value: &Value) -> Option<Vst3RestartFlags> {
    Some(Vst3RestartFlags {
        raw: json_i32(value, "raw")?,
        reload_component: json_bool(value, "reloadComponent")?,
        io_changed: json_bool(value, "ioChanged")?,
        param_values_changed: json_bool(value, "paramValuesChanged")?,
        latency_changed: json_bool(value, "latencyChanged")?,
        param_titles_changed: json_bool(value, "paramTitlesChanged")?,
        midi_cc_assignment_changed: json_bool(value, "midiCcAssignmentChanged")?,
        note_expression_changed: json_bool(value, "noteExpressionChanged")?,
        io_titles_changed: json_bool(value, "ioTitlesChanged")?,
        prefetchable_support_changed: json_bool(value, "prefetchableSupportChanged")?,
        routing_info_changed: json_bool(value, "routingInfoChanged")?,
        keyswitch_changed: json_bool(value, "keyswitchChanged")?,
        param_id_mapping_changed: json_bool(value, "paramIdMappingChanged")?,
        unknown_bits: json_i32(value, "unknownBits")?,
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

fn json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn json_finite_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key)?.as_f64().filter(|value| value.is_finite())
}

fn metadata_invalidation_reasons(
    restart_flags: Vst3RestartFlags,
) -> Vec<Vst3MetadataInvalidationReason> {
    let mut reasons = Vec::new();
    if restart_flags.reload_component {
        reasons.push(Vst3MetadataInvalidationReason::ReloadComponent);
    }
    if restart_flags.io_changed || restart_flags.io_titles_changed {
        reasons.push(Vst3MetadataInvalidationReason::AudioIo);
    }
    if restart_flags.param_values_changed {
        reasons.push(Vst3MetadataInvalidationReason::ParameterValues);
    }
    if restart_flags.param_titles_changed || restart_flags.param_id_mapping_changed {
        reasons.push(Vst3MetadataInvalidationReason::ParameterInfo);
    }
    if restart_flags.latency_changed {
        reasons.push(Vst3MetadataInvalidationReason::Latency);
    }
    if restart_flags.midi_cc_assignment_changed {
        reasons.push(Vst3MetadataInvalidationReason::MidiMapping);
    }
    if restart_flags.note_expression_changed {
        reasons.push(Vst3MetadataInvalidationReason::NoteExpression);
    }
    if restart_flags.routing_info_changed {
        reasons.push(Vst3MetadataInvalidationReason::RoutingInfo);
    }
    if restart_flags.prefetchable_support_changed {
        reasons.push(Vst3MetadataInvalidationReason::PrefetchableSupport);
    }
    if restart_flags.keyswitch_changed {
        reasons.push(Vst3MetadataInvalidationReason::Keyswitches);
    }
    reasons
}

fn metadata_refresh_policy(restart_flags: Vst3RestartFlags) -> Vst3MetadataRefreshPolicy {
    if restart_flags.reload_component {
        return Vst3MetadataRefreshPolicy::ReloadComponent;
    }

    if restart_flags.io_changed
        || restart_flags.io_titles_changed
        || restart_flags.routing_info_changed
    {
        return Vst3MetadataRefreshPolicy::RebuildAudioGraph;
    }

    Vst3MetadataRefreshPolicy::RefreshMetadata
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

    #[test]
    fn forwards_structured_restart_flags() {
        let publisher = ComponentHandlerEventPublisher::new();
        let events = BridgeEventBus::new();
        let instance = instance_record();
        let metrics = json!({
            "runtime": [{
                "streamId": 9,
                "diagnostics": {
                    "componentHandler": {
                        "totalEvents": 1,
                        "recentEvents": [{
                            "sequence": 1,
                            "kind": "restart-component",
                            "flags": 24,
                            "restartFlags": {
                                "raw": 24,
                                "reloadComponent": false,
                                "ioChanged": false,
                                "paramValuesChanged": false,
                                "latencyChanged": true,
                                "paramTitlesChanged": true,
                                "midiCcAssignmentChanged": false,
                                "noteExpressionChanged": false,
                                "ioTitlesChanged": false,
                                "prefetchableSupportChanged": false,
                                "routingInfoChanged": false,
                                "keyswitchChanged": false,
                                "paramIdMappingChanged": false,
                                "unknownBits": 0
                            }
                        }]
                    }
                }
            }]
        });

        publisher.publish_from_worker_metrics(&events, &instance, &metrics);

        let recent = events.recent_since(None);
        let BridgeEventKind::Vst3ComponentHandlerEvent { restart_flags, .. } = &recent[0].kind
        else {
            panic!("expected component handler event");
        };
        let restart_flags = restart_flags.expect("restart flags");
        assert!(restart_flags.latency_changed);
        assert!(restart_flags.param_titles_changed);
        assert_eq!(restart_flags.raw, 24);
        let BridgeEventKind::Vst3MetadataInvalidated {
            reasons,
            refresh_policy,
            restart_flags,
            handler_sequence,
            ..
        } = &recent[1].kind
        else {
            panic!("expected metadata invalidation event");
        };
        assert_eq!(*handler_sequence, 1);
        assert_eq!(
            reasons,
            &vec![
                Vst3MetadataInvalidationReason::ParameterInfo,
                Vst3MetadataInvalidationReason::Latency,
            ]
        );
        assert_eq!(*refresh_policy, Vst3MetadataRefreshPolicy::RefreshMetadata);
        assert_eq!(restart_flags.raw, 24);
    }

    #[test]
    fn classifies_metadata_refresh_policy() {
        let mut flags = Vst3RestartFlags {
            raw: 0,
            reload_component: false,
            io_changed: false,
            param_values_changed: false,
            latency_changed: false,
            param_titles_changed: false,
            midi_cc_assignment_changed: false,
            note_expression_changed: false,
            io_titles_changed: false,
            prefetchable_support_changed: false,
            routing_info_changed: false,
            keyswitch_changed: false,
            param_id_mapping_changed: false,
            unknown_bits: 0,
        };

        flags.latency_changed = true;
        assert_eq!(
            metadata_refresh_policy(flags),
            Vst3MetadataRefreshPolicy::RefreshMetadata
        );

        flags.io_changed = true;
        assert_eq!(
            metadata_refresh_policy(flags),
            Vst3MetadataRefreshPolicy::RebuildAudioGraph
        );

        flags.reload_component = true;
        assert_eq!(
            metadata_refresh_policy(flags),
            Vst3MetadataRefreshPolicy::ReloadComponent
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
                BridgeEventKind::Vst3MetadataInvalidated { .. } => "vst3-metadata-invalidated",
                BridgeEventKind::Vst3ComponentHandlerEventsLost { .. } => {
                    "vst3-component-handler-events-lost"
                }
                _ => "other",
            }
        }
    }
}
