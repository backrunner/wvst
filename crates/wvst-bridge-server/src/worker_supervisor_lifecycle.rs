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

    pub async fn units(&self, instance_id: u64) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.units",
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
        state_base64: Option<String>,
        component_state_base64: Option<String>,
        controller_state_base64: Option<String>,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.setState",
            json!({
                "instanceId": instance_id,
                "stateBase64": state_base64,
                "componentStateBase64": component_state_base64,
                "controllerStateBase64": controller_state_base64,
            }),
        )
        .await
    }

    pub async fn select_unit(
        &self,
        instance_id: u64,
        unit_id: i32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.selectUnit",
            json!({ "instanceId": instance_id, "unitId": unit_id }),
        )
        .await
    }

    pub async fn unit_by_bus(
        &self,
        instance_id: u64,
        direction: String,
        bus_index: i32,
        channel: i32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.unitByBus",
            json!({
                "instanceId": instance_id,
                "direction": direction,
                "busIndex": bus_index,
                "channel": channel,
            }),
        )
        .await
    }

    pub async fn set_unit_program_data(
        &self,
        instance_id: u64,
        list_or_unit_id: i32,
        program_index: i32,
        data_base64: String,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.setUnitProgramData",
            json!({
                "instanceId": instance_id,
                "listOrUnitId": list_or_unit_id,
                "programIndex": program_index,
                "dataBase64": data_base64,
            }),
        )
        .await
    }

    pub async fn program_data_supported(
        &self,
        instance_id: u64,
        list_id: i32,
        program_index: i32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.programData.supported",
            json!({
                "instanceId": instance_id,
                "listId": list_id,
                "programIndex": program_index,
            }),
        )
        .await
    }

    pub async fn get_program_data(
        &self,
        instance_id: u64,
        list_id: i32,
        program_index: i32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.programData.get",
            json!({
                "instanceId": instance_id,
                "listId": list_id,
                "programIndex": program_index,
            }),
        )
        .await
    }

    pub async fn set_program_data(
        &self,
        instance_id: u64,
        list_id: i32,
        program_index: i32,
        data_base64: String,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.programData.set",
            json!({
                "instanceId": instance_id,
                "listId": list_id,
                "programIndex": program_index,
                "dataBase64": data_base64,
            }),
        )
        .await
    }

    pub async fn unit_data_supported(
        &self,
        instance_id: u64,
        unit_id: i32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.unitData.supported",
            json!({ "instanceId": instance_id, "unitId": unit_id }),
        )
        .await
    }

    pub async fn get_unit_data(
        &self,
        instance_id: u64,
        unit_id: i32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.unitData.get",
            json!({ "instanceId": instance_id, "unitId": unit_id }),
        )
        .await
    }

    pub async fn set_unit_data(
        &self,
        instance_id: u64,
        unit_id: i32,
        data_base64: String,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.unitData.set",
            json!({
                "instanceId": instance_id,
                "unitId": unit_id,
                "dataBase64": data_base64,
            }),
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
                let audit = process.lock().await.shutdown().await;
                self.record_shutdown(audit);
                self.remove_process_if_same(instance_id, &process).await;
                Err(error)
            }
        }
    }
}
