use serde_json::{Value, json};

use super::super::{
    WorkerSupervisor, WorkerSupervisorError, worker_supervisor_batch::WorkerBatchRequest,
};

impl WorkerSupervisor {
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

    pub async fn parameter_info(
        &self,
        instance_id: u64,
        parameter_id: u32,
        value_normalized: Option<f64>,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameter.info",
            json!({
                "instanceId": instance_id,
                "parameterId": parameter_id,
                "valueNormalized": value_normalized,
            }),
        )
        .await
    }

    pub async fn parameter_value_by_string(
        &self,
        instance_id: u64,
        parameter_id: u32,
        value: String,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameter.valueByString",
            json!({
                "instanceId": instance_id,
                "parameterId": parameter_id,
                "value": value,
            }),
        )
        .await
    }

    pub async fn parameter_normalized_by_plain(
        &self,
        instance_id: u64,
        parameter_id: u32,
        value_plain: f64,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameter.normalizedByPlain",
            json!({
                "instanceId": instance_id,
                "parameterId": parameter_id,
                "valuePlain": value_plain,
            }),
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

    pub async fn parameter_begin_edit(
        &self,
        instance_id: u64,
        parameter_id: u32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameter.beginEdit",
            json!({
                "instanceId": instance_id,
                "parameterId": parameter_id,
            }),
        )
        .await
    }

    pub async fn parameter_perform_edit(
        &self,
        instance_id: u64,
        parameter_id: u32,
        value_normalized: f64,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameter.performEdit",
            json!({
                "instanceId": instance_id,
                "parameterId": parameter_id,
                "valueNormalized": value_normalized,
            }),
        )
        .await
    }

    pub async fn parameter_end_edit(
        &self,
        instance_id: u64,
        parameter_id: u32,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.parameter.endEdit",
            json!({
                "instanceId": instance_id,
                "parameterId": parameter_id,
            }),
        )
        .await
    }

    pub async fn parameter_edit(
        &self,
        instance_id: u64,
        parameter_id: u32,
        value_normalized: f64,
    ) -> Result<Value, WorkerSupervisorError> {
        let results = self
            .instance_batch_request(
                instance_id,
                vec![
                    WorkerBatchRequest::new(
                        "instance.parameter.beginEdit",
                        json!({
                            "instanceId": instance_id,
                            "parameterId": parameter_id,
                        }),
                    ),
                    WorkerBatchRequest::new(
                        "instance.parameter.performEdit",
                        json!({
                            "instanceId": instance_id,
                            "parameterId": parameter_id,
                            "valueNormalized": value_normalized,
                        }),
                    ),
                    WorkerBatchRequest::new(
                        "instance.parameter.endEdit",
                        json!({
                            "instanceId": instance_id,
                            "parameterId": parameter_id,
                        }),
                    ),
                ],
            )
            .await?;

        Ok(json!({
            "instanceId": instance_id,
            "parameterId": parameter_id,
            "valueNormalized": value_normalized,
            "beginEdit": results.first().cloned().unwrap_or(Value::Null),
            "performEdit": results.get(1).cloned().unwrap_or(Value::Null),
            "endEdit": results.get(2).cloned().unwrap_or(Value::Null),
        }))
    }
}
