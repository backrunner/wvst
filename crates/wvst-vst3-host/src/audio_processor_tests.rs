use std::ffi::c_void;
use std::ptr;
use std::slice;

use crate::vst3_abi::{
    AudioBusBuffers, IAudioProcessor, IAudioProcessorVTable, K_RESULT_OK, ProcessData,
    ProcessSetup, SpeakerArrangement, VST3_SAMPLE_32, VST3_SPEAKER_51, VST3_SPEAKER_STEREO,
};

use super::*;

#[test]
fn sets_up_processes_and_releases_audio_processor() {
    let mut fake = FakeProcessor::new();
    fake.latency_samples = 64;
    fake.tail_samples = 128;
    let mut processor =
        unsafe { Vst3AudioProcessor::from_raw(fake.raw_processor()) }.expect("processor");
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");

    processor.setup_realtime_f32(config).expect("setup");
    assert_eq!(fake.set_bus_arrangement_calls, 1);
    assert_eq!(fake.last_input_count, 1);
    assert_eq!(fake.last_output_count, 1);
    assert_eq!(fake.last_input_arrangement, Some(VST3_SPEAKER_STEREO));
    assert_eq!(fake.last_output_arrangement, Some(VST3_SPEAKER_STEREO));

    let setup = fake.setup.expect("setup captured");
    assert_eq!(setup.max_samples_per_block, 128);
    assert_eq!(setup.sample_rate, 48_000.0);
    assert_eq!(processor.latency_samples(), 64);
    assert_eq!(processor.tail_samples(), 128);

    processor.set_processing(true).expect("start");
    assert!(processor.is_processing());

    let mut buffers = Vst3ProcessBuffers::new(128, 2, 2).expect("buffers");
    buffers
        .prepare_interleaved_f32(2, &[1.0, 2.0, 3.0, 4.0])
        .expect("prepare");
    processor.process(&mut buffers).expect("process");

    let mut output = [0.0; 4];
    buffers
        .copy_output_to_interleaved(2, &mut output)
        .expect("copy");
    assert_eq!(output, [2.0, 4.0, 6.0, 8.0]);
    assert_eq!(fake.process_calls, 1);
    assert_eq!(fake.last_num_samples, 2);

    processor.set_processing(false).expect("stop");
    assert!(!processor.is_processing());
    drop(processor);

    assert_eq!(fake.release_calls, 1);
}

#[test]
fn sets_zero_input_bus_arrangement_for_instruments() {
    let mut fake = FakeProcessor::new();
    let mut processor =
        unsafe { Vst3AudioProcessor::from_raw(fake.raw_processor()) }.expect("processor");
    let config = Vst3ProcessingConfig::new(48_000, 128, 0, 2).expect("config");

    processor.setup_realtime_f32(config).expect("setup");

    assert_eq!(fake.set_bus_arrangement_calls, 1);
    assert_eq!(fake.last_input_count, 0);
    assert_eq!(fake.last_output_count, 1);
    assert_eq!(fake.last_input_arrangement, None);
    assert_eq!(fake.last_output_arrangement, Some(VST3_SPEAKER_STEREO));
}

#[test]
fn sets_surround_bus_arrangements() {
    let mut fake = FakeProcessor::new();
    let mut processor =
        unsafe { Vst3AudioProcessor::from_raw(fake.raw_processor()) }.expect("processor");
    let config = Vst3ProcessingConfig::new(48_000, 128, 6, 6).expect("config");

    processor.setup_realtime_f32(config).expect("setup");

    assert_eq!(fake.set_bus_arrangement_calls, 1);
    assert_eq!(fake.last_input_arrangement, Some(VST3_SPEAKER_51));
    assert_eq!(fake.last_output_arrangement, Some(VST3_SPEAKER_51));
}

#[test]
fn rejects_unsupported_f32_sample_size() {
    let mut fake = FakeProcessor::new();
    fake.can_process_result = -10;
    let mut processor =
        unsafe { Vst3AudioProcessor::from_raw(fake.raw_processor()) }.expect("processor");
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");

    let error = processor
        .setup_realtime_f32(config)
        .expect_err("unsupported sample size");

    assert_eq!(
        error,
        HostError::AudioProcessorCallFailed {
            method: "canProcessSampleSize",
            result: -10
        }
    );
}

#[test]
fn rejects_failed_bus_arrangement_setup() {
    let mut fake = FakeProcessor::new();
    fake.set_bus_arrangement_result = -12;
    let mut processor =
        unsafe { Vst3AudioProcessor::from_raw(fake.raw_processor()) }.expect("processor");
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");

    let error = processor
        .setup_realtime_f32(config)
        .expect_err("bus arrangement failed");

    assert_eq!(
        error,
        HostError::AudioProcessorCallFailed {
            method: "setBusArrangements",
            result: -12
        }
    );
}

