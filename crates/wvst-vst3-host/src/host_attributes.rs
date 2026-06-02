use std::collections::BTreeMap;
use std::ffi::{CStr, c_void};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::vst3_abi::{
    FidString, IAttributeList, IAttributeListVTable, K_RESULT_FALSE, K_RESULT_OK, TUid,
    VST3_FUNKNOWN_IID, VST3_I_ATTRIBUTE_LIST_IID, parse_tuid_hex,
};

#[derive(Debug)]
pub struct Vst3HostAttributeList {
    object: Box<AttributeListObject>,
}

#[derive(Debug, Clone)]
enum AttributeValue {
    Int(i64),
    Float(f64),
    String(Vec<u16>),
    Binary(Vec<u8>),
}

#[repr(C)]
#[derive(Debug)]
struct AttributeListObject {
    iface: IAttributeList,
    ref_count: AtomicU32,
    values: BTreeMap<String, AttributeValue>,
}

impl Vst3HostAttributeList {
    pub fn new() -> Self {
        Self {
            object: Box::new(AttributeListObject {
                iface: IAttributeList {
                    vtable: &ATTRIBUTE_LIST_VTABLE,
                },
                ref_count: AtomicU32::new(1),
                values: BTreeMap::new(),
            }),
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut IAttributeList {
        &mut self.object.iface
    }

    pub fn into_raw(self) -> *mut IAttributeList {
        Box::into_raw(self.object).cast::<IAttributeList>()
    }
}

impl Default for Vst3HostAttributeList {
    fn default() -> Self {
        Self::new()
    }
}

const ATTRIBUTE_LIST_VTABLE: IAttributeListVTable = IAttributeListVTable {
    query_interface: attribute_query_interface,
    add_ref: attribute_add_ref,
    release: attribute_release,
    set_int: attribute_set_int,
    get_int: attribute_get_int,
    set_float: attribute_set_float,
    get_float: attribute_get_float,
    set_string: attribute_set_string,
    get_string: attribute_get_string,
    set_binary: attribute_set_binary,
    get_binary: attribute_get_binary,
};

unsafe extern "system" fn attribute_query_interface(
    this: *mut IAttributeList,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return K_RESULT_FALSE;
    }
    let Some(object) = attribute_object_mut(this) else {
        return K_RESULT_FALSE;
    };
    let Some(iid) = read_tuid(iid) else {
        unsafe { *obj = ptr::null_mut() };
        return K_RESULT_FALSE;
    };

    if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_I_ATTRIBUTE_LIST_IID) {
        object.ref_count.fetch_add(1, Ordering::Relaxed);
        unsafe { *obj = this.cast::<c_void>() };
        K_RESULT_OK
    } else {
        unsafe { *obj = ptr::null_mut() };
        K_RESULT_FALSE
    }
}

unsafe extern "system" fn attribute_add_ref(this: *mut IAttributeList) -> u32 {
    attribute_object_mut(this)
        .map(|object| object.ref_count.fetch_add(1, Ordering::Relaxed) + 1)
        .unwrap_or(0)
}

unsafe extern "system" fn attribute_release(this: *mut IAttributeList) -> u32 {
    let Some(object) = attribute_object_mut(this) else {
        return 0;
    };
    let previous = object.ref_count.fetch_sub(1, Ordering::Release);
    let remaining = previous.saturating_sub(1);
    if remaining == 0 {
        std::sync::atomic::fence(Ordering::Acquire);
        unsafe { drop(Box::from_raw(this.cast::<AttributeListObject>())) };
    }
    remaining
}

unsafe extern "system" fn attribute_set_int(
    this: *mut IAttributeList,
    id: FidString,
    value: i64,
) -> i32 {
    set_attribute(this, id, AttributeValue::Int(value))
}

unsafe extern "system" fn attribute_get_int(
    this: *mut IAttributeList,
    id: FidString,
    value: *mut i64,
) -> i32 {
    if value.is_null() {
        return K_RESULT_FALSE;
    }
    match get_attribute(this, id) {
        Some(AttributeValue::Int(stored)) => {
            unsafe { *value = *stored };
            K_RESULT_OK
        }
        _ => K_RESULT_FALSE,
    }
}

unsafe extern "system" fn attribute_set_float(
    this: *mut IAttributeList,
    id: FidString,
    value: f64,
) -> i32 {
    set_attribute(this, id, AttributeValue::Float(value))
}

unsafe extern "system" fn attribute_get_float(
    this: *mut IAttributeList,
    id: FidString,
    value: *mut f64,
) -> i32 {
    if value.is_null() {
        return K_RESULT_FALSE;
    }
    match get_attribute(this, id) {
        Some(AttributeValue::Float(stored)) => {
            unsafe { *value = *stored };
            K_RESULT_OK
        }
        _ => K_RESULT_FALSE,
    }
}

unsafe extern "system" fn attribute_set_string(
    this: *mut IAttributeList,
    id: FidString,
    value: *const u16,
) -> i32 {
    let Some(value) = utf16_string(value) else {
        return K_RESULT_FALSE;
    };
    set_attribute(this, id, AttributeValue::String(value))
}

