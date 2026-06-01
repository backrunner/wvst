use serde_json::{Value, json};

use super::{WorkerSupervisor, WorkerSupervisorError};

impl WorkerSupervisor {
    pub async fn start_processing(&self, instance_id: u64) -> Result<Value, WorkerSupervisorError> {
        self.processing_request(instance_id, "instance.startProcessing")
            .await
    }

    pub async fn stop_processing(&self, instance_id: u64) -> Result<Value, WorkerSupervisorError> {
        self.processing_request(instance_id, "instance.stopProcessing")
            .await
    }

    async fn processing_request(
        &self,
        instance_id: u64,
        method: &'static str,
    ) -> Result<Value, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.get(&instance_id).cloned() else {
            return Err(WorkerSupervisorError::WorkerMissing { instance_id });
        };

        let result = process
            .lock()
            .await
            .request(method, json!({ "instanceId": instance_id }), self.timeout)
            .await;

        match result {
            Ok(result) => Ok(result),
            Err(error) => {
                process.lock().await.shutdown().await;
                self.remove_process_if_same(instance_id, &process).await;
                Err(error)
            }
        }
    }
}
