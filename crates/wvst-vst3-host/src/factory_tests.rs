use std::ffi::c_void;
use std::ptr;

use super::*;
use crate::HostError;
use crate::vst3_abi::{
    FUnknown, IAudioProcessor, IAudioProcessorVTable, IComponent, IComponentHandler,
    IComponentVTable, IConnectionPoint, IConnectionPointVTable, IEditController,
    IEditControllerVTable, IMessage, K_RESULT_FALSE, K_RESULT_OK, ParameterInfo, ProcessData,
    ProcessSetup, SpeakerArrangement, TUid, VST3_FUNKNOWN_IID, VST3_I_CONNECTION_POINT_IID,
    VST3_SAMPLE_32, parse_tuid_hex,
};

#[test]
fn loaded_component_can_move_between_worker_threads() {
    fn assert_send<T: Send>() {}

    assert_send::<Vst3LoadedComponent>();
}

#[test]
fn rejects_invalid_component_class_id_before_loading_bundle() {
    let error =
        create_vst3_component_probe("/tmp/Missing.vst3", "class-a").expect_err("invalid class id");

    assert!(matches!(error, HostError::InvalidClassId(class_id) if class_id == "class-a"));
}

#[test]
fn rejects_invalid_runtime_class_id_before_loading_bundle() {
    let config = crate::Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let error = match create_vst3_component_instance("/tmp/Missing.vst3", "class-a", config) {
        Ok(_) => panic!("expected invalid class id"),
        Err(error) => error,
    };

    assert!(matches!(error, HostError::InvalidClassId(class_id) if class_id == "class-a"));
}

#[test]
fn loaded_component_connects_and_disconnects_controller_connection_points() {
    let mut component_point = FakeConnectionPoint::new();
    let mut controller_point = FakeConnectionPoint::new();
    let mut component = FakeComponent::new(component_point.raw_connection_point());
    let mut processor = FakeProcessor::new();
    let mut controller = FakeEditController::new(controller_point.raw_connection_point());
    let config = crate::Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let instance = unsafe {
        Vst3ComponentInstance::from_raw_parts(
            component.raw_component(),
            processor.raw_processor(),
            config,
        )
    }
    .expect("component instance");
    let controller =
        unsafe { Vst3EditController::from_raw(controller.raw_controller()) }.expect("controller");
    let mut loaded = Vst3LoadedComponent::new_for_test(instance, Some(controller));

    loaded
        .instance_mut()
        .initialize()
        .expect("component initialize");
    loaded
        .initialize_controller()
        .expect("controller initialize");

    assert_eq!(component_point.add_ref_calls, 1);
    assert_eq!(controller_point.add_ref_calls, 1);
    assert_eq!(component_point.connect_calls, 1);
    assert_eq!(controller_point.connect_calls, 1);
    assert_eq!(
        component_point.last_other,
        controller_point.raw_connection_point().cast()
    );
    assert_eq!(
        controller_point.last_other,
        component_point.raw_connection_point().cast()
    );

    loaded.terminate_controller().expect("controller terminate");

    assert_eq!(component_point.disconnect_calls, 1);
    assert_eq!(controller_point.disconnect_calls, 1);
    assert_eq!(component_point.release_calls, 1);
    assert_eq!(controller_point.release_calls, 1);
}

