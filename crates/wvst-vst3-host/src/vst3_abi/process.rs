#![allow(dead_code)]

use std::ffi::c_void;

use super::{
    SampleRate, SpeakerArrangement, String128, TBool, TQuarterNotes, TSamples, TUid,
    VST3_PROCESS_CONTEXT_BAR_POSITION_VALID, VST3_PROCESS_CONTEXT_CONT_TIME_VALID,
    VST3_PROCESS_CONTEXT_PROJECT_TIME_MUSIC_VALID, VST3_PROCESS_CONTEXT_TEMPO_VALID,
    VST3_PROCESS_CONTEXT_TIME_SIG_VALID, VST3_PROCESS_MODE_REALTIME, VST3_SAMPLE_32,
};

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
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameRate {
    pub frames_per_second: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Chord {
    pub key_note: u8,
    pub root_note: u8,
    pub chord_mask: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessContext {
    pub state: u32,
    pub sample_rate: SampleRate,
    pub project_time_samples: TSamples,
    pub system_time: i64,
    pub continous_time_samples: TSamples,
    pub project_time_music: TQuarterNotes,
    pub bar_position_music: TQuarterNotes,
    pub cycle_start_music: TQuarterNotes,
    pub cycle_end_music: TQuarterNotes,
    pub tempo: f64,
    pub time_sig_numerator: i32,
    pub time_sig_denominator: i32,
    pub chord: Chord,
    pub smpte_offset_subframes: i32,
    pub frame_rate: FrameRate,
    pub samples_to_next_clock: i32,
}

impl ProcessContext {
    pub fn stopped(sample_rate: SampleRate) -> Self {
        Self {
            state: VST3_PROCESS_CONTEXT_CONT_TIME_VALID
                | VST3_PROCESS_CONTEXT_PROJECT_TIME_MUSIC_VALID
                | VST3_PROCESS_CONTEXT_BAR_POSITION_VALID
                | VST3_PROCESS_CONTEXT_TEMPO_VALID
                | VST3_PROCESS_CONTEXT_TIME_SIG_VALID,
            sample_rate,
            project_time_samples: 0,
            system_time: 0,
            continous_time_samples: 0,
            project_time_music: 0.0,
            bar_position_music: 0.0,
            cycle_start_music: 0.0,
            cycle_end_music: 0.0,
            tempo: 120.0,
            time_sig_numerator: 4,
            time_sig_denominator: 4,
            chord: Chord::default(),
            smpte_offset_subframes: 0,
            frame_rate: FrameRate::default(),
            samples_to_next_clock: 0,
        }
    }
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
