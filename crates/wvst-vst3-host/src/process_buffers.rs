use std::ffi::c_void;
use std::ptr;

use wvst_core::audio::MAX_CHANNEL_COUNT;

use crate::event_list::{DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, Vst3EventList, Vst3InputEvent};
use crate::parameter_changes::{
    DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK, Vst3ParameterChange, Vst3ParameterChanges,
};
use crate::vst3_abi::{
    AudioBusBuffers, ProcessContext, ProcessData, VST3_PROCESS_CONTEXT_PLAYING,
    VST3_PROCESS_MODE_REALTIME, VST3_SAMPLE_32,
};
use crate::{HostError, HostResult};

#[derive(Debug)]
pub struct Vst3ProcessBuffers {
    max_frames: usize,
    input_channels: usize,
    output_channels: usize,
    prepared_frames: usize,
    input_samples: Vec<f32>,
    output_samples: Vec<f32>,
    input_channel_ptrs: Vec<*mut f32>,
    output_channel_ptrs: Vec<*mut f32>,
    input_buses: Vec<AudioBusBuffers>,
    output_buses: Vec<AudioBusBuffers>,
    input_events: Vst3EventList,
    input_parameter_changes: Vst3ParameterChanges,
    process_context: Box<ProcessContext>,
    process_data: ProcessData,
}

impl Vst3ProcessBuffers {
    pub fn new(
        sample_rate: f64,
        max_frames: u16,
        input_channels: u16,
        output_channels: u16,
    ) -> HostResult<Self> {
        if max_frames == 0 {
            return Err(HostError::InvalidMaxBlockFrames(max_frames));
        }
        validate_channel_counts(input_channels, output_channels)?;

        let max_frames = usize::from(max_frames);
        let input_channels = usize::from(input_channels);
        let output_channels = usize::from(output_channels);
        let input_samples = vec![0.0; checked_sample_len(max_frames, input_channels)?];
        let output_samples = vec![0.0; checked_sample_len(max_frames, output_channels)?];
        let input_channel_ptrs = vec![ptr::null_mut(); input_channels];
        let output_channel_ptrs = vec![ptr::null_mut(); output_channels];
        let input_buses = if input_channels == 0 {
            Vec::new()
        } else {
            vec![audio_bus(input_channels)]
        };
        let output_buses = vec![audio_bus(output_channels)];
        let input_events = Vst3EventList::new(DEFAULT_MAX_VST3_EVENTS_PER_BLOCK);
        let input_parameter_changes =
            Vst3ParameterChanges::new(DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK);
        let process_context = Box::new(ProcessContext::stopped(sample_rate));

        let mut buffers = Self {
            max_frames,
            input_channels,
            output_channels,
            prepared_frames: 0,
            input_samples,
            output_samples,
            input_channel_ptrs,
            output_channel_ptrs,
            input_buses,
            output_buses,
            input_events,
            input_parameter_changes,
            process_context,
            process_data: empty_process_data(),
        };
        buffers.refresh_abi_pointers();

        Ok(buffers)
    }

    pub const fn max_frames(&self) -> usize {
        self.max_frames
    }

    pub const fn input_channels(&self) -> usize {
        self.input_channels
    }

    pub const fn output_channels(&self) -> usize {
        self.output_channels
    }

    pub fn prepare_interleaved_f32(&mut self, frames: usize, input: &[f32]) -> HostResult<()> {
        self.validate_frames(frames)?;
        self.input_events.clear();
        self.input_parameter_changes.clear();

        let expected_input = checked_sample_len(frames, self.input_channels)?;
        if input.len() != expected_input {
            return Err(HostError::InvalidBufferLength {
                expected: expected_input,
                actual: input.len(),
            });
        }

        for frame in 0..frames {
            for channel in 0..self.input_channels {
                let source_index = frame * self.input_channels + channel;
                let target_index = channel * self.max_frames + frame;
                self.input_samples[target_index] = input[source_index];
            }
        }

        for channel in 0..self.output_channels {
            let range = self
                .channel_range(self.output_channels, channel, frames)
                .ok_or(HostError::InvalidBufferLength {
                    expected: self.max_frames,
                    actual: frames,
                })?;
            self.output_samples[range].fill(0.0);
        }
        self.prepared_frames = frames;
        self.process_data.num_samples = frames as i32;
        self.process_context.state |= VST3_PROCESS_CONTEXT_PLAYING;
        self.process_context.project_time_samples = self.process_context.continous_time_samples;
        self.process_context.continous_time_samples = self
            .process_context
            .continous_time_samples
            .saturating_add(frames as i64);
        self.process_context.project_time_music = samples_to_quarter_notes(
            self.process_context.project_time_samples,
            self.process_context.sample_rate,
        );
        self.process_context.bar_position_music = self.process_context.project_time_music;

        Ok(())
    }

    pub fn prepare_input_events(
        &mut self,
        frames: usize,
        events: &[Vst3InputEvent],
    ) -> HostResult<()> {
        self.validate_frames(frames)?;
        self.input_events.set_events(frames, events)
    }

    pub fn prepare_input_parameter_changes(
        &mut self,
        frames: usize,
        changes: &[Vst3ParameterChange],
    ) -> HostResult<()> {
        self.validate_frames(frames)?;
        self.input_parameter_changes.set_changes(frames, changes)
    }

