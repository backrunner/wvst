use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};
use wvst_shm_mmap::{SharedAudioMmap, SharedAudioMmapError};
use wvst_shm_transport::{SharedAudioLayoutError, SharedAudioRingIoReport};

use super::{WorkerInstance, WorkerIpcState, response_error, response_error_data, response_result};

const SHARED_MEMORY_ERROR_INVALID: i64 = 4220;
const SHARED_MEMORY_ERROR_NOT_FOUND: i64 = 4040;
const SHARED_MEMORY_ERROR_UNAVAILABLE: i64 = 4094;

pub(super) struct WorkerSharedMemoryStream {
    path: PathBuf,
    mmap: SharedAudioMmap,
    input_samples: Vec<f32>,
}

impl WorkerSharedMemoryStream {
    pub(super) fn diagnostics(&self) -> Value {
        json!({
            "schemaVersion": 1,
            "transport": "file-backed-mmap",
            "path": self.path.display().to_string(),
            "layout": self.mmap.layout(),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedMemoryAttachParams {
    instance_id: u64,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedMemoryInstanceParams {
    instance_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedMemoryProcessParams {
    instance_id: u64,
    #[serde(default)]
    frames: Option<u16>,
}

pub(super) fn handle_stream_shared_memory_attach(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<SharedMemoryAttachParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid shared memory attach params: {error}"),
            );
        }
    };
    let Some(instance) = state.instances.get_mut(&params.instance_id) else {
        return response_error(
            id,
            SHARED_MEMORY_ERROR_NOT_FOUND,
            format!("instance not found: {}", params.instance_id),
        );
    };

    let path = PathBuf::from(&params.path);
    let mmap = match SharedAudioMmap::open(&path) {
        Ok(mmap) => mmap,
        Err(error) => return response_mmap_error(id, "open", error),
    };
    if let Err(error) = validate_layout(instance, &mmap) {
        return response_shared_memory_process_error(id, error);
    }
    let input_samples = match input_sample_capacity(instance) {
        Ok(input_samples) => vec![0.0; input_samples],
        Err(error) => return response_shared_memory_process_error(id, error),
    };
    instance.shared_memory = Some(WorkerSharedMemoryStream {
        path,
        mmap,
        input_samples,
    });

    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "streamId": instance.stream_id,
            "attached": true,
            "sharedMemory": instance.shared_memory.as_ref().map(WorkerSharedMemoryStream::diagnostics),
        }),
    )
}

pub(super) fn handle_stream_shared_memory_detach(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<SharedMemoryInstanceParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid shared memory detach params: {error}"),
            );
        }
    };
    let Some(instance) = state.instances.get_mut(&params.instance_id) else {
        return response_error(
            id,
            SHARED_MEMORY_ERROR_NOT_FOUND,
            format!("instance not found: {}", params.instance_id),
        );
    };
    let detached = instance.shared_memory.take().is_some();

    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "streamId": instance.stream_id,
            "detached": detached,
        }),
    )
}

