use std::ffi::c_void;
use std::ptr;
use std::slice;

use crate::vst3_abi::{
    AudioBusBuffers, BusInfo, FUnknown, IAudioProcessor, IAudioProcessorVTable, IComponent,
    IComponentVTable, K_RESULT_OK, ProcessData, ProcessSetup, SpeakerArrangement, TUid,
    VST3_BUS_DIRECTION_INPUT, VST3_BUS_DIRECTION_OUTPUT, VST3_BUS_FLAG_DEFAULT_ACTIVE,
    VST3_BUS_TYPE_MAIN, VST3_MEDIA_TYPE_AUDIO, VST3_SAMPLE_32, VST3_SPEAKER_STEREO,
};

use super::*;

#[test]
fn drives_component_lifecycle_and_audio_process_path() {
    let mut component = FakeComponent::new();
    let mut processor = FakeProcessor::new();
    processor.latency_samples = 32;
    processor.tail_samples = 64;
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let mut instance = unsafe {
        Vst3ComponentInstance::from_raw_parts(
            component.raw_component(),
            processor.raw_processor(),
            config,
        )
    }
    .expect("instance");

    assert_eq!(instance.state(), Vst3LifecycleState::Created);
    instance.initialize().expect("initialize");
    assert_eq!(instance.state(), Vst3LifecycleState::Initialized);
    assert_eq!(component.initialize_calls, 1);

    instance.setup_processing().expect("setup");
    assert_eq!(instance.state(), Vst3LifecycleState::SetupDone);
    assert_eq!(processor.set_bus_arrangement_calls, 1);
    assert_eq!(processor.last_input_arrangement, Some(VST3_SPEAKER_STEREO));
    assert_eq!(processor.last_output_arrangement, Some(VST3_SPEAKER_STEREO));
    assert_eq!(processor.setup.expect("setup").max_samples_per_block, 128);
    assert!(
        !instance
            .audio_buses(Vst3BusDirection::Output)
            .expect("output buses")
            .is_empty()
    );

    instance.activate().expect("activate");
    assert_eq!(instance.state(), Vst3LifecycleState::Activated);
    assert!(component.active);
    assert_eq!(component.activate_bus_calls, 2);
    assert!(component.input_bus_active);
    assert!(component.output_bus_active);

    instance.start_processing().expect("start");
    assert_eq!(instance.state(), Vst3LifecycleState::Processing);
    assert!(processor.processing);
    assert_eq!(instance.latency_samples(), 32);
    assert_eq!(instance.tail_samples(), 64);

    let mut output = [0.0; 4];
    instance
        .process_interleaved_f32(2, &[1.0, 2.0, 3.0, 4.0], &mut output)
        .expect("process");
    assert_eq!(output, [2.0, 4.0, 6.0, 8.0]);
    assert_eq!(processor.process_calls, 1);

    instance.stop_processing().expect("stop");
    assert_eq!(instance.state(), Vst3LifecycleState::Stopped);
    assert!(!processor.processing);

    instance.terminate().expect("terminate");
    assert_eq!(instance.state(), Vst3LifecycleState::Terminated);
    assert!(!component.active);
    assert_eq!(component.activate_bus_calls, 4);
    assert!(!component.input_bus_active);
    assert!(!component.output_bus_active);
    assert_eq!(component.terminate_calls, 1);

    drop(instance);
    assert_eq!(component.release_calls, 1);
    assert_eq!(processor.release_calls, 1);
}

#[test]
fn rejects_process_before_start_without_calling_processor() {
    let mut component = FakeComponent::new();
    let mut processor = FakeProcessor::new();
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let mut instance = unsafe {
        Vst3ComponentInstance::from_raw_parts(
            component.raw_component(),
            processor.raw_processor(),
            config,
        )
    }
    .expect("instance");
    instance.initialize().expect("initialize");
    instance.setup_processing().expect("setup");
    instance.activate().expect("activate");
    let mut output = [0.0; 4];

    let error = instance
        .process_interleaved_f32(2, &[1.0, 2.0, 3.0, 4.0], &mut output)
        .expect_err("process before start");

    assert_eq!(
        error,
        HostError::InvalidLifecycleTransition {
            from: "activated",
            action: "process"
        }
    );
    assert_eq!(processor.process_calls, 0);
}

