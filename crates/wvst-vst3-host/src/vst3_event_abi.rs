use std::ffi::c_void;

pub const VST3_EVENT_TYPE_NOTE_ON: u16 = 0;
pub const VST3_EVENT_TYPE_NOTE_OFF: u16 = 1;
pub const VST3_EVENT_TYPE_DATA: u16 = 2;
pub const VST3_EVENT_TYPE_POLY_PRESSURE: u16 = 3;
pub const VST3_EVENT_TYPE_NOTE_EXPRESSION_VALUE: u16 = 4;
pub const VST3_EVENT_TYPE_NOTE_EXPRESSION_TEXT: u16 = 5;
pub const VST3_EVENT_TYPE_CHORD: u16 = 6;
pub const VST3_EVENT_TYPE_SCALE: u16 = 7;
pub const VST3_EVENT_TYPE_NOTE_EXPRESSION_INT_VALUE: u16 = 8;
pub const VST3_EVENT_TYPE_LEGACY_MIDI_CC_OUT: u16 = 65_535;

#[repr(C)]
#[derive(Debug)]
pub struct IEventList {
    pub vtable: *const IEventListVTable,
}

#[repr(C)]
pub struct IEventListVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IEventList,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IEventList) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IEventList) -> u32,
    pub get_event_count: unsafe extern "system" fn(this: *mut IEventList) -> i32,
    pub get_event:
        unsafe extern "system" fn(this: *mut IEventList, index: i32, event: *mut Event) -> i32,
    pub add_event: unsafe extern "system" fn(this: *mut IEventList, event: *mut Event) -> i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub bus_index: i32,
    pub sample_offset: i32,
    pub ppq_position: f64,
    pub flags: u16,
    pub event_type: u16,
    pub payload: EventPayload,
}

impl Default for Event {
    fn default() -> Self {
        Self {
            bus_index: 0,
            sample_offset: 0,
            ppq_position: 0.0,
            flags: 0,
            event_type: 0,
            payload: EventPayload { raw: [0; 4] },
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union EventPayload {
    pub note_on: NoteOnEvent,
    pub note_off: NoteOffEvent,
    pub data: DataEvent,
    pub poly_pressure: PolyPressureEvent,
    pub note_expression_value: NoteExpressionValueEvent,
    pub note_expression_text: NoteExpressionTextEvent,
    pub note_expression_int_value: NoteExpressionIntValueEvent,
    pub chord: ChordEvent,
    pub scale: ScaleEvent,
    pub midi_cc_out: LegacyMidiCcOutEvent,
    pub raw: [u64; 4],
}

impl std::fmt::Debug for EventPayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("EventPayload")
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteOnEvent {
    pub channel: i16,
    pub pitch: i16,
    pub tuning: f32,
    pub velocity: f32,
    pub length: i32,
    pub note_id: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteOffEvent {
    pub channel: i16,
    pub pitch: i16,
    pub tuning: f32,
    pub velocity: f32,
    pub note_id: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataEvent {
    pub size: u32,
    pub event_type: u32,
    pub bytes: *const u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolyPressureEvent {
    pub channel: i16,
    pub pitch: i16,
    pub pressure: f32,
    pub note_id: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteExpressionValueEvent {
    pub type_id: u32,
    pub note_id: i32,
    pub value: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteExpressionTextEvent {
    pub type_id: u32,
    pub note_id: i32,
    pub text_len: u32,
    pub text: *const u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteExpressionIntValueEvent {
    pub type_id: u32,
    pub note_id: i32,
    pub value: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChordEvent {
    pub root: i16,
    pub bass_note: i16,
    pub mask: i16,
    pub text_len: u16,
    pub text: *const u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleEvent {
    pub root: i16,
    pub mask: i16,
    pub text_len: u16,
    pub text: *const u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegacyMidiCcOutEvent {
    pub control_number: u8,
    pub channel: i8,
    pub value: i8,
    pub value2: i8,
}
