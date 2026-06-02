#![allow(dead_code)]
// Raw VST3 ABI declarations are exposed to this crate incrementally through
// safe facades; not every vtable slot is called in the same milestone.

use std::ffi::{c_char, c_void};

pub use crate::vst3_bus_abi::{
    BusInfo, VST3_BUS_FLAG_CONTROL_VOLTAGE, VST3_BUS_FLAG_DEFAULT_ACTIVE, VST3_BUS_TYPE_AUX,
    VST3_BUS_TYPE_MAIN,
};
pub use crate::vst3_event_abi::{
    Event, EventPayload, IEventList, IEventListVTable, NoteOffEvent, NoteOnEvent,
    PolyPressureEvent, VST3_EVENT_TYPE_NOTE_OFF, VST3_EVENT_TYPE_NOTE_ON,
    VST3_EVENT_TYPE_POLY_PRESSURE,
};

pub const K_RESULT_OK: i32 = 0;
pub const K_RESULT_FALSE: i32 = 1;
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

#[repr(C)]
pub struct IEditController {
    pub vtable: *const IEditControllerVTable,
}

#[repr(C)]
pub struct IEditControllerVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IEditController,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IEditController) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IEditController) -> u32,
    pub initialize:
        unsafe extern "system" fn(this: *mut IEditController, context: *mut FUnknown) -> i32,
    pub terminate: unsafe extern "system" fn(this: *mut IEditController) -> i32,
    pub set_component_state:
        unsafe extern "system" fn(this: *mut IEditController, state: *mut IBStream) -> i32,
    pub set_state:
        unsafe extern "system" fn(this: *mut IEditController, state: *mut IBStream) -> i32,
    pub get_state:
        unsafe extern "system" fn(this: *mut IEditController, state: *mut IBStream) -> i32,
    pub get_parameter_count: unsafe extern "system" fn(this: *mut IEditController) -> i32,
    pub get_parameter_info: unsafe extern "system" fn(
        this: *mut IEditController,
        param_index: i32,
        info: *mut ParameterInfo,
    ) -> i32,
    pub get_param_string_by_value: unsafe extern "system" fn(
        this: *mut IEditController,
        id: ParamId,
        value_normalized: ParamValue,
        string: *mut String128,
    ) -> i32,
    pub get_param_value_by_string: unsafe extern "system" fn(
        this: *mut IEditController,
        id: ParamId,
        string: *mut u16,
        value_normalized: *mut ParamValue,
    ) -> i32,
    pub normalized_param_to_plain: unsafe extern "system" fn(
        this: *mut IEditController,
        id: ParamId,
        value_normalized: ParamValue,
    ) -> ParamValue,
    pub plain_param_to_normalized: unsafe extern "system" fn(
        this: *mut IEditController,
        id: ParamId,
        plain_value: ParamValue,
    ) -> ParamValue,
    pub get_param_normalized:
        unsafe extern "system" fn(this: *mut IEditController, id: ParamId) -> ParamValue,
    pub set_param_normalized: unsafe extern "system" fn(
        this: *mut IEditController,
        id: ParamId,
        value: ParamValue,
    ) -> i32,
    pub set_component_handler: unsafe extern "system" fn(
        this: *mut IEditController,
        handler: *mut IComponentHandler,
    ) -> i32,
    pub create_view:
        unsafe extern "system" fn(this: *mut IEditController, name: *const i8) -> *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParameterInfo {
    pub id: ParamId,
    pub title: String128,
    pub short_title: String128,
    pub units: String128,
    pub step_count: i32,
    pub default_normalized_value: ParamValue,
    pub unit_id: UnitId,
    pub flags: i32,
}

