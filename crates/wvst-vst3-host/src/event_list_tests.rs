use crate::event_list::{
    Vst3AdvancedOutputEventKind, Vst3EventList, Vst3InputEvent, Vst3LegacyMidiCcOutEvent,
    Vst3NoteEvent, Vst3OutputEvent,
};
use crate::vst3_abi::{
    Event, EventPayload, LegacyMidiCcOutEvent, NoteOffEvent, NoteOnEvent, PolyPressureEvent,
    VST3_EVENT_TYPE_CHORD, VST3_EVENT_TYPE_DATA, VST3_EVENT_TYPE_LEGACY_MIDI_CC_OUT,
    VST3_EVENT_TYPE_NOTE_EXPRESSION_INT_VALUE, VST3_EVENT_TYPE_NOTE_EXPRESSION_TEXT,
    VST3_EVENT_TYPE_NOTE_EXPRESSION_VALUE, VST3_EVENT_TYPE_NOTE_OFF, VST3_EVENT_TYPE_NOTE_ON,
    VST3_EVENT_TYPE_POLY_PRESSURE, VST3_EVENT_TYPE_SCALE,
};
use crate::vst3_event_abi::{
    ChordEvent, DataEvent, NoteExpressionIntValueEvent, NoteExpressionTextEvent,
    NoteExpressionValueEvent, ScaleEvent,
};
use crate::{
    VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES, VST3_OUTPUT_PAYLOAD_ENCODING_RAW_BYTES,
    VST3_OUTPUT_PAYLOAD_ENCODING_UTF8, VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED,
};

#[test]
fn exposes_events_through_vst3_event_list_vtable() {
    let mut list = Vst3EventList::new(8);
    list.set_events(
        128,
        &[Vst3InputEvent::NoteOn(Vst3NoteEvent {
            sample_offset: 12,
            channel: 1,
            pitch: 60,
            velocity: 0.5,
            note_id: -1,
        })],
    )
    .expect("events");

    let ptr = list.as_raw_ptr();
    let mut event = Event::default();
    let count = unsafe { ((*(*ptr).vtable).get_event_count)(ptr) };
    let result = unsafe { ((*(*ptr).vtable).get_event)(ptr, 0, &mut event) };

    assert_eq!(count, 1);
    assert_eq!(result, 0);
    assert_eq!(event.sample_offset, 12);
    assert_eq!(event.event_type, VST3_EVENT_TYPE_NOTE_ON);
    assert_eq!(unsafe { event.payload.note_on.pitch }, 60);
}

#[test]
fn normalizes_output_events_and_skips_invalid_payloads() {
    let mut list = Vst3EventList::new(8);
    let ptr = list.as_raw_ptr();
    let mut note_on = Event {
        sample_offset: 2,
        event_type: VST3_EVENT_TYPE_NOTE_ON,
        payload: EventPayload {
            note_on: NoteOnEvent {
                channel: 1,
                pitch: 60,
                tuning: 0.0,
                velocity: 0.5,
                length: 0,
                note_id: 10,
            },
        },
        ..Event::default()
    };
    let mut bad_offset = Event {
        sample_offset: 8,
        event_type: VST3_EVENT_TYPE_NOTE_OFF,
        payload: EventPayload {
            note_off: NoteOffEvent {
                channel: 1,
                pitch: 60,
                tuning: 0.0,
                velocity: 0.5,
                note_id: 10,
            },
        },
        ..Event::default()
    };
    let mut bad_channel = Event {
        sample_offset: 3,
        event_type: VST3_EVENT_TYPE_POLY_PRESSURE,
        payload: EventPayload {
            poly_pressure: PolyPressureEvent {
                channel: 16,
                pitch: 60,
                pressure: 0.5,
                note_id: 10,
            },
        },
        ..Event::default()
    };
    let mut legacy_cc = Event {
        sample_offset: 5,
        event_type: VST3_EVENT_TYPE_LEGACY_MIDI_CC_OUT,
        payload: EventPayload {
            midi_cc_out: LegacyMidiCcOutEvent {
                control_number: 7,
                channel: 2,
                value: 100,
                value2: 0,
            },
        },
        ..Event::default()
    };
    let mut bad_legacy_cc = Event {
        sample_offset: 6,
        event_type: VST3_EVENT_TYPE_LEGACY_MIDI_CC_OUT,
        payload: EventPayload {
            midi_cc_out: LegacyMidiCcOutEvent {
                control_number: 200,
                channel: 2,
                value: 100,
                value2: 0,
            },
        },
        ..Event::default()
    };
    let mut unknown_type = Event {
        sample_offset: 4,
        event_type: 99,
        ..Event::default()
    };
    unsafe {
        ((*(*ptr).vtable).add_event)(ptr, &mut note_on);
        ((*(*ptr).vtable).add_event)(ptr, &mut bad_offset);
        ((*(*ptr).vtable).add_event)(ptr, &mut bad_channel);
        ((*(*ptr).vtable).add_event)(ptr, &mut legacy_cc);
        ((*(*ptr).vtable).add_event)(ptr, &mut bad_legacy_cc);
        ((*(*ptr).vtable).add_event)(ptr, &mut unknown_type);
    }
    let mut events = Vec::new();

    let stats = list.output_events_into(8, &mut events);

    assert_eq!(
        events,
        vec![
            Vst3OutputEvent::NoteOn(Vst3NoteEvent {
                sample_offset: 2,
                channel: 1,
                pitch: 60,
                velocity: 0.5,
                note_id: 10,
            }),
            Vst3OutputEvent::LegacyMidiCcOut(Vst3LegacyMidiCcOutEvent {
                sample_offset: 5,
                control_number: 7,
                channel: 2,
                value: 100,
                value2: 0,
            })
        ]
    );
    assert_eq!(stats.raw_events, 6);
    assert_eq!(stats.normalized_events, 2);
    assert_eq!(stats.filtered_events, 4);
    assert_eq!(stats.invalid_sample_offset_events, 1);
    assert_eq!(stats.invalid_payload_events, 2);
    assert_eq!(stats.advanced_events, 0);
    assert_eq!(stats.unknown_type_events, 1);
}