#[test]
fn loaded_component_notifies_component_and_controller_connection_points() {
    let mut component_point = FakeConnectionPoint::new();
    let mut controller_point = FakeConnectionPoint::new();
    let mut component = FakeComponent::new(component_point.raw_connection_point());
    let mut processor = FakeProcessor::new();
    let mut controller = FakeEditController::new(controller_point.raw_connection_point());
    let config = crate::Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let instance = unsafe {
        Vst3ComponentInstance::from_raw_parts(
            component.raw_component(),
            processor.raw_processor(),
            config,
        )
    }
    .expect("component instance");
    let controller =
        unsafe { Vst3EditController::from_raw(controller.raw_controller()) }.expect("controller");
    let mut loaded = Vst3LoadedComponent::new_for_test(instance, Some(controller));
    loaded
        .instance_mut()
        .initialize()
        .expect("component initialize");
    loaded
        .initialize_controller()
        .expect("controller initialize");

    let mut component_message = Vst3HostMessage::new();
    let component_message_ptr = component_message.as_mut_ptr();
    let mut controller_message = Vst3HostMessage::new();
    let controller_message_ptr = controller_message.as_mut_ptr();

    assert_eq!(
        loaded
            .notify_component(&mut component_message)
            .expect("component notify"),
        Some(())
    );
    assert_eq!(
        loaded
            .notify_controller(&mut controller_message)
            .expect("controller notify"),
        Some(())
    );

    assert_eq!(component_point.notify_calls, 1);
    assert_eq!(component_point.last_message, component_message_ptr);
    assert_eq!(controller_point.notify_calls, 1);
    assert_eq!(controller_point.last_message, controller_message_ptr);
}

#[test]
fn loaded_component_skips_notify_without_connection_points() {
    let mut component = FakeComponent::new(ptr::null_mut());
    let mut processor = FakeProcessor::new();
    let config = crate::Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let instance = unsafe {
        Vst3ComponentInstance::from_raw_parts(
            component.raw_component(),
            processor.raw_processor(),
            config,
        )
    }
    .expect("component instance");
    let loaded = Vst3LoadedComponent::new_for_test(instance, None);
    let mut message = Vst3HostMessage::new();

    assert_eq!(loaded.notify_component(&mut message).expect("notify"), None);
    assert_eq!(
        loaded.notify_controller(&mut message).expect("notify"),
        None
    );
}

#[repr(C)]
struct FakeComponent {
    component: IComponent,
    connection_point: *mut IConnectionPoint,
    initialize_calls: u32,
    release_calls: u32,
}

impl FakeComponent {
    fn new(connection_point: *mut IConnectionPoint) -> Self {
        Self {
            component: IComponent {
                vtable: &FAKE_COMPONENT_VTABLE,
            },
            connection_point,
            initialize_calls: 0,
            release_calls: 0,
        }
    }

    fn raw_component(&mut self) -> *mut c_void {
        (&mut self.component as *mut IComponent).cast()
    }
}

#[repr(C)]
struct FakeProcessor {
    processor: IAudioProcessor,
    release_calls: u32,
}

impl FakeProcessor {
    fn new() -> Self {
        Self {
            processor: IAudioProcessor {
                vtable: &FAKE_PROCESSOR_VTABLE,
            },
            release_calls: 0,
        }
    }

    fn raw_processor(&mut self) -> *mut c_void {
        (&mut self.processor as *mut IAudioProcessor).cast()
    }
}

#[repr(C)]
struct FakeEditController {
    controller: IEditController,
    connection_point: *mut IConnectionPoint,
    initialize_calls: u32,
    terminate_calls: u32,
    release_calls: u32,
}

impl FakeEditController {
    fn new(connection_point: *mut IConnectionPoint) -> Self {
        Self {
            controller: IEditController {
                vtable: &FAKE_CONTROLLER_VTABLE,
            },
            connection_point,
            initialize_calls: 0,
            terminate_calls: 0,
            release_calls: 0,
        }
    }

    fn raw_controller(&mut self) -> *mut c_void {
        (&mut self.controller as *mut IEditController).cast()
    }
}

#[repr(C)]
struct FakeConnectionPoint {
    point: IConnectionPoint,
    add_ref_calls: u32,
    release_calls: u32,
    connect_calls: u32,
    disconnect_calls: u32,
    notify_calls: u32,
    last_other: *mut IConnectionPoint,
    last_message: *mut IMessage,
}

