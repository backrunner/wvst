use wvst_protocol::{AudioFrameHeader, MIDI_EVENT_LEN, MidiEvent, MidiEventKind};
use wvst_vst3_host::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, VST3_MIDI_CONTROLLER_AFTERTOUCH,
    VST3_MIDI_CONTROLLER_PITCH_BEND, Vst3InputEvent, Vst3NoteEvent, Vst3ParameterChange,
    Vst3PolyPressureEvent,
};

use super::ipc_parameter_events::push_mapped_parameter_change;

pub(super) fn decode_midi_events_into(
    input_header: AudioFrameHeader,
    event_payload: &[u8],
    frames: usize,
    destination: &mut Vec<Vst3InputEvent>,
    parameter_changes: &mut Vec<Vst3ParameterChange>,
    parameter_id_for_midi: impl Fn(u8, i16) -> Option<u32>,
) -> Result<(), String> {
    if usize::from(input_header.event_count) > DEFAULT_MAX_VST3_EVENTS_PER_BLOCK {
        return Err(format!(
            "MIDI event count exceeds max block event count: max {}, got {}",
            DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, input_header.event_count
        ));
    }

    let expected_event_bytes = usize::from(input_header.event_count)
        .checked_mul(MIDI_EVENT_LEN)
        .ok_or_else(|| "MIDI event payload length overflow".to_string())?;
    if event_payload.len() != expected_event_bytes {
        return Err(format!(
            "MIDI event payload length mismatch: expected {expected_event_bytes}, got {}",
            event_payload.len()
        ));
    }

    destination.clear();
    for chunk in event_payload.chunks_exact(MIDI_EVENT_LEN) {
        let event = MidiEvent::decode(chunk).map_err(|error| error.to_string())?;
        if usize::from(event.sample_offset) >= frames {
            return Err(format!(
                "MIDI event sample offset is outside block: frames {frames}, offset {}",
                event.sample_offset
            ));
        }
        if let Some(event) = midi_event_to_vst3(event) {
            destination.push(event);
        }
        midi_event_to_parameter_change(frames, event, parameter_changes, &parameter_id_for_midi)?;
    }

    Ok(())
}

fn midi_event_to_vst3(event: MidiEvent) -> Option<Vst3InputEvent> {
    match event.kind {
        MidiEventKind::NoteOn => Some(Vst3InputEvent::NoteOn(note_event(
            event,
            event.data1,
            event.data2,
        ))),
        MidiEventKind::NoteOff => Some(Vst3InputEvent::NoteOff(note_event(
            event,
            event.data1,
            event.data2,
        ))),
        MidiEventKind::PolyAftertouch => Some(Vst3InputEvent::PolyPressure(poly_pressure_event(
            event,
            event.data1,
            event.data2,
        ))),
        MidiEventKind::RawMidi => raw_midi_to_vst3(event),
        MidiEventKind::ControlChange
        | MidiEventKind::PitchBend
        | MidiEventKind::ChannelAftertouch => None,
    }
}

fn midi_event_to_parameter_change(
    frames: usize,
    event: MidiEvent,
    destination: &mut Vec<Vst3ParameterChange>,
    parameter_id_for_midi: &impl Fn(u8, i16) -> Option<u32>,
) -> Result<(), String> {
    match event.kind {
        MidiEventKind::ControlChange => push_midi_controller_change(
            frames,
            destination,
            event.sample_offset,
            event.channel,
            i16::from(event.data1),
            midi_unit_value_f64(event.data2),
            parameter_id_for_midi,
        ),
        MidiEventKind::PitchBend => push_midi_controller_change(
            frames,
            destination,
            event.sample_offset,
            event.channel,
            VST3_MIDI_CONTROLLER_PITCH_BEND,
            midi_pitch_bend_value(event.data1, event.data2),
            parameter_id_for_midi,
        ),
        MidiEventKind::ChannelAftertouch => push_midi_controller_change(
            frames,
            destination,
            event.sample_offset,
            event.channel,
            VST3_MIDI_CONTROLLER_AFTERTOUCH,
            midi_unit_value_f64(event.data1),
            parameter_id_for_midi,
        ),
        MidiEventKind::RawMidi => {
            raw_midi_to_parameter_change(frames, event, destination, parameter_id_for_midi)
        }
        MidiEventKind::NoteOn | MidiEventKind::NoteOff | MidiEventKind::PolyAftertouch => Ok(()),
    }
}

