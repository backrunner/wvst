#![allow(dead_code)]
// Raw VST3 ABI declarations are exposed to this crate incrementally through
// safe facades; not every vtable slot is called in the same milestone.

use std::ffi::{c_char, c_void};

pub use crate::vst3_bus_abi::{
    BusInfo, VST3_BUS_FLAG_CONTROL_VOLTAGE, VST3_BUS_FLAG_DEFAULT_ACTIVE, VST3_BUS_TYPE_AUX,
    VST3_BUS_TYPE_MAIN,
};
pub use crate::vst3_event_abi::{
    Event, EventPayload, IEventList, IEventListVTable, LegacyMidiCcOutEvent, NoteOffEvent,
    NoteOnEvent, PolyPressureEvent, VST3_EVENT_TYPE_CHORD, VST3_EVENT_TYPE_DATA,
    VST3_EVENT_TYPE_LEGACY_MIDI_CC_OUT, VST3_EVENT_TYPE_NOTE_EXPRESSION_INT_VALUE,
    VST3_EVENT_TYPE_NOTE_EXPRESSION_TEXT, VST3_EVENT_TYPE_NOTE_EXPRESSION_VALUE,
    VST3_EVENT_TYPE_NOTE_OFF, VST3_EVENT_TYPE_NOTE_ON, VST3_EVENT_TYPE_POLY_PRESSURE,
    VST3_EVENT_TYPE_SCALE,
};

#[path = "vst3_abi/control.rs"]
mod control;
#[path = "vst3_abi/factory.rs"]
mod factory;
#[path = "vst3_abi/process.rs"]
mod process;

#[allow(unused_imports)]
pub use control::{
    IAttributeList, IAttributeListVTable, IBStream, IBStreamVTable, IComponentHandler,
    IComponentHandler2, IComponentHandler2VTable, IComponentHandlerVTable, IConnectionPoint,
    IConnectionPointVTable, IEditController, IEditControllerVTable, IMessage, IMessageVTable,
    IMidiMapping, IMidiMappingVTable, IParamValueQueue, IParamValueQueueVTable, IParameterChanges,
    IParameterChangesVTable, IProcessContextRequirements, IProcessContextRequirementsVTable,
    IProgramListData, IProgramListDataVTable, IUnitData, IUnitDataVTable, IUnitInfo,
    IUnitInfoVTable, ParameterInfo, ProgramListInfo, UnitInfo,
};
#[allow(unused_imports)]
pub use factory::{IPluginFactory, IPluginFactoryVTable, PClassInfo, PFactoryInfo};
#[allow(unused_imports)]
pub use process::{
    AudioBusBuffers, Chord, FrameRate, IAudioProcessor, IAudioProcessorVTable, IHostApplication,
    IHostApplicationVTable, ProcessContext, ProcessData, ProcessSetup,
};

