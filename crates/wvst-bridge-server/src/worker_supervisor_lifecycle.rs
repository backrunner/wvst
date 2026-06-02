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

    pub async fn parameters(&self, instance_id: u64) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameters",
            json!({ "instanceId": instance_id }),
        )
        .await
    }

    pub async fn parameter_get(
        &self,
        instance_id: u64,
        parameter_id: u32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameter.get",
            json!({ "instanceId": instance_id, "parameterId": parameter_id }),
        )
        .await
    }

    pub async fn parameter_set(
        &self,
        instance_id: u64,
        parameter_id: u32,
        value_normalized: f64,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameter.set",
            json!({
                "instanceId": instance_id,
                "parameterId": parameter_id,
                "valueNormalized": value_normalized,
            }),
        )
        .await
    }

    pub async fn get_state(&self, instance_id: u64) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.getState",
            json!({ "instanceId": instance_id }),
        )
        .await
    }

    pub async fn set_state(
        &self,
        instance_id: u64,
        state_base64: String,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.setState",
            json!({ "instanceId": instance_id, "stateBase64": state_base64 }),
        )
        .await
    }

    async fn processing_request(
        &self,
        instance_id: u64,
        method: &'static str,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(instance_id, method, json!({ "instanceId": instance_id }))
            .await
    }

    async fn instance_request(
        &self,
        instance_id: u64,
        method: &'static str,
        params: Value,
    ) -> Result<Value, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.get(&instance_id).cloned() else {
            return Err(WorkerSupervisorError::WorkerMissing { instance_id });
        };

        let result = process
            .lock()
            .await
            .request(method, params, self.timeout)
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
