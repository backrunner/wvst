use serde_json::{Value, json};
use wvst_vst3_host::HostError;

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
            data: Some(json!({
                "kind": kind,
                "stage": stage,
                "hostError": host_error_kind(&error),
                "message": message,
            })),
        }
    }
}

fn host_error_kind(error: &HostError) -> &'static str {
    match error {
        HostError::AudioProcessorCallFailed { .. } => "audio-processor-call-failed",
        HostError::AudioProcessorReturnedNull => "audio-processor-returned-null",
        HostError::AudioProcessorVTableMissing => "audio-processor-vtable-missing",
        HostError::BundleExecutableNotFound(_) => "bundle-executable-not-found",
        HostError::ComponentCallFailed { .. } => "component-call-failed",
        HostError::ComponentReturnedNull => "component-returned-null",
        HostError::ComponentVTableMissing => "component-vtable-missing",
        HostError::ConnectionPointCallFailed { .. } => "connection-point-call-failed",
        HostError::ConnectionPointMessageNull => "connection-point-message-null",
        HostError::ConnectionPointReturnedNull => "connection-point-returned-null",
        HostError::ConnectionPointVTableMissing => "connection-point-vtable-missing",
        HostError::EditControllerCallFailed { .. } => "edit-controller-call-failed",
        HostError::EditControllerParameterEditAlreadyActive { .. } => {
            "edit-controller-parameter-edit-already-active"
        }
        HostError::EditControllerParameterEditNotActive { .. } => {
            "edit-controller-parameter-edit-not-active"
        }
        HostError::EditControllerReturnedNull => "edit-controller-returned-null",
        HostError::EditControllerVTableMissing => "edit-controller-vtable-missing",
        HostError::FactoryCallFailed { .. } => "factory-call-failed",
        HostError::FactoryReturnedNull => "factory-returned-null",
        HostError::InstanceCreationFailed { .. } => "instance-creation-failed",
        HostError::InstanceReturnedNull { .. } => "instance-returned-null",
        HostError::InvalidClassId(_) => "invalid-class-id",
        HostError::InvalidLifecycleTransition { .. } => "invalid-lifecycle-transition",
        HostError::InterfaceQueryFailed { .. } => "interface-query-failed",
        HostError::InterfaceReturnedNull { .. } => "interface-returned-null",
        HostError::InvalidInterfaceId(_) => "invalid-interface-id",
        HostError::InvalidMaxBlockFrames(_) => "invalid-max-block-frames",
        HostError::InvalidSampleRate(_) => "invalid-sample-rate",
        HostError::UnsupportedSpeakerArrangement(_) => "unsupported-speaker-arrangement",
        HostError::MissingSymbol(_) => "missing-symbol",
        HostError::ModuleLoadFailed(_) => "module-load-failed",
        HostError::UnsupportedPlatform(_) => "unsupported-platform",
        HostError::InvalidChannelCount { .. } => "invalid-channel-count",
        HostError::InvalidBufferLength { .. } => "invalid-buffer-length",
        HostError::InvalidStateStreamSeek { .. } => "invalid-state-stream-seek",
        HostError::StateStreamWriteLimitExceeded { .. } => "state-stream-write-limit-exceeded",
        HostError::InvalidEventCount { .. } => "invalid-event-count",
        HostError::InvalidEventSampleOffset { .. } => "invalid-event-sample-offset",
        HostError::InvalidParameterChangeCount { .. } => "invalid-parameter-change-count",
        HostError::InvalidParameterChangeValue { .. } => "invalid-parameter-change-value",
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
        assert!(error.message().contains("parameter=42"));
    }
}
