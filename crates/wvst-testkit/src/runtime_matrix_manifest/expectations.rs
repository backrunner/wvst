use crate::runtime_matrix::{RuntimeProbeExpectations, RuntimeProbeStatus};

use super::{RuntimeProbeMatrixManifestError, invalid_case};

pub(super) fn validate_expectations(
    case_name: &str,
    expectations: &RuntimeProbeExpectations,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if expectations.expected_status == Some(RuntimeProbeStatus::ExpectationFailed) {
        return Err(invalid_case(
            case_name,
            "expectations.expectedStatus cannot be expectation-failed",
        ));
    }
    if expectations.require_non_zero_output && expectations.require_silent_output {
        return Err(invalid_case(
            case_name,
            "expectations cannot require both non-zero and silent output",
        ));
    }
    if expectations.require_note_response && expectations.require_silent_output {
        return Err(invalid_case(
            case_name,
            "expectations cannot require both note response and silent output",
        ));
    }
    if expectations.require_no_input_bus
        && (expectations.expected_input_bus_index.is_some()
            || expectations.expected_input_bus_channels.is_some())
    {
        return Err(invalid_case(
            case_name,
            "expectations cannot require no input bus and also expect an input bus selection",
        ));
    }
    if expectations.min_component_handler_events == Some(0) {
        return Err(invalid_case(
            case_name,
            "expectations.minComponentHandlerEvents must be greater than 0",
        ));
    }
    if let (Some(min_rms), Some(max_peak)) = (
        expectations.min_output_rms_milli,
        expectations.max_output_peak_milli,
    ) && min_rms > max_peak
    {
        return Err(invalid_case(
            case_name,
            "expectations.minOutputRmsMilli must be <= maxOutputPeakMilli",
        ));
    }
    validate_expected_bus_index(
        case_name,
        "expectations.expectedInputBusIndex",
        expectations.expected_input_bus_index,
    )?;
    validate_expected_bus_index(
        case_name,
        "expectations.expectedOutputBusIndex",
        expectations.expected_output_bus_index,
    )?;
    validate_expected_bus_channels(
        case_name,
        "expectations.expectedInputBusChannels",
        expectations.expected_input_bus_channels,
    )?;
    validate_expected_bus_channels(
        case_name,
        "expectations.expectedOutputBusChannels",
        expectations.expected_output_bus_channels,
    )?;
    if let (Some(expected), Some(required)) = (
        expectations.expected_process_context_requirements,
        expectations.required_process_context_requirements,
    ) && (expected & required) != required
    {
        return Err(invalid_case(
            case_name,
            "expectations.expectedProcessContextRequirements must include requiredProcessContextRequirements",
        ));
    }
    if let Some(kind) = &expectations.expected_tail_kind
        && !matches!(kind.as_str(), "none" | "finite" | "infinite")
    {
        return Err(invalid_case(
            case_name,
            "expectations.expectedTailKind must be one of none, finite, or infinite",
        ));
    }
    if let Some(category) = &expectations.expected_compatibility_category
        && category.trim().is_empty()
    {
        return Err(invalid_case(
            case_name,
            "expectations.expectedCompatibilityCategory must not be empty",
        ));
    }
    if let Some(category) = &expectations.expected_classification_category
        && category.trim().is_empty()
    {
        return Err(invalid_case(
            case_name,
            "expectations.expectedClassificationCategory must not be empty",
        ));
    }
    Ok(())
}

fn validate_expected_bus_index(
    case_name: &str,
    field_name: &str,
    index: Option<i32>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if index.is_some_and(|index| index < 0) {
        return Err(invalid_case(
            case_name,
            format!("{field_name} must be greater than or equal to 0"),
        ));
    }
    Ok(())
}

fn validate_expected_bus_channels(
    case_name: &str,
    field_name: &str,
    channels: Option<u16>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if channels == Some(0) {
        return Err(invalid_case(
            case_name,
            format!("{field_name} must be greater than 0"),
        ));
    }
    Ok(())
}
