use std::ffi::{c_char, c_void};
use std::ptr;

use super::*;
use crate::Vst3BusDirection;
use crate::vst3_abi::{
    IBStream, IUnitInfo, IUnitInfoVTable, K_RESULT_FALSE, K_RESULT_OK, ProgramListInfo, String128,
    TUid, UnitInfo, VST3_BUS_DIRECTION_INPUT, VST3_FUNKNOWN_IID, VST3_I_UNIT_INFO_IID,
    VST3_MEDIA_TYPE_AUDIO, parse_tuid_hex,
};

#[test]
fn reads_units_program_lists_and_releases_unit_info() {
    let mut fake = FakeUnitInfo::new();
    {
        let unit_info =
            unsafe { Vst3UnitInfo::from_raw(fake.as_raw_ptr()) }.expect("unit info facade");
        let metadata = unit_info.metadata().expect("metadata");

        assert_eq!(metadata.selected_unit_id, 1);
        assert_eq!(metadata.units.len(), 2);
        assert_eq!(metadata.units[0].id, 0);
        assert_eq!(metadata.units[0].parent_unit_id, -1);
        assert_eq!(metadata.units[0].name.as_deref(), Some("Root"));
        assert_eq!(metadata.units[0].program_list_id, None);
        assert_eq!(metadata.units[1].id, 1);
        assert_eq!(metadata.units[1].program_list_id, Some(7));
        assert_eq!(metadata.program_lists.len(), 1);
        assert_eq!(metadata.program_lists[0].id, 7);
        assert_eq!(metadata.program_lists[0].name.as_deref(), Some("Factory"));
        assert_eq!(metadata.program_lists[0].program_count, 2);
        assert_eq!(
            metadata.program_lists[0]
                .programs
                .iter()
                .map(|program| program.name.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("Init"), Some("Bass")]
        );
    }

    assert!(fake.released);
}

#[test]
fn controls_unit_selection_bus_mapping_and_program_data() {
    let mut fake = FakeUnitInfo::new();
    {
        let unit_info =
            unsafe { Vst3UnitInfo::from_raw(fake.as_raw_ptr()) }.expect("unit info facade");

        unit_info.select_unit(2).expect("select unit");
        assert_eq!(unit_info.selected_unit(), 2);
        assert_eq!(
            unit_info
                .unit_by_audio_bus(Vst3BusDirection::Input, 0, 1)
                .expect("unit by bus"),
            Some(1)
        );
        assert_eq!(
            unit_info
                .unit_by_audio_bus(Vst3BusDirection::Output, 0, 0)
                .expect("no unit by bus"),
            None
        );
        unit_info
            .set_unit_program_data(7, 1, &[4, 5, 6])
            .expect("set unit program data");
    }

    assert_eq!(fake.last_unit_program_data, vec![4, 5, 6]);
    assert!(fake.released);
}

#[repr(C)]
struct FakeUnitInfo {
    iface: IUnitInfo,
    selected_unit: i32,
    last_unit_program_data: Vec<u8>,
    released: bool,
}

impl FakeUnitInfo {
    fn new() -> Self {
        Self {
            iface: IUnitInfo {
                vtable: &FAKE_UNIT_INFO_VTABLE,
            },
            selected_unit: 1,
            last_unit_program_data: Vec::new(),
            released: false,
        }
    }

    fn as_raw_ptr(&mut self) -> *mut IUnitInfo {
        &mut self.iface
    }
}

static FAKE_UNIT_INFO_VTABLE: IUnitInfoVTable = IUnitInfoVTable {
    query_interface: fake_query_interface,
    add_ref: fake_add_ref,
    release: fake_release,
    get_unit_count: fake_get_unit_count,
    get_unit_info: fake_get_unit_info,
    get_program_list_count: fake_get_program_list_count,
    get_program_list_info: fake_get_program_list_info,
    get_program_name: fake_get_program_name,
    get_program_info: fake_get_program_info,
    has_program_pitch_names: fake_has_program_pitch_names,
    get_program_pitch_name: fake_get_program_pitch_name,
    get_selected_unit: fake_get_selected_unit,
    select_unit: fake_select_unit,
    get_unit_by_bus: fake_get_unit_by_bus,
    set_unit_program_data: fake_set_unit_program_data,
};

unsafe extern "system" fn fake_query_interface(
    this: *mut IUnitInfo,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        unsafe { *obj = ptr::null_mut() };
    }
    let Some(iid) = read_tuid(iid) else {
        return K_RESULT_FALSE;
    };

    if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_I_UNIT_INFO_IID) {
        if !obj.is_null() {
            unsafe { *obj = this.cast() };
        }
        K_RESULT_OK
    } else {
        K_RESULT_FALSE
    }
}

unsafe extern "system" fn fake_add_ref(_this: *mut IUnitInfo) -> u32 {
    1
}

unsafe extern "system" fn fake_release(this: *mut IUnitInfo) -> u32 {
    unsafe { (*this.cast::<FakeUnitInfo>()).released = true };
    1
}

