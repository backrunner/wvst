use std::ffi::c_void;

use crate::{
    HostError, HostResult, Vst3AudioBusInfo, Vst3AudioProcessor, Vst3BusDirection, Vst3BusType,
    Vst3ConnectionPoint, Vst3HostContext, Vst3InputEvent, Vst3Lifecycle, Vst3LifecycleState,
    Vst3ParameterChange, Vst3ProcessBuffers, Vst3ProcessOutput, Vst3ProcessingConfig,
    Vst3ProgramListData, Vst3TailSamples, Vst3UnitData,
};

#[path = "component/handle.rs"]
mod handle;

use handle::Vst3ComponentHandle;

#[derive(Debug)]
pub struct Vst3ComponentInstance {
    component: Vst3ComponentHandle,
    processor: Vst3AudioProcessor,
    host_context: Vst3HostContext,
    lifecycle: Vst3Lifecycle,
    buffers: Vst3ProcessBuffers,
    processing_config: Vst3ProcessingConfig,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Vst3SelectedAudioBuses {
    pub input: Option<Vst3SelectedAudioBus>,
    pub output: Vst3SelectedAudioBus,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Vst3SelectedAudioBus {
    pub direction: Vst3BusDirection,
    pub requested_channels: u16,
    pub requested_index: Option<i32>,
    pub selected_index: i32,
    pub selected: Option<Vst3AudioBusInfo>,
    pub available: Vec<Vst3AudioBusInfo>,
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
            processing_config.sample_rate as f64,
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

    pub fn selected_audio_buses(&mut self) -> HostResult<Vst3SelectedAudioBuses> {
        let input = if self.processing_config.input_channels == 0 {
            None
        } else {
            Some(self.selected_audio_bus(
                Vst3BusDirection::Input,
                self.processing_config.input_channels,
                self.processing_config.input_bus_index,
            )?)
        };
        let output = self.selected_audio_bus(
            Vst3BusDirection::Output,
            self.processing_config.output_channels,
            self.processing_config.output_bus_index,
        )?;
        Ok(Vst3SelectedAudioBuses { input, output })
    }

    fn selected_audio_bus(
        &mut self,
        direction: Vst3BusDirection,
        requested_channels: u16,
        requested_index: Option<i32>,
    ) -> HostResult<Vst3SelectedAudioBus> {
        let available = self.audio_buses(direction)?;
        let selected_index =
            select_audio_bus_index(&available, requested_channels, requested_index, direction)?;
        let selected = available
            .iter()
            .find(|bus| bus.index == selected_index)
            .cloned();
        Ok(Vst3SelectedAudioBus {
            direction,
            requested_channels,
            requested_index,
            selected_index,
            selected,
            available,
        })
    }

    pub fn controller_class_id(&self) -> HostResult<Option<String>> {
        self.component.controller_class_id()
    }

    pub fn connection_point(&self) -> HostResult<Option<Vst3ConnectionPoint>> {
        self.component.connection_point()
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

    pub fn program_list_data(&self) -> HostResult<Option<Vst3ProgramListData>> {
        self.require_state(
            "program-list-data",
            &[
                Vst3LifecycleState::Initialized,
                Vst3LifecycleState::SetupDone,
                Vst3LifecycleState::Activated,
                Vst3LifecycleState::Stopped,
            ],
        )?;
        self.component.program_list_data()
    }

    pub fn unit_data(&self) -> HostResult<Option<Vst3UnitData>> {
        self.require_state(
            "unit-data",
            &[
                Vst3LifecycleState::Initialized,
                Vst3LifecycleState::SetupDone,
                Vst3LifecycleState::Activated,
                Vst3LifecycleState::Stopped,
            ],
        )?;
        self.component.unit_data()
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

    pub fn process_interleaved_f32_with_io_events_and_parameters(
        &mut self,
        frames: usize,
        input: &[f32],
        events: &[Vst3InputEvent],
        parameter_changes: &[Vst3ParameterChange],
        output: &mut [f32],
        process_output: &mut Vst3ProcessOutput,
    ) -> HostResult<()> {
        process_output.clear();
        self.process_interleaved_f32_with_events_and_parameters(
            frames,
            input,
            events,
            parameter_changes,
            output,
        )?;
        process_output.diagnostics.output_events = self.buffers.output_events_and_advanced_into(
            &mut process_output.events,
            &mut process_output.advanced_events,
        );
        process_output.diagnostics.output_parameter_changes = self
            .buffers
            .output_parameter_changes_into(&mut process_output.parameter_changes);
        Ok(())
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

    pub fn tail_info(&self) -> Vst3TailSamples {
        self.processor.tail_info()
    }

    pub fn process_context_requirements(&self) -> Option<u32> {
        self.processor.process_context_requirements()
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

fn select_audio_bus_index(
    buses: &[Vst3AudioBusInfo],
    requested_channels: u16,
    requested_index: Option<i32>,
    direction: Vst3BusDirection,
) -> HostResult<i32> {
    if let Some(index) = requested_index {
        if index < 0
            || !buses
                .iter()
                .any(|bus| bus.index == index && bus.direction == direction)
        {
            return Err(HostError::InvalidAudioBusIndex {
                direction: direction.as_str(),
                index,
            });
        }
        return Ok(index);
    }

    let requested_channels = i32::from(requested_channels);
    Ok(buses
        .iter()
        .find(|bus| bus.bus_type == Vst3BusType::Main && bus.channel_count == requested_channels)
        .or_else(|| {
            buses.iter().find(|bus| {
                bus.bus_type == Vst3BusType::Main && bus.default_active && bus.channel_count > 0
            })
        })
        .or_else(|| buses.iter().find(|bus| bus.channel_count > 0))
        .map_or(0, |bus| bus.index))
}

#[cfg(test)]
#[path = "component_bus_selection_tests.rs"]
mod bus_selection_tests;

#[cfg(test)]
#[path = "component_controller_tests.rs"]
mod component_controller_tests;

#[cfg(test)]
#[path = "component_tests.rs"]
mod component_tests;