#[test]
fn normalizes_known_advanced_output_events_separately_from_unknown_types() {
    let mut list = Vst3EventList::new(8);
    let ptr = list.as_raw_ptr();
    let data_bytes = [0xab; 80];
    let expression_text: [u16; 5] = [
        b'h' as u16,
        b'e' as u16,
        b'l' as u16,
        b'l' as u16,
        b'o' as u16,
    ];
    let chord_text: [u16; 2] = [b'C' as u16, b'M' as u16];
    let scale_text: [u16; 5] = [
        b'm' as u16,
        b'a' as u16,
        b'j' as u16,
        b'o' as u16,
        b'r' as u16,
    ];
    let mut data = Event {
        sample_offset: 1,
        event_type: VST3_EVENT_TYPE_DATA,
        payload: EventPayload {
            data: DataEvent {
                size: data_bytes.len() as u32,
                event_type: 7,
                bytes: data_bytes.as_ptr(),
            },
        },
        ..Event::default()
    };
    let mut note_expression_value = Event {
        sample_offset: 2,
        event_type: VST3_EVENT_TYPE_NOTE_EXPRESSION_VALUE,
        payload: EventPayload {
            note_expression_value: NoteExpressionValueEvent {
                type_id: 42,
                note_id: 10,
                value: 0.75,
            },
        },
        ..Event::default()
    };
    let mut note_expression_text = Event {
        sample_offset: 3,
        event_type: VST3_EVENT_TYPE_NOTE_EXPRESSION_TEXT,
        payload: EventPayload {
            note_expression_text: NoteExpressionTextEvent {
                type_id: 43,
                note_id: 11,
                text_len: 5,
                text: expression_text.as_ptr(),
            },
        },
        ..Event::default()
    };
    let mut note_expression_int_value = Event {
        sample_offset: 4,
        event_type: VST3_EVENT_TYPE_NOTE_EXPRESSION_INT_VALUE,
        payload: EventPayload {
            note_expression_int_value: NoteExpressionIntValueEvent {
                type_id: 44,
                note_id: 12,
                value: 123,
            },
        },
        ..Event::default()
    };
    let mut chord = Event {
        sample_offset: 5,
        event_type: VST3_EVENT_TYPE_CHORD,
        payload: EventPayload {
            chord: ChordEvent {
                root: 0,
                bass_note: 7,
                mask: 0x91,
                text_len: 2,
                text: chord_text.as_ptr(),
            },
        },
        ..Event::default()
    };
    let mut scale = Event {
        sample_offset: 6,
        event_type: VST3_EVENT_TYPE_SCALE,
        payload: EventPayload {
            scale: ScaleEvent {
                root: 2,
                mask: 0x5ab5,
                text_len: 4,
                text: scale_text.as_ptr(),
            },
        },
        ..Event::default()
    };
    let mut bad_note_expression_value = Event {
        sample_offset: 7,
        event_type: VST3_EVENT_TYPE_NOTE_EXPRESSION_VALUE,
        payload: EventPayload {
            note_expression_value: NoteExpressionValueEvent {
                type_id: 45,
                note_id: 13,
                value: f64::NAN,
            },
        },
        ..Event::default()
    };
    let mut unknown = output_event(8, 9_999);

    unsafe {
        ((*(*ptr).vtable).add_event)(ptr, &mut data);
        ((*(*ptr).vtable).add_event)(ptr, &mut note_expression_value);
        ((*(*ptr).vtable).add_event)(ptr, &mut note_expression_text);
        ((*(*ptr).vtable).add_event)(ptr, &mut note_expression_int_value);
        ((*(*ptr).vtable).add_event)(ptr, &mut chord);
        ((*(*ptr).vtable).add_event)(ptr, &mut scale);
        ((*(*ptr).vtable).add_event)(ptr, &mut bad_note_expression_value);
        ((*(*ptr).vtable).add_event)(ptr, &mut unknown);
    }
    let mut events = Vec::new();
    let mut advanced_events = Vec::new();

    let stats = list.output_events_and_advanced_into(16, &mut events, &mut advanced_events);

    assert!(events.is_empty());
    assert_eq!(advanced_events.len(), 6);
    assert_eq!(advanced_events[0].kind, Vst3AdvancedOutputEventKind::Data);
    assert_eq!(advanced_events[0].data_size, 80);
    assert_eq!(advanced_events[0].data_type, 7);
    assert_eq!(
        advanced_events[0].payload_size,
        VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES as u16
    );
    assert_eq!(
        advanced_events[0].payload_encoding,
        VST3_OUTPUT_PAYLOAD_ENCODING_RAW_BYTES
    );
    assert_eq!(
        advanced_events[0].payload_flags & VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED,
        VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED
    );
    assert_eq!(advanced_events[0].payload_bytes(), &[0xab; 64]);
    assert_eq!(
        advanced_events[1].kind,
        Vst3AdvancedOutputEventKind::NoteExpressionValue
    );
    assert_eq!(advanced_events[1].data1, 10);
    assert_eq!(advanced_events[1].value, 0.75);
    assert_eq!(advanced_events[1].data_type, 42);
    assert_eq!(
        advanced_events[2].kind,
        Vst3AdvancedOutputEventKind::NoteExpressionText
    );
    assert_eq!(advanced_events[2].data1, 11);
    assert_eq!(advanced_events[2].data_size, 5);
    assert_eq!(advanced_events[2].data_type, 43);
    assert_eq!(
        advanced_events[2].payload_encoding,
        VST3_OUTPUT_PAYLOAD_ENCODING_UTF8
    );
    assert_eq!(advanced_events[2].payload_bytes(), b"hello");
    assert_eq!(
        advanced_events[3].kind,
        Vst3AdvancedOutputEventKind::NoteExpressionIntValue
    );
    assert_eq!(advanced_events[3].data1, 12);
    assert_eq!(advanced_events[3].data2, 123);
    assert_eq!(advanced_events[3].data_type, 44);
    assert_eq!(advanced_events[4].kind, Vst3AdvancedOutputEventKind::Chord);
    assert_eq!(advanced_events[4].data1, 0);
    assert_eq!(advanced_events[4].data2, 7);
    assert_eq!(advanced_events[4].data_size, 2);
    assert_eq!(advanced_events[4].data_type, 0x91);
    assert_eq!(advanced_events[4].payload_bytes(), b"CM");
    assert_eq!(advanced_events[5].kind, Vst3AdvancedOutputEventKind::Scale);
    assert_eq!(advanced_events[5].data1, 2);
    assert_eq!(advanced_events[5].data2, 0x5ab5);
    assert_eq!(advanced_events[5].data_size, 4);
    assert_eq!(advanced_events[5].payload_bytes(), b"majo");
    assert_eq!(stats.raw_events, 8);
    assert_eq!(stats.normalized_events, 6);
    assert_eq!(stats.filtered_events, 2);
    assert_eq!(stats.invalid_payload_events, 1);
    assert_eq!(stats.advanced_events, 6);
    assert_eq!(stats.advanced_data_events, 1);
    assert_eq!(stats.advanced_note_expression_events, 3);
    assert_eq!(stats.advanced_chord_events, 1);
    assert_eq!(stats.advanced_scale_events, 1);
    assert_eq!(stats.advanced_payload_events, 4);
    assert_eq!(stats.advanced_payload_bytes, 75);
    assert_eq!(stats.advanced_raw_payload_events, 1);
    assert_eq!(stats.advanced_text_payload_events, 3);
    assert_eq!(stats.advanced_truncated_payload_events, 1);
    assert_eq!(stats.advanced_unavailable_payload_events, 0);
    assert_eq!(stats.advanced_invalid_text_payload_events, 0);
    assert_eq!(stats.unknown_type_events, 1);
}

fn output_event(sample_offset: i32, event_type: u16) -> Event {
    Event {
        sample_offset,
        event_type,
        ..Event::default()
    }
}
