use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::vst3_abi::{
    IComponentHandler, IComponentHandlerVTable, K_RESULT_FALSE, K_RESULT_OK, ParamId, ParamValue,
    TUid, VST3_FUNKNOWN_IID, VST3_I_COMPONENT_HANDLER_IID, parse_tuid_hex,
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
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Vst3ComponentHandlerEventKind {
    BeginEdit,
    PerformEdit,
    EndEdit,
    RestartComponent,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ComponentHandlerSnapshot {
    pub total_events: u64,
    pub recent_events: Vec<Vst3ComponentHandlerEvent>,
}

impl Vst3ComponentHandler {
    pub fn new() -> Self {
        Self {
            object: Box::new(ComponentHandlerObject {
                iface: IComponentHandler {
                    vtable: &COMPONENT_HANDLER_VTABLE,
                },
                ref_count: AtomicU32::new(1),
                next_sequence: AtomicU64::new(1),
                events: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut IComponentHandler {
        &mut self.object.iface
    }

    pub fn snapshot(&self) -> Vst3ComponentHandlerSnapshot {
        self.object.snapshot()
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
    ref_count: AtomicU32,
    next_sequence: AtomicU64,
    events: Mutex<Vec<Vst3ComponentHandlerEvent>>,
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

unsafe extern "system" fn handler_query_interface(
    this: *mut IComponentHandler,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return K_RESULT_FALSE;
    }

    let Some(object) = handler_object_mut(this) else {
        return K_RESULT_FALSE;
    };
    let Some(iid) = read_tuid(iid) else {
        unsafe { *obj = ptr::null_mut() };
        return K_RESULT_FALSE;
    };

    if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_I_COMPONENT_HANDLER_IID) {
        object.ref_count.fetch_add(1, Ordering::Relaxed);
        unsafe { *obj = this.cast::<c_void>() };
        K_RESULT_OK
    } else {
        unsafe { *obj = ptr::null_mut() };
        K_RESULT_FALSE
    }
}

unsafe extern "system" fn handler_add_ref(this: *mut IComponentHandler) -> u32 {
    handler_object_mut(this)
        .map(|object| object.ref_count.fetch_add(1, Ordering::Relaxed) + 1)
        .unwrap_or(0)
}

unsafe extern "system" fn handler_release(this: *mut IComponentHandler) -> u32 {
    handler_object_mut(this)
        .map(|object| {
            object
                .ref_count
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    Some(value.saturating_sub(1))
                })
                .unwrap_or(0)
                .saturating_sub(1)
        })
        .unwrap_or(0)
}

unsafe extern "system" fn handler_begin_edit(this: *mut IComponentHandler, id: ParamId) -> i32 {
    if let Some(object) = handler_object_mut(this) {
        object.record(
            Vst3ComponentHandlerEventKind::BeginEdit,
            Some(id),
            None,
            None,
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
            Vst3ComponentHandlerEventKind::PerformEdit,
            Some(id),
            value_normalized.is_finite().then_some(value_normalized),
            None,
        );
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler_end_edit(this: *mut IComponentHandler, id: ParamId) -> i32 {
    if let Some(object) = handler_object_mut(this) {
        object.record(Vst3ComponentHandlerEventKind::EndEdit, Some(id), None, None);
    }
    K_RESULT_OK
}

unsafe extern "system" fn handler_restart_component(
    this: *mut IComponentHandler,
    flags: i32,
) -> i32 {
    if let Some(object) = handler_object_mut(this) {
        object.record(
            Vst3ComponentHandlerEventKind::RestartComponent,
            None,
            None,
            Some(flags),
        );
    }
    K_RESULT_OK
}

fn handler_object_mut(this: *mut IComponentHandler) -> Option<&'static mut ComponentHandlerObject> {
    if this.is_null() {
        return None;
    }

    Some(unsafe { &mut *this.cast::<ComponentHandlerObject>() })
}

impl ComponentHandlerObject {
    fn record(
        &self,
        kind: Vst3ComponentHandlerEventKind,
        parameter_id: Option<u32>,
        value_normalized: Option<f64>,
        flags: Option<i32>,
    ) {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let event = Vst3ComponentHandlerEvent {
            sequence,
            kind,
            parameter_id,
            value_normalized,
            flags,
        };
        if let Ok(mut events) = self.events.lock() {
            if events.len() == MAX_COMPONENT_HANDLER_EVENTS {
                events.remove(0);
            }
            events.push(event);
        }
    }

    fn snapshot(&self) -> Vst3ComponentHandlerSnapshot {
        let total_events = self.next_sequence.load(Ordering::Relaxed).saturating_sub(1);
        let recent_events = self
            .events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default();
        Vst3ComponentHandlerSnapshot {
            total_events,
            recent_events,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_component_handler_interface() {
        let mut handler = Vst3ComponentHandler::new();
        let iid = parse_tuid_hex(VST3_I_COMPONENT_HANDLER_IID).expect("iid");
        let mut object = ptr::null_mut();

        let result = unsafe {
            ((*(*handler.as_mut_ptr()).vtable).query_interface)(
                handler.as_mut_ptr(),
                iid.as_ptr().cast(),
                &mut object,
            )
        };

        assert_eq!(result, K_RESULT_OK);
        assert_eq!(object, handler.as_mut_ptr().cast::<c_void>());
    }

    #[test]
    fn records_edit_and_restart_callbacks() {
        let mut handler = Vst3ComponentHandler::new();
        let pointer = handler.as_mut_ptr();

        unsafe {
            ((*(*pointer).vtable).begin_edit)(pointer, 42);
            ((*(*pointer).vtable).perform_edit)(pointer, 42, 0.75);
            ((*(*pointer).vtable).end_edit)(pointer, 42);
            ((*(*pointer).vtable).restart_component)(pointer, 3);
        }

        let snapshot = handler.snapshot();
        assert_eq!(snapshot.total_events, 4);
        assert_eq!(snapshot.recent_events.len(), 4);
        assert_eq!(
            snapshot.recent_events[0].kind,
            Vst3ComponentHandlerEventKind::BeginEdit
        );
        assert_eq!(snapshot.recent_events[1].parameter_id, Some(42));
        assert_eq!(snapshot.recent_events[1].value_normalized, Some(0.75));
        assert_eq!(
            snapshot.recent_events[3].kind,
            Vst3ComponentHandlerEventKind::RestartComponent
        );
        assert_eq!(snapshot.recent_events[3].flags, Some(3));
    }
}
