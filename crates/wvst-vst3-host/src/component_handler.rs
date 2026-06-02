use std::ffi::{CStr, c_void};
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::vst3_abi::{
    FidString, IComponentHandler, IComponentHandler2, IComponentHandler2VTable,
    IComponentHandlerVTable, K_RESULT_FALSE, K_RESULT_OK, ParamId, ParamValue, TBool, TUid,
    VST3_FUNKNOWN_IID, VST3_I_COMPONENT_HANDLER_IID, VST3_I_COMPONENT_HANDLER2_IID, parse_tuid_hex,
};

const MAX_COMPONENT_HANDLER_EVENTS: usize = 64;

#[derive(Debug)]
pub struct Vst3ComponentHandler {
    object: Box<ComponentHandlerObject>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ComponentHandlerEvent {
    pub sequence: u64,
    pub kind: Vst3ComponentHandlerEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_normalized: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Vst3ComponentHandlerEventKind {
    BeginEdit,
    PerformEdit,
    EndEdit,
    RestartComponent,
    SetDirty,
    RequestOpenEditor,
    StartGroupEdit,
    FinishGroupEdit,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ComponentHandlerSnapshot {
    pub total_events: u64,
    pub recent_events: Vec<Vst3ComponentHandlerEvent>,
}

impl Vst3ComponentHandler {
    pub fn new() -> Self {
        let mut object = Box::new(ComponentHandlerObject {
            iface: IComponentHandler {
                vtable: &COMPONENT_HANDLER_VTABLE,
            },
            iface2: ComponentHandler2Object {
                iface: IComponentHandler2 {
                    vtable: &COMPONENT_HANDLER2_VTABLE,
                },
                owner: ptr::null_mut(),
            },
            ref_count: AtomicU32::new(1),
            next_sequence: AtomicU64::new(1),
            events: Mutex::new(Vec::new()),
        });
        object.iface2.owner = object.as_mut();
        Self { object }
    }

    pub fn as_mut_ptr(&mut self) -> *mut IComponentHandler {
        &mut self.object.iface
    }

    pub fn snapshot(&self) -> Vst3ComponentHandlerSnapshot {
        self.object.snapshot()
    }

    pub fn begin_edit(&self, parameter_id: ParamId) {
        self.object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::BeginEdit)
                .with_parameter_id(parameter_id),
        );
    }

    pub fn perform_edit(&self, parameter_id: ParamId, value_normalized: ParamValue) {
        self.object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::PerformEdit)
                .with_parameter_id(parameter_id)
                .with_value_normalized(value_normalized),
        );
    }

    pub fn end_edit(&self, parameter_id: ParamId) {
        self.object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::EndEdit)
                .with_parameter_id(parameter_id),
        );
    }
}

impl Default for Vst3ComponentHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Debug)]
struct ComponentHandlerObject {
    iface: IComponentHandler,
    iface2: ComponentHandler2Object,
    ref_count: AtomicU32,
    next_sequence: AtomicU64,
    events: Mutex<Vec<Vst3ComponentHandlerEvent>>,
}

#[repr(C)]
#[derive(Debug)]
struct ComponentHandler2Object {
    iface: IComponentHandler2,
    owner: *mut ComponentHandlerObject,
}

const COMPONENT_HANDLER_VTABLE: IComponentHandlerVTable = IComponentHandlerVTable {
    query_interface: handler_query_interface,
    add_ref: handler_add_ref,
    release: handler_release,
    begin_edit: handler_begin_edit,
    perform_edit: handler_perform_edit,
    end_edit: handler_end_edit,
    restart_component: handler_restart_component,
};

const COMPONENT_HANDLER2_VTABLE: IComponentHandler2VTable = IComponentHandler2VTable {
    query_interface: handler2_query_interface,
    add_ref: handler2_add_ref,
    release: handler2_release,
    set_dirty: handler2_set_dirty,
    request_open_editor: handler2_request_open_editor,
    start_group_edit: handler2_start_group_edit,
    finish_group_edit: handler2_finish_group_edit,
};

