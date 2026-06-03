use serde_json::{Value, json};

use super::event_queue::{MAX_QUEUED_EVENTS_LIMIT, SharedMemoryPumpEventQueueError};

#[derive(Debug)]
pub enum SharedMemoryPumpError {
    AlreadyRunning {
        instance_id: u64,
    },
    NotRunning {
        instance_id: u64,
    },
    NotAttached {
        instance_id: u64,
    },
    StreamClosed {
        instance_id: u64,
    },
    InstanceNotProcessing {
        instance_id: u64,
    },
    InvalidFrames {
        frames: u16,
        max_frames: u16,
    },
    InvalidInterval,
    InvalidSchedulingWindow {
        min_micros: u64,
        max_micros: u64,
    },
    InvalidMaxQueuedEvents {
        max_queued_events: u32,
    },
    InvalidEventTarget,
    EmptyEventBatch,
    EventBatchExceedsCapacity {
        requested_events: u64,
        max_queued_events: u32,
    },
    EventQueueCapacityExceeded {
        queued_events: u64,
        requested_events: u64,
        max_queued_events: u32,
    },
    RegistryUnavailable,
}

impl SharedMemoryPumpError {
    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::AlreadyRunning { .. }
            | Self::NotRunning { .. }
            | Self::NotAttached { .. }
            | Self::StreamClosed { .. }
            | Self::InstanceNotProcessing { .. } => 4096,
            Self::InvalidFrames { .. }
            | Self::InvalidInterval
            | Self::InvalidSchedulingWindow { .. }
            | Self::InvalidMaxQueuedEvents { .. }
            | Self::InvalidEventTarget
            | Self::EmptyEventBatch
            | Self::EventBatchExceedsCapacity { .. }
            | Self::EventQueueCapacityExceeded { .. } => 4222,
            Self::RegistryUnavailable => 5037,
        }
    }

    pub fn rpc_message(&self) -> String {
        match self {
            Self::AlreadyRunning { instance_id } => {
                format!("shared memory pump already running for instance {instance_id}")
            }
            Self::NotRunning { instance_id } => {
                format!("shared memory pump is not running for instance {instance_id}")
            }
            Self::NotAttached { instance_id } => {
                format!("shared memory stream is not attached for instance {instance_id}")
            }
            Self::StreamClosed { instance_id } => {
                format!("stream is closed for instance {instance_id}")
            }
            Self::InstanceNotProcessing { instance_id } => {
                format!("instance {instance_id} is not processing")
            }
            Self::InvalidFrames { frames, max_frames } => {
                format!("invalid pump frames {frames}; maxBlockFrames is {max_frames}")
            }
            Self::InvalidInterval => "intervalMicros must be non-zero".to_string(),
            Self::InvalidSchedulingWindow {
                min_micros,
                max_micros,
            } => format!(
                "invalid adaptive pump interval window: minIntervalMicros {min_micros} exceeds maxIntervalMicros {max_micros}"
            ),
            Self::InvalidMaxQueuedEvents { max_queued_events } => format!(
                "invalid maxQueuedEvents {max_queued_events}; supported range is 1..={MAX_QUEUED_EVENTS_LIMIT}"
            ),
            Self::InvalidEventTarget => {
                "provide either targetIteration or delayIterations, not both".to_string()
            }
            Self::EmptyEventBatch => {
                "shared memory pump event batch must include at least one event".to_string()
            }
            Self::EventBatchExceedsCapacity {
                requested_events,
                max_queued_events,
            } => format!(
                "shared memory pump event batch has {requested_events} events; maxQueuedEvents is {max_queued_events}"
            ),
            Self::EventQueueCapacityExceeded {
                queued_events,
                requested_events,
                max_queued_events,
            } => format!(
                "shared memory pump event queue capacity exceeded: queued {queued_events}, requested {requested_events}, maxQueuedEvents {max_queued_events}"
            ),
            Self::RegistryUnavailable => "shared memory pump registry unavailable".to_string(),
        }
    }

    pub fn rpc_data(&self) -> Value {
        match self {
            Self::AlreadyRunning { instance_id } => {
                json!({ "kind": "shared-memory-pump-already-running", "instanceId": instance_id })
            }
            Self::NotRunning { instance_id } => {
                json!({ "kind": "shared-memory-pump-not-running", "instanceId": instance_id })
            }
            Self::NotAttached { instance_id } => {
                json!({ "kind": "shared-memory-not-attached", "instanceId": instance_id })
            }
            Self::StreamClosed { instance_id } => {
                json!({ "kind": "stream-closed", "instanceId": instance_id })
            }
            Self::InstanceNotProcessing { instance_id } => {
                json!({ "kind": "instance-not-processing", "instanceId": instance_id })
            }
            Self::InvalidFrames { frames, max_frames } => {
                json!({ "kind": "invalid-pump-frames", "frames": frames, "maxFrames": max_frames })
            }
            Self::InvalidInterval => json!({ "kind": "invalid-pump-interval" }),
            Self::InvalidSchedulingWindow {
                min_micros,
                max_micros,
            } => json!({
                "kind": "invalid-pump-scheduling-window",
                "minIntervalMicros": min_micros,
                "maxIntervalMicros": max_micros,
            }),
            Self::InvalidMaxQueuedEvents { max_queued_events } => json!({
                "kind": "invalid-pump-max-queued-events",
                "maxQueuedEvents": max_queued_events,
                "limit": MAX_QUEUED_EVENTS_LIMIT,
            }),
            Self::InvalidEventTarget => json!({ "kind": "invalid-pump-event-target" }),
            Self::EmptyEventBatch => json!({ "kind": "empty-pump-event-batch" }),
            Self::EventBatchExceedsCapacity {
                requested_events,
                max_queued_events,
            } => json!({
                "kind": "pump-event-batch-exceeds-capacity",
                "requestedEvents": requested_events,
                "maxQueuedEvents": max_queued_events,
            }),
            Self::EventQueueCapacityExceeded {
                queued_events,
                requested_events,
                max_queued_events,
            } => json!({
                "kind": "pump-event-queue-capacity-exceeded",
                "queuedEvents": queued_events,
                "requestedEvents": requested_events,
                "maxQueuedEvents": max_queued_events,
            }),
            Self::RegistryUnavailable => {
                json!({ "kind": "shared-memory-pump-registry-unavailable" })
            }
        }
    }
}

impl From<SharedMemoryPumpEventQueueError> for SharedMemoryPumpError {
    fn from(error: SharedMemoryPumpEventQueueError) -> Self {
        match error {
            SharedMemoryPumpEventQueueError::EmptyBatch => Self::EmptyEventBatch,
            SharedMemoryPumpEventQueueError::BatchExceedsCapacity {
                requested_events,
                max_queued_events,
            } => Self::EventBatchExceedsCapacity {
                requested_events,
                max_queued_events,
            },
            SharedMemoryPumpEventQueueError::CapacityExceeded {
                queued_events,
                requested_events,
                max_queued_events,
            } => Self::EventQueueCapacityExceeded {
                queued_events,
                requested_events,
                max_queued_events,
            },
        }
    }
}
