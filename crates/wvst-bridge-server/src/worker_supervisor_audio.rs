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
}

impl WorkerAudioConnection {
    pub(super) fn new(stream: TcpStream) -> Self {
        Self { stream }
    }
}

impl WorkerSupervisor {
    pub async fn process_audio_frame(
        &self,
        instance_id: u64,
        frame: Vec<u8>,
    ) -> Result<Vec<u8>, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.get(&instance_id).cloned() else {
            return Err(WorkerSupervisorError::WorkerMissing { instance_id });
        };

        let mut process_guard = process.lock().await;
        if process_guard.audio.is_none() {
            return Err(WorkerSupervisorError::AudioIpcUnavailable { instance_id });
        }

        let sequence = AudioFrameHeader::decode(&frame)
            .map_err(|error| process_guard.protocol_error(error.to_string()))?
            .sequence;
        let response = process_guard
            .audio
            .as_mut()
            .expect("checked audio connection")
            .process_frame(sequence, frame, self.timeout)
            .await;

        match response {
            Ok(frame) => Ok(frame),
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
        sequence: u64,
        frame: Vec<u8>,
        timeout_duration: std::time::Duration,
    ) -> Result<Vec<u8>, WorkerSupervisorError> {
        let request = WorkerAudioIpcMessage::process_request(sequence, frame).map_err(|error| {
            WorkerSupervisorError::Protocol {
                message: error.to_string(),
                stderr: String::new(),
            }
        })?;
        let bytes = request
            .encode()
            .map_err(|error| WorkerSupervisorError::Protocol {
                message: error.to_string(),
                stderr: String::new(),
            })?;

        timeout(timeout_duration, self.stream.write_all(&bytes))
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
            WorkerAudioMessageKind::ProcessResponse => Ok(response.body),
            WorkerAudioMessageKind::ProcessError => {
                let error = decode_process_error_body(&response.body);
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

pub(super) fn decode_process_error_body(body: &[u8]) -> DecodedWorkerAudioError {
    match serde_json::from_slice::<WorkerAudioErrorBody>(body) {
        Ok(error) => DecodedWorkerAudioError {
            message: error.message,
            data: error.data,
        },
        Err(_) => DecodedWorkerAudioError {
            message: String::from_utf8_lossy(body).into_owned(),
            data: None,
        },
    }
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

        let error = decode_process_error_body(body.as_bytes());

        assert_eq!(error.message, "VST3 process failed");
        assert_eq!(
            error.data.expect("worker data")["kind"],
            "vst3-runtime-process"
        );
    }

    #[test]
    fn keeps_legacy_text_process_error_body() {
        let error = decode_process_error_body(b"legacy process error");

        assert_eq!(error.message, "legacy process error");
        assert_eq!(error.data, None);
    }

    #[test]
    fn rejects_oversized_audio_response_bodies_before_allocation() {
        let error = validate_audio_response_body_len(WORKER_AUDIO_IPC_MAX_BODY_LEN + 1)
            .expect_err("oversized body");

        assert!(matches!(error, WorkerSupervisorError::Protocol { .. }));
        assert!(error.rpc_message().contains("body too large"));
    }
}
