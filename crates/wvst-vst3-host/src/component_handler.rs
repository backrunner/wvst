use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::vst3_abi::{
    IComponentHandler, IComponentHandlerVTable, K_RESULT_FALSE, K_RESULT_OK, ParamId, ParamValue,
    TUid, VST3_FUNKNOWN_IID, VST3_I_COMPONENT_HANDLER_IID, parse_tuid_hex,
};

#[derive(Debug)]
pub struct Vst3ComponentHandler {
    object: Box<ComponentHandlerObject>,
}

impl Vst3ComponentHandler {
    pub fn new() -> Self {
        Self {
            object: Box::new(ComponentHandlerObject {
                iface: IComponentHandler {
                    vtable: &COMPONENT_HANDLER_VTABLE,
                },
                ref_count: AtomicU32::new(1),
            }),
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut IComponentHandler {
        &mut self.object.iface
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

unsafe extern "system" fn handler_begin_edit(_this: *mut IComponentHandler, _id: ParamId) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn handler_perform_edit(
    _this: *mut IComponentHandler,
    _id: ParamId,
    _value_normalized: ParamValue,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn handler_end_edit(_this: *mut IComponentHandler, _id: ParamId) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn handler_restart_component(
    _this: *mut IComponentHandler,
    _flags: i32,
) -> i32 {
    K_RESULT_OK
}

fn handler_object_mut(this: *mut IComponentHandler) -> Option<&'static mut ComponentHandlerObject> {
    if this.is_null() {
        return None;
    }

    Some(unsafe { &mut *this.cast::<ComponentHandlerObject>() })
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
}