impl Default for ParameterInfo {
    fn default() -> Self {
        Self {
            id: 0,
            title: [0; 128],
            short_title: [0; 128],
            units: [0; 128],
            step_count: 0,
            default_normalized_value: 0.0,
            unit_id: 0,
            flags: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IComponentHandler {
    pub vtable: *const IComponentHandlerVTable,
}

#[repr(C)]
pub struct IComponentHandlerVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IComponentHandler,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IComponentHandler) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IComponentHandler) -> u32,
    pub begin_edit: unsafe extern "system" fn(this: *mut IComponentHandler, id: ParamId) -> i32,
    pub perform_edit: unsafe extern "system" fn(
        this: *mut IComponentHandler,
        id: ParamId,
        value_normalized: ParamValue,
    ) -> i32,
    pub end_edit: unsafe extern "system" fn(this: *mut IComponentHandler, id: ParamId) -> i32,
    pub restart_component:
        unsafe extern "system" fn(this: *mut IComponentHandler, flags: i32) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IComponentHandler2 {
    pub vtable: *const IComponentHandler2VTable,
}

#[repr(C)]
pub struct IComponentHandler2VTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IComponentHandler2,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IComponentHandler2) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IComponentHandler2) -> u32,
    pub set_dirty: unsafe extern "system" fn(this: *mut IComponentHandler2, state: TBool) -> i32,
    pub request_open_editor:
        unsafe extern "system" fn(this: *mut IComponentHandler2, name: FidString) -> i32,
    pub start_group_edit: unsafe extern "system" fn(this: *mut IComponentHandler2) -> i32,
    pub finish_group_edit: unsafe extern "system" fn(this: *mut IComponentHandler2) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IProcessContextRequirements {
    pub vtable: *const IProcessContextRequirementsVTable,
}

#[repr(C)]
pub struct IProcessContextRequirementsVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IProcessContextRequirements,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IProcessContextRequirements) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IProcessContextRequirements) -> u32,
    pub get_process_context_requirements:
        unsafe extern "system" fn(this: *mut IProcessContextRequirements) -> u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IMidiMapping {
    pub vtable: *const IMidiMappingVTable,
}

#[repr(C)]
pub struct IMidiMappingVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IMidiMapping,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IMidiMapping) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IMidiMapping) -> u32,
    pub get_midi_controller_assignment: unsafe extern "system" fn(
        this: *mut IMidiMapping,
        bus_index: i32,
        channel: i16,
        midi_controller_number: CtrlNumber,
        id: *mut ParamId,
    ) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IParamValueQueue {
    pub vtable: *const IParamValueQueueVTable,
}

#[repr(C)]
pub struct IParamValueQueueVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IParamValueQueue,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IParamValueQueue) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IParamValueQueue) -> u32,
    pub get_parameter_id: unsafe extern "system" fn(this: *mut IParamValueQueue) -> ParamId,
    pub get_point_count: unsafe extern "system" fn(this: *mut IParamValueQueue) -> i32,
    pub get_point: unsafe extern "system" fn(
        this: *mut IParamValueQueue,
        index: i32,
        sample_offset: *mut i32,
        value: *mut ParamValue,
    ) -> i32,
    pub add_point: unsafe extern "system" fn(
        this: *mut IParamValueQueue,
        sample_offset: i32,
        value: ParamValue,
        index: *mut i32,
    ) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IAttributeList {
    pub vtable: *const IAttributeListVTable,
}

#[repr(C)]
pub struct IAttributeListVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IAttributeList,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IAttributeList) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IAttributeList) -> u32,
    pub set_int:
        unsafe extern "system" fn(this: *mut IAttributeList, id: FidString, value: i64) -> i32,
    pub get_int:
        unsafe extern "system" fn(this: *mut IAttributeList, id: FidString, value: *mut i64) -> i32,
    pub set_float:
        unsafe extern "system" fn(this: *mut IAttributeList, id: FidString, value: f64) -> i32,
    pub get_float:
        unsafe extern "system" fn(this: *mut IAttributeList, id: FidString, value: *mut f64) -> i32,
    pub set_string: unsafe extern "system" fn(
        this: *mut IAttributeList,
        id: FidString,
        value: *const u16,
    ) -> i32,
    pub get_string: unsafe extern "system" fn(
        this: *mut IAttributeList,
        id: FidString,
        value: *mut u16,
        size_in_bytes: u32,
    ) -> i32,
    pub set_binary: unsafe extern "system" fn(
        this: *mut IAttributeList,
        id: FidString,
        data: *const c_void,
        size_in_bytes: u32,
    ) -> i32,
    pub get_binary: unsafe extern "system" fn(
        this: *mut IAttributeList,
        id: FidString,
        data: *mut *const c_void,
        size_in_bytes: *mut u32,
    ) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IMessage {
    pub vtable: *const IMessageVTable,
}

