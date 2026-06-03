use wvst_vst3_host::{
    Vst3AdvancedOutputEvent, Vst3InputEvent, Vst3LegacyMidiCcOutEvent, Vst3NoteEvent,
    Vst3OutputEvent, Vst3ParameterChange, Vst3PolyPressureEvent,
};

pub(super) fn sort_input_events_by_sample_offset(events: &mut [Vst3InputEvent]) {
    events.sort_by_key(|event| input_event_sample_offset(*event));
}

pub(super) fn sort_output_events_by_sample_offset(events: &mut [Vst3OutputEvent]) {
    events.sort_by_key(|event| output_event_sample_offset(*event));
}

pub(super) fn sort_advanced_output_events_by_sample_offset(events: &mut [Vst3AdvancedOutputEvent]) {
    events.sort_by_key(|event| event.sample_offset);
}

pub(super) fn sort_parameter_changes_by_sample_offset(changes: &mut [Vst3ParameterChange]) {
    changes.sort_by_key(|change| (change.sample_offset, change.parameter_id));
}

fn input_event_sample_offset(event: Vst3InputEvent) -> u16 {
    match event {
        Vst3InputEvent::NoteOn(event) | Vst3InputEvent::NoteOff(event) => note_sample_offset(event),
        Vst3InputEvent::PolyPressure(event) => poly_pressure_sample_offset(event),
    }
}

fn output_event_sample_offset(event: Vst3OutputEvent) -> u16 {
    match event {
        Vst3OutputEvent::NoteOn(event) | Vst3OutputEvent::NoteOff(event) => {
            note_sample_offset(event)
        }
        Vst3OutputEvent::PolyPressure(event) => poly_pressure_sample_offset(event),
        Vst3OutputEvent::LegacyMidiCcOut(event) => legacy_midi_cc_out_sample_offset(event),
    }
}

const fn note_sample_offset(event: Vst3NoteEvent) -> u16 {
    event.sample_offset
}

const fn poly_pressure_sample_offset(event: Vst3PolyPressureEvent) -> u16 {
    event.sample_offset
}

const fn legacy_midi_cc_out_sample_offset(event: Vst3LegacyMidiCcOutEvent) -> u16 {
    event.sample_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_input_events_stably_by_sample_offset() {
        let mut events = vec![
            Vst3InputEvent::NoteOn(note(12, 60)),
            Vst3InputEvent::NoteOff(note(4, 60)),
            Vst3InputEvent::NoteOn(note(4, 61)),
        ];

        sort_input_events_by_sample_offset(&mut events);

        assert_eq!(
            events,
            vec![
                Vst3InputEvent::NoteOff(note(4, 60)),
                Vst3InputEvent::NoteOn(note(4, 61)),
                Vst3InputEvent::NoteOn(note(12, 60)),
            ]
        );
    }

    #[test]
    fn sorts_output_events_with_legacy_midi_by_sample_offset() {
        let mut events = vec![
            Vst3OutputEvent::LegacyMidiCcOut(legacy_midi_cc_out(12, 7)),
            Vst3OutputEvent::NoteOn(note(4, 60)),
            Vst3OutputEvent::LegacyMidiCcOut(legacy_midi_cc_out(4, 74)),
        ];

        sort_output_events_by_sample_offset(&mut events);

        assert_eq!(
            events,
            vec![
                Vst3OutputEvent::NoteOn(note(4, 60)),
                Vst3OutputEvent::LegacyMidiCcOut(legacy_midi_cc_out(4, 74)),
                Vst3OutputEvent::LegacyMidiCcOut(legacy_midi_cc_out(12, 7)),
            ]
        );
    }

    #[test]
    fn sorts_parameter_changes_by_sample_offset_then_parameter_id() {
        let mut changes = vec![
            parameter_change(8, 20),
            parameter_change(1, 30),
            parameter_change(1, 10),
        ];

        sort_parameter_changes_by_sample_offset(&mut changes);

        assert_eq!(
            changes,
            vec![
                parameter_change(1, 10),
                parameter_change(1, 30),
                parameter_change(8, 20),
            ]
        );
    }

    fn note(sample_offset: u16, pitch: u8) -> Vst3NoteEvent {
        Vst3NoteEvent {
            sample_offset,
            channel: 0,
            pitch,
            velocity: 1.0,
            note_id: -1,
        }
    }

    const fn legacy_midi_cc_out(
        sample_offset: u16,
        control_number: u8,
    ) -> Vst3LegacyMidiCcOutEvent {
        Vst3LegacyMidiCcOutEvent {
            sample_offset,
            control_number,
            channel: 0,
            value: 64,
            value2: 0,
        }
    }

    const fn parameter_change(sample_offset: u16, parameter_id: u32) -> Vst3ParameterChange {
        Vst3ParameterChange {
            sample_offset,
            parameter_id,
            value_normalized: 0.5,
        }
    }
}
