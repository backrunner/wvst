use serde::Deserialize;
use serde_json::json;
use wvst_protocol::{MidiEvent, MidiEventKind, ParameterAutomationEvent};
use wvst_vst3_host::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
    Vst3InputEvent, Vst3ParameterChange,
};

use super::super::ipc_event_ordering::{
    sort_input_events_by_sample_offset, sort_parameter_changes_by_sample_offset,
};
use super::super::ipc_midi::push_midi_event_into;
use super::super::ipc_parameter_events::push_parameter_event_into;
use super::error::SharedMemoryProcessError;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SharedMemoryMidiEventParam {
    sample_offset: u16,
    kind: u8,
    channel: u8,
    data1: u8,
    data2: u8,
    #[serde(default)]
    data3: Option<u8>,
    #[serde(default)]
    data_length: Option<u8>,
    #[serde(default)]
    note_id: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SharedMemoryParameterEventParam {
    sample_offset: u16,
    parameter_id: u32,
    value_normalized: f64,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SharedMemoryInputEventReport {
    pub midi_events: usize,
    pub parameter_events: usize,
    pub vst3_input_events: usize,
    pub vst3_parameter_changes: usize,
}

pub(super) fn apply_shared_memory_process_events(
    midi_events: &[SharedMemoryMidiEventParam],
    parameter_events: &[SharedMemoryParameterEventParam],
    frames: usize,
    input_events: &mut Vec<Vst3InputEvent>,
    parameter_changes: &mut Vec<Vst3ParameterChange>,
    parameter_id_for_midi: impl Fn(u8, i16) -> Option<u32>,
) -> Result<SharedMemoryInputEventReport, SharedMemoryProcessError> {
    input_events.clear();
    parameter_changes.clear();
    if midi_events.len() > DEFAULT_MAX_VST3_EVENTS_PER_BLOCK {
        return Err(process_event_error(
            "midi-events",
            format!(
                "MIDI event count exceeds max block event count: max {}, got {}",
                DEFAULT_MAX_VST3_EVENTS_PER_BLOCK,
                midi_events.len()
            ),
        ));
    }
    if parameter_events.len() > DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK {
        return Err(process_event_error(
            "parameter-events",
            format!(
                "parameter event count exceeds max block change count: max {}, got {}",
                DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
                parameter_events.len()
            ),
        ));
    }

    for event in parameter_events {
        push_parameter_event_into(event.to_protocol()?, frames, parameter_changes)
            .map_err(|error| process_event_error("parameter-events", error))?;
    }

    for event in midi_events {
        push_midi_event_into(
            event.to_protocol()?,
            frames,
            input_events,
            parameter_changes,
            &parameter_id_for_midi,
        )
        .map_err(|error| process_event_error("midi-events", error))?;
    }
    sort_input_events_by_sample_offset(input_events);
    sort_parameter_changes_by_sample_offset(parameter_changes);

    Ok(SharedMemoryInputEventReport {
        midi_events: midi_events.len(),
        parameter_events: parameter_events.len(),
        vst3_input_events: input_events.len(),
        vst3_parameter_changes: parameter_changes.len(),
    })
}

impl SharedMemoryMidiEventParam {
    fn to_protocol(self) -> Result<MidiEvent, SharedMemoryProcessError> {
        let kind = MidiEventKind::try_from(self.kind).map_err(|error| {
            invalid_event_error("midi-events", "invalid MIDI event kind", error.to_string())
        })?;

        let mut event = if kind == MidiEventKind::RawMidi {
            MidiEvent::raw_midi(
                self.sample_offset,
                self.data1,
                self.data2,
                self.data3.unwrap_or(0),
                self.data_length.unwrap_or(3),
            )
        } else {
            if self.data_length.is_some_and(|data_length| data_length != 2) {
                return Err(invalid_event_error(
                    "midi-events",
                    "invalid MIDI dataLength",
                    "non-raw MIDI events must use dataLength 2",
                ));
            }
            if self.data3.is_some_and(|data3| data3 > 127) {
                return Err(invalid_event_error(
                    "midi-events",
                    "invalid MIDI data3",
                    "non-raw MIDI event data3 must be in 0..=127",
                ));
            }
            MidiEvent::new(
                self.sample_offset,
                kind,
                self.channel,
                self.data1,
                self.data2,
            )
        }
        .map_err(|error| invalid_event_error("midi-events", "invalid MIDI event", error))?;

        event.data3 = self.data3.unwrap_or(0);
        event.note_id = self.note_id.unwrap_or(0);
        Ok(event)
    }
}

impl SharedMemoryParameterEventParam {
    fn to_protocol(self) -> Result<ParameterAutomationEvent, SharedMemoryProcessError> {
        ParameterAutomationEvent::new(self.sample_offset, self.parameter_id, self.value_normalized)
            .map_err(|error| {
                invalid_event_error("parameter-events", "invalid parameter event", error)
            })
    }
}

fn invalid_event_error(
    reason: &'static str,
    message: &'static str,
    detail: impl ToString,
) -> SharedMemoryProcessError {
    SharedMemoryProcessError::invalid_with_data(
        reason,
        format!("{message}: {}", detail.to_string()),
        json!({
            "kind": "shared-memory-process-events",
            "reason": reason,
            "message": message,
            "detail": detail.to_string(),
        }),
    )
}

fn process_event_error(reason: &'static str, error: String) -> SharedMemoryProcessError {
    SharedMemoryProcessError::invalid_with_data(
        reason,
        error.clone(),
        json!({
            "kind": "shared-memory-process-events",
            "reason": reason,
            "message": error,
        }),
    )
}
