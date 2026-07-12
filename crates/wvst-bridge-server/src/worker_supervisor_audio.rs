use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use wvst_protocol::{
    AudioFrameHeader, WORKER_AUDIO_IPC_HEADER_LEN, WORKER_AUDIO_IPC_MAX_BODY_LEN,
    WorkerAudioIpcHeader, WorkerAudioIpcMessage, WorkerAudioMessageKind,
};

use super::{WorkerSupervisor, WorkerSupervisorError};

#[derive(Debug)]
pub(super) struct WorkerAudioConnection {
    stream: TcpStream,
    request_buffer: Vec<u8>,
}

impl WorkerAudioConnection {
    pub(super) fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            request_buffer: Vec::with_capacity(WORKER_AUDIO_IPC_HEADER_LEN),
        }
    }
}

impl WorkerSupervisor {
    pub async fn process_audio_frame(
        &self,
        instance_id: u64,
        frame: &[u8],
        expected_output_channels: u16,
    ) -> Result<Vec<u8>, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.get(&instance_id).cloned() else {
            return Err(WorkerSupervisorError::WorkerMissing { instance_id });
        };

        let mut process_guard = process.lock().await;
        if process_guard.audio.is_none() {
            return Err(WorkerSupervisorError::AudioIpcUnavailable { instance_id });
        }

        let request_header = AudioFrameHeader::decode(frame)
            .map_err(|error| process_guard.protocol_error(error.to_string()))?;
        let response = process_guard
            .audio
            .as_mut()
            .expect("checked audio connection")
            .process_frame(
                request_header,
                frame,
                expected_output_channels,
                self.timeout,
            )
            .await;

        match response {
            Ok(frame) => Ok(frame),
            Err(error) if error.is_audio_request_rejection() => Err(error),
            Err(error) => {
                let audit = process_guard.shutdown().await;
                self.record_shutdown(audit);
                drop(process_guard);
                self.remove_process_if_same(instance_id, &process).await;
                Err(error)
            }
        }
    }
}

