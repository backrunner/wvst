use std::ffi::c_void;
use std::ptr::NonNull;

use crate::vst3_abi::{
    BusInfo, FUnknown, IComponent, IComponentVTable, K_RESULT_OK, TUid, VST3_MEDIA_TYPE_AUDIO,
    tuid_hex,
};
use crate::{
    HostError, HostResult, Vst3AudioBusInfo, Vst3AudioProcessor, Vst3BusDirection, Vst3BusType,
    Vst3HostContext, Vst3InputEvent, Vst3Lifecycle, Vst3LifecycleState, Vst3ParameterChange,
    Vst3ProcessBuffers, Vst3ProcessingConfig, state_stream::Vst3StateStream,
};

#[derive(Debug)]
pub struct Vst3ComponentInstance {
    component: Vst3ComponentHandle,
    processor: Vst3AudioProcessor,
    host_context: Vst3HostContext,
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
            host_context: Vst3HostContext::new("WVST"),
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

    pub fn audio_buses(
        &mut self,
        direction: Vst3BusDirection,
    ) -> HostResult<Vec<Vst3AudioBusInfo>> {
        self.require_state(
            "query-audio-buses",
            &[
                Vst3LifecycleState::Created,
                Vst3LifecycleState::Initialized,
                Vst3LifecycleState::SetupDone,
                Vst3LifecycleState::Activated,
                Vst3LifecycleState::Processing,
                Vst3LifecycleState::Stopped,
            ],
        )?;
        self.component.audio_buses(direction)
    }

    pub fn controller_class_id(&self) -> HostResult<Option<String>> {
        self.component.controller_class_id()
    }

    pub fn get_state(&self) -> HostResult<Vec<u8>> {
        self.require_state(
            "get-state",
            &[
                Vst3LifecycleState::Created,
                Vst3LifecycleState::Initialized,
                Vst3LifecycleState::SetupDone,
                Vst3LifecycleState::Activated,
                Vst3LifecycleState::Stopped,
            ],
        )?;
        self.component.get_state()
    }

    pub fn set_state(&mut self, state: &[u8]) -> HostResult<()> {
        self.require_state(
            "set-state",
            &[
                Vst3LifecycleState::Created,
                Vst3LifecycleState::Initialized,
                Vst3LifecycleState::SetupDone,
                Vst3LifecycleState::Activated,
                Vst3LifecycleState::Stopped,
            ],
        )?;
        self.component.set_state(state)
    }

    pub fn initialize(&mut self) -> HostResult<()> {
        self.require_state("initialize", &[Vst3LifecycleState::Created])?;
        self.component
            .initialize(self.host_context.as_funknown_ptr())?;
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
        self.process_interleaved_f32_with_events_and_parameters(frames, input, &[], &[], output)
    }

    pub fn process_interleaved_f32_with_events(
        &mut self,
        frames: usize,
        input: &[f32],
        events: &[Vst3InputEvent],
        output: &mut [f32],
    ) -> HostResult<()> {
        self.process_interleaved_f32_with_events_and_parameters(frames, input, events, &[], output)
    }

