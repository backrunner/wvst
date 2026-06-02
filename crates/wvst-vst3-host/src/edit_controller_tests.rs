use std::ffi::c_void;
use std::ptr;

use crate::vst3_abi::{
    FUnknown, IBStream, IComponentHandler, IEditController, IEditControllerVTable, K_RESULT_OK,
    ParameterInfo, VST3_PARAMETER_CAN_AUTOMATE,
};

use super::*;

#[test]
fn initializes_reads_parameters_and_releases_controller() {
    let mut fake = FakeEditController::new();
    let mut controller =
        unsafe { Vst3EditController::from_raw(fake.raw_controller()) }.expect("controller");

    controller.initialize().expect("initialize");
    let parameters = controller.parameters().expect("parameters");
    controller
        .set_param_normalized(42, 0.75)
        .expect("set parameter");
    controller.begin_edit(42).expect("begin edit");
    controller.perform_edit(42, 0.8).expect("perform edit");
    controller.end_edit(42).expect("end edit");
    let handler_snapshot = controller.component_handler_snapshot();
    let value_string = controller
        .param_string_by_value(42, 0.25)
        .expect("param string")
        .expect("display value");
    let parsed_value = controller
        .param_value_by_string(42, "50")
        .expect("param value");
    let plain_value = controller.normalized_param_to_plain(42, 0.25);
    let normalized_value = controller.plain_param_to_normalized(42, 25.0);
    let state = controller.get_state().expect("state");
    drop(controller);

    assert_eq!(fake.initialize_calls, 1);
    assert!(fake.handler_set);
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].id, 42);
    assert_eq!(parameters[0].title.as_deref(), Some("Gain"));
    assert!(parameters[0].flags.can_automate);
    assert_eq!(fake.value, 0.8);
    assert_eq!(handler_snapshot.total_events, 3);
    assert_eq!(
        handler_snapshot.recent_events[0].kind,
        crate::Vst3ComponentHandlerEventKind::BeginEdit
    );
    assert_eq!(
        handler_snapshot.recent_events[1].kind,
        crate::Vst3ComponentHandlerEventKind::PerformEdit
    );
    assert_eq!(
        handler_snapshot.recent_events[1].value_normalized,
        Some(0.8)
    );
    assert_eq!(
        handler_snapshot.recent_events[2].kind,
        crate::Vst3ComponentHandlerEventKind::EndEdit
    );
    assert_eq!(value_string, "0.5");
    assert_eq!(parsed_value, 0.5);
    assert_eq!(plain_value, 25.0);
    assert_eq!(normalized_value, 0.25);
    assert_eq!(state, [1, 2, 3]);
    assert_eq!(fake.terminate_calls, 1);
    assert_eq!(fake.release_calls, 1);
}

#[test]
fn enforces_parameter_edit_gesture_order() {
    let mut fake = FakeEditController::new();
    let mut controller =
        unsafe { Vst3EditController::from_raw(fake.raw_controller()) }.expect("controller");
    controller.initialize().expect("initialize");

    let perform_without_begin = controller
        .perform_edit(42, 0.7)
        .expect_err("perform without begin");
    let end_without_begin = controller.end_edit(42).expect_err("end without begin");
    controller.begin_edit(42).expect("begin edit");
    let duplicate_begin = controller.begin_edit(42).expect_err("duplicate begin");
    controller.perform_edit(42, 0.8).expect("perform edit");
    controller.end_edit(42).expect("end edit");
    let duplicate_end = controller.end_edit(42).expect_err("duplicate end");
    let handler_snapshot = controller.component_handler_snapshot();

    assert!(matches!(
        perform_without_begin,
        HostError::EditControllerParameterEditNotActive { parameter_id: 42 }
    ));
    assert!(matches!(
        end_without_begin,
        HostError::EditControllerParameterEditNotActive { parameter_id: 42 }
    ));
    assert!(matches!(
        duplicate_begin,
        HostError::EditControllerParameterEditAlreadyActive { parameter_id: 42 }
    ));
    assert!(matches!(
        duplicate_end,
        HostError::EditControllerParameterEditNotActive { parameter_id: 42 }
    ));
    assert_eq!(handler_snapshot.total_events, 3);
    assert_eq!(
        handler_snapshot.recent_events[0].kind,
        crate::Vst3ComponentHandlerEventKind::BeginEdit
    );
    assert_eq!(
        handler_snapshot.recent_events[1].kind,
        crate::Vst3ComponentHandlerEventKind::PerformEdit
    );
    assert_eq!(
        handler_snapshot.recent_events[2].kind,
        crate::Vst3ComponentHandlerEventKind::EndEdit
    );
}

#[repr(C)]
struct FakeEditController {
    controller: IEditController,
    initialize_calls: u32,
    terminate_calls: u32,
    release_calls: u32,
    handler_set: bool,
    value: f64,
}

impl FakeEditController {
    fn new() -> Self {
        Self {
            controller: IEditController {
                vtable: &FAKE_EDIT_CONTROLLER_VTABLE,
            },
            initialize_calls: 0,
            terminate_calls: 0,
            release_calls: 0,
            handler_set: false,
            value: 0.5,
        }
    }

    fn raw_controller(&mut self) -> *mut c_void {
        (&mut self.controller as *mut IEditController).cast()
    }
}