fn raw_midi_to_parameter_change(
    frames: usize,
    event: MidiEvent,
    destination: &mut Vec<Vst3ParameterChange>,
    parameter_id_for_midi: &impl Fn(u8, i16) -> Option<u32>,
) -> Result<(), String> {
    let status = event.data1;
    let channel = status & 0x0f;
    match status & 0xf0 {
        0xb0 => push_midi_controller_change(
            frames,
            destination,
            event.sample_offset,
            channel,
            i16::from(event.data2),
            midi_unit_value_f64(event.data3),
            parameter_id_for_midi,
        ),
        0xd0 => push_midi_controller_change(
            frames,
            destination,
            event.sample_offset,
            channel,
            VST3_MIDI_CONTROLLER_AFTERTOUCH,
            midi_unit_value_f64(event.data2),
            parameter_id_for_midi,
        ),
        0xe0 => push_midi_controller_change(
            frames,
            destination,
            event.sample_offset,
            channel,
            VST3_MIDI_CONTROLLER_PITCH_BEND,
            midi_pitch_bend_value(event.data2, event.data3),
            parameter_id_for_midi,
        ),
        _ => Ok(()),
    }
}

fn push_midi_controller_change(
    frames: usize,
    destination: &mut Vec<Vst3ParameterChange>,
    sample_offset: u16,
    channel: u8,
    controller: i16,
    value_normalized: f64,
    parameter_id_for_midi: &impl Fn(u8, i16) -> Option<u32>,
) -> Result<(), String> {
    let Some(parameter_id) = parameter_id_for_midi(channel, controller) else {
        return Ok(());
    };

    push_mapped_parameter_change(
        frames,
        destination,
        sample_offset,
        parameter_id,
        value_normalized,
    )
}

fn raw_midi_to_vst3(event: MidiEvent) -> Option<Vst3InputEvent> {
    let status = event.data1;
    let channel = status & 0x0f;
    match status & 0xf0 {
        0x80 => Some(Vst3InputEvent::NoteOff(note_event_with_channel(
            event,
            channel,
            event.data2,
            event.data3,
        ))),
        0x90 if event.data3 == 0 => Some(Vst3InputEvent::NoteOff(note_event_with_channel(
            event,
            channel,
            event.data2,
            0,
        ))),
        0x90 => Some(Vst3InputEvent::NoteOn(note_event_with_channel(
            event,
            channel,
            event.data2,
            event.data3,
        ))),
        0xa0 => Some(Vst3InputEvent::PolyPressure(
            poly_pressure_event_with_channel(event, channel, event.data2, event.data3),
        )),
        _ => None,
    }
}

fn note_event(event: MidiEvent, pitch: u8, velocity: u8) -> Vst3NoteEvent {
    note_event_with_channel(event, event.channel, pitch, velocity)
}

fn note_event_with_channel(
    event: MidiEvent,
    channel: u8,
    pitch: u8,
    velocity: u8,
) -> Vst3NoteEvent {
    Vst3NoteEvent {
        sample_offset: event.sample_offset,
        channel,
        pitch,
        velocity: midi_unit_value(velocity),
        note_id: vst3_note_id(event.note_id),
    }
}

fn poly_pressure_event(event: MidiEvent, pitch: u8, pressure: u8) -> Vst3PolyPressureEvent {
    poly_pressure_event_with_channel(event, event.channel, pitch, pressure)
}

fn poly_pressure_event_with_channel(
    event: MidiEvent,
    channel: u8,
    pitch: u8,
    pressure: u8,
) -> Vst3PolyPressureEvent {
    Vst3PolyPressureEvent {
        sample_offset: event.sample_offset,
        channel,
        pitch,
        pressure: midi_unit_value(pressure),
        note_id: vst3_note_id(event.note_id),
    }
}

fn midi_unit_value(value: u8) -> f32 {
    f32::from(value) / 127.0
}

fn midi_unit_value_f64(value: u8) -> f64 {
    f64::from(value) / 127.0
}

fn midi_pitch_bend_value(lsb: u8, msb: u8) -> f64 {
    let value = u16::from(lsb) | (u16::from(msb) << 7);
    f64::from(value) / 16_383.0
}

fn vst3_note_id(value: u32) -> i32 {
    if value == 0 || value > i32::MAX as u32 {
        -1
    } else {
        value as i32
    }
}