#[test]
fn propagates_component_initialize_failure() {
    let mut component = FakeComponent::new();
    component.initialize_result = -11;
    let mut processor = FakeProcessor::new();
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let mut instance = unsafe {
        Vst3ComponentInstance::from_raw_parts(
            component.raw_component(),
            processor.raw_processor(),
            config,
        )
    }
    .expect("instance");

    let error = instance.initialize().expect_err("initialize failed");

    assert_eq!(
        error,
        HostError::ComponentCallFailed {
            method: "initialize",
            result: -11
        }
    );
    assert_eq!(instance.state(), Vst3LifecycleState::Created);
}

#[test]
fn deactivates_audio_buses_when_set_active_fails() {
    let mut component = FakeComponent::new();
    component.set_active_result = -12;
    let mut processor = FakeProcessor::new();
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let mut instance = unsafe {
        Vst3ComponentInstance::from_raw_parts(
            component.raw_component(),
            processor.raw_processor(),
            config,
        )
    }
    .expect("instance");
    instance.initialize().expect("initialize");
    instance.setup_processing().expect("setup");

    let error = instance.activate().expect_err("activate failed");

    assert_eq!(
        error,
        HostError::ComponentCallFailed {
            method: "setActive",
            result: -12
        }
    );
    assert_eq!(instance.state(), Vst3LifecycleState::SetupDone);
    assert_eq!(component.activate_bus_calls, 4);
    assert!(!component.input_bus_active);
    assert!(!component.output_bus_active);
}

#[test]
fn releases_component_when_processor_constructor_fails() {
    let mut component = FakeComponent::new();
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");

    let error = unsafe {
        Vst3ComponentInstance::from_raw_parts(component.raw_component(), ptr::null_mut(), config)
    }
    .expect_err("processor failed");

    assert_eq!(error, HostError::AudioProcessorReturnedNull);
    assert_eq!(component.release_calls, 1);
}

#[repr(C)]
struct FakeComponent {
    component: IComponent,
    initialize_result: i32,
    activate_bus_result: i32,
    set_active_result: i32,
    terminate_result: i32,
    initialize_calls: u32,
    activate_bus_calls: u32,
    set_active_calls: u32,
    terminate_calls: u32,
    release_calls: u32,
    input_bus_active: bool,
    output_bus_active: bool,
    active: bool,
}

impl FakeComponent {
    fn new() -> Self {
        Self {
            component: IComponent {
                vtable: &FAKE_COMPONENT_VTABLE,
            },
            initialize_result: K_RESULT_OK,
            activate_bus_result: K_RESULT_OK,
            set_active_result: K_RESULT_OK,
            terminate_result: K_RESULT_OK,
            initialize_calls: 0,
            activate_bus_calls: 0,
            set_active_calls: 0,
            terminate_calls: 0,
            release_calls: 0,
            input_bus_active: false,
            output_bus_active: false,
            active: false,
        }
    }

    fn raw_component(&mut self) -> *mut c_void {
        (&mut self.component as *mut IComponent).cast()
    }
}

#[repr(C)]
struct FakeProcessor {
    processor: IAudioProcessor,
    can_process_result: i32,
    setup_result: i32,
    set_processing_result: i32,
    process_result: i32,
    set_bus_arrangement_calls: u32,
    last_input_arrangement: Option<SpeakerArrangement>,
    last_output_arrangement: Option<SpeakerArrangement>,
    setup: Option<ProcessSetup>,
    processing: bool,
    set_processing_calls: u32,
    process_calls: u32,
    release_calls: u32,
    latency_samples: u32,
    tail_samples: u32,
}

impl FakeProcessor {
    fn new() -> Self {
        Self {
            processor: IAudioProcessor {
                vtable: &FAKE_PROCESSOR_VTABLE,
            },
            can_process_result: K_RESULT_OK,
            setup_result: K_RESULT_OK,
            set_processing_result: K_RESULT_OK,
            process_result: K_RESULT_OK,
            set_bus_arrangement_calls: 0,
            last_input_arrangement: None,
            last_output_arrangement: None,
            setup: None,
            processing: false,
            set_processing_calls: 0,
            process_calls: 0,
            release_calls: 0,
            latency_samples: 0,
            tail_samples: 0,
        }
    }

