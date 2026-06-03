use crate::runtime_matrix::RuntimeProbeExpectations;

use super::{RuntimeProbeCaseManifest, RuntimeProbeMatrixManifestError, invalid_case};

pub(super) fn validate_coverage_tags(
    case: &RuntimeProbeCaseManifest,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    let Some(evidence) = &case.evidence else {
        return Ok(());
    };

    let expectations = case.expectations.as_ref();
    for tag in &evidence.tags {
        match tag.trim() {
            "automation" => validate_automation_tag(case, expectations)?,
            "controller-rich" => validate_controller_rich_tag(case, expectations)?,
            "note-response" => validate_note_response_tag(case, expectations)?,
            "output-events" => validate_output_events_tag(case, expectations)?,
            "zero-input" => validate_zero_input_tag(case, expectations)?,
            _ => {}
        }
    }
    Ok(())
}

fn validate_automation_tag(
    case: &RuntimeProbeCaseManifest,
    expectations: Option<&RuntimeProbeExpectations>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    let has_automation_expectation = expectations
        .and_then(|expectations| expectations.min_automatable_parameters)
        .is_some_and(|count| count > 0);
    if case.parameter_changes.is_empty() || !has_automation_expectation {
        return Err(tag_error(
            case,
            "automation",
            "parameterChanges and expectations.minAutomatableParameters > 0",
        ));
    }
    Ok(())
}

fn validate_controller_rich_tag(
    case: &RuntimeProbeCaseManifest,
    expectations: Option<&RuntimeProbeExpectations>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    let Some(expectations) = expectations else {
        return Err(tag_error(
            case,
            "controller-rich",
            "controller expectations",
        ));
    };
    let has_parameters = expectations
        .min_parameter_count
        .is_some_and(|count| count > 0)
        && expectations
            .min_automatable_parameters
            .is_some_and(|count| count > 0);
    let has_controller_surface = expectations.require_component_state
        || expectations.require_controller_state
        || expectations.require_controller_component_state_sync
        || expectations.require_component_handler
        || expectations.require_connection_points
        || expectations.require_unit_info;
    if !has_parameters || !has_controller_surface {
        return Err(tag_error(
            case,
            "controller-rich",
            "minParameterCount/minAutomatableParameters and at least one controller surface expectation",
        ));
    }
    Ok(())
}

fn validate_note_response_tag(
    case: &RuntimeProbeCaseManifest,
    expectations: Option<&RuntimeProbeExpectations>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    let requires_note_response = expectations.is_some_and(|expectations| {
        expectations.require_note_response && expectations.require_non_zero_output
    });
    if case.note.is_none() || !requires_note_response {
        return Err(tag_error(
            case,
            "note-response",
            "note plus expectations.requireNoteResponse and requireNonZeroOutput",
        ));
    }
    Ok(())
}

fn validate_output_events_tag(
    case: &RuntimeProbeCaseManifest,
    expectations: Option<&RuntimeProbeExpectations>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if !expectations.is_some_and(has_output_event_expectation) {
        return Err(tag_error(
            case,
            "output-events",
            "at least one positive output event expectation",
        ));
    }
    Ok(())
}

fn validate_zero_input_tag(
    case: &RuntimeProbeCaseManifest,
    expectations: Option<&RuntimeProbeExpectations>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    let require_no_input_bus =
        expectations.is_some_and(|expectations| expectations.require_no_input_bus);
    if case.input_channels != 0 || !require_no_input_bus {
        return Err(tag_error(
            case,
            "zero-input",
            "inputChannels=0 and expectations.requireNoInputBus",
        ));
    }
    Ok(())
}

fn has_output_event_expectation(expectations: &RuntimeProbeExpectations) -> bool {
    [
        expectations.min_output_events,
        expectations.min_output_parameter_changes,
        expectations.min_normalized_output_events,
        expectations.min_advanced_output_events,
        expectations.min_advanced_data_output_events,
        expectations.min_advanced_note_expression_output_events,
        expectations.min_advanced_chord_output_events,
        expectations.min_advanced_scale_output_events,
        expectations.min_advanced_payload_events,
        expectations.min_advanced_payload_bytes,
        expectations.min_advanced_raw_payload_events,
        expectations.min_advanced_text_payload_events,
        expectations.min_normalized_output_parameter_changes,
    ]
    .into_iter()
    .flatten()
    .any(|count| count > 0)
}

fn tag_error(
    case: &RuntimeProbeCaseManifest,
    tag: &'static str,
    requirement: &'static str,
) -> RuntimeProbeMatrixManifestError {
    invalid_case(
        case.name.clone(),
        format!("evidence tag {tag:?} requires {requirement}"),
    )
}
