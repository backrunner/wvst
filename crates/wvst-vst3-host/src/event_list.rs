use std::ffi::c_void;
use std::ptr;

use crate::vst3_abi::{
    Event, EventPayload, IEventList, IEventListVTable, NoteOffEvent, NoteOnEvent,
    PolyPressureEvent, VST3_EVENT_TYPE_NOTE_OFF, VST3_EVENT_TYPE_NOTE_ON,
    VST3_EVENT_TYPE_POLY_PRESSURE,
};
use crate::{HostError, HostResult};

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
                max_events,
            }),
        }
    }

    pub fn as_raw_ptr(&mut self) -> *mut IEventList {
        &mut self.object.iface
    }

    pub fn clear(&mut self) {
        self.object.events.clear();
    }

    pub fn events(&self) -> &[Event] {
        &self.object.events
    }

    pub fn output_events_into(&self, frames: usize, destination: &mut Vec<Vst3OutputEvent>) {
        destination.clear();
        for event in &self.object.events {
            if let Some(event) = from_vst3_event(frames, *event) {
                destination.push(event);
            }
        }
    }

    pub fn set_events(&mut self, frames: usize, events: &[Vst3InputEvent]) -> HostResult<()> {
        if events.len() > self.object.max_events {
            return Err(HostError::InvalidEventCount {
                max: self.object.max_events,
                actual: events.len(),
            });
        }

        self.object.events.clear();
        for event in events {
            self.object.events.push(to_vst3_event(frames, *event)?);
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Debug)]
struct EventListObject {
    iface: IEventList,
    events: Vec<Event>,
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
    object.events.push(unsafe { *event });
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

fn from_vst3_event(frames: usize, event: Event) -> Option<Vst3OutputEvent> {
    if event.sample_offset < 0 || event.sample_offset as usize >= frames {
        return None;
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
        }
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        unsafe {
            ((*(*ptr).vtable).add_event)(ptr, &mut note_on);
            ((*(*ptr).vtable).add_event)(ptr, &mut bad_offset);
            ((*(*ptr).vtable).add_event)(ptr, &mut bad_channel);
        }
        let mut events = Vec::new();

        list.output_events_into(8, &mut events);

        assert_eq!(
            events,
            vec![Vst3OutputEvent::NoteOn(Vst3NoteEvent {
                sample_offset: 2,
                channel: 1,
                pitch: 60,
                velocity: 0.5,
                note_id: 10,
            })]
        );
    }
}