#[repr(C)]
pub struct IMessageVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IMessage,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IMessage) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IMessage) -> u32,
    pub get_message_id: unsafe extern "system" fn(this: *mut IMessage) -> FidString,
    pub set_message_id: unsafe extern "system" fn(this: *mut IMessage, id: FidString),
    pub get_attributes: unsafe extern "system" fn(this: *mut IMessage) -> *mut IAttributeList,
}

#[repr(C)]
#[derive(Debug)]
pub struct IConnectionPoint {
    pub vtable: *const IConnectionPointVTable,
}

#[repr(C)]
pub struct IConnectionPointVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IConnectionPoint,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IConnectionPoint) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IConnectionPoint) -> u32,
    pub connect:
        unsafe extern "system" fn(this: *mut IConnectionPoint, other: *mut IConnectionPoint) -> i32,
    pub disconnect:
        unsafe extern "system" fn(this: *mut IConnectionPoint, other: *mut IConnectionPoint) -> i32,
    pub notify:
        unsafe extern "system" fn(this: *mut IConnectionPoint, message: *mut IMessage) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IParameterChanges {
    pub vtable: *const IParameterChangesVTable,
}

#[repr(C)]
pub struct IParameterChangesVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IParameterChanges,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IParameterChanges) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IParameterChanges) -> u32,
    pub get_parameter_count: unsafe extern "system" fn(this: *mut IParameterChanges) -> i32,
    pub get_parameter_data: unsafe extern "system" fn(
        this: *mut IParameterChanges,
        index: i32,
    ) -> *mut IParamValueQueue,
    pub add_parameter_data: unsafe extern "system" fn(
        this: *mut IParameterChanges,
        id: *const ParamId,
        index: *mut i32,
    ) -> *mut IParamValueQueue,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UnitInfo {
    pub id: UnitId,
    pub parent_unit_id: UnitId,
    pub name: String128,
    pub program_list_id: ProgramListId,
}

impl Default for UnitInfo {
    fn default() -> Self {
        Self {
            id: 0,
            parent_unit_id: -1,
            name: [0; 128],
            program_list_id: VST3_NO_PROGRAM_LIST_ID,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProgramListInfo {
    pub id: ProgramListId,
    pub name: String128,
    pub program_count: i32,
}

impl Default for ProgramListInfo {
    fn default() -> Self {
        Self {
            id: VST3_NO_PROGRAM_LIST_ID,
            name: [0; 128],
            program_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IUnitInfo {
    pub vtable: *const IUnitInfoVTable,
}

#[repr(C)]
pub struct IUnitInfoVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IUnitInfo) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IUnitInfo) -> u32,
    pub get_unit_count: unsafe extern "system" fn(this: *mut IUnitInfo) -> i32,
    pub get_unit_info: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        unit_index: i32,
        info: *mut UnitInfo,
    ) -> i32,
    pub get_program_list_count: unsafe extern "system" fn(this: *mut IUnitInfo) -> i32,
    pub get_program_list_info: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        list_index: i32,
        info: *mut ProgramListInfo,
    ) -> i32,
    pub get_program_name: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        list_id: ProgramListId,
        program_index: i32,
        name: *mut String128,
    ) -> i32,
    pub get_program_info: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        list_id: ProgramListId,
        program_index: i32,
        attribute_id: *const c_char,
        attribute_value: *mut String128,
    ) -> i32,
    pub has_program_pitch_names: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        list_id: ProgramListId,
        program_index: i32,
    ) -> i32,
    pub get_program_pitch_name: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        list_id: ProgramListId,
        program_index: i32,
        midi_pitch: i16,
        name: *mut String128,
    ) -> i32,
    pub get_selected_unit: unsafe extern "system" fn(this: *mut IUnitInfo) -> UnitId,
    pub select_unit: unsafe extern "system" fn(this: *mut IUnitInfo, unit_id: UnitId) -> i32,
    pub get_unit_by_bus: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        media_type: i32,
        direction: i32,
        bus_index: i32,
        channel: i32,
        unit_id: *mut UnitId,
    ) -> i32,
    pub set_unit_program_data: unsafe extern "system" fn(
        this: *mut IUnitInfo,
        list_or_unit_id: i32,
        program_index: i32,
        data: *mut IBStream,
    ) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IProgramListData {
    pub vtable: *const IProgramListDataVTable,
}