unsafe extern "system" fn handler_query_interface(
    this: *mut IComponentHandler,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    let Some(object) = handler_object_mut(this) else {
        return clear_query_object(obj);
    };
    query_interface(object, iid, obj)
}

unsafe extern "system" fn handler_add_ref(this: *mut IComponentHandler) -> u32 {
    handler_object_mut(this).map(add_ref).unwrap_or(0)
}

unsafe extern "system" fn handler_release(this: *mut IComponentHandler) -> u32 {
    handler_object_mut(this).map(release).unwrap_or(0)
}

unsafe extern "system" fn handler_begin_edit(this: *mut IComponentHandler, id: ParamId) -> i32 {
    if let Some(object) = handler_object_mut(this) {
        object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::BeginEdit)
                .with_parameter_id(id),
        );
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler_perform_edit(
    this: *mut IComponentHandler,
    id: ParamId,
    value_normalized: ParamValue,
) -> i32 {
    if let Some(object) = handler_object_mut(this) {
        object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::PerformEdit)
                .with_parameter_id(id)
                .with_value_normalized(value_normalized),
        );
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler_end_edit(this: *mut IComponentHandler, id: ParamId) -> i32 {
    if let Some(object) = handler_object_mut(this) {
        object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::EndEdit)
                .with_parameter_id(id),
        );
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler_restart_component(
    this: *mut IComponentHandler,
    flags: i32,
) -> i32 {
    if let Some(object) = handler_object_mut(this) {
        object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::RestartComponent)
                .with_flags(flags),
        );
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler2_query_interface(
    this: *mut IComponentHandler2,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    let Some(object) = handler2_object_mut(this) else {
        return clear_query_object(obj);
    };
    query_interface(object, iid, obj)
}

unsafe extern "system" fn handler2_add_ref(this: *mut IComponentHandler2) -> u32 {
    handler2_object_mut(this).map(add_ref).unwrap_or(0)
}

unsafe extern "system" fn handler2_release(this: *mut IComponentHandler2) -> u32 {
    handler2_object_mut(this).map(release).unwrap_or(0)
}

unsafe extern "system" fn handler2_set_dirty(this: *mut IComponentHandler2, state: TBool) -> i32 {
    if let Some(object) = handler2_object_mut(this) {
        object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::SetDirty)
                .with_dirty(state != 0),
        );
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler2_request_open_editor(
    this: *mut IComponentHandler2,
    name: FidString,
) -> i32 {
    if let Some(object) = handler2_object_mut(this) {
        object.record(
            ComponentHandlerEventRecord::new(Vst3ComponentHandlerEventKind::RequestOpenEditor)
                .with_editor_name(fid_string(name)),
        );
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler2_start_group_edit(this: *mut IComponentHandler2) -> i32 {
    if let Some(object) = handler2_object_mut(this) {
        object.record(ComponentHandlerEventRecord::new(
            Vst3ComponentHandlerEventKind::StartGroupEdit,
        ));
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler2_finish_group_edit(this: *mut IComponentHandler2) -> i32 {
    if let Some(object) = handler2_object_mut(this) {
        object.record(ComponentHandlerEventRecord::new(
            Vst3ComponentHandlerEventKind::FinishGroupEdit,
        ));
    }
    K_RESULT_OK
}

fn query_interface(
    object: &mut ComponentHandlerObject,
    iid: *const i8,
    destination: *mut *mut c_void,
) -> i32 {
    if destination.is_null() {
        return K_RESULT_FALSE;
    }
    let Some(iid) = read_tuid(iid) else {
        unsafe { *destination = ptr::null_mut() };
        return K_RESULT_FALSE;
    };

    let requested = if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_I_COMPONENT_HANDLER_IID) {
        (&mut object.iface as *mut IComponentHandler).cast::<c_void>()
    } else if iid == tuid(VST3_I_COMPONENT_HANDLER2_IID) {
        (&mut object.iface2.iface as *mut IComponentHandler2).cast::<c_void>()
    } else {
        ptr::null_mut()
    };

    if requested.is_null() {
        unsafe { *destination = ptr::null_mut() };
        return K_RESULT_FALSE;
    }

    add_ref(object);
    unsafe { *destination = requested };
    K_RESULT_OK
}

fn clear_query_object(destination: *mut *mut c_void) -> i32 {
    if !destination.is_null() {
        unsafe { *destination = ptr::null_mut() };
    }
    K_RESULT_FALSE
}

fn handler_object_mut(this: *mut IComponentHandler) -> Option<&'static mut ComponentHandlerObject> {
    if this.is_null() {
        return None;
    }
    // The VST3 ABI pointer is the first field of ComponentHandlerObject.
    Some(unsafe { &mut *this.cast::<ComponentHandlerObject>() })
}

fn handler2_object_mut(
    this: *mut IComponentHandler2,
) -> Option<&'static mut ComponentHandlerObject> {
    if this.is_null() {
        return None;
    }

    // IComponentHandler2 is stored in a wrapper whose first field is the ABI pointer.
    let handler2 = unsafe { &mut *this.cast::<ComponentHandler2Object>() };
    if handler2.owner.is_null() {
        return None;
    }
    Some(unsafe { &mut *handler2.owner })
}

fn add_ref(object: &mut ComponentHandlerObject) -> u32 {
    object.ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

fn release(object: &mut ComponentHandlerObject) -> u32 {
    object
        .ref_count
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            Some(value.saturating_sub(1))
        })
        .unwrap_or(0)
        .saturating_sub(1)
}