unsafe extern "system" fn attribute_get_string(
    this: *mut IAttributeList,
    id: FidString,
    value: *mut u16,
    size_in_bytes: u32,
) -> i32 {
    if value.is_null() || size_in_bytes < 2 {
        return K_RESULT_FALSE;
    }
    let capacity = (size_in_bytes / 2) as usize;
    match get_attribute(this, id) {
        Some(AttributeValue::String(stored)) => {
            copy_utf16_with_nul(stored, value, capacity);
            K_RESULT_OK
        }
        _ => K_RESULT_FALSE,
    }
}

unsafe extern "system" fn attribute_set_binary(
    this: *mut IAttributeList,
    id: FidString,
    data: *const c_void,
    size_in_bytes: u32,
) -> i32 {
    if data.is_null() && size_in_bytes > 0 {
        return K_RESULT_FALSE;
    }
    let bytes = if size_in_bytes == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size_in_bytes as usize) }.to_vec()
    };
    set_attribute(this, id, AttributeValue::Binary(bytes))
}

unsafe extern "system" fn attribute_get_binary(
    this: *mut IAttributeList,
    id: FidString,
    data: *mut *const c_void,
    size_in_bytes: *mut u32,
) -> i32 {
    if data.is_null() || size_in_bytes.is_null() {
        return K_RESULT_FALSE;
    }
    match get_attribute(this, id) {
        Some(AttributeValue::Binary(stored)) => {
            unsafe {
                *data = stored.as_ptr().cast::<c_void>();
                *size_in_bytes = u32::try_from(stored.len()).unwrap_or(u32::MAX);
            }
            K_RESULT_OK
        }
        _ => K_RESULT_FALSE,
    }
}

fn set_attribute(this: *mut IAttributeList, id: FidString, value: AttributeValue) -> i32 {
    let Some(id) = fid_string(id) else {
        return K_RESULT_FALSE;
    };
    let Some(object) = attribute_object_mut(this) else {
        return K_RESULT_FALSE;
    };
    object.values.insert(id, value);
    K_RESULT_OK
}

fn get_attribute(this: *mut IAttributeList, id: FidString) -> Option<&'static AttributeValue> {
    let id = fid_string(id)?;
    let object = attribute_object(this)?;
    object.values.get(&id)
}

fn attribute_object(this: *mut IAttributeList) -> Option<&'static AttributeListObject> {
    if this.is_null() {
        return None;
    }
    Some(unsafe { &*this.cast::<AttributeListObject>() })
}

fn attribute_object_mut(this: *mut IAttributeList) -> Option<&'static mut AttributeListObject> {
    if this.is_null() {
        return None;
    }
    Some(unsafe { &mut *this.cast::<AttributeListObject>() })
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

fn utf16_string(value: *const u16) -> Option<Vec<u16>> {
    if value.is_null() {
        return None;
    }
    let mut len = 0;
    unsafe {
        while *value.add(len) != 0 {
            len += 1;
        }
        Some(std::slice::from_raw_parts(value, len).to_vec())
    }
}

fn copy_utf16_with_nul(source: &[u16], output: *mut u16, capacity: usize) {
    if capacity == 0 {
        return;
    }
    let copy_len = source.len().min(capacity.saturating_sub(1));
    if copy_len > 0 {
        unsafe { ptr::copy_nonoverlapping(source.as_ptr(), output, copy_len) };
    }
    unsafe { *output.add(copy_len) = 0 };
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
    fn attribute_list_round_trips_float_string_and_binary() {
        let list = Vst3HostAttributeList::new();
        let raw = list.into_raw();
        let float_key = CString::new("float").expect("key");
        let string_key = CString::new("string").expect("key");
        let binary_key = CString::new("binary").expect("key");
        let input_string = [b'o' as u16, b'k' as u16, 0];
        let input_binary = [1_u8, 2, 3];
        let mut float_value = 0.0;
        let mut string_output = [0_u16; 8];
        let mut binary_ptr = ptr::null();
        let mut binary_len = 0;

        unsafe {
            ((*(*raw).vtable).set_float)(raw, float_key.as_ptr(), 0.25);
            ((*(*raw).vtable).set_string)(raw, string_key.as_ptr(), input_string.as_ptr());
            ((*(*raw).vtable).set_binary)(
                raw,
                binary_key.as_ptr(),
                input_binary.as_ptr().cast(),
                input_binary.len() as u32,
            );
        }

        assert_eq!(
            unsafe { ((*(*raw).vtable).get_float)(raw, float_key.as_ptr(), &mut float_value) },
            K_RESULT_OK
        );
        assert_eq!(float_value, 0.25);
        assert_eq!(
            unsafe {
                ((*(*raw).vtable).get_string)(
                    raw,
                    string_key.as_ptr(),
                    string_output.as_mut_ptr(),
                    (string_output.len() * 2) as u32,
                )
            },
            K_RESULT_OK
        );
        assert_eq!(&string_output[..3], &input_string);
        assert_eq!(
            unsafe {
                ((*(*raw).vtable).get_binary)(
                    raw,
                    binary_key.as_ptr(),
                    &mut binary_ptr,
                    &mut binary_len,
                )
            },
            K_RESULT_OK
        );
        let binary =
            unsafe { std::slice::from_raw_parts(binary_ptr.cast::<u8>(), binary_len as usize) };
        assert_eq!(binary, input_binary);

        unsafe {
            ((*(*raw).vtable).release)(raw);
        }
    }
}