unsafe extern "system" fn fake_get_unit_count(_this: *mut IUnitInfo) -> i32 {
    2
}

unsafe extern "system" fn fake_get_unit_info(
    _this: *mut IUnitInfo,
    unit_index: i32,
    info: *mut UnitInfo,
) -> i32 {
    if info.is_null() {
        return K_RESULT_FALSE;
    }

    let mut unit = UnitInfo::default();
    match unit_index {
        0 => {
            unit.id = 0;
            unit.parent_unit_id = -1;
            write_string128(&mut unit.name, "Root");
        }
        1 => {
            unit.id = 1;
            unit.parent_unit_id = 0;
            unit.program_list_id = 7;
            write_string128(&mut unit.name, "Synth");
        }
        _ => return K_RESULT_FALSE,
    }
    unsafe { *info = unit };
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_program_list_count(_this: *mut IUnitInfo) -> i32 {
    1
}

unsafe extern "system" fn fake_get_program_list_info(
    _this: *mut IUnitInfo,
    list_index: i32,
    info: *mut ProgramListInfo,
) -> i32 {
    if info.is_null() || list_index != 0 {
        return K_RESULT_FALSE;
    }

    let mut list = ProgramListInfo {
        id: 7,
        program_count: 2,
        ..ProgramListInfo::default()
    };
    write_string128(&mut list.name, "Factory");
    unsafe { *info = list };
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_program_name(
    _this: *mut IUnitInfo,
    list_id: i32,
    program_index: i32,
    name: *mut String128,
) -> i32 {
    if name.is_null() || list_id != 7 {
        return K_RESULT_FALSE;
    }

    let value = match program_index {
        0 => "Init",
        1 => "Bass",
        _ => return K_RESULT_FALSE,
    };
    unsafe { write_string128(&mut *name, value) };
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_program_info(
    _this: *mut IUnitInfo,
    _list_id: i32,
    _program_index: i32,
    _attribute_id: *const c_char,
    _attribute_value: *mut String128,
) -> i32 {
    K_RESULT_FALSE
}

unsafe extern "system" fn fake_has_program_pitch_names(
    _this: *mut IUnitInfo,
    _list_id: i32,
    _program_index: i32,
) -> i32 {
    K_RESULT_FALSE
}

unsafe extern "system" fn fake_get_program_pitch_name(
    _this: *mut IUnitInfo,
    _list_id: i32,
    _program_index: i32,
    _midi_pitch: i16,
    _name: *mut String128,
) -> i32 {
    K_RESULT_FALSE
}

unsafe extern "system" fn fake_get_selected_unit(_this: *mut IUnitInfo) -> i32 {
    unsafe { (*_this.cast::<FakeUnitInfo>()).selected_unit }
}

unsafe extern "system" fn fake_select_unit(this: *mut IUnitInfo, unit_id: i32) -> i32 {
    unsafe { (*this.cast::<FakeUnitInfo>()).selected_unit = unit_id };
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_unit_by_bus(
    _this: *mut IUnitInfo,
    media_type: i32,
    direction: i32,
    bus_index: i32,
    channel: i32,
    unit_id: *mut i32,
) -> i32 {
    if media_type == VST3_MEDIA_TYPE_AUDIO
        && direction == VST3_BUS_DIRECTION_INPUT
        && bus_index == 0
        && channel == 1
        && !unit_id.is_null()
    {
        unsafe { *unit_id = 1 };
        return K_RESULT_OK;
    }
    K_RESULT_FALSE
}

unsafe extern "system" fn fake_set_unit_program_data(
    this: *mut IUnitInfo,
    list_or_unit_id: i32,
    program_index: i32,
    data: *mut IBStream,
) -> i32 {
    if list_or_unit_id != 7 || program_index != 1 {
        return K_RESULT_FALSE;
    }
    unsafe { (*this.cast::<FakeUnitInfo>()).last_unit_program_data = read_stream(data) };
    K_RESULT_OK
}

fn write_string128(destination: &mut String128, value: &str) {
    destination.fill(0);
    for (index, unit) in value.encode_utf16().take(127).enumerate() {
        destination[index] = unit;
    }
}

fn read_tuid(iid: *const i8) -> Option<TUid> {
    if iid.is_null() {
        return None;
    }

    let mut value = [0; 16];
    unsafe { ptr::copy_nonoverlapping(iid.cast::<u8>(), value.as_mut_ptr(), 16) };
    Some(value)
}

fn tuid(value: &str) -> TUid {
    parse_tuid_hex(value).expect("valid built-in iid")
}

fn read_stream(stream: *mut IBStream) -> Vec<u8> {
    if stream.is_null() {
        return Vec::new();
    }
    let mut output = [0_u8; 8];
    let mut read = 0;
    let result =
        unsafe { ((*(*stream).vtable).read)(stream, output.as_mut_ptr().cast(), 8, &mut read) };
    assert_eq!(result, K_RESULT_OK);
    output[..usize::try_from(read).expect("read count")].to_vec()
}
