use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::vst3_abi::{
    FUnknown, IHostApplication, IHostApplicationVTable, K_RESULT_FALSE, K_RESULT_OK, String128,
    TUid, VST3_FUNKNOWN_IID, VST3_I_HOST_APPLICATION_IID, parse_tuid_hex,
};

#[derive(Debug)]
pub struct Vst3HostContext {
    object: Box<HostContextObject>,
}

impl Vst3HostContext {
    pub fn new(host_name: &str) -> Self {
        Self {
            object: Box::new(HostContextObject {
                iface: IHostApplication {
                    vtable: &HOST_APPLICATION_VTABLE,
                },
                ref_count: AtomicU32::new(1),
                host_name: string128(host_name),
            }),
        }
    }

    pub fn as_funknown_ptr(&mut self) -> *mut FUnknown {
        (&mut self.object.iface as *mut IHostApplication).cast::<FUnknown>()
    }
}

#[repr(C)]
#[derive(Debug)]
struct HostContextObject {
    iface: IHostApplication,
    ref_count: AtomicU32,
    host_name: String128,
}

const HOST_APPLICATION_VTABLE: IHostApplicationVTable = IHostApplicationVTable {
    query_interface: host_query_interface,
    add_ref: host_add_ref,
    release: host_release,
    get_name: host_get_name,
    create_instance: host_create_instance,
};

unsafe extern "system" fn host_query_interface(
    this: *mut IHostApplication,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return K_RESULT_FALSE;
    }

    let Some(object) = host_object_mut(this) else {
        return K_RESULT_FALSE;
    };
    let Some(iid) = read_tuid(iid) else {
        unsafe { *obj = ptr::null_mut() };
        return K_RESULT_FALSE;
    };

    if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_I_HOST_APPLICATION_IID) {
        object.ref_count.fetch_add(1, Ordering::Relaxed);
        unsafe { *obj = this.cast::<c_void>() };
        K_RESULT_OK
    } else {
        unsafe { *obj = ptr::null_mut() };
        K_RESULT_FALSE
    }
}

unsafe extern "system" fn host_add_ref(this: *mut IHostApplication) -> u32 {
    host_object_mut(this)
        .map(|object| object.ref_count.fetch_add(1, Ordering::Relaxed) + 1)
        .unwrap_or(0)
}

unsafe extern "system" fn host_release(this: *mut IHostApplication) -> u32 {
    host_object_mut(this)
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

unsafe extern "system" fn host_get_name(this: *mut IHostApplication, name: *mut String128) -> i32 {
    if name.is_null() {
        return K_RESULT_FALSE;
    }

    let Some(object) = host_object(this) else {
        return K_RESULT_FALSE;
    };

    unsafe { *name = object.host_name };
    K_RESULT_OK
}

unsafe extern "system" fn host_create_instance(
    _this: *mut IHostApplication,
    _cid: *mut TUid,
    _iid: *mut TUid,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        unsafe { *obj = ptr::null_mut() };
    }
    K_RESULT_FALSE
}

fn host_object(this: *mut IHostApplication) -> Option<&'static HostContextObject> {
    if this.is_null() {
        return None;
    }

    // SAFETY: Vst3HostContext exposes pointers to HostContextObject.iface,
    // which is the first field of the repr(C) object. The object is boxed, so
    // moves of Vst3HostContext do not invalidate the address.
    Some(unsafe { &*this.cast::<HostContextObject>() })
}

fn host_object_mut(this: *mut IHostApplication) -> Option<&'static mut HostContextObject> {
    if this.is_null() {
        return None;
    }

    // SAFETY: See host_object. Mutability is required only for refcount changes
    // and is safe because refcount itself is atomic.
    Some(unsafe { &mut *this.cast::<HostContextObject>() })
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

fn string128(value: &str) -> String128 {
    let mut output = [0; 128];
    for (index, unit) in value.encode_utf16().take(127).enumerate() {
        output[index] = unit;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_host_application_name() {
        let mut context = Vst3HostContext::new("WVST");
        let host = context.as_funknown_ptr().cast::<IHostApplication>();
        let mut name = [0; 128];

        let result = unsafe { ((*(*host).vtable).get_name)(host, &mut name) };

        assert_eq!(result, K_RESULT_OK);
        assert_eq!(String::from_utf16_lossy(&name[..4]), "WVST");
    }

    #[test]
    fn query_interface_returns_host_application() {
        let mut context = Vst3HostContext::new("WVST");
        let host = context.as_funknown_ptr().cast::<IHostApplication>();
        let iid = parse_tuid_hex(VST3_I_HOST_APPLICATION_IID).expect("iid");
        let mut object = ptr::null_mut();

        let result =
            unsafe { ((*(*host).vtable).query_interface)(host, iid.as_ptr().cast(), &mut object) };

        assert_eq!(result, K_RESULT_OK);
        assert_eq!(object, host.cast::<c_void>());
    }
}
