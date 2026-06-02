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
}
