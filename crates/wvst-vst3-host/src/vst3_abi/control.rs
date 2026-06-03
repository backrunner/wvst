#![allow(dead_code)]

use std::ffi::{c_char, c_void};

use super::{
    CtrlNumber, FUnknown, FidString, ParamId, ParamValue, ProgramListId, String128, TBool, UnitId,
    VST3_NO_PROGRAM_LIST_ID,
};

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