    pub fn process_interleaved_f32_with_events_and_parameters(
        &mut self,
        frames: usize,
        input: &[f32],
        events: &[Vst3InputEvent],
        parameter_changes: &[Vst3ParameterChange],
        output: &mut [f32],
    ) -> HostResult<()> {
        self.require_state("process", &[Vst3LifecycleState::Processing])?;
        self.buffers.prepare_interleaved_f32(frames, input)?;
        self.buffers.prepare_input_events(frames, events)?;
        self.buffers
            .prepare_input_parameter_changes(frames, parameter_changes)?;
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

    fn initialize(&mut self, context: *mut FUnknown) -> HostResult<()> {
        self.call_result("initialize", |component, vtable| unsafe {
            // SAFETY: The component pointer and vtable were validated by
            // from_raw. `context` is owned by the component holder and remains
            // live until after component termination/drop.
            (vtable.initialize)(component, context)
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
            self.activate_selected_audio_bus(
                Vst3BusDirection::Input,
                config.input_channels,
                active,
            )?;
        }
        self.activate_selected_audio_bus(Vst3BusDirection::Output, config.output_channels, active)
    }

    fn activate_selected_audio_bus(
        &mut self,
        direction: Vst3BusDirection,
        channels: u16,
        active: bool,
    ) -> HostResult<()> {
        let buses = self.audio_buses(direction)?;
        let index = select_audio_bus_index(&buses, channels);
        self.activate_audio_bus(direction.as_abi(), index, active)
    }

    fn activate_audio_bus(&mut self, direction: i32, index: i32, active: bool) -> HostResult<()> {
        let state = if active { 1 } else { 0 };
        self.call_result("activateBus", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw; WVST's
            // MVP process buffers expose one selected audio bus per direction.
            (vtable.activate_bus)(component, VST3_MEDIA_TYPE_AUDIO, direction, index, state)
        })
    }

    fn controller_class_id(&self) -> HostResult<Option<String>> {
        let mut class_id: TUid = [0; 16];
        let result = unsafe {
            // SAFETY: component and vtable were validated by from_raw; class_id
            // is writable stack storage for the component's controller TUID.
            (self.vtable().get_controller_class_id)(self.component.as_ptr(), &mut class_id)
        };
        if result != K_RESULT_OK {
            return Err(HostError::ComponentCallFailed {
                method: "getControllerClassId",
                result,
            });
        }

        Ok((class_id != [0; 16]).then(|| tuid_hex(&class_id)))
    }

    fn get_state(&self) -> HostResult<Vec<u8>> {
        let mut stream = Vst3StateStream::writable();
        let result = unsafe {
            // SAFETY: component/vtable were validated by from_raw. The stream
            // object remains live for the duration of this call.
            (self.vtable().get_state)(self.component.as_ptr(), stream.as_mut_ptr().cast())
        };
        if result != K_RESULT_OK {
            return Err(HostError::ComponentCallFailed {
                method: "getState",
                result,
            });
        }
        Ok(stream.into_bytes())
    }

    fn set_state(&mut self, state: &[u8]) -> HostResult<()> {
        let mut stream = Vst3StateStream::from_bytes(state.to_vec());
        self.call_result("setState", |component, vtable| unsafe {
            // SAFETY: component/vtable were validated by from_raw. The stream
            // object remains live for the duration of this call.
            (vtable.set_state)(component, stream.as_mut_ptr().cast())
        })
    }

    fn audio_buses(&mut self, direction: Vst3BusDirection) -> HostResult<Vec<Vst3AudioBusInfo>> {
        let direction_abi = direction.as_abi();
        let count = self.call_count("getBusCount", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw.
            (vtable.get_bus_count)(component, VST3_MEDIA_TYPE_AUDIO, direction_abi)
        })?;
        let mut buses = Vec::with_capacity(count as usize);

        for index in 0..count {
            let mut info = BusInfo::default();
            self.call_result("getBusInfo", |component, vtable| unsafe {
                // SAFETY: `info` is stack storage matching the VST3 BusInfo ABI.
                (vtable.get_bus_info)(
                    component,
                    VST3_MEDIA_TYPE_AUDIO,
                    direction_abi,
                    index,
                    (&mut info as *mut BusInfo).cast(),
                )
            })?;
            buses.push(Vst3AudioBusInfo::from_abi(index, direction, info));
        }

        Ok(buses)
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

    fn call_count(
        &mut self,
        method: &'static str,
        call: impl FnOnce(*mut IComponent, &IComponentVTable) -> i32,
    ) -> HostResult<i32> {
        let result = call(self.component.as_ptr(), self.vtable());
        if result >= 0 {
            Ok(result)
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

fn select_audio_bus_index(buses: &[Vst3AudioBusInfo], requested_channels: u16) -> i32 {
    let requested_channels = i32::from(requested_channels);
    buses
        .iter()
        .find(|bus| bus.bus_type == Vst3BusType::Main && bus.channel_count == requested_channels)
        .or_else(|| {
            buses.iter().find(|bus| {
                bus.bus_type == Vst3BusType::Main && bus.default_active && bus.channel_count > 0
            })
        })
        .or_else(|| buses.iter().find(|bus| bus.channel_count > 0))
        .map_or(0, |bus| bus.index)
}

#[cfg(test)]
mod bus_selection_tests {
    use super::*;

    #[test]
    fn prefers_main_bus_matching_requested_channels() {
        let buses = [
            test_bus(0, 1, true, Vst3BusType::Main),
            test_bus(2, 2, false, Vst3BusType::Main),
        ];

        assert_eq!(select_audio_bus_index(&buses, 2), 2);
    }

    fn test_bus(
        index: i32,
        channel_count: i32,
        default_active: bool,
        bus_type: Vst3BusType,
    ) -> Vst3AudioBusInfo {
        Vst3AudioBusInfo {
            index,
            direction: Vst3BusDirection::Output,
            channel_count,
            bus_type,
            default_active,
            control_voltage: false,
            name: None,
        }
    }
}

#[cfg(test)]
#[path = "component_controller_tests.rs"]
mod component_controller_tests;

#[cfg(test)]
#[path = "component_tests.rs"]
mod component_tests;