pub const K_RESULT_OK: i32 = 0;
pub const K_RESULT_FALSE: i32 = 1;
pub const K_NOT_IMPLEMENTED: i32 = 3;
pub const VST3_INFINITE_TAIL_SAMPLES: u32 = u32::MAX;
pub const VST3_FUNKNOWN_IID: &str = "0000000000000000C000000000000046";
pub const VST3_I_PLUGIN_BASE_IID: &str = "22888DDB156E45AE8358B34808190625";
pub const VST3_I_COMPONENT_IID: &str = "E831FF31F2D54301928EBBEE25697802";
pub const VST3_I_AUDIO_PROCESSOR_IID: &str = "42043F99B7DA453CA569E79D9AAEC33D";
pub const VST3_I_EDIT_CONTROLLER_IID: &str = "DCD7BBE37742448DA874AACC979C759E";
pub const VST3_I_MIDI_MAPPING_IID: &str = "DF0FF9F749B74669B63AB7327ADBF5E5";
pub const VST3_I_PARAM_VALUE_QUEUE_IID: &str = "01263A18ED074F6F98C9D3564686F9BA";
pub const VST3_I_PARAMETER_CHANGES_IID: &str = "A47796630BB64A56B44384A8466FEB9D";
pub const VST3_I_UNIT_INFO_IID: &str = "3D4BD6B5913A4FD2A886E768A5EB92C1";
pub const VST3_I_PROGRAM_LIST_DATA_IID: &str = "8683B01F7B354F70A2651DEC353AF4FF";
pub const VST3_I_UNIT_DATA_IID: &str = "6C389611D391455DB870B83394A0EFDD";
pub const VST3_I_CONNECTION_POINT_IID: &str = "70A4156F6E6E4026989148BFAA60D8D1";
pub const VST3_I_COMPONENT_HANDLER_IID: &str = "93A0BEA30BD045DB8E890B0CC1E46AC6";
pub const VST3_I_COMPONENT_HANDLER2_IID: &str = "F040B4B3A36045ECABCDC045B4D5A2CC";
pub const VST3_I_PROCESS_CONTEXT_REQUIREMENTS_IID: &str = "2A654303EF764E3D95B5FE83730EF6D0";
pub const VST3_I_HOST_APPLICATION_IID: &str = "58E595CCDB2D49698B6AAF8C36A664E5";
pub const VST3_I_ATTRIBUTE_LIST_IID: &str = "1E5F0AEBCC7F4533A254401138AD5EE4";
pub const VST3_I_MESSAGE_IID: &str = "936F033BC6C047DBBB0882F813C1E613";
pub const VST3_IBSTREAM_IID: &str = "C3BF6EA2309947529B6BF9901EE33E9B";
pub const VST3_PROCESS_MODE_REALTIME: i32 = 0;
pub const VST3_SAMPLE_32: i32 = 0;
pub const VST3_PARAMETER_CAN_AUTOMATE: i32 = 1 << 0;
pub const VST3_PARAMETER_IS_READ_ONLY: i32 = 1 << 1;
pub const VST3_PARAMETER_IS_WRAP_AROUND: i32 = 1 << 2;
pub const VST3_PARAMETER_IS_LIST: i32 = 1 << 3;
pub const VST3_PARAMETER_IS_HIDDEN: i32 = 1 << 4;
pub const VST3_PARAMETER_IS_PROGRAM_CHANGE: i32 = 1 << 15;
pub const VST3_PARAMETER_IS_BYPASS: i32 = 1 << 16;
pub const VST3_MIDI_CONTROLLER_AFTERTOUCH: CtrlNumber = 128;
pub const VST3_MIDI_CONTROLLER_PITCH_BEND: CtrlNumber = 129;
pub const VST3_MIDI_CONTROLLER_PROGRAM_CHANGE: CtrlNumber = 130;
pub const VST3_MIDI_CONTROLLER_POLY_PRESSURE: CtrlNumber = 131;
pub const VST3_MIDI_CONTROLLER_QUARTER_FRAME: CtrlNumber = 132;
pub const VST3_MIDI_CONTROLLER_SONG_SELECT: CtrlNumber = 133;
pub const VST3_MIDI_CONTROLLER_SONG_POINTER: CtrlNumber = 134;
pub const VST3_MIDI_CONTROLLER_CABLE_SELECT: CtrlNumber = 135;
pub const VST3_MIDI_CONTROLLER_TUNE_REQUEST: CtrlNumber = 136;
pub const VST3_MIDI_CONTROLLER_CLOCK_START: CtrlNumber = 137;
pub const VST3_MIDI_CONTROLLER_CLOCK_CONTINUE: CtrlNumber = 138;
pub const VST3_MIDI_CONTROLLER_CLOCK_STOP: CtrlNumber = 139;
pub const VST3_MIDI_CONTROLLER_ACTIVE_SENSING: CtrlNumber = 140;
pub const VST3_NO_PROGRAM_LIST_ID: ProgramListId = -1;
pub const VST3_STREAM_SEEK_SET: i32 = 0;
pub const VST3_STREAM_SEEK_CUR: i32 = 1;
pub const VST3_STREAM_SEEK_END: i32 = 2;
pub const VST3_MEDIA_TYPE_AUDIO: i32 = 0;
pub const VST3_BUS_DIRECTION_INPUT: i32 = 0;
pub const VST3_BUS_DIRECTION_OUTPUT: i32 = 1;
pub const VST3_SPEAKER_MONO: SpeakerArrangement = 1 << 19;
pub const VST3_SPEAKER_STEREO: SpeakerArrangement = 0x03;
pub const VST3_SPEAKER_30_CINE: SpeakerArrangement = 0x07;
pub const VST3_SPEAKER_40_MUSIC: SpeakerArrangement = 0x33;
pub const VST3_SPEAKER_50: SpeakerArrangement = 0x37;
pub const VST3_SPEAKER_51: SpeakerArrangement = 0x3f;
pub const VST3_SPEAKER_61_CINE: SpeakerArrangement = 0x13f;
pub const VST3_SPEAKER_71_CINE: SpeakerArrangement = 0xff;
pub const VST3_PROCESS_CONTEXT_PLAYING: u32 = 1 << 1;
pub const VST3_PROCESS_CONTEXT_CYCLE_ACTIVE: u32 = 1 << 2;
pub const VST3_PROCESS_CONTEXT_RECORDING: u32 = 1 << 3;
pub const VST3_PROCESS_CONTEXT_SYSTEM_TIME_VALID: u32 = 1 << 8;
pub const VST3_PROCESS_CONTEXT_PROJECT_TIME_MUSIC_VALID: u32 = 1 << 9;
pub const VST3_PROCESS_CONTEXT_TEMPO_VALID: u32 = 1 << 10;
pub const VST3_PROCESS_CONTEXT_BAR_POSITION_VALID: u32 = 1 << 11;
pub const VST3_PROCESS_CONTEXT_CYCLE_VALID: u32 = 1 << 12;
pub const VST3_PROCESS_CONTEXT_TIME_SIG_VALID: u32 = 1 << 13;
pub const VST3_PROCESS_CONTEXT_SMPTE_VALID: u32 = 1 << 14;
pub const VST3_PROCESS_CONTEXT_CLOCK_VALID: u32 = 1 << 15;
pub const VST3_PROCESS_CONTEXT_CONT_TIME_VALID: u32 = 1 << 17;
pub const VST3_PROCESS_CONTEXT_CHORD_VALID: u32 = 1 << 18;
pub const VST3_PROCESS_CONTEXT_NEED_SYSTEM_TIME: u32 = 1 << 0;
pub const VST3_PROCESS_CONTEXT_NEED_CONT_TIME: u32 = 1 << 1;
pub const VST3_PROCESS_CONTEXT_NEED_PROJECT_TIME_MUSIC: u32 = 1 << 2;
pub const VST3_PROCESS_CONTEXT_NEED_BAR_POSITION_MUSIC: u32 = 1 << 3;
pub const VST3_PROCESS_CONTEXT_NEED_CYCLE_MUSIC: u32 = 1 << 4;
pub const VST3_PROCESS_CONTEXT_NEED_SAMPLES_TO_NEXT_CLOCK: u32 = 1 << 5;
pub const VST3_PROCESS_CONTEXT_NEED_TEMPO: u32 = 1 << 6;
pub const VST3_PROCESS_CONTEXT_NEED_TIME_SIGNATURE: u32 = 1 << 7;
pub const VST3_PROCESS_CONTEXT_NEED_CHORD: u32 = 1 << 8;
pub const VST3_PROCESS_CONTEXT_NEED_FRAME_RATE: u32 = 1 << 9;
pub const VST3_PROCESS_CONTEXT_NEED_TRANSPORT_STATE: u32 = 1 << 10;

