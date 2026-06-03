use serde_json::{Value, json};
use wvst_vst3_host::HostError;

const COMPATIBILITY_DIAGNOSTIC_SCHEMA_VERSION: u16 = 1;

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
                "compatibility": compatibility_diagnostic(&error),
                "message": message,
            })),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CompatibilityDiagnostic {
    schema_version: u16,
    category: &'static str,
    hint: &'static str,
}

const fn compatibility_diagnostic(error: &HostError) -> CompatibilityDiagnostic {
    let (category, hint) = compatibility_category_and_hint(error);
    CompatibilityDiagnostic {
        schema_version: COMPATIBILITY_DIAGNOSTIC_SCHEMA_VERSION,
        category,
        hint,
    }
}

const fn compatibility_category_and_hint(error: &HostError) -> (&'static str, &'static str) {
    match error {
        HostError::BundleExecutableNotFound(_)
        | HostError::FactoryCallFailed { .. }
        | HostError::FactoryReturnedNull
        | HostError::MissingSymbol(_)
        | HostError::ModuleLoadFailed(_) => (
            "plugin-loading",
            "verify the VST3 bundle path, platform executable, module exports, and plugin installation",
        ),
        HostError::InvalidClassId(_)
        | HostError::InstanceCreationFailed { .. }
        | HostError::InstanceReturnedNull { .. } => (
            "class-or-interface",
            "refresh plugin metadata and verify the requested VST3 class id and interface",
        ),
        HostError::AudioProcessorReturnedNull
        | HostError::AudioProcessorVTableMissing
        | HostError::ComponentReturnedNull
        | HostError::ComponentVTableMissing
        | HostError::EditControllerReturnedNull
        | HostError::EditControllerVTableMissing
        | HostError::InterfaceQueryFailed { .. }
        | HostError::InterfaceReturnedNull { .. }
        | HostError::InvalidInterfaceId(_) => (
            "required-interface",
            "the plugin did not expose a required VST3 interface for this operation",
        ),
        HostError::AudioProcessorCallFailed { .. }
        | HostError::ComponentCallFailed { .. }
        | HostError::EditControllerCallFailed { .. } => (
            "plugin-call",
            "the plugin returned a failure from a VST3 runtime or controller call",
        ),
        HostError::ConnectionPointCallFailed { .. }
        | HostError::ConnectionPointMessageNull
        | HostError::ConnectionPointReturnedNull
        | HostError::ConnectionPointVTableMissing => (
            "connection-point",
            "the plugin controller/component messaging path is unavailable or rejected the message",
        ),
        HostError::EditControllerParameterEditAlreadyActive { .. }
        | HostError::EditControllerParameterEditNotActive { .. } => (
            "parameter-edit-gesture",
            "the parameter edit gesture order is invalid for this controller",
        ),
        HostError::InvalidChannelCount { .. }
        | HostError::InvalidMaxBlockFrames(_)
        | HostError::InvalidSampleRate(_)
        | HostError::UnsupportedSpeakerArrangement(_) => (
            "processing-configuration",
            "adjust sample rate, block size, channel count, or bus arrangement for this plugin",
        ),
        HostError::InvalidBufferLength { .. } => (
            "audio-buffer",
            "the audio buffer shape does not match the negotiated processing configuration",
        ),
        HostError::InvalidStateStreamSeek { .. }
        | HostError::StateStreamWriteLimitExceeded { .. } => (
            "state-data",
            "the plugin state, program data, or unit data payload is invalid or exceeds the configured limit",
        ),
        HostError::InvalidEventCount { .. }
        | HostError::InvalidEventSampleOffset { .. }
        | HostError::InvalidParameterChangeCount { .. }
        | HostError::InvalidParameterChangeValue { .. } => (
            "event-automation",
            "MIDI, note, or parameter automation input is outside the supported block range or value range",
        ),
        HostError::InvalidLifecycleTransition { .. } => (
            "lifecycle",
            "the plugin instance received a lifecycle operation in an invalid state",
        ),
        HostError::UnsupportedPlatform(_) => (
            "platform",
            "this VST3 runtime path is not supported on the current platform",
        ),
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
        assert_eq!(data["compatibility"]["schemaVersion"], 1);
        assert_eq!(data["compatibility"]["category"], "parameter-edit-gesture");
        assert!(error.message().contains("parameter=42"));
    }

    #[test]
    fn classifies_host_errors_into_compatibility_categories() {
        assert_eq!(
            compatibility_diagnostic(&HostError::ModuleLoadFailed("bad bundle".to_string()))
                .category,
            "plugin-loading"
        );
        assert_eq!(
            compatibility_diagnostic(&HostError::InvalidClassId("bad".to_string())).category,
            "class-or-interface"
        );
        assert_eq!(
            compatibility_diagnostic(&HostError::UnsupportedSpeakerArrangement(9)).category,
            "processing-configuration"
        );
        assert_eq!(
            compatibility_diagnostic(&HostError::InvalidEventSampleOffset {
                frames: 128,
                actual: 129
            })
            .category,
            "event-automation"
        );
    }
}