    fn raw_processor(&mut self) -> *mut c_void {
        (&mut self.processor as *mut IAudioProcessor).cast()
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

unsafe extern "system" fn fake_component_query_interface(
    _this: *mut IComponent,
    _iid: *const i8,
    _obj: *mut *mut c_void,
) -> i32 {
    -1
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
    fake.initialize_result
}

unsafe extern "system" fn fake_component_terminate(this: *mut IComponent) -> i32 {
    let fake = unsafe { fake_component_mut(this) };
    fake.terminate_calls += 1;
    fake.terminate_result
}

unsafe extern "system" fn fake_get_controller_class_id(
    _this: *mut IComponent,
    class_id: *mut TUid,
) -> i32 {
    if !class_id.is_null() {
        unsafe {
            *class_id = [0; 16];
        }
    }
    K_RESULT_OK
}

unsafe extern "system" fn fake_set_io_mode(_this: *mut IComponent, _mode: i32) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_bus_count(
    _this: *mut IComponent,
    media_type: i32,
    direction: i32,
) -> i32 {
    let valid_direction =
        direction == VST3_BUS_DIRECTION_INPUT || direction == VST3_BUS_DIRECTION_OUTPUT;
    if media_type == VST3_MEDIA_TYPE_AUDIO && valid_direction {
        1
    } else {
        0
    }
}

unsafe extern "system" fn fake_get_bus_info(
    _this: *mut IComponent,
    media_type: i32,
    direction: i32,
    index: i32,
    bus: *mut c_void,
) -> i32 {
    if media_type != VST3_MEDIA_TYPE_AUDIO || index != 0 || bus.is_null() {
        return -1;
    }

    let mut info = BusInfo {
        media_type,
        direction,
        channel_count: 2,
        name: [0; 128],
        bus_type: VST3_BUS_TYPE_MAIN,
        flags: VST3_BUS_FLAG_DEFAULT_ACTIVE,
    };
    let name = match direction {
        VST3_BUS_DIRECTION_INPUT => "Main In",
        _ => "Main Out",
    };
    for (index, unit) in name.encode_utf16().enumerate() {
        info.name[index] = unit;
    }

    unsafe { *bus.cast::<BusInfo>() = info };
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_routing_info(
    _this: *mut IComponent,
    _input: *mut c_void,
    _output: *mut c_void,
) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_activate_bus(
    this: *mut IComponent,
    media_type: i32,
    direction: i32,
    index: i32,
    state: u8,
) -> i32 {
    let fake = unsafe { fake_component_mut(this) };
    fake.activate_bus_calls += 1;
    if media_type != VST3_MEDIA_TYPE_AUDIO || index != 0 {
        return -1;
    }

    match direction {
        VST3_BUS_DIRECTION_INPUT => fake.input_bus_active = state != 0,
        VST3_BUS_DIRECTION_OUTPUT => fake.output_bus_active = state != 0,
        _ => return -1,
    }

    fake.activate_bus_result
}

unsafe extern "system" fn fake_set_active(this: *mut IComponent, state: u8) -> i32 {
    let fake = unsafe { fake_component_mut(this) };
    fake.set_active_calls += 1;
    fake.active = state != 0;
    fake.set_active_result
}

unsafe extern "system" fn fake_set_state(_this: *mut IComponent, _state: *mut c_void) -> i32 {
    K_RESULT_OK
}

unsafe extern "system" fn fake_get_state(_this: *mut IComponent, _state: *mut c_void) -> i32 {
    K_RESULT_OK
}

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

unsafe extern "system" fn fake_processor_query_interface(
    _this: *mut IAudioProcessor,
    _iid: *const i8,
    _obj: *mut *mut c_void,
) -> i32 {
    -1
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
    this: *mut IAudioProcessor,
    inputs: *mut SpeakerArrangement,
    input_count: i32,
    outputs: *mut SpeakerArrangement,
    output_count: i32,
) -> i32 {
    let fake = unsafe { fake_processor_mut(this) };
    fake.set_bus_arrangement_calls += 1;
    fake.last_input_arrangement = if input_count == 0 || inputs.is_null() {
        None
    } else {
        Some(unsafe { *inputs })
    };
    fake.last_output_arrangement = if output_count == 0 || outputs.is_null() {
        None
    } else {
        Some(unsafe { *outputs })
    };
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
    this: *mut IAudioProcessor,
    sample_size: i32,
) -> i32 {
    let fake = unsafe { fake_processor_mut(this) };
    if sample_size == VST3_SAMPLE_32 {
        fake.can_process_result
    } else {
        -1
    }
}

unsafe extern "system" fn fake_get_latency_samples(this: *mut IAudioProcessor) -> u32 {
    unsafe { fake_processor_mut(this) }.latency_samples
}

unsafe extern "system" fn fake_setup_processing(
    this: *mut IAudioProcessor,
    setup: *mut ProcessSetup,
) -> i32 {
    let fake = unsafe { fake_processor_mut(this) };
    if !setup.is_null() {
        fake.setup = Some(unsafe { *setup });
    }
    fake.setup_result
}

unsafe extern "system" fn fake_set_processing(this: *mut IAudioProcessor, state: u8) -> i32 {
    let fake = unsafe { fake_processor_mut(this) };
    fake.set_processing_calls += 1;
    fake.processing = state != 0;
    fake.set_processing_result
}

unsafe extern "system" fn fake_process(this: *mut IAudioProcessor, data: *mut ProcessData) -> i32 {
    let fake = unsafe { fake_processor_mut(this) };
    fake.process_calls += 1;
    if fake.process_result != K_RESULT_OK {
        return fake.process_result;
    }
    if data.is_null() {
        return -1;
    }

    process_first_audio_bus(unsafe { &mut *data })
}

unsafe extern "system" fn fake_get_tail_samples(this: *mut IAudioProcessor) -> u32 {
    unsafe { fake_processor_mut(this) }.tail_samples
}

fn process_first_audio_bus(data: &mut ProcessData) -> i32 {
    if data.num_samples < 0 || data.num_inputs <= 0 || data.outputs.is_null() {
        return -1;
    }

    let frames = data.num_samples as usize;
    let input_bus = unsafe { first_bus(data.inputs, data.num_inputs) };
    let output_bus = unsafe { first_bus_mut(data.outputs, data.num_outputs) };
    let input_channels = input_bus.num_channels as usize;
    let output_channels = output_bus.num_channels as usize;
    let input_ptrs = unsafe { channel_ptrs(input_bus, input_channels) };
    let output_ptrs = unsafe { channel_ptrs_mut(output_bus, output_channels) };

    for channel in 0..output_channels {
        let output = unsafe { slice::from_raw_parts_mut(output_ptrs[channel], frames) };
        if channel < input_channels {
            let input = unsafe { slice::from_raw_parts(input_ptrs[channel], frames) };
            for frame in 0..frames {
                output[frame] = input[frame] * 2.0;
            }
        } else {
            output.fill(0.0);
        }
    }

    K_RESULT_OK
}

unsafe fn fake_component_mut<'a>(this: *mut IComponent) -> &'a mut FakeComponent {
    unsafe { &mut *(this.cast::<FakeComponent>()) }
}

unsafe fn fake_processor_mut<'a>(this: *mut IAudioProcessor) -> &'a mut FakeProcessor {
    unsafe { &mut *(this.cast::<FakeProcessor>()) }
}

unsafe fn first_bus<'a>(buses: *mut AudioBusBuffers, count: i32) -> &'a AudioBusBuffers {
    unsafe { &slice::from_raw_parts(buses, count as usize)[0] }
}

unsafe fn first_bus_mut<'a>(buses: *mut AudioBusBuffers, count: i32) -> &'a mut AudioBusBuffers {
    unsafe { &mut slice::from_raw_parts_mut(buses, count as usize)[0] }
}

unsafe fn channel_ptrs<'a>(bus: &AudioBusBuffers, channels: usize) -> &'a [*mut f32] {
    unsafe { slice::from_raw_parts(bus.channel_buffers32, channels) }
}

unsafe fn channel_ptrs_mut<'a>(bus: &mut AudioBusBuffers, channels: usize) -> &'a mut [*mut f32] {
    unsafe { slice::from_raw_parts_mut(bus.channel_buffers32, channels) }
}
