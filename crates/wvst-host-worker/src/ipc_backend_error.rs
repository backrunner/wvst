use serde_json::Value;
use wvst_vst3_host::HostError;

use crate::vst3_error_data::vst3_error_data;

#[derive(Debug)]
pub(super) struct WorkerBackendError {
    message: String,
    data: Option<Value>,
}

impl WorkerBackendError {
    pub(super) fn plain(error: impl std::fmt::Display) -> Self {
        Self {
            message: error.to_string(),
            data: None,
        }
    }

    pub(super) fn vst3_init(stage: &'static str, error: HostError) -> Self {
        Self::vst3_error("vst3-runtime-init", stage, error)
    }

    pub(super) fn vst3_process(stage: &'static str, error: HostError) -> Self {
        Self::vst3_error("vst3-runtime-process", stage, error)
    }

    pub(super) fn vst3_control(stage: &'static str, error: HostError) -> Self {
        Self::vst3_error("vst3-runtime-control", stage, error)
    }

    pub(super) fn message(&self) -> &str {
        &self.message
    }

    pub(super) fn data(&self) -> Option<&Value> {
        self.data.as_ref()
    }

    fn vst3_error(kind: &'static str, stage: &'static str, error: HostError) -> Self {
        let message = error.to_string();
        Self {
            message: message.clone(),
            data: Some(vst3_error_data(kind, stage, &error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_vst3_control_errors_with_host_error_kind() {
        let error = WorkerBackendError::vst3_control(
            "controller.perform-edit",
            HostError::EditControllerParameterEditNotActive { parameter_id: 42 },
        );
        let data = error.data().expect("structured data");

        assert_eq!(data["kind"], "vst3-runtime-control");
        assert_eq!(data["stage"], "controller.perform-edit");
        assert_eq!(
            data["hostError"],
            "edit-controller-parameter-edit-not-active"
        );
        assert_eq!(data["compatibility"]["schemaVersion"], 1);
        assert_eq!(data["compatibility"]["category"], "parameter-edit-gesture");
        assert!(error.message().contains("parameter=42"));
    }
}
