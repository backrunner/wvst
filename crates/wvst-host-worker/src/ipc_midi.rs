use wvst_protocol::{AudioFrameHeader, MIDI_EVENT_LEN, MidiEvent, MidiEventKind};
use wvst_vst3_host::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, VST3_MIDI_CONTROLLER_ACTIVE_SENSING,
    VST3_MIDI_CONTROLLER_AFTERTOUCH, VST3_MIDI_CONTROLLER_CABLE_SELECT,
    VST3_MIDI_CONTROLLER_CLOCK_CONTINUE, VST3_MIDI_CONTROLLER_CLOCK_START,
    VST3_MIDI_CONTROLLER_CLOCK_STOP, VST3_MIDI_CONTROLLER_PITCH_BEND,
    VST3_MIDI_CONTROLLER_POLY_PRESSURE, VST3_MIDI_CONTROLLER_PROGRAM_CHANGE,
    VST3_MIDI_CONTROLLER_QUARTER_FRAME, VST3_MIDI_CONTROLLER_SONG_POINTER,
    VST3_MIDI_CONTROLLER_SONG_SELECT, VST3_MIDI_CONTROLLER_TUNE_REQUEST, Vst3InputEvent,
    Vst3LegacyMidiCcOutEvent, Vst3NoteEvent, Vst3OutputEvent, Vst3ParameterChange,
    Vst3PolyPressureEvent,
};

use super::ipc_event_ordering::{
    sort_input_events_by_sample_offset, sort_parameter_changes_by_sample_offset,
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
        push_midi_event_into(
            event,
            frames,
            destination,
            parameter_changes,
            &parameter_id_for_midi,
        )?;
    }
    sort_input_events_by_sample_offset(destination);
    sort_parameter_changes_by_sample_offset(parameter_changes);

    Ok(())
}

pub(super) fn push_midi_event_into(
    event: MidiEvent,
    frames: usize,
    destination: &mut Vec<Vst3InputEvent>,
    parameter_changes: &mut Vec<Vst3ParameterChange>,
    parameter_id_for_midi: &impl Fn(u8, i16) -> Option<u32>,
) -> Result<(), String> {
    if usize::from(event.sample_offset) >= frames {
        return Err(format!(
            "MIDI event sample offset is outside block: frames {frames}, offset {}",
            event.sample_offset
        ));
    }
    if let Some(event) = midi_event_to_vst3(event) {
        destination.push(event);
    }
    midi_event_to_parameter_change(frames, event, parameter_changes, parameter_id_for_midi)
}

