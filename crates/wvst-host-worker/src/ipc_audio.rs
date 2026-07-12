use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{Value, json};
use wvst_core::ChannelCount;
use wvst_protocol::{
    AUDIO_FRAME_HEADER_LEN, AudioFrameHeader, WORKER_AUDIO_IPC_HEADER_LEN,
    WORKER_AUDIO_IPC_MAX_BODY_LEN, WorkerAudioIpcHeader, WorkerAudioIpcMessage,
    WorkerAudioMessageKind,
};

use super::WorkerIpcState;
use super::ipc_event_ordering::{
    sort_advanced_output_events_by_sample_offset, sort_output_events_by_sample_offset,
    sort_parameter_changes_by_sample_offset,
};
use super::ipc_midi::{decode_midi_events_into, encode_midi_events_into};
use super::ipc_parameter_events::{decode_parameter_events_into, encode_parameter_events_into};
use super::ipc_vst3_events::encode_vst3_output_events_into;

const AUDIO_ERROR_INVALID_REQUEST: u16 = 4220;
const AUDIO_ERROR_NOT_FOUND: u16 = 4040;

pub(super) fn spawn_audio_thread(address: String, state: Arc<Mutex<WorkerIpcState>>) {
    thread::spawn(move || {
        if let Err(error) = serve_audio_connection(&address, state) {
            eprintln!("worker audio IPC stopped: {error}");
        }
    });
}

fn serve_audio_connection(address: &str, state: Arc<Mutex<WorkerIpcState>>) -> Result<(), String> {
    let mut stream = TcpStream::connect(address).map_err(|error| error.to_string())?;
    let mut request_body = Vec::new();
    let mut response_body = Vec::new();

    loop {
        let header = match read_message_into(&mut stream, &mut request_body)? {
            Some(header) => header,
            None => return Ok(()),
        };
        let sequence = header.sequence;

        match process_message_into(header, &request_body, &state, &mut response_body) {
            Ok(()) => write_ipc_message(
                &mut stream,
                WorkerAudioMessageKind::ProcessResponse,
                sequence,
                0,
                &response_body,
            )?,
            Err(error) => {
                response_body.clear();
                response_body.extend_from_slice(error.body().as_bytes());
                write_ipc_message(
                    &mut stream,
                    WorkerAudioMessageKind::ProcessError,
                    sequence,
                    error.status_code,
                    &response_body,
                )?;
            }
        }
    }
}

#[cfg(test)]
fn process_message(
    message: WorkerAudioIpcMessage,
    state: &Arc<Mutex<WorkerIpcState>>,
) -> Result<Vec<u8>, AudioProcessError> {
    let mut output = Vec::new();
    process_message_into(message.header, &message.body, state, &mut output)?;
    Ok(output)
}