impl FakeConnectionPoint {
    fn new() -> Self {
        Self {
            point: IConnectionPoint {
                vtable: &FAKE_CONNECTION_POINT_VTABLE,
            },
            add_ref_calls: 0,
            release_calls: 0,
            connect_calls: 0,
            disconnect_calls: 0,
            notify_calls: 0,
            last_other: ptr::null_mut(),
            last_message: ptr::null_mut(),
        }
    }

    fn raw_connection_point(&mut self) -> *mut IConnectionPoint {
        &mut self.point
    }
}

static FAKE_COMPONENT_VTABLE: IComponentVTable = IComponentVTable {
    query_interface: fake_component_query_interface,
    add_ref: fake_component_add_ref,
    release: fake_component_release,
    initialize: fake_component_initialize,
    terminate: fake_component_terminate,
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

static FAKE_PROCESSOR_VTABLE: IAudioProcessorVTable = IAudioProcessorVTable {
    query_interface: fake_processor_query_interface,
    add_ref: fake_processor_add_ref,
    release: fake_processor_release,
    set_bus_arrangements: fake_set_bus_arrangements,
    get_bus_arrangement: fake_get_bus_arrangement,
    can_process_sample_size: fake_can_process_sample_size,
    get_latency_samples: fake_get_latency_samples,
    setup_processing: fake_setup_processing,
    set_processing: fake_set_processing,
    process: fake_process,
    get_tail_samples: fake_get_tail_samples,
};

static FAKE_CONTROLLER_VTABLE: IEditControllerVTable = IEditControllerVTable {
    query_interface: fake_controller_query_interface,
    add_ref: fake_controller_add_ref,
    release: fake_controller_release,
    initialize: fake_controller_initialize,
    terminate: fake_controller_terminate,
    set_component_state: fake_controller_set_component_state,
    set_state: fake_controller_set_state,
    get_state: fake_controller_get_state,
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

static FAKE_CONNECTION_POINT_VTABLE: IConnectionPointVTable = IConnectionPointVTable {
    query_interface: fake_connection_query_interface,
    add_ref: fake_connection_add_ref,
    release: fake_connection_release,
    connect: fake_connection_connect,
    disconnect: fake_connection_disconnect,
    notify: fake_connection_notify,
};

unsafe extern "system" fn fake_component_query_interface(
    this: *mut IComponent,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return 1;
    }
    unsafe { *obj = ptr::null_mut() };
    let fake = unsafe { fake_component_mut(this) };
    if unsafe { tuid_from_raw(iid) } == tuid(VST3_I_CONNECTION_POINT_IID) {
        if fake.connection_point.is_null() {
            return K_RESULT_FALSE;
        }
        unsafe { *obj = fake.connection_point.cast() };
        let _ = unsafe { fake_connection_add_ref(fake.connection_point) };
        K_RESULT_OK
    } else {
        1
    }
}

unsafe extern "system" fn fake_component_add_ref(_this: *mut IComponent) -> u32 {
    1
}

unsafe extern "system" fn fake_component_release(this: *mut IComponent) -> u32 {
    let fake = unsafe { fake_component_mut(this) };
    fake.release_calls += 1;
    fake.release_calls
}

unsafe extern "system" fn fake_component_initialize(
    this: *mut IComponent,
    _context: *mut FUnknown,
) -> i32 {
    let fake = unsafe { fake_component_mut(this) };
    fake.initialize_calls += 1;
    K_RESULT_OK
}

unsafe extern "system" fn fake_component_terminate(_this: *mut IComponent) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_controller_class_id(
    _this: *mut IComponent,
    class_id: *mut TUid,
) -> i32 {
    if !class_id.is_null() {
        unsafe { *class_id = [0; 16] };
    }
    K_RESULT_OK
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
    1
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

unsafe extern "system" fn fake_processor_query_interface(
    _this: *mut IAudioProcessor,
    _iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        unsafe { *obj = ptr::null_mut() };
    }
    K_RESULT_FALSE
}

unsafe extern "system" fn fake_processor_add_ref(_this: *mut IAudioProcessor) -> u32 {
    1
}

unsafe extern "system" fn fake_processor_release(this: *mut IAudioProcessor) -> u32 {
    let fake = unsafe { fake_processor_mut(this) };
    fake.release_calls += 1;
    fake.release_calls
}

unsafe extern "system" fn fake_set_bus_arrangements(
    _this: *mut IAudioProcessor,
    _inputs: *mut SpeakerArrangement,
    _input_count: i32,
    _outputs: *mut SpeakerArrangement,
    _output_count: i32,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_bus_arrangement(
    _this: *mut IAudioProcessor,
    _direction: i32,
    _index: i32,
    _arrangement: *mut SpeakerArrangement,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_can_process_sample_size(
    _this: *mut IAudioProcessor,
    sample_size: i32,
) -> i32 {
    if sample_size == VST3_SAMPLE_32 {
        K_RESULT_OK
    } else {
        1
    }
}

unsafe extern "system" fn fake_get_latency_samples(_this: *mut IAudioProcessor) -> u32 {
    0
}

unsafe extern "system" fn fake_setup_processing(
    _this: *mut IAudioProcessor,
    _setup: *mut ProcessSetup,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_set_processing(_this: *mut IAudioProcessor, _state: u8) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_process(
    _this: *mut IAudioProcessor,
    _data: *mut ProcessData,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_tail_samples(_this: *mut IAudioProcessor) -> u32 {
    0
}

unsafe extern "system" fn fake_controller_query_interface(
    this: *mut IEditController,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return 1;
    }
    unsafe { *obj = ptr::null_mut() };
    let fake = unsafe { fake_controller_mut(this) };
    if unsafe { tuid_from_raw(iid) } == tuid(VST3_I_CONNECTION_POINT_IID) {
        unsafe { *obj = fake.connection_point.cast() };
        let _ = unsafe { fake_connection_add_ref(fake.connection_point) };
        K_RESULT_OK
    } else {
        1
    }
}

unsafe extern "system" fn fake_controller_add_ref(_this: *mut IEditController) -> u32 {
    1
}

unsafe extern "system" fn fake_controller_release(this: *mut IEditController) -> u32 {
    let fake = unsafe { fake_controller_mut(this) };
    fake.release_calls += 1;
    fake.release_calls
}

unsafe extern "system" fn fake_controller_initialize(
    this: *mut IEditController,
    _context: *mut FUnknown,
) -> i32 {
    let fake = unsafe { fake_controller_mut(this) };
    fake.initialize_calls += 1;
    K_RESULT_OK
}

unsafe extern "system" fn fake_controller_terminate(this: *mut IEditController) -> i32 {
    let fake = unsafe { fake_controller_mut(this) };
    fake.terminate_calls += 1;
    K_RESULT_OK
}

unsafe extern "system" fn fake_controller_set_component_state(
    _this: *mut IEditController,
    _state: *mut crate::vst3_abi::IBStream,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_controller_set_state(
    _this: *mut IEditController,
    _state: *mut crate::vst3_abi::IBStream,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_controller_get_state(
    _this: *mut IEditController,
    _state: *mut crate::vst3_abi::IBStream,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_parameter_count(_this: *mut IEditController) -> i32 {
    0
}

unsafe extern "system" fn fake_get_parameter_info(
    _this: *mut IEditController,
    _param_index: i32,
    _info: *mut ParameterInfo,
) -> i32 {
    1
}

unsafe extern "system" fn fake_get_param_string_by_value(
    _this: *mut IEditController,
    _id: u32,
    _value_normalized: f64,
    _string: *mut [u16; 128],
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_param_value_by_string(
    _this: *mut IEditController,
    _id: u32,
    _string: *mut u16,
    value_normalized: *mut f64,
) -> i32 {
    if !value_normalized.is_null() {
        unsafe { *value_normalized = 0.0 };
    }
    K_RESULT_OK
}

unsafe extern "system" fn fake_normalized_param_to_plain(
    _this: *mut IEditController,
    _id: u32,
    value_normalized: f64,
) -> f64 {
    value_normalized
}

unsafe extern "system" fn fake_plain_param_to_normalized(
    _this: *mut IEditController,
    _id: u32,
    plain_value: f64,
) -> f64 {
    plain_value
}

unsafe extern "system" fn fake_get_param_normalized(_this: *mut IEditController, _id: u32) -> f64 {
    0.0
}

unsafe extern "system" fn fake_set_param_normalized(
    _this: *mut IEditController,
    _id: u32,
    _value: f64,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_set_component_handler(
    _this: *mut IEditController,
    handler: *mut IComponentHandler,
) -> i32 {
    if handler.is_null() { 1 } else { K_RESULT_OK }
}

unsafe extern "system" fn fake_create_view(
    _this: *mut IEditController,
    _name: *const i8,
) -> *mut c_void {
    ptr::null_mut()
}

unsafe extern "system" fn fake_connection_query_interface(
    this: *mut IConnectionPoint,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return 1;
    }
    unsafe { *obj = ptr::null_mut() };
    let iid = unsafe { tuid_from_raw(iid) };
    if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_I_CONNECTION_POINT_IID) {
        unsafe { *obj = this.cast() };
        let _ = unsafe { fake_connection_add_ref(this) };
        K_RESULT_OK
    } else {
        1
    }
}

unsafe extern "system" fn fake_connection_add_ref(this: *mut IConnectionPoint) -> u32 {
    let fake = unsafe { fake_connection_mut(this) };
    fake.add_ref_calls += 1;
    fake.add_ref_calls
}

unsafe extern "system" fn fake_connection_release(this: *mut IConnectionPoint) -> u32 {
    let fake = unsafe { fake_connection_mut(this) };
    fake.release_calls += 1;
    fake.release_calls
}

unsafe extern "system" fn fake_connection_connect(
    this: *mut IConnectionPoint,
    other: *mut IConnectionPoint,
) -> i32 {
    let fake = unsafe { fake_connection_mut(this) };
    fake.connect_calls += 1;
    fake.last_other = other;
    K_RESULT_OK
}

unsafe extern "system" fn fake_connection_disconnect(
    this: *mut IConnectionPoint,
    other: *mut IConnectionPoint,
) -> i32 {
    let fake = unsafe { fake_connection_mut(this) };
    fake.disconnect_calls += 1;
    fake.last_other = other;
    K_RESULT_OK
}

unsafe extern "system" fn fake_connection_notify(
    this: *mut IConnectionPoint,
    message: *mut IMessage,
) -> i32 {
    let fake = unsafe { fake_connection_mut(this) };
    fake.notify_calls += 1;
    fake.last_message = message;
    K_RESULT_OK
}

unsafe fn fake_component_mut<'a>(this: *mut IComponent) -> &'a mut FakeComponent {
    unsafe { &mut *this.cast::<FakeComponent>() }
}

unsafe fn fake_processor_mut<'a>(this: *mut IAudioProcessor) -> &'a mut FakeProcessor {
    unsafe { &mut *this.cast::<FakeProcessor>() }
}

unsafe fn fake_controller_mut<'a>(this: *mut IEditController) -> &'a mut FakeEditController {
    unsafe { &mut *this.cast::<FakeEditController>() }
}

unsafe fn fake_connection_mut<'a>(this: *mut IConnectionPoint) -> &'a mut FakeConnectionPoint {
    unsafe { &mut *this.cast::<FakeConnectionPoint>() }
}

fn tuid(value: &str) -> TUid {
    parse_tuid_hex(value).expect("valid iid")
}

unsafe fn tuid_from_raw(raw: *const i8) -> TUid {
    let mut output = [0; 16];
    if !raw.is_null() {
        unsafe { output.copy_from_slice(std::slice::from_raw_parts(raw.cast::<u8>(), 16)) };
    }
    output
}