pub(super) fn handle_stream_shared_memory_process(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    let params = match serde_json::from_value::<SharedMemoryProcessParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid shared memory process params: {error}"),
            );
        }
    };
    let Some(instance) = state.instances.get_mut(&params.instance_id) else {
        return response_error(
            id,
            SHARED_MEMORY_ERROR_NOT_FOUND,
            format!("instance not found: {}", params.instance_id),
        );
    };

    let frames = params.frames.unwrap_or(instance.max_block_frames);
    match process_shared_memory_once(params.instance_id, instance, frames) {
        Ok(report) => response_result(id, json!(report)),
        Err(error) => response_shared_memory_process_error(id, error),
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SharedMemoryProcessReport {
    instance_id: u64,
    stream_id: u64,
    frames: u16,
    input: SharedAudioRingIoReport,
    output: SharedAudioRingIoReport,
    output_event_count: usize,
    output_parameter_change_count: usize,
}

fn process_shared_memory_once(
    instance_id: u64,
    instance: &mut WorkerInstance,
    frames: u16,
) -> Result<SharedMemoryProcessReport, SharedMemoryProcessError> {
    if frames == 0 {
        return Err(SharedMemoryProcessError::invalid(
            "frames must be greater than zero",
        ));
    }
    if frames > instance.max_block_frames {
        return Err(SharedMemoryProcessError::invalid(format!(
            "frame count exceeds max block size: max {}, got {frames}",
            instance.max_block_frames
        )));
    }
    if !instance.processing {
        return Err(SharedMemoryProcessError::unavailable(format!(
            "instance for stream {} is not processing",
            instance.stream_id
        )));
    }
    if !instance.backend.supports_binary_audio_process() {
        return Err(SharedMemoryProcessError::invalid(format!(
            "backend {:?} does not expose binary audio processing yet",
            instance.backend.kind()
        )));
    }

    let Some(shared_memory) = instance.shared_memory.as_mut() else {
        return Err(SharedMemoryProcessError::unavailable(
            "shared memory stream is not attached",
        ));
    };
    let layout = shared_memory.mmap.layout().clone();
    ensure_output_writable(&layout, shared_memory.mmap.memory(), u64::from(frames))?;

    let input_len = usize::from(frames)
        .checked_mul(instance.input_channels)
        .ok_or_else(|| SharedMemoryProcessError::invalid("input sample length overflow"))?;
    let input_samples = &mut shared_memory.input_samples[..input_len];
    let input_report = layout.input.read_interleaved_f32(
        shared_memory.mmap.memory_mut(),
        input_samples,
        u64::from(frames),
    )?;

    let (input, output) = instance
        .buffers
        .prepare_process_samples(usize::from(frames), input_samples)
        .map_err(SharedMemoryProcessError::invalid)?;
    instance.events.clear();
    instance.parameter_changes.clear();
    instance
        .backend
        .process_interleaved_f32(
            usize::from(frames),
            input,
            &instance.events,
            &instance.parameter_changes,
            output,
            &mut instance.process_output,
        )
        .map_err(SharedMemoryProcessError::backend)?;
    let output_report = layout.output.write_interleaved_f32(
        shared_memory.mmap.memory_mut(),
        output,
        u64::from(frames),
    )?;

    Ok(SharedMemoryProcessReport {
        instance_id,
        stream_id: instance.stream_id,
        frames,
        input: input_report,
        output: output_report,
        output_event_count: instance.process_output.events.len(),
        output_parameter_change_count: instance.process_output.parameter_changes.len(),
    })
}

fn ensure_output_writable(
    layout: &wvst_shm_transport::SharedAudioTransportLayout,
    memory: &[u8],
    frames: u64,
) -> Result<(), SharedMemoryProcessError> {
    let cursor = layout.output.cursor_state(memory)?;
    let available = layout.output.writable_frames(cursor.cursor())?;
    if frames > available {
        return Err(SharedMemoryProcessError::unavailable(format!(
            "shared memory output ring is full: requested {frames}, available {available}"
        )));
    }
    Ok(())
}

fn validate_layout(
    instance: &WorkerInstance,
    mmap: &SharedAudioMmap,
) -> Result<(), SharedMemoryProcessError> {
    let config = mmap.layout().config;
    if config.sample_rate_hz != instance.sample_rate {
        return Err(SharedMemoryProcessError::invalid(format!(
            "shared memory sample rate mismatch: expected {}, got {}",
            instance.sample_rate, config.sample_rate_hz
        )));
    }
    if config.block_frames != instance.max_block_frames {
        return Err(SharedMemoryProcessError::invalid(format!(
            "shared memory block size mismatch: expected {}, got {}",
            instance.max_block_frames, config.block_frames
        )));
    }
    if usize::from(config.input_channels) != instance.input_channels {
        return Err(SharedMemoryProcessError::invalid(format!(
            "shared memory input channel mismatch: expected {}, got {}",
            instance.input_channels, config.input_channels
        )));
    }
    if usize::from(config.output_channels) != instance.output_channels {
        return Err(SharedMemoryProcessError::invalid(format!(
            "shared memory output channel mismatch: expected {}, got {}",
            instance.output_channels, config.output_channels
        )));
    }
    Ok(())
}

fn input_sample_capacity(instance: &WorkerInstance) -> Result<usize, SharedMemoryProcessError> {
    usize::from(instance.max_block_frames)
        .checked_mul(instance.input_channels)
        .ok_or_else(|| SharedMemoryProcessError::invalid("input sample capacity overflow"))
}

#[derive(Debug)]
struct SharedMemoryProcessError {
    code: i64,
    stage: &'static str,
    message: String,
    data: Option<Value>,
}

impl SharedMemoryProcessError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_INVALID,
            stage: "validate",
            message: message.into(),
            data: None,
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_UNAVAILABLE,
            stage: "process",
            message: message.into(),
            data: None,
        }
    }

    fn backend(error: super::WorkerBackendError) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_INVALID,
            stage: "backend-process",
            message: error.message().to_string(),
            data: error.data().cloned(),
        }
    }
}

impl From<SharedAudioLayoutError> for SharedMemoryProcessError {
    fn from(error: SharedAudioLayoutError) -> Self {
        Self {
            code: SHARED_MEMORY_ERROR_INVALID,
            stage: "shared-memory-layout",
            message: error.to_string(),
            data: None,
        }
    }
}

fn response_mmap_error(id: Value, stage: &'static str, error: SharedAudioMmapError) -> String {
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

fn response_shared_memory_process_error(id: Value, error: SharedMemoryProcessError) -> String {
    let mut data = json!({
        "kind": "shared-memory-audio",
        "stage": error.stage,
    });
    if let (Some(map), Some(worker_data)) = (data.as_object_mut(), error.data) {
        map.insert("workerData".to_string(), worker_data);
    }
    response_error_data(id, error.code, error.message, data)
}

#[cfg(test)]
#[path = "ipc_shared_memory_tests.rs"]
mod tests;
