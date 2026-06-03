use crate::event_payload_capture::{
    CapturedVst3OutputPayload, VST3_OUTPUT_PAYLOAD_ENCODING_RAW_BYTES,
    VST3_OUTPUT_PAYLOAD_ENCODING_UTF8, VST3_OUTPUT_PAYLOAD_FLAG_INVALID_TEXT,
    VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED, VST3_OUTPUT_PAYLOAD_FLAG_UNAVAILABLE,
};
use crate::vst3_abi::{
    Event, LegacyMidiCcOutEvent, VST3_EVENT_TYPE_CHORD, VST3_EVENT_TYPE_DATA,
    VST3_EVENT_TYPE_LEGACY_MIDI_CC_OUT, VST3_EVENT_TYPE_NOTE_EXPRESSION_INT_VALUE,
    VST3_EVENT_TYPE_NOTE_EXPRESSION_TEXT, VST3_EVENT_TYPE_NOTE_EXPRESSION_VALUE,
    VST3_EVENT_TYPE_NOTE_OFF, VST3_EVENT_TYPE_NOTE_ON, VST3_EVENT_TYPE_POLY_PRESSURE,
    VST3_EVENT_TYPE_SCALE, VST3_MIDI_CONTROLLER_ACTIVE_SENSING, VST3_MIDI_CONTROLLER_AFTERTOUCH,
    VST3_MIDI_CONTROLLER_CABLE_SELECT, VST3_MIDI_CONTROLLER_CLOCK_CONTINUE,
    VST3_MIDI_CONTROLLER_CLOCK_START, VST3_MIDI_CONTROLLER_CLOCK_STOP,
    VST3_MIDI_CONTROLLER_PITCH_BEND, VST3_MIDI_CONTROLLER_POLY_PRESSURE,
    VST3_MIDI_CONTROLLER_PROGRAM_CHANGE, VST3_MIDI_CONTROLLER_QUARTER_FRAME,
    VST3_MIDI_CONTROLLER_SONG_POINTER, VST3_MIDI_CONTROLLER_SONG_SELECT,
    VST3_MIDI_CONTROLLER_TUNE_REQUEST,
};