pub type TBool = u8;
pub type TUid = [u8; 16];
pub type ParamId = u32;
pub type ParamValue = f64;
pub type CtrlNumber = i16;
pub type UnitId = i32;
pub type ProgramListId = i32;
pub type SampleRate = f64;
pub type TSamples = i64;
pub type TQuarterNotes = f64;
pub type SpeakerArrangement = u64;
pub type String128 = [u16; 128];
pub type FidString = *const c_char;

#[repr(C)]
pub struct FUnknown {
    pub vtable: *const FUnknownVTable,
}

#[repr(C)]
pub struct FUnknownVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut FUnknown,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut FUnknown) -> u32,
    pub release: unsafe extern "system" fn(this: *mut FUnknown) -> u32,
}

#[repr(C)]
pub struct IPluginBase {
    pub vtable: *const IPluginBaseVTable,
}

#[repr(C)]
pub struct IPluginBaseVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IPluginBase,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IPluginBase) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IPluginBase) -> u32,
    pub initialize:
        unsafe extern "system" fn(this: *mut IPluginBase, context: *mut FUnknown) -> i32,
    pub terminate: unsafe extern "system" fn(this: *mut IPluginBase) -> i32,
}

#[repr(C)]
pub struct IComponent {
    pub vtable: *const IComponentVTable,
}