#[repr(C)]
pub struct IProgramListDataVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IProgramListData,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IProgramListData) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IProgramListData) -> u32,
    pub program_data_supported:
        unsafe extern "system" fn(this: *mut IProgramListData, list_id: ProgramListId) -> i32,
    pub get_program_data: unsafe extern "system" fn(
        this: *mut IProgramListData,
        list_id: ProgramListId,
        program_index: i32,
        data: *mut IBStream,
    ) -> i32,
    pub set_program_data: unsafe extern "system" fn(
        this: *mut IProgramListData,
        list_id: ProgramListId,
        program_index: i32,
        data: *mut IBStream,
    ) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IUnitData {
    pub vtable: *const IUnitDataVTable,
}

#[repr(C)]
pub struct IUnitDataVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IUnitData,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IUnitData) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IUnitData) -> u32,
    pub unit_data_supported:
        unsafe extern "system" fn(this: *mut IUnitData, unit_id: UnitId) -> i32,
    pub get_unit_data: unsafe extern "system" fn(
        this: *mut IUnitData,
        unit_id: UnitId,
        data: *mut IBStream,
    ) -> i32,
    pub set_unit_data: unsafe extern "system" fn(
        this: *mut IUnitData,
        unit_id: UnitId,
        data: *mut IBStream,
    ) -> i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IBStream {
    pub vtable: *const IBStreamVTable,
}

#[repr(C)]
pub struct IBStreamVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IBStream,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IBStream) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IBStream) -> u32,
    pub read: unsafe extern "system" fn(
        this: *mut IBStream,
        buffer: *mut c_void,
        num_bytes: i32,
        num_bytes_read: *mut i32,
    ) -> i32,
    pub write: unsafe extern "system" fn(
        this: *mut IBStream,
        buffer: *mut c_void,
        num_bytes: i32,
        num_bytes_written: *mut i32,
    ) -> i32,
    pub seek: unsafe extern "system" fn(
        this: *mut IBStream,
        pos: i64,
        mode: i32,
        result: *mut i64,
    ) -> i32,
    pub tell: unsafe extern "system" fn(this: *mut IBStream, pos: *mut i64) -> i32,
}

#[repr(C)]
pub struct IAudioProcessor {
    pub vtable: *const IAudioProcessorVTable,
}

#[repr(C)]
pub struct IAudioProcessorVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IAudioProcessor) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IAudioProcessor) -> u32,
    pub set_bus_arrangements: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        inputs: *mut SpeakerArrangement,
        input_count: i32,
        outputs: *mut SpeakerArrangement,
        output_count: i32,
    ) -> i32,
    pub get_bus_arrangement: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        direction: i32,
        index: i32,
        arrangement: *mut SpeakerArrangement,
    ) -> i32,
    pub can_process_sample_size:
        unsafe extern "system" fn(this: *mut IAudioProcessor, sample_size: i32) -> i32,
    pub get_latency_samples: unsafe extern "system" fn(this: *mut IAudioProcessor) -> u32,
    pub setup_processing:
        unsafe extern "system" fn(this: *mut IAudioProcessor, setup: *mut ProcessSetup) -> i32,
    pub set_processing: unsafe extern "system" fn(this: *mut IAudioProcessor, state: TBool) -> i32,
    pub process:
        unsafe extern "system" fn(this: *mut IAudioProcessor, data: *mut ProcessData) -> i32,
    pub get_tail_samples: unsafe extern "system" fn(this: *mut IAudioProcessor) -> u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct IHostApplication {
    pub vtable: *const IHostApplicationVTable,
}

#[repr(C)]
pub struct IHostApplicationVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IHostApplication,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IHostApplication) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IHostApplication) -> u32,
    pub get_name:
        unsafe extern "system" fn(this: *mut IHostApplication, name: *mut String128) -> i32,
    pub create_instance: unsafe extern "system" fn(
        this: *mut IHostApplication,
        cid: *mut TUid,
        iid: *mut TUid,
        obj: *mut *mut c_void,
    ) -> i32,
}

#[repr(C)]
pub struct IPluginFactory {
    pub vtable: *const IPluginFactoryVTable,
}

