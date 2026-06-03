use super::*;
use wvst_vst3_host::{Vst3ParameterFlags, Vst3ParameterInfo};

#[test]
fn summarizes_parameter_flags() {
    let summary = parameter_summary(&[
        parameter(flags(true, false, false, false, false)),
        parameter(flags(false, true, true, true, true)),
    ]);

    assert_eq!(summary.count, 2);
    assert_eq!(summary.automatable, 1);
    assert_eq!(summary.read_only, 1);
    assert_eq!(summary.hidden, 1);
    assert_eq!(summary.bypass, 1);
    assert_eq!(summary.program_change, 1);
}

#[test]
fn selects_only_automatable_writable_parameter_for_edit_probe() {
    let read_only = parameter_with_id(1, flags(true, true, false, false, false));
    let not_automatable = parameter_with_id(2, flags(false, false, false, false, false));
    let writable = parameter_with_id(3, flags(true, false, false, false, false));
    let parameters = [read_only, not_automatable, writable];

    let selected = edit_probe_parameter(&parameters, None).expect("selected parameter");

    assert_eq!(selected.id, 3);
}

#[test]
fn selects_requested_edit_probe_parameter_when_eligible() {
    let first = parameter_with_id(1, flags(true, false, false, false, false));
    let requested = parameter_with_id(2, flags(true, false, true, false, false));
    let parameters = [first, requested];

    let selected =
        edit_probe_parameter(&parameters, Some(2)).expect("requested parameter selected");

    assert_eq!(selected.id, 2);
}

#[test]
fn rejects_requested_edit_probe_parameter_when_missing_or_read_only() {
    let read_only = parameter_with_id(1, flags(true, true, false, false, false));
    let parameters = [read_only];

    assert_eq!(
        edit_probe_parameter(&parameters, Some(99)).expect_err("missing parameter"),
        "requested-parameter-not-found"
    );
    assert_eq!(
        edit_probe_parameter(&parameters, Some(1)).expect_err("read-only parameter"),
        "requested-parameter-not-eligible"
    );
}

#[test]
fn validates_normalized_edit_probe_value() {
    assert!(is_normalized_value(0.0));
    assert!(is_normalized_value(1.0));
    assert!(!is_normalized_value(-0.01));
    assert!(!is_normalized_value(1.01));
    assert!(!is_normalized_value(f64::NAN));
}

#[test]
fn builds_connection_notify_probe_message_with_typed_attributes() {
    connection_notify_probe_message().expect("probe message");
}

#[test]
fn skips_connection_notify_probe_with_reason() {
    let summary = skipped_connection_notify_probe("disabled-by-options");

    assert!(!summary.attempted);
    assert!(!summary.success);
    assert_eq!(
        summary.skipped_reason.as_deref(),
        Some("disabled-by-options")
    );
}

#[test]
fn state_roundtrip_succeeds_when_bytes_match() {
    let state = [1, 2, 3];
    let summary =
        finish_state_roundtrip(started_state_roundtrip(&state), &state, Ok(state.to_vec()));

    assert!(summary.attempted);
    assert!(summary.success);
    assert_eq!(summary.bytes_before, Some(3));
    assert_eq!(summary.bytes_after, Some(3));
    assert_eq!(summary.error, None);
}

#[test]
fn state_roundtrip_reports_changed_bytes() {
    let state = [1, 2, 3];
    let summary =
        finish_state_roundtrip(started_state_roundtrip(&state), &state, Ok(vec![1, 2, 4]));

    assert!(summary.attempted);
    assert!(!summary.success);
    assert_eq!(summary.bytes_before, Some(3));
    assert_eq!(summary.bytes_after, Some(3));
    assert_eq!(
        summary.error.as_deref(),
        Some("state bytes changed after roundtrip")
    );
}

fn parameter(flags: Vst3ParameterFlags) -> Vst3ParameterInfo {
    parameter_with_id(42, flags)
}

fn parameter_with_id(id: u32, flags: Vst3ParameterFlags) -> Vst3ParameterInfo {
    Vst3ParameterInfo {
        id,
        title: None,
        short_title: None,
        units: None,
        step_count: 0,
        default_normalized_value: 0.0,
        unit_id: 0,
        flags,
    }
}

fn flags(
    can_automate: bool,
    read_only: bool,
    hidden: bool,
    program_change: bool,
    bypass: bool,
) -> Vst3ParameterFlags {
    Vst3ParameterFlags {
        raw: 0,
        can_automate,
        read_only,
        wrap_around: false,
        list: false,
        hidden,
        program_change,
        bypass,
    }
}