#[repr(C)]
pub struct IComponentVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IComponent,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IComponent) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IComponent) -> u32,
    pub initialize: unsafe extern "system" fn(this: *mut IComponent, context: *mut FUnknown) -> i32,
    pub terminate: unsafe extern "system" fn(this: *mut IComponent) -> i32,
    pub get_controller_class_id:
        unsafe extern "system" fn(this: *mut IComponent, class_id: *mut TUid) -> i32,
    pub set_io_mode: unsafe extern "system" fn(this: *mut IComponent, mode: i32) -> i32,
    pub get_bus_count:
        unsafe extern "system" fn(this: *mut IComponent, media_type: i32, direction: i32) -> i32,
    pub get_bus_info: unsafe extern "system" fn(
        this: *mut IComponent,
        media_type: i32,
        direction: i32,
        index: i32,
        bus: *mut c_void,
    ) -> i32,
    pub get_routing_info: unsafe extern "system" fn(
        this: *mut IComponent,
        input: *mut c_void,
        output: *mut c_void,
    ) -> i32,
    pub activate_bus: unsafe extern "system" fn(
        this: *mut IComponent,
        media_type: i32,
        direction: i32,
        index: i32,
        state: TBool,
    ) -> i32,
    pub set_active: unsafe extern "system" fn(this: *mut IComponent, state: TBool) -> i32,
    pub set_state: unsafe extern "system" fn(this: *mut IComponent, state: *mut c_void) -> i32,
    pub get_state: unsafe extern "system" fn(this: *mut IComponent, state: *mut c_void) -> i32,
}

pub fn fixed_string<const N: usize>(value: &[c_char; N]) -> Option<String> {
    let bytes = value
        .iter()
        .take_while(|value| **value != 0)
        .map(|value| *value as u8)
        .collect::<Vec<u8>>();
    let text = String::from_utf8_lossy(&bytes).trim().to_string();

    if text.is_empty() { None } else { Some(text) }
}

pub fn tuid_hex(value: &[u8; 16]) -> String {
    value
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

pub fn normalize_fuid_string(value: &str) -> Option<String> {
    let mut normalized = String::with_capacity(32);
    for character in value.trim().chars() {
        match character {
            '{' | '}' | '-' => {}
            value if value.is_ascii_hexdigit() => normalized.push(value.to_ascii_uppercase()),
            _ => return None,
        }
    }

    if normalized.len() == 32 {
        Some(normalized)
    } else {
        None
    }
}

pub fn parse_tuid_hex(value: &str) -> Option<TUid> {
    let normalized = normalize_fuid_string(value)?;
    let mut tuid = [0; 16];

    for (index, byte) in tuid.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&normalized[offset..offset + 2], 16).ok()?;
    }

    Some(tuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_fixed_string_at_nul() {
        let mut value = [0; 8];
        value[0] = b'A' as c_char;
        value[1] = b'B' as c_char;

        assert_eq!(fixed_string(&value).as_deref(), Some("AB"));
    }

    #[test]
    fn renders_tuid_as_hex() {
        assert_eq!(tuid_hex(&[1; 16]), "01010101010101010101010101010101");
    }

    #[test]
    fn normalizes_fuid_strings() {
        assert_eq!(
            normalize_fuid_string("{e831ff31-f2d5-4301-928e-bbee25697802}").as_deref(),
            Some(VST3_I_COMPONENT_IID)
        );
        assert!(normalize_fuid_string("class-a").is_none());
    }

    #[test]
    fn parses_fuid_strings_to_tuid_bytes() {
        assert_eq!(
            parse_tuid_hex(VST3_I_AUDIO_PROCESSOR_IID),
            Some([
                0x42, 0x04, 0x3f, 0x99, 0xb7, 0xda, 0x45, 0x3c, 0xa5, 0x69, 0xe7, 0x9d, 0x9a, 0xae,
                0xc3, 0x3d,
            ])
        );
        assert!(parse_tuid_hex("invalid").is_none());
    }

    #[test]
    fn parses_host_application_iid() {
        assert_eq!(
            parse_tuid_hex(VST3_I_HOST_APPLICATION_IID),
            Some([
                0x58, 0xe5, 0x95, 0xcc, 0xdb, 0x2d, 0x49, 0x69, 0x8b, 0x6a, 0xaf, 0x8c, 0x36, 0xa6,
                0x64, 0xe5,
            ])
        );
    }

    #[test]
    fn creates_realtime_f32_process_setup() {
        assert_eq!(
            ProcessSetup::realtime_f32(128, 48_000.0),
            ProcessSetup {
                process_mode: VST3_PROCESS_MODE_REALTIME,
                symbolic_sample_size: VST3_SAMPLE_32,
                max_samples_per_block: 128,
                sample_rate: 48_000.0,
            }
        );
    }
}
