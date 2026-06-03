use crate::runtime_matrix::{RuntimeProbeExpectations, RuntimeProbePluginKind};

use super::{
    RuntimeProbeCaseManifest, RuntimeProbeEvidenceRequirements, RuntimeProbeMatrixManifestError,
};

pub(super) fn validate_third_party_expectation_quality(
    case: &RuntimeProbeCaseManifest,
    requirements: &RuntimeProbeEvidenceRequirements,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if requirements.require_third_party_audio_health_expectations {
        validate_audio_health_expectations(
            case,
            expectations_for(case, "requireThirdPartyAudioHealthExpectations")?,
        )?;
    }
    if requirements.require_third_party_process_timing_expectations {
        validate_process_timing_expectations(
            case,
            expectations_for(case, "requireThirdPartyProcessTimingExpectations")?,
        )?;
    }
    if requirements.require_third_party_runtime_health_expectations {
        validate_runtime_health_expectations(
            case,
            expectations_for(case, "requireThirdPartyRuntimeHealthExpectations")?,
        )?;
    }
    if requirements.require_third_party_note_response_expectations
        && third_party_case_needs_note_response_expectations(case)
    {
        validate_note_response_expectations(
            case,
            expectations_for(case, "requireThirdPartyNoteResponseExpectations")?,
        )?;
    }
    if requirements.require_third_party_controller_rich_expectations
        && case_has_tag(case, "controller-rich")
    {
        validate_controller_rich_expectations(
            case,
            expectations_for(case, "requireThirdPartyControllerRichExpectations")?,
        )?;
    }
    if requirements.require_third_party_output_event_expectations
        && case_has_tag(case, "output-events")
    {
        validate_output_event_expectations(
            case,
            expectations_for(case, "requireThirdPartyOutputEventExpectations")?,
        )?;
    }
    Ok(())
}

fn expectations_for<'a>(
    case: &'a RuntimeProbeCaseManifest,
    gate_name: &'static str,
) -> Result<&'a RuntimeProbeExpectations, RuntimeProbeMatrixManifestError> {
    case.expectations
        .as_ref()
        .ok_or_else(|| weak_expectations(gate_name, case, "a non-empty expectations object"))
}

fn validate_audio_health_expectations(
    case: &RuntimeProbeCaseManifest,
    expectations: &RuntimeProbeExpectations,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if expectations.max_non_finite_output_samples.is_none()
        || expectations.max_clipped_output_samples.is_none()
    {
        return Err(weak_expectations(
            "requireThirdPartyAudioHealthExpectations",
            case,
            "maxNonFiniteOutputSamples and maxClippedOutputSamples",
        ));
    }
    Ok(())
}

fn validate_process_timing_expectations(
    case: &RuntimeProbeCaseManifest,
    expectations: &RuntimeProbeExpectations,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if expectations.max_process_time_p95_micros.is_none()
        || expectations.max_process_time_p99_micros.is_none()
        || expectations.max_process_time_max_micros.is_none()
    {
        return Err(weak_expectations(
            "requireThirdPartyProcessTimingExpectations",
            case,
            "maxProcessTimeP95Micros, maxProcessTimeP99Micros and maxProcessTimeMaxMicros",
        ));
    }
    Ok(())
}

fn validate_runtime_health_expectations(
    case: &RuntimeProbeCaseManifest,
    expectations: &RuntimeProbeExpectations,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if !expectations.require_tail_info || expectations.max_latency_samples.is_none() {
        return Err(weak_expectations(
            "requireThirdPartyRuntimeHealthExpectations",
            case,
            "requireTailInfo and maxLatencySamples",
        ));
    }
    Ok(())
}

fn validate_note_response_expectations(
    case: &RuntimeProbeCaseManifest,
    expectations: &RuntimeProbeExpectations,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if case.note.is_none()
        || !expectations.require_note_response
        || expectations.max_note_to_audio_frames.is_none()
        || expectations.max_note_to_audio_micros.is_none()
    {
        return Err(weak_expectations(
            "requireThirdPartyNoteResponseExpectations",
            case,
            "note, requireNoteResponse, maxNoteToAudioFrames and maxNoteToAudioMicros",
        ));
    }
    Ok(())
}

fn validate_controller_rich_expectations(
    case: &RuntimeProbeCaseManifest,
    expectations: &RuntimeProbeExpectations,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if expectations.min_parameter_count.is_none()
        || expectations.min_automatable_parameters.is_none()
        || !expectations.require_component_state_roundtrip
        || !expectations.require_controller_state_roundtrip
        || !expectations.require_component_handler_edit_probe
        || !expectations.require_connection_notify_probe
    {
        return Err(weak_expectations(
            "requireThirdPartyControllerRichExpectations",
            case,
            "minParameterCount, minAutomatableParameters, state roundtrip, edit probe and connection notify probe",
        ));
    }
    Ok(())
}

fn validate_output_event_expectations(
    case: &RuntimeProbeCaseManifest,
    expectations: &RuntimeProbeExpectations,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if !has_output_event_minimum(expectations)
        || expectations.max_filtered_output_events.is_none()
        || expectations.max_filtered_output_parameter_changes.is_none()
    {
        return Err(weak_expectations(
            "requireThirdPartyOutputEventExpectations",
            case,
            "an output-event minimum plus maxFilteredOutputEvents and maxFilteredOutputParameterChanges",
        ));
    }
    Ok(())
}

fn third_party_case_needs_note_response_expectations(case: &RuntimeProbeCaseManifest) -> bool {
    case.evidence
        .as_ref()
        .is_some_and(|evidence| evidence.plugin_kind == RuntimeProbePluginKind::Instrument)
        || case_has_tag(case, "note-response")
}

fn case_has_tag(case: &RuntimeProbeCaseManifest, needle: &str) -> bool {
    case.evidence.as_ref().is_some_and(|evidence| {
        evidence
            .tags
            .iter()
            .any(|tag| tag.trim().eq_ignore_ascii_case(needle))
    })
}

fn has_output_event_minimum(expectations: &RuntimeProbeExpectations) -> bool {
    expectations.min_output_events.is_some()
        || expectations.min_output_parameter_changes.is_some()
        || expectations.min_normalized_output_events.is_some()
        || expectations.min_advanced_output_events.is_some()
        || expectations.min_advanced_data_output_events.is_some()
        || expectations
            .min_advanced_note_expression_output_events
            .is_some()
        || expectations.min_advanced_chord_output_events.is_some()
        || expectations.min_advanced_scale_output_events.is_some()
        || expectations.min_advanced_payload_events.is_some()
        || expectations
            .min_normalized_output_parameter_changes
            .is_some()
}

fn weak_expectations(
    gate_name: &'static str,
    case: &RuntimeProbeCaseManifest,
    expected: &'static str,
) -> RuntimeProbeMatrixManifestError {
    RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements {
        message: format!(
            "{gate_name} expected case {:?} expectations to include {expected}",
            case.name
        ),
    }
}
