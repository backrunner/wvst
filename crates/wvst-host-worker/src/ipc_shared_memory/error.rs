use serde_json::{Value, json};
use wvst_shm_mmap::SharedAudioMmapError;
use wvst_shm_transport::SharedAudioLayoutError;

use super::super::{WorkerBackendError, response_error_data};

pub(super) const SHARED_MEMORY_ERROR_INVALID: i64 = 4220;
pub(super) const SHARED_MEMORY_ERROR_NOT_FOUND: i64 = 4040;
pub(super) const SHARED_MEMORY_ERROR_UNAVAILABLE: i64 = 4094;

#[derive(Debug)]
pub(super) struct SharedMemoryProcessError {
    code: i64,
    stage: &'static str,
    reason: &'static str,
    message: String,
    data: Option<Value>,
}

impl SharedMemoryProcessError {
    pub(super) fn invalid(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_INVALID,
            stage: "validate",
            reason,
            message: message.into(),
            data: None,
        }
    }

    pub(super) fn invalid_with_data(
        reason: &'static str,
        message: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_INVALID,
            stage: "validate",
            reason,
            message: message.into(),
            data: Some(data),
        }
    }

    pub(super) fn unavailable(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_UNAVAILABLE,
            stage: "process",
            reason,
            message: message.into(),
            data: None,
        }
    }

    pub(super) fn backend(error: WorkerBackendError) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_INVALID,
            stage: "backend-process",
            reason: "backend-process",
            message: error.message().to_string(),
            data: error.data().cloned(),
        }
    }

    pub(super) fn layout(stage: &'static str, error: SharedAudioLayoutError) -> Self {
        let (code, reason, data) = match error {
            SharedAudioLayoutError::InsufficientReadableFrames {
                requested,
                available,
            } => (
                SHARED_MEMORY_ERROR_UNAVAILABLE,
                "input-underrun",
                Some(ring_error_data("input-underrun", requested, available)),
            ),
            SharedAudioLayoutError::InsufficientWritableFrames {
                requested,
                available,
            } => (
                SHARED_MEMORY_ERROR_UNAVAILABLE,
                "output-backpressure",
                Some(ring_error_data("output-backpressure", requested, available)),
            ),
            ref error => (
                SHARED_MEMORY_ERROR_INVALID,
                "layout-error",
                Some(json!({
                    "kind": "shared-audio-layout",
                    "message": error.to_string(),
                })),
            ),
        };
        Self {
            code,
            stage,
            reason,
            message: error.to_string(),
            data,
        }
    }

    pub(super) fn ring_unavailable(reason: &'static str, requested: u64, available: u64) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_UNAVAILABLE,
            stage: "output-ring-write",
            reason,
            message: format!(
                "shared memory output ring is full: requested {requested}, available {available}"
            ),
            data: Some(ring_error_data(reason, requested, available)),
        }
    }
}

impl From<SharedAudioLayoutError> for SharedMemoryProcessError {
    fn from(error: SharedAudioLayoutError) -> Self {
        Self::layout("shared-memory-layout", error)
    }
}

pub(super) fn response_mmap_error(
    id: Value,
    stage: &'static str,
    error: SharedAudioMmapError,
) -> String {
    response_error_data(
        id,
        SHARED_MEMORY_ERROR_INVALID,
        error.to_string(),
        json!({
            "kind": "shared-memory-mmap",
            "stage": stage,
            "message": error.to_string(),
        }),
    )
}

pub(super) fn response_shared_memory_process_error(
    id: Value,
    error: SharedMemoryProcessError,
) -> String {
    let mut data = json!({
        "kind": "shared-memory-audio",
        "stage": error.stage,
        "reason": error.reason,
    });
    if let (Some(map), Some(worker_data)) = (data.as_object_mut(), error.data) {
        map.insert("workerData".to_string(), worker_data);
    }
    response_error_data(id, error.code, error.message, data)
}

fn ring_error_data(reason: &'static str, requested: u64, available: u64) -> Value {
    json!({
        "kind": "shared-audio-ring",
        "reason": reason,
        "requestedFrames": requested,
        "availableFrames": available,
    })
}
