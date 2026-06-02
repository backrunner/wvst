use std::ffi::c_void;
use std::ptr;
use std::ptr::NonNull;

use crate::vst3_abi::{
    IAudioProcessor, IAudioProcessorVTable, K_RESULT_OK, ProcessSetup, VST3_SAMPLE_32,
    VST3_SPEAKER_30_CINE, VST3_SPEAKER_40_MUSIC, VST3_SPEAKER_50, VST3_SPEAKER_51,
    VST3_SPEAKER_61_CINE, VST3_SPEAKER_71_CINE, VST3_SPEAKER_MONO, VST3_SPEAKER_STEREO,
};
use crate::{HostError, HostResult, Vst3ProcessBuffers, Vst3ProcessingConfig};

#[derive(Debug)]
pub struct Vst3AudioProcessor {
    processor: NonNull<IAudioProcessor>,
    processing: bool,
}

impl Vst3AudioProcessor {
    /// # Safety
    ///
    /// `processor` must be an owned, valid VST3 `IAudioProcessor` pointer with
    /// a non-null vtable. This wrapper calls `release` exactly once on drop.
    pub unsafe fn from_raw(processor: *mut c_void) -> HostResult<Self> {
        let processor = NonNull::new(processor.cast::<IAudioProcessor>())
            .ok_or(HostError::AudioProcessorReturnedNull)?;

        // SAFETY: The caller guarantees the pointer is a valid IAudioProcessor
        // object for the lifetime transferred to this wrapper.
        let vtable = unsafe { processor.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::AudioProcessorVTableMissing);
        }

        Ok(Self {
            processor,
            processing: false,
        })
    }

    pub const fn is_processing(&self) -> bool {
        self.processing
    }

    pub fn setup_realtime_f32(&mut self, config: Vst3ProcessingConfig) -> HostResult<()> {
        self.call_result("canProcessSampleSize", |processor, vtable| unsafe {
            // SAFETY: vtable and processor were validated by from_raw; the
            // sample-size value is a VST3 ABI constant.
            (vtable.can_process_sample_size)(processor, VST3_SAMPLE_32)
        })?;
        self.set_bus_arrangements(config)?;

        let mut setup = ProcessSetup::realtime_f32(
            i32::from(config.max_block_frames),
            config.sample_rate as f64,
        );
        self.call_result("setupProcessing", |processor, vtable| unsafe {
            // SAFETY: setup points to a stack ProcessSetup with repr(C) layout
            // and is valid for the duration of the ABI call.
            (vtable.setup_processing)(processor, &mut setup)
        })
    }

    pub fn set_processing(&mut self, processing: bool) -> HostResult<()> {
        let state = if processing { 1 } else { 0 };
        self.call_result("setProcessing", |processor, vtable| unsafe {
            // SAFETY: vtable and processor were validated by from_raw; VST3
            // TBool accepts 0/1 state values.
            (vtable.set_processing)(processor, state)
        })?;
        self.processing = processing;

        Ok(())
    }

    fn set_bus_arrangements(&mut self, config: Vst3ProcessingConfig) -> HostResult<()> {
        let mut input = optional_speaker_arrangement(config.input_channels)?;
        let mut output = speaker_arrangement(config.output_channels)?;
        let (inputs, input_count) = match input.as_mut() {
            Some(arrangement) => (arrangement as *mut _, 1),
            None => (ptr::null_mut(), 0),
        };

        self.call_result("setBusArrangements", |processor, vtable| unsafe {
            // SAFETY: input/output arrangements point to stack values that live
            // for this ABI call; zero-input instruments pass a null input
            // pointer with count 0 as required by the VST3 API shape.
            (vtable.set_bus_arrangements)(processor, inputs, input_count, &mut output, 1)
        })
    }

    pub fn process(&mut self, buffers: &mut Vst3ProcessBuffers) -> HostResult<()> {
        self.call_result("process", |processor, vtable| unsafe {
            // SAFETY: Vst3ProcessBuffers owns stable repr(C) ProcessData,
            // AudioBusBuffers and channel pointer arrays for the ABI call.
            (vtable.process)(processor, buffers.process_data_mut())
        })
    }

    pub fn latency_samples(&self) -> u32 {
        let processor = self.processor.as_ptr();
        let vtable = self.vtable();

        // SAFETY: vtable and processor were validated by from_raw; this VST3
        // query takes no additional pointers.
        unsafe { (vtable.get_latency_samples)(processor) }
    }

    pub fn tail_samples(&self) -> u32 {
        let processor = self.processor.as_ptr();
        let vtable = self.vtable();

        // SAFETY: vtable and processor were validated by from_raw; this VST3
        // query takes no additional pointers.
        unsafe { (vtable.get_tail_samples)(processor) }
    }

    fn call_result(
        &mut self,
        method: &'static str,
        call: impl FnOnce(*mut IAudioProcessor, &IAudioProcessorVTable) -> i32,
    ) -> HostResult<()> {
        let result = call(self.processor.as_ptr(), self.vtable());
        if result == K_RESULT_OK {
            Ok(())
        } else {
            Err(HostError::AudioProcessorCallFailed { method, result })
        }
    }

    fn vtable(&self) -> &IAudioProcessorVTable {
        // SAFETY: from_raw validated both the object pointer and the vtable
        // pointer. The wrapper owns the reference until Drop calls release.
        unsafe { &*self.processor.as_ref().vtable }
    }
}

fn optional_speaker_arrangement(
    channels: u16,
) -> HostResult<Option<crate::vst3_abi::SpeakerArrangement>> {
    if channels == 0 {
        Ok(None)
    } else {
        speaker_arrangement(channels).map(Some)
    }
}

fn speaker_arrangement(channels: u16) -> HostResult<crate::vst3_abi::SpeakerArrangement> {
    match channels {
        1 => Ok(VST3_SPEAKER_MONO),
        2 => Ok(VST3_SPEAKER_STEREO),
        3 => Ok(VST3_SPEAKER_30_CINE),
        4 => Ok(VST3_SPEAKER_40_MUSIC),
        5 => Ok(VST3_SPEAKER_50),
        6 => Ok(VST3_SPEAKER_51),
        7 => Ok(VST3_SPEAKER_61_CINE),
        8 => Ok(VST3_SPEAKER_71_CINE),
        _ => Err(HostError::UnsupportedSpeakerArrangement(channels)),
    }
}

impl Drop for Vst3AudioProcessor {
    fn drop(&mut self) {
        let processor = self.processor.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one owned IAudioProcessor reference to this
        // wrapper; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(processor);
        }
    }
}

#[cfg(test)]
#[path = "audio_processor_tests.rs"]
mod audio_processor_tests;
