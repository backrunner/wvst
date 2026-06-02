use std::ffi::{CStr, CString, c_void};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::host_attributes::Vst3HostAttributeList;
use crate::vst3_abi::{
    FidString, IAttributeList, IMessage, IMessageVTable, K_RESULT_FALSE, K_RESULT_OK, TUid,
    VST3_FUNKNOWN_IID, VST3_I_MESSAGE_IID, parse_tuid_hex,
};

#[derive(Debug)]
pub struct Vst3HostMessage {
    object: Box<MessageObject>,
}

#[repr(C)]
#[derive(Debug)]
struct MessageObject {
    iface: IMessage,
    ref_count: AtomicU32,
    message_id: Option<CString>,
    attributes: Vst3HostAttributeList,
}

impl Vst3HostMessage {
    pub fn new() -> Self {
        Self {
            object: Box::new(MessageObject {
                iface: IMessage {
                    vtable: &MESSAGE_VTABLE,
                },
                ref_count: AtomicU32::new(1),
                message_id: None,
                attributes: Vst3HostAttributeList::new(),
            }),
        }
    }

    pub fn into_raw(self) -> *mut IMessage {
        Box::into_raw(self.object).cast::<IMessage>()
    }

    pub fn as_mut_ptr(&mut self) -> *mut IMessage {
        &mut self.object.iface
    }

    pub fn set_id(&mut self, id: &str) -> Result<(), std::ffi::NulError> {
        self.object.message_id = Some(CString::new(id)?);
        Ok(())
    }

    pub fn attributes_mut(&mut self) -> &mut Vst3HostAttributeList {
        &mut self.object.attributes
    }
}

impl Default for Vst3HostMessage {
    fn default() -> Self {
        Self::new()
    }
}

const MESSAGE_VTABLE: IMessageVTable = IMessageVTable {
    query_interface: message_query_interface,
    add_ref: message_add_ref,
    release: message_release,
    get_message_id: message_get_id,
    set_message_id: message_set_id,
    get_attributes: message_get_attributes,
};

unsafe extern "system" fn message_query_interface(
    this: *mut IMessage,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return K_RESULT_FALSE;
    }
    let Some(object) = message_object_mut(this) else {
        return K_RESULT_FALSE;
    };
    let Some(iid) = read_tuid(iid) else {
        unsafe { *obj = ptr::null_mut() };
        return K_RESULT_FALSE;
    };

    if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_I_MESSAGE_IID) {
        object.ref_count.fetch_add(1, Ordering::Relaxed);
        unsafe { *obj = this.cast::<c_void>() };
        K_RESULT_OK
    } else {
        unsafe { *obj = ptr::null_mut() };
        K_RESULT_FALSE
    }
}

unsafe extern "system" fn message_add_ref(this: *mut IMessage) -> u32 {
    message_object_mut(this)
        .map(|object| object.ref_count.fetch_add(1, Ordering::Relaxed) + 1)
        .unwrap_or(0)
}

unsafe extern "system" fn message_release(this: *mut IMessage) -> u32 {
    let Some(object) = message_object_mut(this) else {
        return 0;
    };
    let previous = object.ref_count.fetch_sub(1, Ordering::Release);
    let remaining = previous.saturating_sub(1);
    if remaining == 0 {
        std::sync::atomic::fence(Ordering::Acquire);
        unsafe { drop(Box::from_raw(this.cast::<MessageObject>())) };
    }
    remaining
}

unsafe extern "system" fn message_get_id(this: *mut IMessage) -> FidString {
    message_object(this)
        .and_then(|object| object.message_id.as_ref())
        .map_or(ptr::null(), |id| id.as_ptr())
}

unsafe extern "system" fn message_set_id(this: *mut IMessage, id: FidString) {
    let Some(object) = message_object_mut(this) else {
        return;
    };
    object.message_id = fid_string(id).and_then(|id| CString::new(id).ok());
}

unsafe extern "system" fn message_get_attributes(this: *mut IMessage) -> *mut IAttributeList {
    let Some(object) = message_object_mut(this) else {
        return ptr::null_mut();
    };
    let attributes = object.attributes.as_mut_ptr();
    unsafe { ((*(*attributes).vtable).add_ref)(attributes) };
    attributes
}

fn message_object(this: *mut IMessage) -> Option<&'static MessageObject> {
    if this.is_null() {
        return None;
    }
    Some(unsafe { &*this.cast::<MessageObject>() })
}

fn message_object_mut(this: *mut IMessage) -> Option<&'static mut MessageObject> {
    if this.is_null() {
        return None;
    }
    Some(unsafe { &mut *this.cast::<MessageObject>() })
}

fn fid_string(value: FidString) -> Option<String> {
    if value.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .ok()
        .map(str::to_string)
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
    use crate::vst3_abi::K_RESULT_OK;
    use std::ffi::CString;

    #[test]
    fn message_stores_id_and_attributes() {
        let message = Vst3HostMessage::new();
        let raw = message.into_raw();
        let id = CString::new("TextMessage").expect("id");

        unsafe { ((*(*raw).vtable).set_message_id)(raw, id.as_ptr()) };
        let returned = unsafe { ((*(*raw).vtable).get_message_id)(raw) };
        let attributes = unsafe { ((*(*raw).vtable).get_attributes)(raw) };
        let key = CString::new("answer").expect("key");
        let mut value = 0;

        let set_result = unsafe { ((*(*attributes).vtable).set_int)(attributes, key.as_ptr(), 42) };
        let get_result =
            unsafe { ((*(*attributes).vtable).get_int)(attributes, key.as_ptr(), &mut value) };

        assert_eq!(
            unsafe { CStr::from_ptr(returned) }.to_str().ok(),
            Some("TextMessage")
        );
        assert_eq!(set_result, K_RESULT_OK);
        assert_eq!(get_result, K_RESULT_OK);
        assert_eq!(value, 42);

        unsafe {
            ((*(*attributes).vtable).release)(attributes);
            ((*(*raw).vtable).release)(raw);
        }
    }
}
