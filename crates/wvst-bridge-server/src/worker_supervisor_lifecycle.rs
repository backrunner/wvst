use serde_json::{Value, json};

use super::{WorkerSupervisor, WorkerSupervisorError, worker_supervisor_batch::WorkerBatchRequest};

impl WorkerSupervisor {
    pub async fn start_processing(&self, instance_id: u64) -> Result<Value, WorkerSupervisorError> {
        self.processing_request(instance_id, "instance.startProcessing")
            .await
    }

    pub async fn stop_processing(&self, instance_id: u64) -> Result<Value, WorkerSupervisorError> {
        self.processing_request(instance_id, "instance.stopProcessing")
            .await
    }

    pub async fn attach_shared_memory(
        &self,
        instance_id: u64,
        path: String,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "stream.sharedMemory.attach",
            json!({ "instanceId": instance_id, "path": path }),
        )
        .await
    }

    pub async fn detach_shared_memory(
        &self,
        instance_id: u64,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "stream.sharedMemory.detach",
            json!({ "instanceId": instance_id }),
        )
        .await
    }

    pub async fn process_shared_memory(
        &self,
        instance_id: u64,
        frames: Option<u16>,
        midi_events: Vec<Value>,
        parameter_events: Vec<Value>,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "stream.sharedMemory.process",
            json!({
                "instanceId": instance_id,
                "frames": frames,
                "midiEvents": midi_events,
                "parameterEvents": parameter_events,
            }),
        )
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

    pub async fn metadata_refresh(
        &self,
        instance_id: u64,
        include_state: bool,
        include_worker_metrics: bool,
    ) -> Result<Value, WorkerSupervisorError> {
        let mut requests = vec![
            WorkerBatchRequest::new("instance.parameters", json!({ "instanceId": instance_id })),
            WorkerBatchRequest::new("instance.units", json!({ "instanceId": instance_id })),
        ];
        let state_index = if include_state {
            let index = requests.len();
            requests.push(WorkerBatchRequest::new(
                "instance.getState",
                json!({ "instanceId": instance_id }),
            ));
            Some(index)
        } else {
            None
        };
        let worker_index = if include_worker_metrics {
            let index = requests.len();
            requests.push(WorkerBatchRequest::new("worker.metrics", json!({})));
            Some(index)
        } else {
            None
        };

        let results = self.instance_batch_request(instance_id, requests).await?;
        Ok(metadata_refresh_value(
            instance_id,
            &results,
            0,
            1,
            state_index,
            worker_index,
        ))
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
        component_state_base64: Option<String>,
        controller_state_base64: Option<String>,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.setState",
            json!({
                "instanceId": instance_id,
                "componentStateBase64": component_state_base64,
                "controllerStateBase64": controller_state_base64,
            }),
        )
        .await
    }

    pub async fn set_state_and_refresh(
        &self,
        instance_id: u64,
        component_state_base64: Option<String>,
        controller_state_base64: Option<String>,
        include_state: bool,
        include_worker_metrics: bool,
    ) -> Result<Value, WorkerSupervisorError> {
        self.write_then_refresh(
            instance_id,
            WorkerBatchRequest::new(
                "instance.setState",
                json!({
                    "instanceId": instance_id,
                    "componentStateBase64": component_state_base64,
                    "controllerStateBase64": controller_state_base64,
                }),
            ),
            "setState",
            include_state,
            include_worker_metrics,
        )
        .await
    }

    pub async fn notify_component(
        &self,
        instance_id: u64,
        message_id: String,
        attributes: Value,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.connection.notifyComponent",
            json!({ "instanceId": instance_id, "messageId": message_id, "attributes": attributes }),
        )
        .await
    }

    pub async fn notify_component_and_refresh(
        &self,
        instance_id: u64,
        message_id: String,
        attributes: Value,
        include_state: bool,
        include_worker_metrics: bool,
    ) -> Result<Value, WorkerSupervisorError> {
        self.write_then_refresh(
            instance_id,
            WorkerBatchRequest::new(
                "instance.connection.notifyComponent",
                json!({ "instanceId": instance_id, "messageId": message_id, "attributes": attributes }),
            ),
            "notifyComponent",
            include_state,
            include_worker_metrics,
        )
        .await
    }

    pub async fn notify_controller(
        &self,
        instance_id: u64,
        message_id: String,
        attributes: Value,
    ) -> Result<Value, WorkerSupervisorError> {
        self.instance_request(
            instance_id,
            "instance.connection.notifyController",
            json!({ "instanceId": instance_id, "messageId": message_id, "attributes": attributes }),
        )
        .await
    }

    pub async fn notify_controller_and_refresh(
        &self,
        instance_id: u64,
        message_id: String,
        attributes: Value,
        include_state: bool,
        include_worker_metrics: bool,
    ) -> Result<Value, WorkerSupervisorError> {
        self.write_then_refresh(
            instance_id,
            WorkerBatchRequest::new(
                "instance.connection.notifyController",
                json!({ "instanceId": instance_id, "messageId": message_id, "attributes": attributes }),
            ),
            "notifyController",
            include_state,
            include_worker_metrics,
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

    pub async fn set_unit_program_data_and_refresh(
        &self,
        instance_id: u64,
        list_or_unit_id: i32,
        program_index: i32,
        data_base64: String,
        include_state: bool,
        include_worker_metrics: bool,
    ) -> Result<Value, WorkerSupervisorError> {
        self.write_then_refresh(
            instance_id,
            WorkerBatchRequest::new(
                "instance.setUnitProgramData",
                json!({
                    "instanceId": instance_id,
                    "listOrUnitId": list_or_unit_id,
                    "programIndex": program_index,
                    "dataBase64": data_base64,
                }),
            ),
            "setUnitProgramData",
            include_state,
            include_worker_metrics,
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

    pub async fn set_program_data_and_refresh(
        &self,
        instance_id: u64,
        list_id: i32,
        program_index: i32,
        data_base64: String,
        include_state: bool,
        include_worker_metrics: bool,
    ) -> Result<Value, WorkerSupervisorError> {
        self.write_then_refresh(
            instance_id,
            WorkerBatchRequest::new(
                "instance.programData.set",
                json!({
                    "instanceId": instance_id,
                    "listId": list_id,
                    "programIndex": program_index,
                    "dataBase64": data_base64,
                }),
            ),
            "setProgramData",
            include_state,
            include_worker_metrics,
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

    pub async fn set_unit_data_and_refresh(
        &self,
        instance_id: u64,
        unit_id: i32,
        data_base64: String,
        include_state: bool,
        include_worker_metrics: bool,
    ) -> Result<Value, WorkerSupervisorError> {
        self.write_then_refresh(
            instance_id,
            WorkerBatchRequest::new(
                "instance.unitData.set",
                json!({
                    "instanceId": instance_id,
                    "unitId": unit_id,
                    "dataBase64": data_base64,
                }),
            ),
            "setUnitData",
            include_state,
            include_worker_metrics,
        )
        .await
    }

    async fn write_then_refresh(
        &self,
        instance_id: u64,
        write_request: WorkerBatchRequest,
        write_result_key: &'static str,
        include_state: bool,
        include_worker_metrics: bool,
    ) -> Result<Value, WorkerSupervisorError> {
        let mut requests = vec![
            write_request,
            WorkerBatchRequest::new("instance.parameters", json!({ "instanceId": instance_id })),
            WorkerBatchRequest::new("instance.units", json!({ "instanceId": instance_id })),
        ];
        let state_index = if include_state {
            let index = requests.len();
            requests.push(WorkerBatchRequest::new(
                "instance.getState",
                json!({ "instanceId": instance_id }),
            ));
            Some(index)
        } else {
            None
        };
        let worker_index = if include_worker_metrics {
            let index = requests.len();
            requests.push(WorkerBatchRequest::new("worker.metrics", json!({})));
            Some(index)
        } else {
            None
        };

        let results = self.instance_batch_request(instance_id, requests).await?;
        let metadata =
            metadata_refresh_value(instance_id, &results, 1, 2, state_index, worker_index);
        let mut output = serde_json::Map::new();
        output.insert("instanceId".to_string(), json!(instance_id));
        output.insert(
            write_result_key.to_string(),
            results.first().cloned().unwrap_or(Value::Null),
        );
        output.insert("metadata".to_string(), metadata);

        Ok(Value::Object(output))
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

fn metadata_refresh_value(
    instance_id: u64,
    results: &[Value],
    parameters_index: usize,
    units_index: usize,
    state_index: Option<usize>,
    worker_index: Option<usize>,
) -> Value {
    let parameters = results
        .get(parameters_index)
        .and_then(|result| result.get("parameters"))
        .cloned()
        .unwrap_or(Value::Null);
    let unit_info = results
        .get(units_index)
        .and_then(|result| result.get("unitInfo"))
        .cloned()
        .unwrap_or(Value::Null);
    let state = state_index
        .and_then(|index| results.get(index))
        .cloned()
        .unwrap_or(Value::Null);
    let worker = worker_index
        .and_then(|index| results.get(index))
        .cloned()
        .unwrap_or(Value::Null);

    json!({
        "instanceId": instance_id,
        "parameters": parameters,
        "unitInfo": unit_info,
        "state": state,
        "worker": worker,
    })
}