    pub fn copy_output_to_interleaved(&self, frames: usize, output: &mut [f32]) -> HostResult<()> {
        self.validate_prepared_frames(frames)?;

        let expected_output = checked_sample_len(frames, self.output_channels)?;
        if output.len() != expected_output {
            return Err(HostError::InvalidBufferLength {
                expected: expected_output,
                actual: output.len(),
            });
        }

        for frame in 0..frames {
            for channel in 0..self.output_channels {
                let source_index = channel * self.max_frames + frame;
                let target_index = frame * self.output_channels + channel;
                output[target_index] = self.output_samples[source_index];
            }
        }

        Ok(())
    }

    pub fn input_channel(&self, channel: usize, frames: usize) -> Option<&[f32]> {
        if frames > self.prepared_frames {
            return None;
        }

        self.channel_range(self.input_channels, channel, frames)
            .map(|range| &self.input_samples[range])
    }

    pub fn output_channel_mut(&mut self, channel: usize, frames: usize) -> Option<&mut [f32]> {
        if frames > self.prepared_frames {
            return None;
        }

        self.channel_range(self.output_channels, channel, frames)
            .map(|range| &mut self.output_samples[range])
    }

    pub(crate) fn process_data_mut(&mut self) -> &mut ProcessData {
        &mut self.process_data
    }

    fn validate_frames(&self, frames: usize) -> HostResult<()> {
        if frames > self.max_frames {
            return Err(HostError::InvalidBufferLength {
                expected: self.max_frames,
                actual: frames,
            });
        }

        Ok(())
    }

    fn validate_prepared_frames(&self, frames: usize) -> HostResult<()> {
        self.validate_frames(frames)?;
        if frames > self.prepared_frames {
            return Err(HostError::InvalidBufferLength {
                expected: self.prepared_frames,
                actual: frames,
            });
        }

        Ok(())
    }

    fn channel_range(
        &self,
        channels: usize,
        channel: usize,
        frames: usize,
    ) -> Option<std::ops::Range<usize>> {
        if channel >= channels || frames > self.max_frames {
            return None;
        }

        let start = channel * self.max_frames;
        Some(start..start + frames)
    }

    fn refresh_abi_pointers(&mut self) {
        for channel in 0..self.input_channels {
            self.input_channel_ptrs[channel] =
                self.input_samples[channel * self.max_frames..].as_mut_ptr();
        }
        for channel in 0..self.output_channels {
            self.output_channel_ptrs[channel] =
                self.output_samples[channel * self.max_frames..].as_mut_ptr();
        }

        if let Some(bus) = self.input_buses.first_mut() {
            bus.channel_buffers32 = self.input_channel_ptrs.as_mut_ptr();
        }
        if let Some(bus) = self.output_buses.first_mut() {
            bus.channel_buffers32 = self.output_channel_ptrs.as_mut_ptr();
        }

        self.process_data.inputs = if self.input_buses.is_empty() {
            ptr::null_mut()
        } else {
            self.input_buses.as_mut_ptr()
        };
        self.process_data.outputs = self.output_buses.as_mut_ptr();
        self.process_data.num_inputs = self.input_buses.len() as i32;
        self.process_data.num_outputs = self.output_buses.len() as i32;
        self.process_data.input_events = self.input_events.as_raw_ptr().cast::<c_void>();
        self.process_data.input_parameter_changes =
            self.input_parameter_changes.as_raw_ptr().cast::<c_void>();
        self.process_data.process_context =
            (&mut *self.process_context as *mut ProcessContext).cast::<c_void>();
    }
}

fn validate_channel_counts(input_channels: u16, output_channels: u16) -> HostResult<()> {
    if output_channels == 0
        || input_channels > MAX_CHANNEL_COUNT
        || output_channels > MAX_CHANNEL_COUNT
    {
        return Err(HostError::InvalidChannelCount {
            input: usize::from(input_channels),
            output: usize::from(output_channels),
        });
    }

    Ok(())
}

fn checked_sample_len(frames: usize, channels: usize) -> HostResult<usize> {
    frames
        .checked_mul(channels)
        .ok_or(HostError::InvalidBufferLength {
            expected: usize::MAX,
            actual: frames,
        })
}

fn audio_bus(channels: usize) -> AudioBusBuffers {
    AudioBusBuffers {
        num_channels: channels as i32,
        silence_flags: 0,
        channel_buffers32: ptr::null_mut(),
    }
}

fn empty_process_data() -> ProcessData {
    ProcessData {
        process_mode: VST3_PROCESS_MODE_REALTIME,
        symbolic_sample_size: VST3_SAMPLE_32,
        num_samples: 0,
        num_inputs: 0,
        num_outputs: 0,
        inputs: ptr::null_mut(),
        outputs: ptr::null_mut(),
        input_parameter_changes: ptr::null_mut::<c_void>(),
        output_parameter_changes: ptr::null_mut::<c_void>(),
        input_events: ptr::null_mut::<c_void>(),
        output_events: ptr::null_mut::<c_void>(),
        process_context: ptr::null_mut::<c_void>(),
    }
}

fn samples_to_quarter_notes(samples: i64, sample_rate: f64) -> f64 {
    if sample_rate <= 0.0 {
        return 0.0;
    }
    let seconds = samples as f64 / sample_rate;
    seconds * (120.0 / 60.0)
}

#[cfg(test)]
#[path = "process_buffers_tests.rs"]
mod process_buffers_tests;