static FAKE_EDIT_CONTROLLER_VTABLE: IEditControllerVTable = IEditControllerVTable {
    query_interface: fake_query_interface,
    add_ref: fake_add_ref,
    release: fake_release,
    initialize: fake_initialize,
    terminate: fake_terminate,
    set_component_state: fake_set_component_state,
    set_state: fake_set_state,
    get_state: fake_get_state,
    get_parameter_count: fake_get_parameter_count,
    get_parameter_info: fake_get_parameter_info,
    get_param_string_by_value: fake_get_param_string_by_value,
    get_param_value_by_string: fake_get_param_value_by_string,
    normalized_param_to_plain: fake_normalized_param_to_plain,
    plain_param_to_normalized: fake_plain_param_to_normalized,
    get_param_normalized: fake_get_param_normalized,
    set_param_normalized: fake_set_param_normalized,
    set_component_handler: fake_set_component_handler,
    create_view: fake_create_view,
};

unsafe extern "system" fn fake_query_interface(
    _this: *mut IEditController,
    _iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        unsafe { *obj = ptr::null_mut() };
    }
    -1
}

unsafe extern "system" fn fake_add_ref(_this: *mut IEditController) -> u32 {
    1
}

unsafe extern "system" fn fake_release(this: *mut IEditController) -> u32 {
    let fake = unsafe { fake_controller_mut(this) };
    fake.release_calls += 1;
    fake.release_calls
}

unsafe extern "system" fn fake_initialize(
    this: *mut IEditController,
    _context: *mut FUnknown,
) -> i32 {
    let fake = unsafe { fake_controller_mut(this) };
    fake.initialize_calls += 1;
    K_RESULT_OK
}

unsafe extern "system" fn fake_terminate(this: *mut IEditController) -> i32 {
    let fake = unsafe { fake_controller_mut(this) };
    fake.terminate_calls += 1;
    K_RESULT_OK
}

unsafe extern "system" fn fake_set_component_state(
    _this: *mut IEditController,
    _state: *mut IBStream,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_set_state(
    _this: *mut IEditController,
    _state: *mut IBStream,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_state(_this: *mut IEditController, state: *mut IBStream) -> i32 {
    let bytes = [1_u8, 2, 3];
    let mut written = 0;
    unsafe {
        ((*(*state).vtable).write)(
            state,
            bytes.as_ptr().cast_mut().cast(),
            bytes.len() as i32,
            &mut written,
        )
    }
}

unsafe extern "system" fn fake_get_parameter_count(_this: *mut IEditController) -> i32 {
    1
}

unsafe extern "system" fn fake_get_parameter_info(
    _this: *mut IEditController,
    index: i32,
    info: *mut ParameterInfo,
) -> i32 {
    if index != 0 || info.is_null() {
        return -1;
    }

    let mut parameter = ParameterInfo {
        id: 42,
        step_count: 0,
        default_normalized_value: 0.5,
        unit_id: 0,
        flags: VST3_PARAMETER_CAN_AUTOMATE,
        ..ParameterInfo::default()
    };
    for (index, unit) in "Gain".encode_utf16().enumerate() {
        parameter.title[index] = unit;
        parameter.short_title[index] = unit;
    }
    for (index, unit) in "dB".encode_utf16().enumerate() {
        parameter.units[index] = unit;
    }

    unsafe { *info = parameter };
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_param_string_by_value(
    _this: *mut IEditController,
    _id: u32,
    _value_normalized: f64,
    string: *mut [u16; 128],
) -> i32 {
    if !string.is_null() {
        for (index, unit) in "0.5".encode_utf16().enumerate() {
            unsafe { (*string)[index] = unit };
        }
    }
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_param_value_by_string(
    _this: *mut IEditController,
    _id: u32,
    _string: *mut u16,
    value_normalized: *mut f64,
) -> i32 {
    if !value_normalized.is_null() {
        unsafe { *value_normalized = 0.5 };
    }
    K_RESULT_OK
}

unsafe extern "system" fn fake_normalized_param_to_plain(
    _this: *mut IEditController,
    _id: u32,
    value_normalized: f64,
) -> f64 {
    value_normalized * 100.0
}

unsafe extern "system" fn fake_plain_param_to_normalized(
    _this: *mut IEditController,
    _id: u32,
    plain_value: f64,
) -> f64 {
    plain_value / 100.0
}

unsafe extern "system" fn fake_get_param_normalized(this: *mut IEditController, _id: u32) -> f64 {
    unsafe { fake_controller_mut(this) }.value
}

unsafe extern "system" fn fake_set_param_normalized(
    this: *mut IEditController,
    _id: u32,
    value: f64,
) -> i32 {
    unsafe { fake_controller_mut(this) }.value = value;
    K_RESULT_OK
}

unsafe extern "system" fn fake_set_component_handler(
    this: *mut IEditController,
    handler: *mut IComponentHandler,
) -> i32 {
    unsafe { fake_controller_mut(this) }.handler_set = !handler.is_null();
    K_RESULT_OK
}

unsafe extern "system" fn fake_create_view(
    _this: *mut IEditController,
    _name: *const i8,
) -> *mut c_void {
    ptr::null_mut()
}

unsafe fn fake_controller_mut<'a>(this: *mut IEditController) -> &'a mut FakeEditController {
    unsafe { &mut *this.cast::<FakeEditController>() }
}
