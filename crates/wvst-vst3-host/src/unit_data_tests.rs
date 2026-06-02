use std::ffi::c_void;
use std::ptr;

use super::*;
use crate::vst3_abi::{
    IBStream, IProgramListData, IProgramListDataVTable, IUnitData, IUnitDataVTable, K_RESULT_FALSE,
    K_RESULT_OK,
};

#[test]
fn program_list_data_reads_sets_and_releases() {
    let mut fake = FakeProgramListData::new();
    {
        let data =
            unsafe { Vst3ProgramListData::from_raw(fake.as_raw_ptr()) }.expect("program data");

        assert!(data.program_data_supported(7).expect("supported"));
        assert!(!data.program_data_supported(8).expect("unsupported"));
        assert_eq!(
            data.get_program_data(7, 1).expect("get program data"),
            vec![1, 2, 3]
        );
        data.set_program_data(7, 1, &[5, 6, 7])
            .expect("set program data");
    }

    assert_eq!(fake.last_set, vec![5, 6, 7]);
    assert!(fake.released);
}

#[test]
fn unit_data_reads_sets_and_releases() {
    let mut fake = FakeUnitData::new();
    {
        let data = unsafe { Vst3UnitData::from_raw(fake.as_raw_ptr()) }.expect("unit data");

        assert!(data.unit_data_supported(2).expect("supported"));
        assert!(!data.unit_data_supported(3).expect("unsupported"));
        assert_eq!(data.get_unit_data(2).expect("get unit data"), vec![9, 8]);
        data.set_unit_data(2, &[4, 3]).expect("set unit data");
    }

    assert_eq!(fake.last_set, vec![4, 3]);
    assert!(fake.released);
}

#[repr(C)]
struct FakeProgramListData {
    iface: IProgramListData,
    last_set: Vec<u8>,
    released: bool,
}

impl FakeProgramListData {
    fn new() -> Self {
        Self {
            iface: IProgramListData {
                vtable: &PROGRAM_LIST_DATA_VTABLE,
            },
            last_set: Vec::new(),
            released: false,
        }
    }

    fn as_raw_ptr(&mut self) -> *mut IProgramListData {
        &mut self.iface
    }
}

#[repr(C)]
struct FakeUnitData {
    iface: IUnitData,
    last_set: Vec<u8>,
    released: bool,
}

impl FakeUnitData {
    fn new() -> Self {
        Self {
            iface: IUnitData {
                vtable: &UNIT_DATA_VTABLE,
            },
            last_set: Vec::new(),
            released: false,
        }
    }

    fn as_raw_ptr(&mut self) -> *mut IUnitData {
        &mut self.iface
    }
}

static PROGRAM_LIST_DATA_VTABLE: IProgramListDataVTable = IProgramListDataVTable {
    query_interface: fake_program_query_interface,
    add_ref: fake_program_add_ref,
    release: fake_program_release,
    program_data_supported: fake_program_data_supported,
    get_program_data: fake_get_program_data,
    set_program_data: fake_set_program_data,
};

static UNIT_DATA_VTABLE: IUnitDataVTable = IUnitDataVTable {
    query_interface: fake_unit_query_interface,
    add_ref: fake_unit_add_ref,
    release: fake_unit_release,
    unit_data_supported: fake_unit_data_supported,
    get_unit_data: fake_get_unit_data,
    set_unit_data: fake_set_unit_data,
};

unsafe extern "system" fn fake_program_query_interface(
    _this: *mut IProgramListData,
    _iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        unsafe { *obj = ptr::null_mut() };
    }
    K_RESULT_FALSE
}

unsafe extern "system" fn fake_program_add_ref(_this: *mut IProgramListData) -> u32 {
    1
}

unsafe extern "system" fn fake_program_release(this: *mut IProgramListData) -> u32 {
    unsafe { (*this.cast::<FakeProgramListData>()).released = true };
    1
}

unsafe extern "system" fn fake_program_data_supported(
    _this: *mut IProgramListData,
    list_id: i32,
) -> i32 {
    if list_id == 7 {
        K_RESULT_OK
    } else {
        K_RESULT_FALSE
    }
}

unsafe extern "system" fn fake_get_program_data(
    _this: *mut IProgramListData,
    list_id: i32,
    program_index: i32,
    data: *mut IBStream,
) -> i32 {
    if list_id != 7 || program_index != 1 {
        return K_RESULT_FALSE;
    }
    write_stream(data, &[1, 2, 3])
}

unsafe extern "system" fn fake_set_program_data(
    this: *mut IProgramListData,
    list_id: i32,
    program_index: i32,
    data: *mut IBStream,
) -> i32 {
    if list_id != 7 || program_index != 1 {
        return K_RESULT_FALSE;
    }
    unsafe { (*this.cast::<FakeProgramListData>()).last_set = read_stream(data) };
    K_RESULT_OK
}

unsafe extern "system" fn fake_unit_query_interface(
    _this: *mut IUnitData,
    _iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        unsafe { *obj = ptr::null_mut() };
    }
    K_RESULT_FALSE
}

unsafe extern "system" fn fake_unit_add_ref(_this: *mut IUnitData) -> u32 {
    1
}

unsafe extern "system" fn fake_unit_release(this: *mut IUnitData) -> u32 {
    unsafe { (*this.cast::<FakeUnitData>()).released = true };
    1
}

unsafe extern "system" fn fake_unit_data_supported(_this: *mut IUnitData, unit_id: i32) -> i32 {
    if unit_id == 2 {
        K_RESULT_OK
    } else {
        K_RESULT_FALSE
    }
}

unsafe extern "system" fn fake_get_unit_data(
    _this: *mut IUnitData,
    unit_id: i32,
    data: *mut IBStream,
) -> i32 {
    if unit_id != 2 {
        return K_RESULT_FALSE;
    }
    write_stream(data, &[9, 8])
}

unsafe extern "system" fn fake_set_unit_data(
    this: *mut IUnitData,
    unit_id: i32,
    data: *mut IBStream,
) -> i32 {
    if unit_id != 2 {
        return K_RESULT_FALSE;
    }
    unsafe { (*this.cast::<FakeUnitData>()).last_set = read_stream(data) };
    K_RESULT_OK
}

fn write_stream(stream: *mut IBStream, bytes: &[u8]) -> i32 {
    if stream.is_null() {
        return K_RESULT_FALSE;
    }
    let mut written = 0;
    unsafe {
        ((*(*stream).vtable).write)(
            stream,
            bytes.as_ptr().cast_mut().cast(),
            i32::try_from(bytes.len()).expect("test bytes fit i32"),
            &mut written,
        )
    }
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
