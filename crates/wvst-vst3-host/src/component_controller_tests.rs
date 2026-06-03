use std::ffi::c_void;

use crate::vst3_abi::{
    FUnknown, IComponent, IComponentVTable, K_NOT_IMPLEMENTED, K_RESULT_FALSE, K_RESULT_OK, TUid,
};

use super::*;

#[test]
fn reads_nonzero_controller_class_id() {
    let mut component = FakeComponent::new([0x2a; 16], K_RESULT_OK);
    let handle =
        unsafe { Vst3ComponentHandle::from_raw(component.raw_component()) }.expect("handle");

    assert_eq!(
        handle.controller_class_id().expect("controller").as_deref(),
        Some("2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a")
    );

    drop(handle);
    assert_eq!(component.release_calls, 1);
}

#[test]
fn treats_zero_controller_class_id_as_missing() {
    let mut component = FakeComponent::new([0; 16], K_RESULT_OK);
    let handle =
        unsafe { Vst3ComponentHandle::from_raw(component.raw_component()) }.expect("handle");

    assert_eq!(handle.controller_class_id().expect("controller"), None);
}

#[test]
fn treats_optional_controller_result_codes_as_missing() {
    for result in [K_RESULT_FALSE, K_NOT_IMPLEMENTED] {
        let mut component = FakeComponent::new([0; 16], result);
        let handle =
            unsafe { Vst3ComponentHandle::from_raw(component.raw_component()) }.expect("handle");

        assert_eq!(handle.controller_class_id().expect("controller"), None);
    }
}

#[test]
fn propagates_controller_class_id_failure() {
    let mut component = FakeComponent::new([0; 16], -17);
    let handle =
        unsafe { Vst3ComponentHandle::from_raw(component.raw_component()) }.expect("handle");

    assert_eq!(
        handle
            .controller_class_id()
            .expect_err("controller failure"),
        HostError::ComponentCallFailed {
            method: "getControllerClassId",
            result: -17
        }
    );
}

#[repr(C)]
struct FakeComponent {
    component: IComponent,
    controller_class_id: TUid,
    controller_result: i32,
    release_calls: u32,
}

impl FakeComponent {
    fn new(controller_class_id: TUid, controller_result: i32) -> Self {
        Self {
            component: IComponent {
                vtable: &FAKE_COMPONENT_VTABLE,
            },
            controller_class_id,
            controller_result,
            release_calls: 0,
        }
    }

    fn raw_component(&mut self) -> *mut c_void {
        (&mut self.component as *mut IComponent).cast()
    }
}

static FAKE_COMPONENT_VTABLE: IComponentVTable = IComponentVTable {
    query_interface: fake_query_interface,
    add_ref: fake_add_ref,
    release: fake_release,
    initialize: fake_initialize,
    terminate: fake_terminate,
    get_controller_class_id: fake_get_controller_class_id,
    set_io_mode: fake_set_io_mode,
    get_bus_count: fake_get_bus_count,
    get_bus_info: fake_get_bus_info,
    get_routing_info: fake_get_routing_info,
    activate_bus: fake_activate_bus,
    set_active: fake_set_active,
    set_state: fake_set_state,
    get_state: fake_get_state,
};

unsafe extern "system" fn fake_query_interface(
    _this: *mut IComponent,
    _iid: *const i8,
    _obj: *mut *mut c_void,
) -> i32 {
    -1
}

unsafe extern "system" fn fake_add_ref(_this: *mut IComponent) -> u32 {
    1
}

unsafe extern "system" fn fake_release(this: *mut IComponent) -> u32 {
    let fake = unsafe { fake_component_mut(this) };
    fake.release_calls += 1;
    fake.release_calls
}

unsafe extern "system" fn fake_initialize(_this: *mut IComponent, _context: *mut FUnknown) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_terminate(_this: *mut IComponent) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_controller_class_id(
    this: *mut IComponent,
    class_id: *mut TUid,
) -> i32 {
    let fake = unsafe { fake_component_mut(this) };
    if !class_id.is_null() {
        unsafe {
            *class_id = fake.controller_class_id;
        }
    }
    fake.controller_result
}

unsafe extern "system" fn fake_set_io_mode(_this: *mut IComponent, _mode: i32) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_bus_count(
    _this: *mut IComponent,
    _media_type: i32,
    _direction: i32,
) -> i32 {
    0
}

unsafe extern "system" fn fake_get_bus_info(
    _this: *mut IComponent,
    _media_type: i32,
    _direction: i32,
    _index: i32,
    _bus: *mut c_void,
) -> i32 {
    -1
}

unsafe extern "system" fn fake_get_routing_info(
    _this: *mut IComponent,
    _input: *mut c_void,
    _output: *mut c_void,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_activate_bus(
    _this: *mut IComponent,
    _media_type: i32,
    _direction: i32,
    _index: i32,
    _state: u8,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_set_active(_this: *mut IComponent, _state: u8) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_set_state(_this: *mut IComponent, _state: *mut c_void) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_state(_this: *mut IComponent, _state: *mut c_void) -> i32 {
    K_RESULT_OK
}

unsafe fn fake_component_mut<'a>(this: *mut IComponent) -> &'a mut FakeComponent {
    unsafe { &mut *(this.cast::<FakeComponent>()) }
}
