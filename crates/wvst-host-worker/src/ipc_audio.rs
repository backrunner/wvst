use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;

use wvst_core::ChannelCount;
use wvst_protocol::{
    AUDIO_FRAME_HEADER_LEN, AudioFrameHeader, MIDI_EVENT_LEN, MidiEvent,
    WORKER_AUDIO_IPC_HEADER_LEN, WorkerAudioIpcHeader, WorkerAudioMessageKind,
};

use super::WorkerIpcState;

#[cfg(test)]
use wvst_protocol::WorkerAudioIpcMessage;

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
                response_body.extend_from_slice(error.message.as_bytes());
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
    let audio_payload_len = input_header
        .audio_payload_len()
        .map_err(|error| AudioProcessError::invalid(error.to_string()))?
        as usize;
    let audio_payload_end = AUDIO_FRAME_HEADER_LEN
        .checked_add(audio_payload_len)
        .ok_or_else(|| AudioProcessError::invalid("audio payload length overflow"))?;
    validate_midi_events(input_header, &message_body[audio_payload_end..])?;

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
    let (input, output) = instance
        .buffers
        .prepare_process(
            frames,
            &message_body[AUDIO_FRAME_HEADER_LEN..audio_payload_end],
        )
        .map_err(AudioProcessError::invalid)?;
    instance
        .backend
        .process_interleaved_f32(frames, input, output)
        .map_err(AudioProcessError::invalid)?;

    encode_output_frame_into(input_header, output_channels, output, output_body)
}

fn validate_midi_events(
    input_header: AudioFrameHeader,
    event_payload: &[u8],
) -> Result<(), AudioProcessError> {
    let expected_event_bytes = usize::from(input_header.event_count)
        .checked_mul(MIDI_EVENT_LEN)
        .ok_or_else(|| AudioProcessError::invalid("MIDI event payload length overflow"))?;
    if event_payload.len() != expected_event_bytes {
        return Err(AudioProcessError::invalid(format!(
            "MIDI event payload length mismatch: expected {expected_event_bytes}, got {}",
            event_payload.len()
        )));
    }

    for chunk in event_payload.chunks_exact(MIDI_EVENT_LEN) {
        MidiEvent::decode(chunk).map_err(|error| AudioProcessError::invalid(error.to_string()))?;
    }

    Ok(())
}

fn encode_output_frame_into(
    input_header: AudioFrameHeader,
    output_channels: usize,
    output: &[f32],
    destination: &mut Vec<u8>,
) -> Result<(), AudioProcessError> {
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
    let header = WorkerAudioIpcHeader::new(kind, status_code, sequence, body_len);
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
}

impl AudioProcessError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            status_code: AUDIO_ERROR_INVALID_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status_code: AUDIO_ERROR_NOT_FOUND,
            message: message.into(),
        }
    }
}

#[cfg(test)]
#[path = "ipc_audio_tests.rs"]
mod tests;
