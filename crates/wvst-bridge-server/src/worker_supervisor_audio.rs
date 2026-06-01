use serde::Deserialize;
use serde_json::{Value, json};

use super::{WorkerSupervisor, WorkerSupervisorError};

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerAudioProcessResult {
    pub stream_id: u64,
    pub input_channels: usize,
    pub output_channels: usize,
    pub output: Vec<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerAudioProcessResponse {
    stream_id: u64,
    input_channels: usize,
    output_channels: usize,
    output: Vec<f32>,
}

impl WorkerSupervisor {
    pub async fn process_interleaved_f32(
        &self,
        instance_id: u64,
        frames: u16,
        input: Vec<f32>,
    ) -> Result<WorkerAudioProcessResult, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.get(&instance_id).cloned() else {
            return Err(WorkerSupervisorError::WorkerMissing { instance_id });
        };

        let mut process_guard = process.lock().await;
        let response = process_guard
            .request(
                "debug.processInterleavedF32",
                json!({
                    "instanceId": instance_id,
                    "frames": frames,
                    "input": input,
                }),
                self.timeout,
            )
            .await;

        match response {
            Ok(value) => parse_process_response(value, &process_guard.stderr.snapshot().await),
            Err(error) => {
                process_guard.shutdown().await;
                drop(process_guard);
                self.remove_process_if_same(instance_id, &process).await;
                Err(error)
            }
        }
    }
}

fn parse_process_response(
    value: Value,
    stderr: &str,
) -> Result<WorkerAudioProcessResult, WorkerSupervisorError> {
    let response =
        serde_json::from_value::<WorkerAudioProcessResponse>(value).map_err(|error| {
            WorkerSupervisorError::Protocol {
                message: format!("invalid debug audio process response: {error}"),
                stderr: stderr.to_string(),
            }
        })?;

    Ok(WorkerAudioProcessResult {
        stream_id: response.stream_id,
        input_channels: response.input_channels,
        output_channels: response.output_channels,
        output: response.output,
    })
}