impl ComponentHandlerObject {
    fn record(&self, record: ComponentHandlerEventRecord) {
        let event = Vst3ComponentHandlerEvent {
            sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
            kind: record.kind,
            parameter_id: record.parameter_id,
            value_normalized: record.value_normalized,
            flags: record.flags,
            dirty: record.dirty,
            editor_name: record.editor_name,
        };
        if let Ok(mut events) = self.events.lock() {
            if events.len() == MAX_COMPONENT_HANDLER_EVENTS {
                events.remove(0);
            }
            events.push(event);
        }
    }

    fn snapshot(&self) -> Vst3ComponentHandlerSnapshot {
        Vst3ComponentHandlerSnapshot {
            total_events: self.next_sequence.load(Ordering::Relaxed).saturating_sub(1),
            recent_events: self
                .events
                .lock()
                .map(|events| events.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
struct ComponentHandlerEventRecord {
    kind: Vst3ComponentHandlerEventKind,
    parameter_id: Option<u32>,
    value_normalized: Option<f64>,
    flags: Option<i32>,
    dirty: Option<bool>,
    editor_name: Option<String>,
}

impl ComponentHandlerEventRecord {
    fn new(kind: Vst3ComponentHandlerEventKind) -> Self {
        Self {
            kind,
            parameter_id: None,
            value_normalized: None,
            flags: None,
            dirty: None,
            editor_name: None,
        }
    }

    fn with_parameter_id(mut self, parameter_id: ParamId) -> Self {
        self.parameter_id = Some(parameter_id);
        self
    }

    fn with_value_normalized(mut self, value_normalized: ParamValue) -> Self {
        self.value_normalized = value_normalized.is_finite().then_some(value_normalized);
        self
    }

    fn with_flags(mut self, flags: i32) -> Self {
        self.flags = Some(flags);
        self
    }

    fn with_dirty(mut self, dirty: bool) -> Self {
        self.dirty = Some(dirty);
        self
    }

    fn with_editor_name(mut self, editor_name: Option<String>) -> Self {
        self.editor_name = editor_name;
        self
    }
}

fn read_tuid(iid: *const i8) -> Option<TUid> {
    if iid.is_null() {
        return None;
    }

    let mut value = [0; 16];
    unsafe {
        value.copy_from_slice(std::slice::from_raw_parts(iid.cast::<u8>(), 16));
    }
    Some(value)
}

fn tuid(value: &str) -> TUid {
    parse_tuid_hex(value).expect("built-in VST3 interface id is valid")
}

fn fid_string(value: FidString) -> Option<String> {
    if value.is_null() {
        return None;
    }

    unsafe { CStr::from_ptr(value) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

#[cfg(test)]
#[path = "component_handler_tests.rs"]
mod component_handler_tests;
