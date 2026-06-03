use wvst_protocol::{AudioFrameHeader, PARAMETER_AUTOMATION_EVENT_LEN, ParameterAutomationEvent};
use wvst_vst3_host::{DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK, Vst3ParameterChange};

use super::ipc_event_ordering::sort_parameter_changes_by_sample_offset;

pub(super) fn decode_parameter_events_into(
    input_header: AudioFrameHeader,
    payload: &[u8],
    frames: usize,
    destination: &mut Vec<Vst3ParameterChange>,
) -> Result<(), String> {
    if usize::from(input_header.parameter_event_count)
        > DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK
    {
        return Err(format!(
            "parameter event count exceeds max block change count: max {}, got {}",
            DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK, input_header.parameter_event_count
        ));
    }

    let expected_event_bytes = usize::from(input_header.parameter_event_count)
        .checked_mul(PARAMETER_AUTOMATION_EVENT_LEN)
        .ok_or_else(|| "parameter event payload length overflow".to_string())?;
    if payload.len() != expected_event_bytes {
        return Err(format!(
            "parameter event payload length mismatch: expected {expected_event_bytes}, got {}",
            payload.len()
        ));
    }

    destination.clear();
    for chunk in payload.chunks_exact(PARAMETER_AUTOMATION_EVENT_LEN) {
        let event = ParameterAutomationEvent::decode(chunk).map_err(|error| error.to_string())?;
        push_parameter_event_into(event, frames, destination)?;
    }
    sort_parameter_changes_by_sample_offset(destination);
    Ok(())
}

pub(super) fn encode_parameter_events_into(
    changes: &[Vst3ParameterChange],
    destination: &mut [u8],
) -> Result<(), String> {
    let expected_event_bytes = changes
        .len()
        .checked_mul(PARAMETER_AUTOMATION_EVENT_LEN)
        .ok_or_else(|| "parameter output event payload length overflow".to_string())?;
    if destination.len() != expected_event_bytes {
        return Err(format!(
            "parameter output event payload length mismatch: expected {expected_event_bytes}, got {}",
            destination.len()
        ));
    }

    for (index, change) in changes.iter().enumerate() {
        ParameterAutomationEvent::new(
            change.sample_offset,
            change.parameter_id,
            change.value_normalized,
        )
        .map_err(|error| error.to_string())?
        .encode(&mut destination[index * PARAMETER_AUTOMATION_EVENT_LEN..])
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub(super) fn push_mapped_parameter_change(
    frames: usize,
    destination: &mut Vec<Vst3ParameterChange>,
    sample_offset: u16,
    parameter_id: u32,
    value_normalized: f64,
) -> Result<(), String> {
    push_parameter_event_into(
        ParameterAutomationEvent::new(sample_offset, parameter_id, value_normalized)
            .map_err(|error| error.to_string())?,
        frames,
        destination,
    )
}

pub(super) fn push_parameter_event_into(
    event: ParameterAutomationEvent,
    frames: usize,
    destination: &mut Vec<Vst3ParameterChange>,
) -> Result<(), String> {
    if usize::from(event.sample_offset) >= frames {
        return Err(format!(
            "parameter event sample offset is outside block: frames {frames}, offset {}",
            event.sample_offset
        ));
    }
    if destination.len() >= DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK {
        return Err(format!(
            "parameter event count exceeds max block change count: max {}, got {}",
            DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
            destination.len() + 1
        ));
    }

    destination.push(Vst3ParameterChange {
        sample_offset: event.sample_offset,
        parameter_id: event.parameter_id,
        value_normalized: event.value_normalized,
    });

    Ok(())
}