#[repr(C)]
pub struct IPluginFactoryVTable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut IPluginFactory,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut IPluginFactory) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IPluginFactory) -> u32,
    pub get_factory_info:
        unsafe extern "system" fn(this: *mut IPluginFactory, info: *mut PFactoryInfo) -> i32,
    pub count_classes: unsafe extern "system" fn(this: *mut IPluginFactory) -> i32,
    pub get_class_info: unsafe extern "system" fn(
        this: *mut IPluginFactory,
        index: i32,
        info: *mut PClassInfo,
    ) -> i32,
    pub create_instance: unsafe extern "system" fn(
        this: *mut IPluginFactory,
        cid: *const i8,
        iid: *const i8,
        obj: *mut *mut c_void,
    ) -> i32,
}

#[repr(C)]
pub struct PFactoryInfo {
    pub vendor: [c_char; 64],
    pub url: [c_char; 256],
    pub email: [c_char; 128],
    pub flags: i32,
}

impl Default for PFactoryInfo {
    fn default() -> Self {
        Self {
            vendor: [0; 64],
            url: [0; 256],
            email: [0; 128],
            flags: 0,
        }
    }
}

#[repr(C)]
pub struct PClassInfo {
    pub cid: [u8; 16],
    pub cardinality: i32,
    pub category: [c_char; 32],
    pub name: [c_char; 64],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessSetup {
    pub process_mode: i32,
    pub symbolic_sample_size: i32,
    pub max_samples_per_block: i32,
    pub sample_rate: SampleRate,
}

impl ProcessSetup {
    pub fn realtime_f32(max_samples_per_block: i32, sample_rate: SampleRate) -> Self {
        Self {
            process_mode: VST3_PROCESS_MODE_REALTIME,
            symbolic_sample_size: VST3_SAMPLE_32,
            max_samples_per_block,
            sample_rate,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioBusBuffers {
    pub num_channels: i32,
    pub silence_flags: u64,
    pub channel_buffers32: *mut *mut f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameRate {
    pub frames_per_second: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Chord {
    pub key_note: u8,
    pub root_note: u8,
    pub chord_mask: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessContext {
    pub state: u32,
    pub sample_rate: SampleRate,
    pub project_time_samples: TSamples,
    pub system_time: i64,
    pub continous_time_samples: TSamples,
    pub project_time_music: TQuarterNotes,
    pub bar_position_music: TQuarterNotes,
    pub cycle_start_music: TQuarterNotes,
    pub cycle_end_music: TQuarterNotes,
    pub tempo: f64,
    pub time_sig_numerator: i32,
    pub time_sig_denominator: i32,
    pub chord: Chord,
    pub smpte_offset_subframes: i32,
    pub frame_rate: FrameRate,
    pub samples_to_next_clock: i32,
}

impl ProcessContext {
    pub fn stopped(sample_rate: SampleRate) -> Self {
        Self {
            state: VST3_PROCESS_CONTEXT_CONT_TIME_VALID
                | VST3_PROCESS_CONTEXT_PROJECT_TIME_MUSIC_VALID
                | VST3_PROCESS_CONTEXT_BAR_POSITION_VALID
                | VST3_PROCESS_CONTEXT_TEMPO_VALID
                | VST3_PROCESS_CONTEXT_TIME_SIG_VALID,
            sample_rate,
            project_time_samples: 0,
            system_time: 0,
            continous_time_samples: 0,
            project_time_music: 0.0,
            bar_position_music: 0.0,
            cycle_start_music: 0.0,
            cycle_end_music: 0.0,
            tempo: 120.0,
            time_sig_numerator: 4,
            time_sig_denominator: 4,
            chord: Chord::default(),
            smpte_offset_subframes: 0,
            frame_rate: FrameRate::default(),
            samples_to_next_clock: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessData {
    pub process_mode: i32,
    pub symbolic_sample_size: i32,
    pub num_samples: i32,
    pub num_inputs: i32,
    pub num_outputs: i32,
    pub inputs: *mut AudioBusBuffers,
    pub outputs: *mut AudioBusBuffers,
    pub input_parameter_changes: *mut c_void,
    pub output_parameter_changes: *mut c_void,
    pub input_events: *mut c_void,
    pub output_events: *mut c_void,
    pub process_context: *mut c_void,
}

impl Default for PClassInfo {
    fn default() -> Self {
        Self {
            cid: [0; 16],
            cardinality: 0,
            category: [0; 32],
            name: [0; 64],
        }
    }
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
