use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(super) const DEFAULT_MAX_QUEUED_EVENTS: u32 = 1024;
pub(super) const MAX_QUEUED_EVENTS_LIMIT: u32 = 65_536;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SharedMemoryPumpEventOverflowPolicy {
    #[default]
    Reject,
    DropOldest,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpEnqueueEventsParams {
    pub instance_id: u64,
    #[serde(default)]
    pub target_iteration: Option<u64>,
    #[serde(default)]
    pub delay_iterations: Option<u64>,
    #[serde(default)]
    pub midi_events: Vec<Value>,
    #[serde(default)]
    pub parameter_events: Vec<Value>,
    #[serde(default)]
    pub overflow_policy: SharedMemoryPumpEventOverflowPolicy,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpClearEventsParams {
    pub instance_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpEventQueueStatus {
    pub max_queued_events: u32,
    pub pending_batches: u64,
    pub pending_events: u64,
    pub pending_midi_events: u64,
    pub pending_parameter_events: u64,
    pub next_sequence: u64,
    pub oldest_target_iteration: Option<u64>,
    pub newest_target_iteration: Option<u64>,
    pub enqueued_batches: u64,
    pub enqueued_events: u64,
    pub drained_batches: u64,
    pub drained_events: u64,
    pub late_batches: u64,
    pub late_events: u64,
    pub dropped_batches: u64,
    pub dropped_events: u64,
    pub cleared_batches: u64,
    pub cleared_events: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpEnqueueEventsResult {
    pub instance_id: u64,
    pub target_iteration: u64,
    pub sequence: u64,
    pub queued_events: u64,
    pub queued_midi_events: u64,
    pub queued_parameter_events: u64,
    pub dropped_events: u64,
    pub dropped_batches: u64,
    pub status: SharedMemoryPumpEventQueueStatus,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpClearEventsResult {
    pub instance_id: u64,
    pub cleared_events: u64,
    pub cleared_batches: u64,
    pub status: SharedMemoryPumpEventQueueStatus,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SharedMemoryPumpQueuedEventBatch {
    target_iteration: u64,
    sequence: u64,
    midi_events: Vec<Value>,
    parameter_events: Vec<Value>,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub(super) struct SharedMemoryPumpEventDrain {
    pub(super) midi_events: Vec<Value>,
    pub(super) parameter_events: Vec<Value>,
    pub(super) batches: u64,
    pub(super) events: u64,
    pub(super) late_batches: u64,
    pub(super) late_events: u64,
}

#[derive(Debug, Default)]
pub(super) struct SharedMemoryPumpEventQueue {
    max_queued_events: u32,
    pending: Vec<SharedMemoryPumpQueuedEventBatch>,
    pending_events: u64,
    next_sequence: u64,
    enqueued_batches: u64,
    enqueued_events: u64,
    drained_batches: u64,
    drained_events: u64,
    late_batches: u64,
    late_events: u64,
    dropped_batches: u64,
    dropped_events: u64,
    cleared_batches: u64,
    cleared_events: u64,
}

impl SharedMemoryPumpEventQueue {
    pub(super) fn new(max_queued_events: u32) -> Self {
        Self {
            max_queued_events,
            next_sequence: 1,
            ..Self::default()
        }
    }

    pub(super) fn enqueue(
        &mut self,
        target_iteration: u64,
        midi_events: Vec<Value>,
        parameter_events: Vec<Value>,
        overflow_policy: SharedMemoryPumpEventOverflowPolicy,
    ) -> Result<SharedMemoryPumpEnqueueEventsResult, SharedMemoryPumpEventQueueError> {
        let incoming_events = event_count(midi_events.len(), parameter_events.len());
        if incoming_events == 0 {
            return Err(SharedMemoryPumpEventQueueError::EmptyBatch);
        }
        if incoming_events > u64::from(self.max_queued_events) {
            return Err(SharedMemoryPumpEventQueueError::BatchExceedsCapacity {
                requested_events: incoming_events,
                max_queued_events: self.max_queued_events,
            });
        }

        let dropped = self.reserve_capacity(incoming_events, overflow_policy)?;
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        let queued_midi_events = midi_events.len() as u64;
        let queued_parameter_events = parameter_events.len() as u64;
        self.pending.push(SharedMemoryPumpQueuedEventBatch {
            target_iteration,
            sequence,
            midi_events,
            parameter_events,
        });
        self.pending
            .sort_by_key(|batch| (batch.target_iteration, batch.sequence));
        self.pending_events = self.pending_events.saturating_add(incoming_events);
        self.enqueued_batches = self.enqueued_batches.saturating_add(1);
        self.enqueued_events = self.enqueued_events.saturating_add(incoming_events);

        Ok(SharedMemoryPumpEnqueueEventsResult {
            instance_id: 0,
            target_iteration,
            sequence,
            queued_events: incoming_events,
            queued_midi_events,
            queued_parameter_events,
            dropped_events: dropped.events,
            dropped_batches: dropped.batches,
            status: self.status(),
        })
    }

    pub(super) fn drain_ready(&mut self, iteration: u64) -> SharedMemoryPumpEventDrain {
        let ready_count = self
            .pending
            .iter()
            .take_while(|batch| batch.target_iteration <= iteration)
            .count();
        if ready_count == 0 {
            return SharedMemoryPumpEventDrain::default();
        }

        let mut drain = SharedMemoryPumpEventDrain::default();
        let ready: Vec<_> = self.pending.drain(..ready_count).collect();
        for mut batch in ready {
            let batch_events = event_count(batch.midi_events.len(), batch.parameter_events.len());
            drain.batches = drain.batches.saturating_add(1);
            drain.events = drain.events.saturating_add(batch_events);
            if batch.target_iteration < iteration {
                drain.late_batches = drain.late_batches.saturating_add(1);
                drain.late_events = drain.late_events.saturating_add(batch_events);
            }
            drain.midi_events.append(&mut batch.midi_events);
            drain.parameter_events.append(&mut batch.parameter_events);
        }
        self.pending_events = self.pending_events.saturating_sub(drain.events);
        self.drained_batches = self.drained_batches.saturating_add(drain.batches);
        self.drained_events = self.drained_events.saturating_add(drain.events);
        self.late_batches = self.late_batches.saturating_add(drain.late_batches);
        self.late_events = self.late_events.saturating_add(drain.late_events);
        drain
    }

    pub(super) fn clear(&mut self) -> (u64, u64) {
        let batches = self.pending.len() as u64;
        let events = self.pending_events;
        self.pending.clear();
        self.pending_events = 0;
        self.cleared_batches = self.cleared_batches.saturating_add(batches);
        self.cleared_events = self.cleared_events.saturating_add(events);
        (batches, events)
    }

    pub(super) fn status(&self) -> SharedMemoryPumpEventQueueStatus {
        let (pending_midi_events, pending_parameter_events) =
            self.pending.iter().fold((0_u64, 0_u64), |acc, batch| {
                (
                    acc.0.saturating_add(batch.midi_events.len() as u64),
                    acc.1.saturating_add(batch.parameter_events.len() as u64),
                )
            });
        SharedMemoryPumpEventQueueStatus {
            max_queued_events: self.max_queued_events,
            pending_batches: self.pending.len() as u64,
            pending_events: self.pending_events,
            pending_midi_events,
            pending_parameter_events,
            next_sequence: self.next_sequence,
            oldest_target_iteration: self.pending.first().map(|batch| batch.target_iteration),
            newest_target_iteration: self.pending.last().map(|batch| batch.target_iteration),
            enqueued_batches: self.enqueued_batches,
            enqueued_events: self.enqueued_events,
            drained_batches: self.drained_batches,
            drained_events: self.drained_events,
            late_batches: self.late_batches,
            late_events: self.late_events,
            dropped_batches: self.dropped_batches,
            dropped_events: self.dropped_events,
            cleared_batches: self.cleared_batches,
            cleared_events: self.cleared_events,
        }
    }

    fn reserve_capacity(
        &mut self,
        incoming_events: u64,
        overflow_policy: SharedMemoryPumpEventOverflowPolicy,
    ) -> Result<DroppedEvents, SharedMemoryPumpEventQueueError> {
        let max_queued_events = u64::from(self.max_queued_events);
        if self.pending_events.saturating_add(incoming_events) <= max_queued_events {
            return Ok(DroppedEvents::default());
        }
        if overflow_policy == SharedMemoryPumpEventOverflowPolicy::Reject {
            return Err(SharedMemoryPumpEventQueueError::CapacityExceeded {
                queued_events: self.pending_events,
                requested_events: incoming_events,
                max_queued_events: self.max_queued_events,
            });
        }

        let mut dropped = DroppedEvents::default();
        while self.pending_events.saturating_add(incoming_events) > max_queued_events {
            let Some(batch) = self.pending.first() else {
                break;
            };
            let batch_events = event_count(batch.midi_events.len(), batch.parameter_events.len());
            let _ = self.pending.remove(0);
            self.pending_events = self.pending_events.saturating_sub(batch_events);
            dropped.batches = dropped.batches.saturating_add(1);
            dropped.events = dropped.events.saturating_add(batch_events);
        }
        self.dropped_batches = self.dropped_batches.saturating_add(dropped.batches);
        self.dropped_events = self.dropped_events.saturating_add(dropped.events);
        Ok(dropped)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum SharedMemoryPumpEventQueueError {
    EmptyBatch,
    BatchExceedsCapacity {
        requested_events: u64,
        max_queued_events: u32,
    },
    CapacityExceeded {
        queued_events: u64,
        requested_events: u64,
        max_queued_events: u32,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
struct DroppedEvents {
    batches: u64,
    events: u64,
}

fn event_count(midi_events: usize, parameter_events: usize) -> u64 {
    midi_events.saturating_add(parameter_events) as u64
}
