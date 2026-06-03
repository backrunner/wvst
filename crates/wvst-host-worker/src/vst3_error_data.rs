use serde_json::{Value, json};
use wvst_vst3_host::HostError;

const COMPATIBILITY_DIAGNOSTIC_SCHEMA_VERSION: u16 = 1;

pub(crate) fn vst3_error_data(kind: &'static str, stage: &'static str, error: &HostError) -> Value {
    let message = error.to_string();
    json!({
        "kind": kind,
        "stage": stage,
        "hostError": host_error_kind(error),
        "compatibility": compatibility_diagnostic(error),
        "message": message,
    })
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
        | HostError::InvalidAudioBusIndex { .. }
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
        HostError::InvalidAudioBusIndex { .. } => "invalid-audio-bus-index",
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

    #[test]
    fn builds_vst3_error_data() {
        let data = vst3_error_data(
            "vst3-runtime-process",
            "component.process",
            &HostError::AudioProcessorCallFailed {
                method: "process",
                result: -1,
            },
        );

        assert_eq!(data["kind"], "vst3-runtime-process");
        assert_eq!(data["stage"], "component.process");
        assert_eq!(data["hostError"], "audio-processor-call-failed");
        assert_eq!(data["compatibility"]["schemaVersion"], 1);
        assert_eq!(data["compatibility"]["category"], "plugin-call");
    }
}