impl WorkerAudioConnection {
    async fn process_frame(
        &mut self,
        request_header: AudioFrameHeader,
        frame: &[u8],
        expected_output_channels: u16,
        timeout_duration: std::time::Duration,
    ) -> Result<Vec<u8>, WorkerSupervisorError> {
        let sequence = request_header.sequence;
        encode_audio_process_request(&mut self.request_buffer, sequence, frame)?;

        timeout(
            timeout_duration,
            self.stream.write_all(&self.request_buffer),
        )
        .await
        .map_err(|_| WorkerSupervisorError::Timeout {
            method: "audio.processFrame",
            timeout_ms: timeout_duration.as_millis(),
            stderr: String::new(),
        })?
        .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;
        timeout(timeout_duration, self.stream.flush())
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method: "audio.processFrame",
                timeout_ms: timeout_duration.as_millis(),
                stderr: String::new(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;

        let response = self.read_response(timeout_duration).await?;
        if response.header.sequence != sequence {
            return Err(WorkerSupervisorError::Protocol {
                message: format!(
                    "audio response sequence mismatch: expected {sequence}, got {}",
                    response.header.sequence
                ),
                stderr: String::new(),
            });
        }

        match response.header.kind {
            WorkerAudioMessageKind::ProcessResponse => {
                validate_audio_response_frame(
                    &response.body,
                    request_header,
                    expected_output_channels,
                )?;
                Ok(response.body)
            }
            WorkerAudioMessageKind::ProcessError => {
                let error = decode_process_error_body(&response.body)?;
                Err(WorkerSupervisorError::WorkerRejected {
                    code: i64::from(response.header.status_code),
                    message: error.message,
                    data: error.data,
                    stderr: String::new(),
                })
            }
            WorkerAudioMessageKind::ProcessRequest => Err(WorkerSupervisorError::Protocol {
                message: "worker returned request on response path".to_string(),
                stderr: String::new(),
            }),
        }
    }

    async fn read_response(
        &mut self,
        timeout_duration: std::time::Duration,
    ) -> Result<WorkerAudioIpcMessage, WorkerSupervisorError> {
        let mut header_bytes = [0; WORKER_AUDIO_IPC_HEADER_LEN];
        timeout(timeout_duration, self.stream.read_exact(&mut header_bytes))
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method: "audio.processFrame",
                timeout_ms: timeout_duration.as_millis(),
                stderr: String::new(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;

        let header = WorkerAudioIpcHeader::decode(&header_bytes).map_err(|error| {
            WorkerSupervisorError::Protocol {
                message: error.to_string(),
                stderr: String::new(),
            }
        })?;
        validate_audio_response_body_len(header.body_len)?;
        let mut body = vec![0; header.body_len as usize];
        timeout(timeout_duration, self.stream.read_exact(&mut body))
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method: "audio.processFrame",
                timeout_ms: timeout_duration.as_millis(),
                stderr: String::new(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;

        Ok(WorkerAudioIpcMessage { header, body })
    }
}

fn validate_audio_response_body_len(body_len: u32) -> Result<(), WorkerSupervisorError> {
    if body_len > WORKER_AUDIO_IPC_MAX_BODY_LEN {
        return Err(WorkerSupervisorError::Protocol {
            message: format!(
                "audio response body too large: max {}, got {}",
                WORKER_AUDIO_IPC_MAX_BODY_LEN, body_len
            ),
            stderr: String::new(),
        });
    }

    Ok(())
}

fn validate_audio_response_frame(
    frame: &[u8],
    request: AudioFrameHeader,
    expected_output_channels: u16,
) -> Result<(), WorkerSupervisorError> {
    let response =
        AudioFrameHeader::decode(frame).map_err(|error| WorkerSupervisorError::Protocol {
            message: format!("invalid audio response frame: {error}"),
            stderr: String::new(),
        })?;
    let expected_len = wvst_protocol::AUDIO_FRAME_HEADER_LEN
        .checked_add(response.payload_len as usize)
        .ok_or_else(|| WorkerSupervisorError::Protocol {
            message: "audio response frame length overflow".to_string(),
            stderr: String::new(),
        })?;
    if frame.len() != expected_len
        || response.stream_id != request.stream_id
        || response.sequence != request.sequence
        || response.sent_frame_time != request.sent_frame_time
        || response.sample_rate != request.sample_rate
        || response.frames != request.frames
        || response.channels.get() != expected_output_channels
    {
        return Err(WorkerSupervisorError::Protocol {
            message: "audio response frame does not match request or instance configuration"
                .to_string(),
            stderr: String::new(),
        });
    }
    Ok(())
}

fn encode_audio_process_request(
    buffer: &mut Vec<u8>,
    sequence: u64,
    frame: &[u8],
) -> Result<(), WorkerSupervisorError> {
    let body_len = u32::try_from(frame.len()).map_err(|_| WorkerSupervisorError::Protocol {
        message: "audio request body too large".to_string(),
        stderr: String::new(),
    })?;
    let header = WorkerAudioIpcMessage::header(
        WorkerAudioMessageKind::ProcessRequest,
        0,
        sequence,
        body_len,
    )
    .map_err(|error| WorkerSupervisorError::Protocol {
        message: error.to_string(),
        stderr: String::new(),
    })?;

    buffer.clear();
    buffer.resize(WORKER_AUDIO_IPC_HEADER_LEN, 0);
    header
        .encode(&mut buffer[..WORKER_AUDIO_IPC_HEADER_LEN])
        .map_err(|error| WorkerSupervisorError::Protocol {
            message: error.to_string(),
            stderr: String::new(),
        })?;
    buffer.extend_from_slice(frame);

    Ok(())
}

#[derive(Debug, Deserialize)]
struct WorkerAudioErrorBody {
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct DecodedWorkerAudioError {
    pub(super) message: String,
    pub(super) data: Option<Value>,
}

pub(super) fn decode_process_error_body(
    body: &[u8],
) -> Result<DecodedWorkerAudioError, WorkerSupervisorError> {
    serde_json::from_slice::<WorkerAudioErrorBody>(body)
        .map(|error| DecodedWorkerAudioError {
            message: error.message,
            data: error.data,
        })
        .map_err(|error| WorkerSupervisorError::Protocol {
            message: format!("invalid audio process error body: {error}"),
            stderr: String::new(),
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn decodes_structured_process_error_body() {
        let body = json!({
            "message": "VST3 process failed",
            "data": {
                "kind": "vst3-runtime-process",
                "stage": "component.process",
            }
        })
        .to_string();

        let error = decode_process_error_body(body.as_bytes()).expect("structured body");

        assert_eq!(error.message, "VST3 process failed");
        assert_eq!(
            error.data.expect("worker data")["kind"],
            "vst3-runtime-process"
        );
    }

    #[test]
    fn classifies_invalid_audio_requests_as_nonfatal_rejections() {
        let error = WorkerSupervisorError::WorkerRejected {
            code: 4220,
            message: "sample rate mismatch".to_string(),
            data: Some(serde_json::json!({
                "schemaVersion": 1,
                "kind": "audio-request-invalid"
            })),
            stderr: String::new(),
        };

        assert!(error.is_audio_request_rejection());
    }

    #[test]
    fn rejects_malformed_process_error_body() {
        let error = decode_process_error_body(b"process error").expect_err("malformed body");

        assert!(matches!(error, WorkerSupervisorError::Protocol { .. }));
        assert!(
            error
                .rpc_message()
                .contains("invalid audio process error body")
        );
    }

    #[test]
    fn rejects_oversized_audio_response_bodies_before_allocation() {
        let error = validate_audio_response_body_len(WORKER_AUDIO_IPC_MAX_BODY_LEN + 1)
            .expect_err("oversized body");

        assert!(matches!(error, WorkerSupervisorError::Protocol { .. }));
        assert!(error.rpc_message().contains("body too large"));
    }

    #[test]
    fn encodes_audio_process_requests_into_reused_buffer() {
        let frame = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut buffer = Vec::with_capacity(128);

        encode_audio_process_request(&mut buffer, 42, &frame).expect("encode");
        let capacity = buffer.capacity();
        let header =
            WorkerAudioIpcHeader::decode(&buffer[..WORKER_AUDIO_IPC_HEADER_LEN]).expect("header");
        assert_eq!(header.kind, WorkerAudioMessageKind::ProcessRequest);
        assert_eq!(header.sequence, 42);
        assert_eq!(header.body_len, frame.len() as u32);
        assert_eq!(&buffer[WORKER_AUDIO_IPC_HEADER_LEN..], frame.as_slice());

        encode_audio_process_request(&mut buffer, 43, &frame).expect("encode again");

        assert_eq!(buffer.capacity(), capacity);
        let header =
            WorkerAudioIpcHeader::decode(&buffer[..WORKER_AUDIO_IPC_HEADER_LEN]).expect("header");
        assert_eq!(header.sequence, 43);
    }
}