use super::{
    Vst3AdvancedOutputEvent, Vst3AdvancedOutputEventKind, Vst3LegacyMidiCcOutEvent, Vst3NoteEvent,
    Vst3OutputEvent, Vst3OutputEventStats, Vst3PolyPressureEvent,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum OutputEventFilterReason {
    InvalidSampleOffset,
    InvalidPayload,
    UnknownType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum NormalizedOutputEvent {
    Midi(Vst3OutputEvent),
    Advanced(Vst3AdvancedOutputEvent),
}

pub(super) fn from_vst3_event(
    frames: usize,
    event: Event,
    captured_payload: CapturedVst3OutputPayload,
) -> Result<NormalizedOutputEvent, OutputEventFilterReason> {
    if event.sample_offset < 0 || event.sample_offset as usize >= frames {
        return Err(OutputEventFilterReason::InvalidSampleOffset);
    }
    let sample_offset = event.sample_offset as u16;

    match event.event_type {
        VST3_EVENT_TYPE_NOTE_ON => {
            // SAFETY: The VST3 event type tag says this union arm is a note-on payload.
            let note = unsafe { event.payload.note_on };
            note_event_from_payload(
                sample_offset,
                note.channel,
                note.pitch,
                note.velocity,
                note.note_id,
            )
            .map(Vst3OutputEvent::NoteOn)
            .map(NormalizedOutputEvent::Midi)
            .ok_or(OutputEventFilterReason::InvalidPayload)
        }
        VST3_EVENT_TYPE_NOTE_OFF => {
            // SAFETY: The VST3 event type tag says this union arm is a note-off payload.
            let note = unsafe { event.payload.note_off };
            note_event_from_payload(
                sample_offset,
                note.channel,
                note.pitch,
                note.velocity,
                note.note_id,
            )
            .map(Vst3OutputEvent::NoteOff)
            .map(NormalizedOutputEvent::Midi)
            .ok_or(OutputEventFilterReason::InvalidPayload)
        }
        VST3_EVENT_TYPE_POLY_PRESSURE => {
            // SAFETY: The VST3 event type tag says this union arm is a poly-pressure payload.
            let pressure = unsafe { event.payload.poly_pressure };
            poly_pressure_event_from_payload(
                sample_offset,
                pressure.channel,
                pressure.pitch,
                pressure.pressure,
                pressure.note_id,
            )
            .map(Vst3OutputEvent::PolyPressure)
            .map(NormalizedOutputEvent::Midi)
            .ok_or(OutputEventFilterReason::InvalidPayload)
        }
        VST3_EVENT_TYPE_LEGACY_MIDI_CC_OUT => {
            // SAFETY: The VST3 event type tag says this union arm is a legacy MIDI CC payload.
            let midi_cc = unsafe { event.payload.midi_cc_out };
            legacy_midi_cc_out_event_from_payload(sample_offset, midi_cc)
                .map(Vst3OutputEvent::LegacyMidiCcOut)
                .map(NormalizedOutputEvent::Midi)
                .ok_or(OutputEventFilterReason::InvalidPayload)
        }
        VST3_EVENT_TYPE_DATA => advanced_output_event_from_payload(
            sample_offset,
            event,
            captured_payload,
            Vst3AdvancedOutputEventKind::Data,
        )
        .map(NormalizedOutputEvent::Advanced)
        .ok_or(OutputEventFilterReason::InvalidPayload),
        VST3_EVENT_TYPE_NOTE_EXPRESSION_VALUE => advanced_output_event_from_payload(
            sample_offset,
            event,
            captured_payload,
            Vst3AdvancedOutputEventKind::NoteExpressionValue,
        )
        .map(NormalizedOutputEvent::Advanced)
        .ok_or(OutputEventFilterReason::InvalidPayload),
        VST3_EVENT_TYPE_NOTE_EXPRESSION_TEXT => advanced_output_event_from_payload(
            sample_offset,
            event,
            captured_payload,
            Vst3AdvancedOutputEventKind::NoteExpressionText,
        )
        .map(NormalizedOutputEvent::Advanced)
        .ok_or(OutputEventFilterReason::InvalidPayload),
        VST3_EVENT_TYPE_NOTE_EXPRESSION_INT_VALUE => advanced_output_event_from_payload(
            sample_offset,
            event,
            captured_payload,
            Vst3AdvancedOutputEventKind::NoteExpressionIntValue,
        )
        .map(NormalizedOutputEvent::Advanced)
        .ok_or(OutputEventFilterReason::InvalidPayload),
        VST3_EVENT_TYPE_CHORD => advanced_output_event_from_payload(
            sample_offset,
            event,
            captured_payload,
            Vst3AdvancedOutputEventKind::Chord,
        )
        .map(NormalizedOutputEvent::Advanced)
        .ok_or(OutputEventFilterReason::InvalidPayload),
        VST3_EVENT_TYPE_SCALE => advanced_output_event_from_payload(
            sample_offset,
            event,
            captured_payload,
            Vst3AdvancedOutputEventKind::Scale,
        )
        .map(NormalizedOutputEvent::Advanced)
        .ok_or(OutputEventFilterReason::InvalidPayload),
        _ => Err(OutputEventFilterReason::UnknownType),
    }
}

pub(super) fn observe_advanced_payload_stats(
    stats: &mut Vst3OutputEventStats,
    event: Vst3AdvancedOutputEvent,
) {
    if event.payload_size > 0 {
        stats.advanced_payload_events = stats.advanced_payload_events.saturating_add(1);
        stats.advanced_payload_bytes = stats
            .advanced_payload_bytes
            .saturating_add(u32::from(event.payload_size));
    }
    match event.payload_encoding {
        VST3_OUTPUT_PAYLOAD_ENCODING_RAW_BYTES if event.payload_size > 0 => {
            stats.advanced_raw_payload_events = stats.advanced_raw_payload_events.saturating_add(1);
        }
        VST3_OUTPUT_PAYLOAD_ENCODING_UTF8 if event.payload_size > 0 => {
            stats.advanced_text_payload_events =
                stats.advanced_text_payload_events.saturating_add(1);
        }
        _ => {}
    }
    if (event.payload_flags & VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED) != 0 {
        stats.advanced_truncated_payload_events =
            stats.advanced_truncated_payload_events.saturating_add(1);
    }
    if (event.payload_flags & VST3_OUTPUT_PAYLOAD_FLAG_UNAVAILABLE) != 0 {
        stats.advanced_unavailable_payload_events =
            stats.advanced_unavailable_payload_events.saturating_add(1);
    }
    if (event.payload_flags & VST3_OUTPUT_PAYLOAD_FLAG_INVALID_TEXT) != 0 {
        stats.advanced_invalid_text_payload_events =
            stats.advanced_invalid_text_payload_events.saturating_add(1);
    }
}

fn note_event_from_payload(
    sample_offset: u16,
    channel: i16,
    pitch: i16,
    velocity: f32,
    note_id: i32,
) -> Option<Vst3NoteEvent> {
    Some(Vst3NoteEvent {
        sample_offset,
        channel: channel_to_u8(channel)?,
        pitch: data7_to_u8(pitch)?,
        velocity: unit_f32(velocity)?,
        note_id,
    })
}

fn poly_pressure_event_from_payload(
    sample_offset: u16,
    channel: i16,
    pitch: i16,
    pressure: f32,
    note_id: i32,
) -> Option<Vst3PolyPressureEvent> {
    Some(Vst3PolyPressureEvent {
        sample_offset,
        channel: channel_to_u8(channel)?,
        pitch: data7_to_u8(pitch)?,
        pressure: unit_f32(pressure)?,
        note_id,
    })
}

fn legacy_midi_cc_out_event_from_payload(
    sample_offset: u16,
    event: LegacyMidiCcOutEvent,
) -> Option<Vst3LegacyMidiCcOutEvent> {
    let control_number = legacy_control_number(event.control_number)?;
    Some(Vst3LegacyMidiCcOutEvent {
        sample_offset,
        control_number,
        channel: channel_to_u8(i16::from(event.channel))?,
        value: data7_to_u8(i16::from(event.value))?,
        value2: data7_to_u8(i16::from(event.value2))?,
    })
}

fn advanced_output_event_from_payload(
    sample_offset: u16,
    event: Event,
    captured_payload: CapturedVst3OutputPayload,
    kind: Vst3AdvancedOutputEventKind,
) -> Option<Vst3AdvancedOutputEvent> {
    let mut output = Vst3AdvancedOutputEvent {
        sample_offset,
        kind,
        vst3_event_type: event.event_type,
        bus_index: event.bus_index,
        payload_size: captured_payload.size,
        payload_encoding: captured_payload.encoding,
        payload_flags: captured_payload.flags,
        payload: captured_payload.bytes,
        ..Vst3AdvancedOutputEvent::default()
    };

    match kind {
        Vst3AdvancedOutputEventKind::Data => {
            // SAFETY: The VST3 event type tag says this union arm is a data payload.
            let data = unsafe { event.payload.data };
            output.data_size = data.size;
            output.data_type = data.event_type;
        }
        Vst3AdvancedOutputEventKind::NoteExpressionValue => {
            // SAFETY: The VST3 event type tag says this union arm is a note-expression value payload.
            let value = unsafe { event.payload.note_expression_value };
            output.data1 = value.note_id;
            output.value = unit_f64(value.value)?;
            output.data_type = value.type_id;
        }
        Vst3AdvancedOutputEventKind::NoteExpressionText => {
            // SAFETY: The VST3 event type tag says this union arm is a note-expression text payload.
            let text = unsafe { event.payload.note_expression_text };
            output.data1 = text.note_id;
            output.data_size = text.text_len;
            output.data_type = text.type_id;
        }
        Vst3AdvancedOutputEventKind::NoteExpressionIntValue => {
            // SAFETY: The VST3 event type tag says this union arm is a note-expression int payload.
            let value = unsafe { event.payload.note_expression_int_value };
            output.data1 = value.note_id;
            output.data2 = value.value;
            output.data_type = value.type_id;
        }
        Vst3AdvancedOutputEventKind::Chord => {
            // SAFETY: The VST3 event type tag says this union arm is a chord payload.
            let chord = unsafe { event.payload.chord };
            output.data1 = note_name_to_i32(chord.root)?;
            output.data2 = note_name_to_i32(chord.bass_note)?;
            output.data_size = u32::from(chord.text_len);
            output.data_type = chord_mask_to_u32(chord.mask)?;
        }
        Vst3AdvancedOutputEventKind::Scale => {
            // SAFETY: The VST3 event type tag says this union arm is a scale payload.
            let scale = unsafe { event.payload.scale };
            output.data1 = note_name_to_i32(scale.root)?;
            output.data2 = chord_mask_to_u32(scale.mask)? as i32;
            output.data_size = u32::from(scale.text_len);
        }
    }

    Some(output)
}

fn legacy_control_number(control_number: u8) -> Option<u8> {
    match i16::from(control_number) {
        0..=127
        | VST3_MIDI_CONTROLLER_AFTERTOUCH
        | VST3_MIDI_CONTROLLER_PITCH_BEND
        | VST3_MIDI_CONTROLLER_PROGRAM_CHANGE
        | VST3_MIDI_CONTROLLER_POLY_PRESSURE
        | VST3_MIDI_CONTROLLER_QUARTER_FRAME
        | VST3_MIDI_CONTROLLER_SONG_SELECT
        | VST3_MIDI_CONTROLLER_SONG_POINTER
        | VST3_MIDI_CONTROLLER_CABLE_SELECT
        | VST3_MIDI_CONTROLLER_TUNE_REQUEST
        | VST3_MIDI_CONTROLLER_CLOCK_START
        | VST3_MIDI_CONTROLLER_CLOCK_CONTINUE
        | VST3_MIDI_CONTROLLER_CLOCK_STOP
        | VST3_MIDI_CONTROLLER_ACTIVE_SENSING => Some(control_number),
        _ => None,
    }
}

fn channel_to_u8(channel: i16) -> Option<u8> {
    if (0..=15).contains(&channel) {
        Some(channel as u8)
    } else {
        None
    }
}

fn data7_to_u8(value: i16) -> Option<u8> {
    if (0..=127).contains(&value) {
        Some(value as u8)
    } else {
        None
    }
}

fn unit_f32(value: f32) -> Option<f32> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Some(value)
    } else {
        None
    }
}

fn unit_f64(value: f64) -> Option<f64> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Some(value)
    } else {
        None
    }
}

fn note_name_to_i32(value: i16) -> Option<i32> {
    if (0..=127).contains(&value) {
        Some(i32::from(value))
    } else {
        None
    }
}

fn chord_mask_to_u32(value: i16) -> Option<u32> {
    if value >= 0 {
        Some(u32::from(value as u16))
    } else {
        None
    }
}
