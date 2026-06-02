use std::ffi::c_void;
use std::ptr;
use std::ptr::NonNull;

use crate::vst3_abi::{
    FUnknown, IComponent, IComponentVTable, K_RESULT_OK, VST3_BUS_DIRECTION_INPUT,
    VST3_BUS_DIRECTION_OUTPUT, VST3_MEDIA_TYPE_AUDIO,
};
use crate::{
    HostError, HostResult, Vst3AudioProcessor, Vst3InputEvent, Vst3Lifecycle, Vst3LifecycleState,
    Vst3ProcessBuffers, Vst3ProcessingConfig,
};

#[derive(Debug)]
pub struct Vst3ComponentInstance {
    component: Vst3ComponentHandle,
    processor: Vst3AudioProcessor,
    lifecycle: Vst3Lifecycle,
    buffers: Vst3ProcessBuffers,
    processing_config: Vst3ProcessingConfig,
}

impl Vst3ComponentInstance {
    /// # Safety
    ///
    /// `component` must be an owned, valid VST3 `IComponent` pointer.
    /// `processor` must be an owned, valid VST3 `IAudioProcessor` pointer for
    /// the same component instance. This holder releases both pointers on drop.
    pub unsafe fn from_raw_parts(
        component: *mut c_void,
        processor: *mut c_void,
        processing_config: Vst3ProcessingConfig,
    ) -> HostResult<Self> {
        let component = unsafe { Vst3ComponentHandle::from_raw(component)? };
        let processor = unsafe { Vst3AudioProcessor::from_raw(processor)? };
        let buffers = Vst3ProcessBuffers::new(
            processing_config.max_block_frames,
            processing_config.input_channels,
            processing_config.output_channels,
        )?;

        Ok(Self {
            component,
            processor,
            lifecycle: Vst3Lifecycle::created(),
            buffers,
            processing_config,
        })
    }

    pub const fn state(&self) -> Vst3LifecycleState {
        self.lifecycle.state()
    }

    pub const fn processing_config(&self) -> Vst3ProcessingConfig {
        self.processing_config
    }

    pub fn initialize(&mut self) -> HostResult<()> {
        self.require_state("initialize", &[Vst3LifecycleState::Created])?;
        self.component.initialize()?;
        self.lifecycle.initialize()
    }

    pub fn setup_processing(&mut self) -> HostResult<()> {
        self.require_state("setup-processing", &[Vst3LifecycleState::Initialized])?;
        self.processor.setup_realtime_f32(self.processing_config)?;
        self.lifecycle.setup_processing(self.processing_config)
    }

    pub fn activate(&mut self) -> HostResult<()> {
        self.require_state("activate", &[Vst3LifecycleState::SetupDone])?;
        self.component
            .activate_audio_buses(self.processing_config, true)?;
        if let Err(error) = self.component.set_active(true) {
            let _ = self
                .component
                .activate_audio_buses(self.processing_config, false);
            return Err(error);
        }
        self.lifecycle.activate()
    }

    pub fn start_processing(&mut self) -> HostResult<()> {
        self.require_state(
            "start-processing",
            &[Vst3LifecycleState::Activated, Vst3LifecycleState::Stopped],
        )?;
        self.processor.set_processing(true)?;
        self.lifecycle.start_processing()
    }

    pub fn process_interleaved_f32(
        &mut self,
        frames: usize,
        input: &[f32],
        output: &mut [f32],
    ) -> HostResult<()> {
        self.process_interleaved_f32_with_events(frames, input, &[], output)
    }

    pub fn process_interleaved_f32_with_events(
        &mut self,
        frames: usize,
        input: &[f32],
        events: &[Vst3InputEvent],
        output: &mut [f32],
    ) -> HostResult<()> {
        self.require_state("process", &[Vst3LifecycleState::Processing])?;
        self.buffers.prepare_interleaved_f32(frames, input)?;
        self.buffers.prepare_input_events(frames, events)?;
        self.processor.process(&mut self.buffers)?;
        self.buffers.copy_output_to_interleaved(frames, output)
    }

    pub fn stop_processing(&mut self) -> HostResult<()> {
        self.require_state("stop-processing", &[Vst3LifecycleState::Processing])?;
        self.processor.set_processing(false)?;
        self.lifecycle.stop_processing()
    }