#[test]
fn rejects_unsupported_speaker_arrangement() {
    let mut fake = FakeProcessor::new();
    let mut processor =
        unsafe { Vst3AudioProcessor::from_raw(fake.raw_processor()) }.expect("processor");
    let config = Vst3ProcessingConfig::new(48_000, 128, 9, 2).expect("config");

    let error = processor
        .setup_realtime_f32(config)
        .expect_err("unsupported arrangement");

    assert_eq!(error, HostError::UnsupportedSpeakerArrangement(9));
    assert_eq!(fake.set_bus_arrangement_calls, 0);
}

#[test]
fn rejects_failed_process_call() {
    let mut fake = FakeProcessor::new();
    fake.process_result = -20;
    let mut processor =
        unsafe { Vst3AudioProcessor::from_raw(fake.raw_processor()) }.expect("processor");
    let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");
    let mut buffers = Vst3ProcessBuffers::new(128, 2, 2).expect("buffers");
    buffers
        .prepare_interleaved_f32(2, &[1.0, 2.0, 3.0, 4.0])
        .expect("prepare");

    processor.setup_realtime_f32(config).expect("setup");
    let error = processor.process(&mut buffers).expect_err("process failed");

    assert_eq!(
        error,
        HostError::AudioProcessorCallFailed {
            method: "process",
            result: -20
        }
    );
}

#[test]
fn rejects_null_audio_processor_pointer() {
    let error =
        unsafe { Vst3AudioProcessor::from_raw(ptr::null_mut()) }.expect_err("null processor");

    assert_eq!(error, HostError::AudioProcessorReturnedNull);
}

#[repr(C)]
struct FakeProcessor {
    processor: IAudioProcessor,
    can_process_result: i32,
    setup_result: i32,
    set_bus_arrangement_result: i32,
    set_processing_result: i32,
    process_result: i32,
    setup: Option<ProcessSetup>,
    processing: bool,
    set_bus_arrangement_calls: u32,
    last_input_count: i32,
    last_output_count: i32,
    last_input_arrangement: Option<SpeakerArrangement>,
    last_output_arrangement: Option<SpeakerArrangement>,
    process_calls: u32,
    release_calls: u32,
    last_num_samples: i32,
    latency_samples: u32,
    tail_samples: u32,
}

impl FakeProcessor {
    fn new() -> Self {
        Self {
            processor: IAudioProcessor {
                vtable: &FAKE_AUDIO_PROCESSOR_VTABLE,
            },
            can_process_result: K_RESULT_OK,
            setup_result: K_RESULT_OK,
            set_bus_arrangement_result: K_RESULT_OK,
            set_processing_result: K_RESULT_OK,
            process_result: K_RESULT_OK,
            setup: None,
            processing: false,
            set_bus_arrangement_calls: 0,
            last_input_count: -1,
            last_output_count: -1,
            last_input_arrangement: None,
            last_output_arrangement: None,
            process_calls: 0,
            release_calls: 0,
            last_num_samples: 0,
            latency_samples: 0,
            tail_samples: 0,
        }
    }

    fn raw_processor(&mut self) -> *mut c_void {
        (&mut self.processor as *mut IAudioProcessor).cast()
    }
}

static FAKE_AUDIO_PROCESSOR_VTABLE: IAudioProcessorVTable = IAudioProcessorVTable {
    query_interface: fake_query_interface,
    add_ref: fake_add_ref,
    release: fake_release,
    set_bus_arrangements: fake_set_bus_arrangements,
    get_bus_arrangement: fake_get_bus_arrangement,
    can_process_sample_size: fake_can_process_sample_size,
    get_latency_samples: fake_get_latency_samples,
    setup_processing: fake_setup_processing,
    set_processing: fake_set_processing,
    process: fake_process,
    get_tail_samples: fake_get_tail_samples,
};

unsafe extern "system" fn fake_query_interface(
    _this: *mut IAudioProcessor,
    _iid: *const i8,
    _obj: *mut *mut c_void,
) -> i32 {
    -1
}

unsafe extern "system" fn fake_add_ref(_this: *mut IAudioProcessor) -> u32 {
    1
}

unsafe extern "system" fn fake_release(this: *mut IAudioProcessor) -> u32 {
    let fake = unsafe { fake_mut(this) };
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
    let fake = unsafe { fake_mut(this) };
    fake.set_bus_arrangement_calls += 1;
    fake.last_input_count = input_count;
    fake.last_output_count = output_count;
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
    fake.set_bus_arrangement_result
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
    let fake = unsafe { fake_mut(this) };
    if sample_size == VST3_SAMPLE_32 {
        fake.can_process_result
    } else {
        -1
    }
}