fn process_message_into(
    message_header: WorkerAudioIpcHeader,
    message_body: &[u8],
    state: &Arc<Mutex<WorkerIpcState>>,
    output_body: &mut Vec<u8>,
) -> Result<(), AudioProcessError> {
    output_body.clear();

    if message_header.kind != WorkerAudioMessageKind::ProcessRequest {
        return Err(AudioProcessError::invalid(format!(
            "unexpected worker audio message kind: {:?}",
            message_header.kind
        )));
    }

    let input_header = AudioFrameHeader::decode(message_body)
        .map_err(|error| AudioProcessError::invalid(error.to_string()))?;
    let expected_len = AUDIO_FRAME_HEADER_LEN
        .checked_add(input_header.payload_len as usize)
        .ok_or_else(|| AudioProcessError::invalid("audio frame length overflow"))?;
    if message_body.len() != expected_len {
        return Err(AudioProcessError::invalid(format!(
            "audio frame length mismatch: expected {expected_len}, got {}",
            message_body.len()
        )));
    }
    if input_header.vst3_output_event_count != 0 {
        return Err(AudioProcessError::invalid(
            "VST3 output event section is only valid in worker responses",
        ));
    }
    let audio_payload_len = input_header
        .audio_payload_len()
        .map_err(|error| AudioProcessError::invalid(error.to_string()))?
        as usize;
    let audio_payload_end = AUDIO_FRAME_HEADER_LEN
        .checked_add(audio_payload_len)
        .ok_or_else(|| AudioProcessError::invalid("audio payload length overflow"))?;
    let midi_payload_len = input_header
        .midi_event_payload_len()
        .map_err(|error| AudioProcessError::invalid(error.to_string()))?
        as usize;
    let midi_payload_end = audio_payload_end
        .checked_add(midi_payload_len)
        .ok_or_else(|| AudioProcessError::invalid("MIDI payload length overflow"))?;
    let parameter_payload_len = input_header
        .parameter_event_payload_len()
        .map_err(|error| AudioProcessError::invalid(error.to_string()))?
        as usize;
    let parameter_payload_end = midi_payload_end
        .checked_add(parameter_payload_len)
        .ok_or_else(|| AudioProcessError::invalid("parameter payload length overflow"))?;
    if parameter_payload_end != message_body.len() {
        return Err(AudioProcessError::invalid(format!(
            "audio frame section length mismatch: expected {parameter_payload_end}, got {}",
            message_body.len()
        )));
    }
    let midi_payload = &message_body[audio_payload_end..midi_payload_end];
    let parameter_payload = &message_body[midi_payload_end..parameter_payload_end];

    let mut state = state
        .lock()
        .map_err(|_| AudioProcessError::invalid("worker state mutex poisoned"))?;
    let instance = state
        .instances
        .values_mut()
        .find(|instance| instance.stream_id == input_header.stream_id.get())
        .ok_or_else(|| {
            AudioProcessError::not_found(format!(
                "stream not found: {}",
                input_header.stream_id.get()
            ))
        })?;

    if !instance.processing {
        return Err(AudioProcessError::invalid(format!(
            "instance for stream {} is not processing",
            input_header.stream_id.get()
        )));
    }

    if !instance.backend.supports_binary_audio_process() {
        return Err(AudioProcessError::invalid(format!(
            "backend {:?} does not expose binary audio processing yet",
            instance.backend.kind()
        )));
    }

    if input_header.sample_rate.get() != instance.sample_rate {
        return Err(AudioProcessError::invalid(format!(
            "sample rate mismatch: expected {}, got {}",
            instance.sample_rate,
            input_header.sample_rate.get()
        )));
    }

    if input_header.frames.get() > instance.max_block_frames {
        return Err(AudioProcessError::invalid(format!(
            "frame count exceeds max block size: max {}, got {}",
            instance.max_block_frames,
            input_header.frames.get()
        )));
    }

    if usize::from(input_header.channels.get()) != instance.input_channels {
        return Err(AudioProcessError::invalid(format!(
            "input channel mismatch: expected {}, got {}",
            instance.input_channels,
            input_header.channels.get()
        )));
    }

    let frames = usize::from(input_header.frames.get());
    let output_channels = instance.output_channels;
    decode_parameter_events_into(
        input_header,
        parameter_payload,
        frames,
        &mut instance.parameter_changes,
    )
    .map_err(AudioProcessError::invalid)?;
    {
        let backend = &instance.backend;
        let events = &mut instance.events;
        let parameter_changes = &mut instance.parameter_changes;
        decode_midi_events_into(
            input_header,
            midi_payload,
            frames,
            events,
            parameter_changes,
            |channel, controller| backend.midi_controller_param_id(channel, controller),
        )
        .map_err(AudioProcessError::invalid)?;
    }
    let (input, output) = instance
        .buffers
        .prepare_process(
            frames,
            &message_body[AUDIO_FRAME_HEADER_LEN..audio_payload_end],
        )
        .map_err(AudioProcessError::invalid)?;
    instance
        .backend
        .process_interleaved_f32(
            frames,
            input,
            &instance.events,
            &instance.parameter_changes,
            output,
            &mut instance.process_output,
        )
        .map_err(AudioProcessError::from_backend_error)?;
    encode_output_frame_into(
        input_header,
        output_channels,
        output,
        &mut instance.process_output.events,
        &mut instance.process_output.advanced_events,
        &mut instance.process_output.parameter_changes,
        output_body,
    )
}

