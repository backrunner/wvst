use std::ffi::c_void;
use std::ptr;

use serde::Serialize;

use crate::event_payload_capture::{
    CapturedVst3OutputPayload, VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES,
    VST3_OUTPUT_PAYLOAD_ENCODING_NONE,
};
use crate::vst3_abi::{
    Event, EventPayload, IEventList, IEventListVTable, NoteOffEvent, NoteOnEvent,
    PolyPressureEvent, VST3_EVENT_TYPE_NOTE_OFF, VST3_EVENT_TYPE_NOTE_ON,
    VST3_EVENT_TYPE_POLY_PRESSURE,
};
use crate::{HostError, HostResult};

#[path = "event_list/output.rs"]
mod output;

use output::{
    NormalizedOutputEvent, OutputEventFilterReason, from_vst3_event, observe_advanced_payload_stats,
};

pub const DEFAULT_MAX_VST3_EVENTS_PER_BLOCK: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Vst3InputEvent {
    NoteOn(Vst3NoteEvent),
    NoteOff(Vst3NoteEvent),
    PolyPressure(Vst3PolyPressureEvent),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vst3NoteEvent {
    pub sample_offset: u16,
    pub channel: u8,
    pub pitch: u8,
    pub velocity: f32,
    pub note_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vst3PolyPressureEvent {
    pub sample_offset: u16,
    pub channel: u8,
    pub pitch: u8,
    pub pressure: f32,
    pub note_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Vst3OutputEvent {
    NoteOn(Vst3NoteEvent),
    NoteOff(Vst3NoteEvent),
    PolyPressure(Vst3PolyPressureEvent),
    LegacyMidiCcOut(Vst3LegacyMidiCcOutEvent),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Vst3AdvancedOutputEventKind {
    Data,
    NoteExpressionValue,
    NoteExpressionText,
    NoteExpressionIntValue,
    Chord,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vst3AdvancedOutputEvent {
    pub sample_offset: u16,
    pub kind: Vst3AdvancedOutputEventKind,
    pub vst3_event_type: u16,
    pub bus_index: i32,
    pub data1: i32,
    pub data2: i32,
    pub value: f64,
    pub data_size: u32,
    pub data_type: u32,
    pub payload_size: u16,
    pub payload_encoding: u8,
    pub payload_flags: u8,
    pub payload: [u8; VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES],
}

impl Vst3AdvancedOutputEvent {
    pub fn payload_bytes(&self) -> &[u8] {
        let payload_size = usize::from(self.payload_size).min(self.payload.len());
        &self.payload[..payload_size]
    }
}

impl Default for Vst3AdvancedOutputEvent {
    fn default() -> Self {
        Self {
            sample_offset: 0,
            kind: Vst3AdvancedOutputEventKind::Data,
            vst3_event_type: 0,
            bus_index: 0,
            data1: 0,
            data2: 0,
            value: 0.0,
            data_size: 0,
            data_type: 0,
            payload_size: 0,
            payload_encoding: VST3_OUTPUT_PAYLOAD_ENCODING_NONE,
            payload_flags: 0,
            payload: [0; VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vst3LegacyMidiCcOutEvent {
    pub sample_offset: u16,
    pub control_number: u8,
    pub channel: u8,
    pub value: u8,
    pub value2: u8,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3OutputEventStats {
    pub raw_events: u32,
    pub normalized_events: u32,
    pub filtered_events: u32,
    pub invalid_sample_offset_events: u32,
    pub invalid_payload_events: u32,
    pub advanced_events: u32,
    pub advanced_data_events: u32,
    pub advanced_note_expression_events: u32,
    pub advanced_chord_events: u32,
    pub advanced_scale_events: u32,
    pub advanced_payload_events: u32,
    pub advanced_payload_bytes: u32,
    pub advanced_raw_payload_events: u32,
    pub advanced_text_payload_events: u32,
    pub advanced_truncated_payload_events: u32,
    pub advanced_unavailable_payload_events: u32,
    pub advanced_invalid_text_payload_events: u32,
    pub unknown_type_events: u32,
}

#[derive(Debug)]
pub struct Vst3EventList {
    object: Box<EventListObject>,
}

impl Vst3EventList {
    pub fn new(max_events: usize) -> Self {
        Self {
            object: Box::new(EventListObject {
                iface: IEventList {
                    vtable: &EVENT_LIST_VTABLE,
                },
                events: Vec::with_capacity(max_events),
                captured_payloads: Vec::with_capacity(max_events),
                max_events,
            }),
        }
    }

    pub fn as_raw_ptr(&mut self) -> *mut IEventList {
        &mut self.object.iface
    }

    pub fn clear(&mut self) {
        self.object.events.clear();
        self.object.captured_payloads.clear();
    }

    pub fn events(&self) -> &[Event] {
        &self.object.events
    }

    pub fn output_events_into(
        &self,
        frames: usize,
        destination: &mut Vec<Vst3OutputEvent>,
    ) -> Vst3OutputEventStats {
        let mut advanced = Vec::new();
        self.output_events_and_advanced_into(frames, destination, &mut advanced)
    }

    pub fn output_events_and_advanced_into(
        &self,
        frames: usize,
        destination: &mut Vec<Vst3OutputEvent>,
        advanced_destination: &mut Vec<Vst3AdvancedOutputEvent>,
    ) -> Vst3OutputEventStats {
        destination.clear();
        advanced_destination.clear();
        let mut stats = Vst3OutputEventStats {
            raw_events: self.object.events.len() as u32,
            ..Vst3OutputEventStats::default()
        };
        for (index, event) in self.object.events.iter().enumerate() {
            let captured_payload = self
                .object
                .captured_payloads
                .get(index)
                .copied()
                .unwrap_or_default();
            match from_vst3_event(frames, *event, captured_payload) {
                Ok(NormalizedOutputEvent::Midi(event)) => {
                    stats.normalized_events = stats.normalized_events.saturating_add(1);
                    destination.push(event);
                }
                Ok(NormalizedOutputEvent::Advanced(event)) => {
                    stats.normalized_events = stats.normalized_events.saturating_add(1);
                    stats.advanced_events = stats.advanced_events.saturating_add(1);
                    match event.kind {
                        Vst3AdvancedOutputEventKind::Data => {
                            stats.advanced_data_events =
                                stats.advanced_data_events.saturating_add(1);
                        }
                        Vst3AdvancedOutputEventKind::NoteExpressionValue
                        | Vst3AdvancedOutputEventKind::NoteExpressionText
                        | Vst3AdvancedOutputEventKind::NoteExpressionIntValue => {
                            stats.advanced_note_expression_events =
                                stats.advanced_note_expression_events.saturating_add(1);
                        }
                        Vst3AdvancedOutputEventKind::Chord => {
                            stats.advanced_chord_events =
                                stats.advanced_chord_events.saturating_add(1);
                        }
                        Vst3AdvancedOutputEventKind::Scale => {
                            stats.advanced_scale_events =
                                stats.advanced_scale_events.saturating_add(1);
                        }
                    }
                    observe_advanced_payload_stats(&mut stats, event);
                    advanced_destination.push(event);
                }
                Err(reason) => {
                    stats.filtered_events = stats.filtered_events.saturating_add(1);
                    match reason {
                        OutputEventFilterReason::InvalidSampleOffset => {
                            stats.invalid_sample_offset_events =
                                stats.invalid_sample_offset_events.saturating_add(1);
                        }
                        OutputEventFilterReason::InvalidPayload => {
                            stats.invalid_payload_events =
                                stats.invalid_payload_events.saturating_add(1);
                        }
                        OutputEventFilterReason::UnknownType => {
                            stats.unknown_type_events = stats.unknown_type_events.saturating_add(1);
                        }
                    }
                }
            }
        }
        stats
    }

    pub fn set_events(&mut self, frames: usize, events: &[Vst3InputEvent]) -> HostResult<()> {
        if events.len() > self.object.max_events {
            return Err(HostError::InvalidEventCount {
                max: self.object.max_events,
                actual: events.len(),
            });
        }

        self.object.events.clear();
        self.object.captured_payloads.clear();
        for event in events {
            self.object.events.push(to_vst3_event(frames, *event)?);
            self.object
                .captured_payloads
                .push(CapturedVst3OutputPayload::empty());
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Debug)]
struct EventListObject {
    iface: IEventList,
    events: Vec<Event>,
    captured_payloads: Vec<CapturedVst3OutputPayload>,
    max_events: usize,
}

const EVENT_LIST_VTABLE: IEventListVTable = IEventListVTable {
    query_interface: event_list_query_interface,
    add_ref: event_list_add_ref,
    release: event_list_release,
    get_event_count: event_list_get_event_count,
    get_event: event_list_get_event,
    add_event: event_list_add_event,
};

unsafe extern "system" fn event_list_query_interface(
    _this: *mut IEventList,
    _iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        // SAFETY: `obj` is an out pointer provided by the caller. A null
        // response means this MVP event list does not expose extra interfaces.
        unsafe { *obj = ptr::null_mut() };
    }
    1
}

unsafe extern "system" fn event_list_add_ref(_this: *mut IEventList) -> u32 {
    1
}

unsafe extern "system" fn event_list_release(_this: *mut IEventList) -> u32 {
    1
}

unsafe extern "system" fn event_list_get_event_count(this: *mut IEventList) -> i32 {
    event_list_object(this).map_or(0, |object| object.events.len() as i32)
}

unsafe extern "system" fn event_list_get_event(
    this: *mut IEventList,
    index: i32,
    event: *mut Event,
) -> i32 {
    if event.is_null() || index < 0 {
        return 1;
    }

    let Some(object) = event_list_object(this) else {
        return 1;
    };
    let Some(source) = object.events.get(index as usize) else {
        return 1;
    };

    // SAFETY: `event` was checked for null and points to caller-owned Event
    // storage. We copy a plain repr(C) Event value.
    unsafe { *event = *source };
    0
}

unsafe extern "system" fn event_list_add_event(this: *mut IEventList, event: *mut Event) -> i32 {
    if event.is_null() {
        return 1;
    }

    let Some(object) = event_list_object_mut(this) else {
        return 1;
    };
    if object.events.len() >= object.max_events {
        return 1;
    }

    // SAFETY: `event` was checked for null and is only copied immediately.
    let event = unsafe { *event };
    let captured_payload = CapturedVst3OutputPayload::for_event(event);
    object.events.push(event);
    object.captured_payloads.push(captured_payload);
    0
}

fn event_list_object(this: *mut IEventList) -> Option<&'static EventListObject> {
    if this.is_null() {
        return None;
    }

    // SAFETY: Vst3EventList exposes pointers to EventListObject.iface, which is
    // the first field of the repr(C) object. The object is boxed, so moves of
    // Vst3EventList do not invalidate the address.
    Some(unsafe { &*this.cast::<EventListObject>() })
}

fn event_list_object_mut(this: *mut IEventList) -> Option<&'static mut EventListObject> {
    if this.is_null() {
        return None;
    }

    // SAFETY: See event_list_object. Mutable access is only used through the
    // VST3 process callback while WVST holds exclusive access to buffers.
    Some(unsafe { &mut *this.cast::<EventListObject>() })
}

fn to_vst3_event(frames: usize, event: Vst3InputEvent) -> HostResult<Event> {
    let sample_offset = match event {
        Vst3InputEvent::NoteOn(event) => event.sample_offset,
        Vst3InputEvent::NoteOff(event) => event.sample_offset,
        Vst3InputEvent::PolyPressure(event) => event.sample_offset,
    };
    if usize::from(sample_offset) >= frames {
        return Err(HostError::InvalidEventSampleOffset {
            frames,
            actual: usize::from(sample_offset),
        });
    }

    Ok(match event {
        Vst3InputEvent::NoteOn(event) => note_on_event(event),
        Vst3InputEvent::NoteOff(event) => note_off_event(event),
        Vst3InputEvent::PolyPressure(event) => poly_pressure_event(event),
    })
}

fn event_header(sample_offset: u16, event_type: u16) -> Event {
    Event {
        bus_index: 0,
        sample_offset: i32::from(sample_offset),
        ppq_position: 0.0,
        flags: 0,
        event_type,
        payload: EventPayload { raw: [0; 4] },
    }
}

fn note_on_event(event: Vst3NoteEvent) -> Event {
    let mut vst_event = event_header(event.sample_offset, VST3_EVENT_TYPE_NOTE_ON);
    vst_event.payload = EventPayload {
        note_on: NoteOnEvent {
            channel: i16::from(event.channel),
            pitch: i16::from(event.pitch),
            tuning: 0.0,
            velocity: event.velocity,
            length: 0,
            note_id: event.note_id,
        },
    };
    vst_event
}

fn note_off_event(event: Vst3NoteEvent) -> Event {
    let mut vst_event = event_header(event.sample_offset, VST3_EVENT_TYPE_NOTE_OFF);
    vst_event.payload = EventPayload {
        note_off: NoteOffEvent {
            channel: i16::from(event.channel),
            pitch: i16::from(event.pitch),
            tuning: 0.0,
            velocity: event.velocity,
            note_id: event.note_id,
        },
    };
    vst_event
}

fn poly_pressure_event(event: Vst3PolyPressureEvent) -> Event {
    let mut vst_event = event_header(event.sample_offset, VST3_EVENT_TYPE_POLY_PRESSURE);
    vst_event.payload = EventPayload {
        poly_pressure: PolyPressureEvent {
            channel: i16::from(event.channel),
            pitch: i16::from(event.pitch),
            pressure: event.pressure,
            note_id: event.note_id,
        },
    };
    vst_event
}