unsafe extern "system" fn fake_get_latency_samples(this: *mut IAudioProcessor) -> u32 {
    unsafe { fake_mut(this) }.latency_samples
}

unsafe extern "system" fn fake_setup_processing(
    this: *mut IAudioProcessor,
    setup: *mut ProcessSetup,
) -> i32 {
    let fake = unsafe { fake_mut(this) };
    if !setup.is_null() {
        // SAFETY: The caller supplied a non-null ProcessSetup pointer for the
        // duration of this fake ABI call.
        fake.setup = Some(unsafe { *setup });
    }
    fake.setup_result
}

unsafe extern "system" fn fake_set_processing(this: *mut IAudioProcessor, state: u8) -> i32 {
    let fake = unsafe { fake_mut(this) };
    fake.processing = state != 0;
    fake.set_processing_result
}

unsafe extern "system" fn fake_process(this: *mut IAudioProcessor, data: *mut ProcessData) -> i32 {
    let fake = unsafe { fake_mut(this) };
    fake.process_calls += 1;
    if fake.process_result != K_RESULT_OK {
        return fake.process_result;
    }
    if data.is_null() {
        return -1;
    }

    // SAFETY: Vst3ProcessBuffers supplies a valid ProcessData pointer for this
    // call; tests keep the backing buffers alive for the full call duration.
    let data = unsafe { &mut *data };
    fake.last_num_samples = data.num_samples;
    process_first_audio_bus(data)
}

unsafe extern "system" fn fake_get_tail_samples(this: *mut IAudioProcessor) -> u32 {
    unsafe { fake_mut(this) }.tail_samples
}

fn process_first_audio_bus(data: &mut ProcessData) -> i32 {
    if data.num_samples < 0 || data.num_outputs <= 0 || data.outputs.is_null() {
        return -1;
    }

    let frames = data.num_samples as usize;
    let output_bus = unsafe { first_bus_mut(data.outputs, data.num_outputs) };
    let output_channels = output_bus.num_channels as usize;
    let output_ptrs = unsafe { channel_ptrs_mut(output_bus, output_channels) };

    if data.num_inputs <= 0 || data.inputs.is_null() {
        write_instrument_output(output_ptrs, frames);
        return K_RESULT_OK;
    }

    let input_bus = unsafe { first_bus(data.inputs, data.num_inputs) };
    let input_channels = input_bus.num_channels as usize;
    let input_ptrs = unsafe { channel_ptrs(input_bus, input_channels) };

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

fn write_instrument_output(outputs: &mut [*mut f32], frames: usize) {
    for output in outputs {
        let output = unsafe { slice::from_raw_parts_mut(*output, frames) };
        for (frame, sample) in output.iter_mut().enumerate() {
            *sample = frame as f32;
        }
    }
}

unsafe fn fake_mut<'a>(this: *mut IAudioProcessor) -> &'a mut FakeProcessor {
    // SAFETY: FakeProcessor is repr(C) and stores IAudioProcessor as its first
    // field, so the interface pointer has the same address as the fake object.
    unsafe { &mut *(this.cast::<FakeProcessor>()) }
}

unsafe fn first_bus<'a>(buses: *mut AudioBusBuffers, count: i32) -> &'a AudioBusBuffers {
    // SAFETY: The fake only requests the first bus after validating count > 0.
    unsafe { &slice::from_raw_parts(buses, count as usize)[0] }
}

unsafe fn first_bus_mut<'a>(buses: *mut AudioBusBuffers, count: i32) -> &'a mut AudioBusBuffers {
    // SAFETY: The fake only requests the first bus after validating count > 0.
    unsafe { &mut slice::from_raw_parts_mut(buses, count as usize)[0] }
}

unsafe fn channel_ptrs<'a>(bus: &AudioBusBuffers, channels: usize) -> &'a [*mut f32] {
    // SAFETY: Vst3ProcessBuffers binds channel_buffers32 to an array with
    // num_channels entries.
    unsafe { slice::from_raw_parts(bus.channel_buffers32, channels) }
}

unsafe fn channel_ptrs_mut<'a>(bus: &mut AudioBusBuffers, channels: usize) -> &'a mut [*mut f32] {
    // SAFETY: Vst3ProcessBuffers binds channel_buffers32 to an array with
    // num_channels entries.
    unsafe { slice::from_raw_parts_mut(bus.channel_buffers32, channels) }
}