pub(super) fn encode_midi_events_into(
    events: &[Vst3OutputEvent],
    destination: &mut [u8],
) -> Result<(), String> {
    let expected_event_bytes = events
        .len()
        .checked_mul(MIDI_EVENT_LEN)
        .ok_or_else(|| "MIDI output event payload length overflow".to_string())?;
    if destination.len() != expected_event_bytes {
        return Err(format!(
            "MIDI output event payload length mismatch: expected {expected_event_bytes}, got {}",
            destination.len()
        ));
    }

    for (index, event) in events.iter().enumerate() {
        let event = vst3_output_event_to_midi_event(*event)?;
        event
            .encode(&mut destination[index * MIDI_EVENT_LEN..])
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn vst3_output_event_to_midi_event(event: Vst3OutputEvent) -> Result<MidiEvent, String> {
    match event {
        Vst3OutputEvent::NoteOn(event) => midi_note_event(MidiEventKind::NoteOn, event),
        Vst3OutputEvent::NoteOff(event) => midi_note_event(MidiEventKind::NoteOff, event),
        Vst3OutputEvent::PolyPressure(event) => {
            let mut midi_event = MidiEvent::new(
                event.sample_offset,
                MidiEventKind::PolyAftertouch,
                event.channel,
                event.pitch,
                unit_value_to_midi(event.pressure),
            )
            .map_err(|error| error.to_string())?;
            midi_event.note_id = midi_note_id(event.note_id);
            Ok(midi_event)
        }
        Vst3OutputEvent::LegacyMidiCcOut(event) => legacy_midi_cc_out_event(event),
    }
}

fn legacy_midi_cc_out_event(event: Vst3LegacyMidiCcOutEvent) -> Result<MidiEvent, String> {
    match i16::from(event.control_number) {
        0..=127 => MidiEvent::new(
            event.sample_offset,
            MidiEventKind::ControlChange,
            event.channel,
            event.control_number,
            event.value,
        )
        .map_err(|error| error.to_string()),
        VST3_MIDI_CONTROLLER_AFTERTOUCH => MidiEvent::new(
            event.sample_offset,
            MidiEventKind::ChannelAftertouch,
            event.channel,
            event.value,
            0,
        )
        .map_err(|error| error.to_string()),
        VST3_MIDI_CONTROLLER_PITCH_BEND => MidiEvent::new(
            event.sample_offset,
            MidiEventKind::PitchBend,
            event.channel,
            event.value,
            event.value2,
        )
        .map_err(|error| error.to_string()),
        VST3_MIDI_CONTROLLER_PROGRAM_CHANGE => raw_channel_event(event, 0xc0, 2),
        VST3_MIDI_CONTROLLER_POLY_PRESSURE => MidiEvent::new(
            event.sample_offset,
            MidiEventKind::PolyAftertouch,
            event.channel,
            event.value,
            event.value2,
        )
        .map_err(|error| error.to_string()),
        VST3_MIDI_CONTROLLER_QUARTER_FRAME => raw_system_event(event, 0xf1, 2),
        VST3_MIDI_CONTROLLER_SONG_SELECT => raw_system_event(event, 0xf3, 2),
        VST3_MIDI_CONTROLLER_SONG_POINTER => raw_system_event(event, 0xf2, 3),
        VST3_MIDI_CONTROLLER_CABLE_SELECT => raw_system_event(event, 0xf5, 2),
        VST3_MIDI_CONTROLLER_TUNE_REQUEST => raw_system_event(event, 0xf6, 1),
        VST3_MIDI_CONTROLLER_CLOCK_START => raw_system_event(event, 0xfa, 1),
        VST3_MIDI_CONTROLLER_CLOCK_CONTINUE => raw_system_event(event, 0xfb, 1),
        VST3_MIDI_CONTROLLER_CLOCK_STOP => raw_system_event(event, 0xfc, 1),
        VST3_MIDI_CONTROLLER_ACTIVE_SENSING => raw_system_event(event, 0xfe, 1),
        _ => Err(format!(
            "unsupported VST3 legacy MIDI control number {}",
            event.control_number
        )),
    }
}

fn raw_channel_event(
    event: Vst3LegacyMidiCcOutEvent,
    status_base: u8,
    data_len: u8,
) -> Result<MidiEvent, String> {
    MidiEvent::raw_midi(
        event.sample_offset,
        status_base | event.channel,
        event.value,
        event.value2,
        data_len,
    )
    .map_err(|error| error.to_string())
}

fn raw_system_event(
    event: Vst3LegacyMidiCcOutEvent,
    status: u8,
    data_len: u8,
) -> Result<MidiEvent, String> {
    MidiEvent::raw_midi(
        event.sample_offset,
        status,
        event.value,
        event.value2,
        data_len,
    )
    .map_err(|error| error.to_string())
}

fn midi_note_event(kind: MidiEventKind, event: Vst3NoteEvent) -> Result<MidiEvent, String> {
    let mut midi_event = MidiEvent::new(
        event.sample_offset,
        kind,
        event.channel,
        event.pitch,
        unit_value_to_midi(event.velocity),
    )
    .map_err(|error| error.to_string())?;
    midi_event.note_id = midi_note_id(event.note_id);
    Ok(midi_event)
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

fn unit_value_to_midi(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 127.0).round() as u8
}

fn midi_note_id(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

fn vst3_note_id(value: u32) -> i32 {
    if value == 0 || value > i32::MAX as u32 {
        -1
    } else {
        value as i32
    }
}
