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
pub const VST3_I_HOST_APPLICATION_IID: &str = "58E595CCDB2D49698B6AAF8C36A664E5";
pub const VST3_PROCESS_MODE_REALTIME: i32 = 0;
pub const VST3_SAMPLE_32: i32 = 0;
pub const VST3_MEDIA_TYPE_AUDIO: i32 = 0;
pub const VST3_BUS_DIRECTION_INPUT: i32 = 0;
pub const VST3_BUS_DIRECTION_OUTPUT: i32 = 1;
pub const VST3_SPEAKER_MONO: SpeakerArrangement = 1 << 19;
pub const VST3_SPEAKER_STEREO: SpeakerArrangement = 0x03;

pub type TBool = u8;
pub type TUid = [u8; 16];
pub type SampleRate = f64;
pub type SpeakerArrangement = u64;
pub type String128 = [u16; 128];

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
