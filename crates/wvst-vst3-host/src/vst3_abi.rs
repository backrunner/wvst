use std::ffi::{c_char, c_void};

pub const K_RESULT_OK: i32 = 0;
pub const VST3_I_COMPONENT_IID: &str = "E831FF31F2D54301928EBBEE25697802";

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
}