fn encode_output_frame_into(
    input_header: AudioFrameHeader,
    output_channels: usize,
    output: &[f32],
    events: &mut [wvst_vst3_host::Vst3OutputEvent],
    advanced_events: &mut [wvst_vst3_host::Vst3AdvancedOutputEvent],
    parameter_changes: &mut [wvst_vst3_host::Vst3ParameterChange],
    destination: &mut Vec<u8>,
) -> Result<(), AudioProcessError> {
    sort_output_events_by_sample_offset(events);
    sort_advanced_output_events_by_sample_offset(advanced_events);
    sort_parameter_changes_by_sample_offset(parameter_changes);

    let output_channels = u16::try_from(output_channels)
        .map_err(|_| AudioProcessError::invalid("output channel count overflows u16"))?;
    let output_channels = ChannelCount::new(output_channels)
        .map_err(|error| AudioProcessError::invalid(error.to_string()))?;
    let header = AudioFrameHeader::new_f32(
        input_header.stream_id,
        input_header.sequence,
        input_header.sent_frame_time,
        input_header.sample_rate,
        input_header.frames,
        output_channels,
        input_header.flags,
    )
    .map_err(|error| AudioProcessError::invalid(error.to_string()))?
    .with_all_event_counts(
        u16::try_from(events.len())
            .map_err(|_| AudioProcessError::invalid("MIDI output event count overflows u16"))?,
        u16::try_from(parameter_changes.len()).map_err(|_| {
            AudioProcessError::invalid("parameter output event count overflows u16")
        })?,
        u16::try_from(advanced_events.len())
            .map_err(|_| AudioProcessError::invalid("VST3 output event count overflows u16"))?,
    )
    .map_err(|error| AudioProcessError::invalid(error.to_string()))?;
    let frame_len = AUDIO_FRAME_HEADER_LEN + header.payload_len as usize;
    destination.resize(frame_len, 0);
    header
        .encode(&mut destination[..AUDIO_FRAME_HEADER_LEN])
        .map_err(|error| AudioProcessError::invalid(error.to_string()))?;

    let mut offset = AUDIO_FRAME_HEADER_LEN;
    for sample in output {
        destination[offset..offset + 4].copy_from_slice(&sample.to_le_bytes());
        offset += 4;
    }
    let midi_end = offset
        + header.midi_event_payload_len().map_err(|error| {
            AudioProcessError::invalid(format!("MIDI output event payload length failed: {error}"))
        })? as usize;
    encode_midi_events_into(events, &mut destination[offset..midi_end])
        .map_err(AudioProcessError::invalid)?;
    offset = midi_end;

    let parameter_end = offset
        + header.parameter_event_payload_len().map_err(|error| {
            AudioProcessError::invalid(format!(
                "parameter output event payload length failed: {error}"
            ))
        })? as usize;
    encode_parameter_events_into(parameter_changes, &mut destination[offset..parameter_end])
        .map_err(AudioProcessError::invalid)?;
    offset = parameter_end;

    let vst3_event_end = offset
        + header.vst3_output_event_payload_len().map_err(|error| {
            AudioProcessError::invalid(format!("VST3 output event payload length failed: {error}"))
        })? as usize;
    encode_vst3_output_events_into(advanced_events, &mut destination[offset..vst3_event_end])
        .map_err(AudioProcessError::invalid)?;

    Ok(())
}

#[cfg(test)]
fn read_f32_payload(payload: &[u8]) -> Result<Vec<f32>, AudioProcessError> {
    if payload.len() % 4 != 0 {
        return Err(AudioProcessError::invalid(
            "f32 payload is not 4-byte aligned",
        ));
    }

    Ok(payload
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn read_message_into(
    stream: &mut TcpStream,
    body: &mut Vec<u8>,
) -> Result<Option<WorkerAudioIpcHeader>, String> {
    let mut header_bytes = [0; WORKER_AUDIO_IPC_HEADER_LEN];
    match stream.read_exact(&mut header_bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.to_string()),
    }

    let header = WorkerAudioIpcHeader::decode(&header_bytes).map_err(|error| error.to_string())?;
    if header.body_len > WORKER_AUDIO_IPC_MAX_BODY_LEN {
        return Err(format!(
            "worker audio body too large: max {}, got {}",
            WORKER_AUDIO_IPC_MAX_BODY_LEN, header.body_len
        ));
    }
    body.resize(header.body_len as usize, 0);
    stream.read_exact(body).map_err(|error| error.to_string())?;

    Ok(Some(header))
}

fn write_ipc_message(
    stream: &mut TcpStream,
    kind: WorkerAudioMessageKind,
    sequence: u64,
    status_code: u16,
    body: &[u8],
) -> Result<(), String> {
    let body_len = u32::try_from(body.len()).map_err(|_| "worker audio body too large")?;
    let header = WorkerAudioIpcMessage::header(kind, status_code, sequence, body_len)
        .map_err(|error| error.to_string())?;
    let mut header_bytes = [0; WORKER_AUDIO_IPC_HEADER_LEN];
    header
        .encode(&mut header_bytes)
        .map_err(|error| error.to_string())?;
    stream
        .write_all(&header_bytes)
        .map_err(|error| error.to_string())?;
    stream.write_all(body).map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())
}

#[derive(Debug)]
struct AudioProcessError {
    status_code: u16,
    message: String,
    data: Option<Value>,
}

impl AudioProcessError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            status_code: AUDIO_ERROR_INVALID_REQUEST,
            message: message.into(),
            data: Some(json!({
                "schemaVersion": 1,
                "kind": "audio-request-invalid",
            })),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status_code: AUDIO_ERROR_NOT_FOUND,
            message: message.into(),
            data: Some(json!({
                "schemaVersion": 1,
                "kind": "audio-stream-not-found",
            })),
        }
    }

    fn from_backend_error(error: super::ipc_backend_error::WorkerBackendError) -> Self {
        Self {
            status_code: AUDIO_ERROR_INVALID_REQUEST,
            message: error.message().to_string(),
            data: error.data().cloned(),
        }
    }

    fn body(&self) -> String {
        json!({
            "message": self.message,
            "data": self.data,
        })
        .to_string()
    }
}

#[cfg(test)]
#[path = "ipc_audio_advanced_vst3_tests.rs"]
mod advanced_vst3_tests;

#[cfg(test)]
#[path = "ipc_audio_legacy_midi_tests.rs"]
mod legacy_midi_tests;

#[cfg(test)]
#[path = "ipc_audio_tests.rs"]
mod tests;