    pub fn terminate(&mut self) -> HostResult<()> {
        self.require_state(
            "terminate",
            &[
                Vst3LifecycleState::Initialized,
                Vst3LifecycleState::SetupDone,
                Vst3LifecycleState::Activated,
                Vst3LifecycleState::Stopped,
            ],
        )?;
        if matches!(
            self.lifecycle.state(),
            Vst3LifecycleState::Activated | Vst3LifecycleState::Stopped
        ) {
            self.component.set_active(false)?;
            self.component
                .activate_audio_buses(self.processing_config, false)?;
        }
        self.component.terminate()?;
        self.lifecycle.terminate()
    }

    pub fn latency_samples(&self) -> u32 {
        self.processor.latency_samples()
    }

    pub fn tail_samples(&self) -> u32 {
        self.processor.tail_samples()
    }

    fn require_state(
        &self,
        action: &'static str,
        allowed: &[Vst3LifecycleState],
    ) -> HostResult<()> {
        if allowed.contains(&self.lifecycle.state()) {
            Ok(())
        } else {
            Err(HostError::InvalidLifecycleTransition {
                from: self.lifecycle.state().as_str(),
                action,
            })
        }
    }
}

#[derive(Debug)]
struct Vst3ComponentHandle {
    component: NonNull<IComponent>,
}

impl Vst3ComponentHandle {
    unsafe fn from_raw(component: *mut c_void) -> HostResult<Self> {
        let component =
            NonNull::new(component.cast::<IComponent>()).ok_or(HostError::ComponentReturnedNull)?;

        // SAFETY: The caller guarantees the pointer is a valid IComponent
        // object for the lifetime transferred to this handle.
        let vtable = unsafe { component.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::ComponentVTableMissing);
        }

        Ok(Self { component })
    }

    fn initialize(&mut self) -> HostResult<()> {
        self.call_result("initialize", |component, vtable| unsafe {
            // SAFETY: Null host context is a temporary MVP host-context stub.
            // The component pointer and vtable were validated by from_raw.
            (vtable.initialize)(component, ptr::null_mut::<FUnknown>())
        })
    }

    fn set_active(&mut self, active: bool) -> HostResult<()> {
        let state = if active { 1 } else { 0 };
        self.call_result("setActive", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw; VST3
            // TBool accepts 0/1 state values.
            (vtable.set_active)(component, state)
        })
    }

    fn activate_audio_buses(
        &mut self,
        config: Vst3ProcessingConfig,
        active: bool,
    ) -> HostResult<()> {
        if config.input_channels > 0 {
            self.activate_audio_bus(VST3_BUS_DIRECTION_INPUT, active)?;
        }
        self.activate_audio_bus(VST3_BUS_DIRECTION_OUTPUT, active)
    }

    fn activate_audio_bus(&mut self, direction: i32, active: bool) -> HostResult<()> {
        let state = if active { 1 } else { 0 };
        self.call_result("activateBus", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw; WVST's
            // MVP process buffers expose at most one main audio bus per
            // direction, so index 0 is the only bus activated here.
            (vtable.activate_bus)(component, VST3_MEDIA_TYPE_AUDIO, direction, 0, state)
        })
    }

    fn terminate(&mut self) -> HostResult<()> {
        self.call_result("terminate", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw.
            (vtable.terminate)(component)
        })
    }

    fn call_result(
        &mut self,
        method: &'static str,
        call: impl FnOnce(*mut IComponent, &IComponentVTable) -> i32,
    ) -> HostResult<()> {
        let result = call(self.component.as_ptr(), self.vtable());
        if result == K_RESULT_OK {
            Ok(())
        } else {
            Err(HostError::ComponentCallFailed { method, result })
        }
    }

    fn vtable(&self) -> &IComponentVTable {
        // SAFETY: from_raw validated both the object pointer and the vtable
        // pointer. The handle owns the reference until Drop calls release.
        unsafe { &*self.component.as_ref().vtable }
    }
}

impl Drop for Vst3ComponentHandle {
    fn drop(&mut self) {
        let component = self.component.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one owned IComponent reference to this
        // handle; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(component);
        }
    }
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod component_tests;
