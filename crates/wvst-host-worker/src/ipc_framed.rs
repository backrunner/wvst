use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use wvst_protocol::{
    WORKER_CONTROL_IPC_HEADER_LEN, WORKER_CONTROL_IPC_MAX_BODY_LEN, WorkerControlIpcBatch,
    WorkerControlIpcBatchRole, WorkerControlIpcHeader, WorkerControlIpcMessage,
    WorkerControlMessageKind,
};

use super::{WorkerIpcState, handle_ipc_line, ipc_audio};

enum FramedControlRequest {
    Single {
        sequence: u64,
        request: String,
    },
    Batch {
        sequence: u64,
        requests: Vec<(u64, String)>,
    },
}

pub fn serve_framed_stdio(audio_connect: Option<String>) -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    serve_framed_with_audio(stdin.lock(), stdout.lock(), audio_connect)
}

#[cfg(test)]
pub(super) fn serve_framed(reader: impl Read, writer: impl Write) -> Result<(), String> {
    serve_framed_with_audio(reader, writer, None)
}

fn serve_framed_with_audio(
    mut reader: impl Read,
    mut writer: impl Write,
    audio_connect: Option<String>,
) -> Result<(), String> {
    let state = Arc::new(Mutex::new(WorkerIpcState::default()));
    if let Some(address) = audio_connect {
        ipc_audio::spawn_audio_thread(address, Arc::clone(&state));
    }

    while let Some(request) = read_control_request(&mut reader)? {
        match request {
            FramedControlRequest::Single { sequence, request } => {
                let response = {
                    let mut state = state
                        .lock()
                        .map_err(|_| "worker state mutex poisoned".to_string())?;
                    handle_ipc_line(&request, &mut state)
                };
                write_control_response(&mut writer, sequence, response)?;
            }
            FramedControlRequest::Batch { sequence, requests } => {
                let responses = {
                    let mut state = state
                        .lock()
                        .map_err(|_| "worker state mutex poisoned".to_string())?;
                    requests
                        .into_iter()
                        .map(|(child_sequence, request)| {
                            control_response_message(
                                child_sequence,
                                handle_ipc_line(&request, &mut state),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?
                };
                write_control_batch_response(&mut writer, sequence, responses)?;
            }
        }
    }

    Ok(())
}

fn read_control_request(reader: &mut impl Read) -> Result<Option<FramedControlRequest>, String> {
    let mut header_bytes = [0; WORKER_CONTROL_IPC_HEADER_LEN];
    match reader.read_exact(&mut header_bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.to_string()),
    }
    let header =
        WorkerControlIpcHeader::decode(&header_bytes).map_err(|error| error.to_string())?;
    if !matches!(
        header.kind,
        WorkerControlMessageKind::Request | WorkerControlMessageKind::BatchRequest
    ) {
        return Err(format!(
            "invalid control request frame kind: {:?}",
            header.kind
        ));
    }
    if header.status_code != 0 {
        return Err(format!(
            "invalid control request status code: {}",
            header.status_code
        ));
    }
    if header.body_len > WORKER_CONTROL_IPC_MAX_BODY_LEN {
        return Err(format!(
            "control request body too large: max {}, got {}",
            WORKER_CONTROL_IPC_MAX_BODY_LEN, header.body_len
        ));
    }
    let body_len = usize::try_from(header.body_len).map_err(|_| "control body too large")?;
    let mut body = vec![0; body_len];
    reader
        .read_exact(&mut body)
        .map_err(|error| error.to_string())?;

    if header.kind == WorkerControlMessageKind::BatchRequest {
        return read_control_batch_request(header.sequence, &body).map(Some);
    }

    String::from_utf8(body)
        .map(|request| {
            Some(FramedControlRequest::Single {
                sequence: header.sequence,
                request,
            })
        })
        .map_err(|error| format!("invalid control request utf8: {error}"))
}

fn read_control_batch_request(sequence: u64, body: &[u8]) -> Result<FramedControlRequest, String> {
    let batch = WorkerControlIpcBatch::decode_body(WorkerControlIpcBatchRole::Request, body)
        .map_err(|error| error.to_string())?;
    let requests = batch
        .into_messages()
        .into_iter()
        .map(|message| {
            String::from_utf8(message.body)
                .map(|request| (message.header.sequence, request))
                .map_err(|error| format!("invalid control batch request utf8: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FramedControlRequest::Batch { sequence, requests })
}

fn write_control_response(
    writer: &mut impl Write,
    sequence: u64,
    response: String,
) -> Result<(), String> {
    let message = control_response_message(sequence, response)?;
    let frame = message.encode().map_err(|error| error.to_string())?;
    writer
        .write_all(&frame)
        .map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

fn write_control_batch_response(
    writer: &mut impl Write,
    sequence: u64,
    responses: Vec<WorkerControlIpcMessage>,
) -> Result<(), String> {
    let message = WorkerControlIpcBatch::response_frame(sequence, responses)
        .map_err(|error| error.to_string())?;
    let frame = message.encode().map_err(|error| error.to_string())?;
    writer
        .write_all(&frame)
        .map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

fn control_response_message(
    sequence: u64,
    response: String,
) -> Result<WorkerControlIpcMessage, String> {
    let status_code = control_response_status_code(&response);
    let body = response.into_bytes();
    match status_code {
        Some(status_code) => WorkerControlIpcMessage::error(sequence, status_code, body),
        None => WorkerControlIpcMessage::response(sequence, body),
    }
    .map_err(|error| error.to_string())
}

#[cfg(test)]
pub(super) fn control_response_status_code(response: &str) -> Option<u16> {
    control_response_status_code_inner(response)
}

#[cfg(not(test))]
fn control_response_status_code(response: &str) -> Option<u16> {
    control_response_status_code_inner(response)
}

fn control_response_status_code_inner(response: &str) -> Option<u16> {
    let value = serde_json::from_str::<Value>(response).ok()?;
    let code = value.get("error")?.get("code")?.as_i64()?;
    Some(json_rpc_code_to_status_code(code))
}

fn json_rpc_code_to_status_code(code: i64) -> u16 {
    u16::try_from(code)
        .ok()
        .filter(|code| *code != 0)
        .unwrap_or(1)
}
