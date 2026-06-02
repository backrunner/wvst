use serde::Serialize;
use wvst_scanner::PluginDescriptor;
use wvst_vst3_host::{
    HeadlessPluginInstance, HostError, VST3_MIDI_CONTROLLER_AFTERTOUCH,
    VST3_MIDI_CONTROLLER_PITCH_BEND, Vst3BusDirection, Vst3InputEvent, Vst3LifecycleState,
    Vst3LoadedComponent, Vst3ParameterChange, Vst3ParameterInfo, Vst3ProcessingConfig,
    Vst3UnitMetadata, create_vst3_component_instance,
};

use super::InstanceCreateParams;

pub(super) enum WorkerBackend {
    Passthrough(HeadlessPluginInstance),
    Vst3Runtime(Box<Vst3RuntimeBackend>),
}

pub(super) struct Vst3RuntimeBackend {
    component: Vst3LoadedComponent,
    midi_mapping: Vst3MidiMappingCache,
}

#[derive(Debug, Clone)]
struct Vst3MidiMappingCache {
    assignments: [Option<u32>; MIDI_MAPPING_SLOT_COUNT],
}

const MIDI_MAPPING_CHANNELS: usize = 16;
const MIDI_MAPPING_CONTROLLER_COUNT: usize = 130;
const MIDI_MAPPING_SLOT_COUNT: usize = MIDI_MAPPING_CHANNELS * MIDI_MAPPING_CONTROLLER_COUNT;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum WorkerBackendKind {
    Passthrough,
    Vst3Runtime,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkerBackendDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    component_handler: Option<wvst_vst3_host::Vst3ComponentHandlerSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_context_requirements: Option<u32>,
}

impl WorkerBackend {
    pub(super) fn from_create_params(
        params: &InstanceCreateParams,
        descriptor: &PluginDescriptor,
    ) -> Result<Self, String> {
        let Some(class_id) = params.class_id.as_deref() else {
            return Self::passthrough(descriptor, params);
        };

        if !std::path::Path::new(&params.plugin_path).is_dir() {
            return Self::passthrough(descriptor, params);
        }

        let config = processing_config(params)?;
        match create_vst3_component_instance(&params.plugin_path, class_id, config) {
            Ok(mut component) => {
                component
                    .instance_mut()
                    .initialize()
                    .map_err(error_message)?;
                component.initialize_controller().map_err(error_message)?;
                let midi_mapping = Vst3MidiMappingCache::from_component(&component);
                component
                    .instance_mut()
                    .setup_processing()
                    .map_err(error_message)?;
                component.instance_mut().activate().map_err(error_message)?;
                Ok(Self::Vst3Runtime(Box::new(Vst3RuntimeBackend {
                    component,
                    midi_mapping,
                })))
            }
            Err(HostError::InvalidClassId(_)) => Self::passthrough(descriptor, params),
            Err(error) => Err(error.to_string()),
        }
    }

    pub(super) fn kind(&self) -> WorkerBackendKind {
        match self {
            Self::Passthrough(_) => WorkerBackendKind::Passthrough,
            Self::Vst3Runtime(_) => WorkerBackendKind::Vst3Runtime,
        }
    }

    pub(super) fn supports_binary_audio_process(&self) -> bool {
        true
    }

    pub(super) fn latency_samples(&self) -> u32 {
        match self {
            Self::Passthrough(_) => 0,
            Self::Vst3Runtime(runtime) => runtime.component.instance().latency_samples(),
        }
    }

    pub(super) fn tail_samples(&self) -> u32 {
        match self {
            Self::Passthrough(_) => 0,
            Self::Vst3Runtime(runtime) => runtime.component.instance().tail_samples(),
        }
    }

    pub(super) fn diagnostics(&self) -> WorkerBackendDiagnostics {
        match self {
            Self::Passthrough(_) => WorkerBackendDiagnostics {
                component_handler: None,
                process_context_requirements: None,
            },
            Self::Vst3Runtime(runtime) => WorkerBackendDiagnostics {
                component_handler: runtime.component.component_handler_snapshot(),
                process_context_requirements: runtime
                    .component
                    .instance()
                    .process_context_requirements(),
            },
        }
    }

    pub(super) fn controller_class_id(&self) -> Option<String> {
        match self {
            Self::Passthrough(_) => None,
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance()
                .controller_class_id()
                .ok()
                .flatten(),
        }
    }

    pub(super) fn parameters(&self) -> Result<Vec<Vst3ParameterInfo>, String> {
        match self {
            Self::Passthrough(_) => Ok(Vec::new()),
            Self::Vst3Runtime(runtime) => runtime.component.parameters().map_err(error_message),
        }
    }

    pub(super) fn unit_metadata(&self) -> Result<Option<Vst3UnitMetadata>, String> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| {
                    controller.unit_info().and_then(|unit_info| {
                        unit_info.map(|unit_info| unit_info.metadata()).transpose()
                    })
                })
                .transpose()
                .map(|value| value.flatten())
                .map_err(error_message),
        }
    }

    pub(super) fn get_param_normalized(&self, id: u32) -> Option<f64> {
        match self {
            Self::Passthrough(_) => None,
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.get_param_normalized(id)),
        }
    }

    pub(super) fn set_param_normalized(&self, id: u32, value: f64) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Err("edit controller not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| "edit controller not available".to_string())
                .and_then(|controller| {
                    controller
                        .set_param_normalized(id, value)
                        .map_err(error_message)
                }),
        }
    }

    pub(super) fn controller_state(&self) -> Result<Option<Vec<u8>>, String> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => {
                runtime.component.controller_state().map_err(error_message)
            }
        }
    }

    pub(super) fn component_state(&self) -> Result<Option<Vec<u8>>, String> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .component_state()
                .map(Some)
                .map_err(error_message),
        }
    }

    pub(super) fn set_controller_state(&self, state: &[u8]) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Err("edit controller not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| "edit controller not available".to_string())
                .and_then(|controller| controller.set_state(state).map_err(error_message)),
        }
    }

    pub(super) fn set_component_state(&mut self, state: &[u8]) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Err("component state not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_component_state(state)
                .map_err(error_message),
        }
    }

    pub(super) fn select_unit(&self, unit_id: i32) -> Result<i32, String> {
        match self {
            Self::Passthrough(_) => Err("unit info not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .select_unit(unit_id)
                .map_err(error_message)?
                .ok_or_else(|| "unit info not available".to_string()),
        }
    }

    pub(super) fn unit_by_audio_bus(
        &self,
        direction: Vst3BusDirection,
        bus_index: i32,
        channel: i32,
    ) -> Result<Option<i32>, String> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .unit_by_audio_bus(direction, bus_index, channel)
                .map_err(error_message),
        }
    }

    pub(super) fn set_unit_program_data(
        &self,
        list_or_unit_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Err("unit info not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_unit_program_data(list_or_unit_id, program_index, data)
                .map_err(error_message)?
                .ok_or_else(|| "unit info not available".to_string()),
        }
    }

    pub(super) fn program_data_supported(&self, list_id: i32) -> Result<bool, String> {
        match self {
            Self::Passthrough(_) => Ok(false),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .program_data_supported(list_id)
                .map(|supported| supported.unwrap_or(false))
                .map_err(error_message),
        }
    }

    pub(super) fn get_program_data(
        &self,
        list_id: i32,
        program_index: i32,
    ) -> Result<Vec<u8>, String> {
        match self {
            Self::Passthrough(_) => Err("program list data not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .get_program_data(list_id, program_index)
                .map_err(error_message)?
                .ok_or_else(|| "program list data not available".to_string()),
        }
    }

    pub(super) fn set_program_data(
        &self,
        list_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Err("program list data not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_program_data(list_id, program_index, data)
                .map_err(error_message)?
                .ok_or_else(|| "program list data not available".to_string()),
        }
    }

    pub(super) fn unit_data_supported(&self, unit_id: i32) -> Result<bool, String> {
        match self {
            Self::Passthrough(_) => Ok(false),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .unit_data_supported(unit_id)
                .map(|supported| supported.unwrap_or(false))
                .map_err(error_message),
        }
    }

    pub(super) fn get_unit_data(&self, unit_id: i32) -> Result<Vec<u8>, String> {
        match self {
            Self::Passthrough(_) => Err("unit data not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .get_unit_data(unit_id)
                .map_err(error_message)?
                .ok_or_else(|| "unit data not available".to_string()),
        }
    }

    pub(super) fn set_unit_data(&self, unit_id: i32, data: &[u8]) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Err("unit data not available".to_string()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_unit_data(unit_id, data)
                .map_err(error_message)?
                .ok_or_else(|| "unit data not available".to_string()),
        }
    }

    pub(super) fn start_processing(&mut self) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Ok(()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .start_processing()
                .map_err(error_message),
        }
    }

    pub(super) fn stop_processing(&mut self) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Ok(()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .stop_processing()
                .map_err(error_message),
        }
    }

    pub(super) fn process_interleaved_f32(
        &mut self,
        frames: usize,
        input: &[f32],
        events: &[Vst3InputEvent],
        parameter_changes: &[Vst3ParameterChange],
        output: &mut [f32],
    ) -> Result<(), String> {
        match self {
            Self::Passthrough(plugin) => plugin
                .process_interleaved_f32(frames, input, output)
                .map(|_| ())
                .map_err(error_message),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .process_interleaved_f32_with_events_and_parameters(
                    frames,
                    input,
                    events,
                    parameter_changes,
                    output,
                )
                .map_err(error_message),
        }
    }

    pub(super) fn midi_controller_param_id(&self, channel: u8, controller: i16) -> Option<u32> {
        match self {
            Self::Passthrough(_) => None,
            Self::Vst3Runtime(runtime) => runtime.midi_mapping.get(channel, controller),
        }
    }

    pub(super) fn terminate(&mut self, processing: bool) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Ok(()),
            Self::Vst3Runtime(runtime) => {
                if processing {
                    runtime
                        .component
                        .instance_mut()
                        .stop_processing()
                        .map_err(error_message)?;
                }
                if runtime.component.instance().state() != Vst3LifecycleState::Terminated {
                    runtime
                        .component
                        .instance_mut()
                        .terminate()
                        .map_err(error_message)?;
                }
                runtime
                    .component
                    .terminate_controller()
                    .map_err(error_message)?;
                Ok(())
            }
        }
    }

    fn passthrough(
        descriptor: &PluginDescriptor,
        params: &InstanceCreateParams,
    ) -> Result<Self, String> {
        HeadlessPluginInstance::new(descriptor, params.input_channels, params.output_channels)
            .map(Self::Passthrough)
            .map_err(error_message)
    }
}

impl Vst3MidiMappingCache {
    fn from_component(component: &Vst3LoadedComponent) -> Self {
        let mut cache = Self {
            assignments: [None; MIDI_MAPPING_SLOT_COUNT],
        };
        let Some(controller) = component.controller() else {
            return cache;
        };
        let Ok(Some(mapping)) = controller.midi_mapping() else {
            return cache;
        };

        for channel in 0..MIDI_MAPPING_CHANNELS {
            for controller_number in 0..MIDI_MAPPING_CONTROLLER_COUNT {
                let Ok(channel_u8) = u8::try_from(channel) else {
                    continue;
                };
                let Ok(controller_i16) = i16::try_from(controller_number) else {
                    continue;
                };
                if let Ok(Some(parameter_id)) = mapping.assignment(0, channel_u8, controller_i16) {
                    cache.set(channel_u8, controller_i16, parameter_id);
                }
            }
        }

        cache
    }

    fn get(&self, channel: u8, controller: i16) -> Option<u32> {
        slot(channel, controller).and_then(|slot| self.assignments[slot])
    }

    fn set(&mut self, channel: u8, controller: i16, parameter_id: u32) {
        if let Some(slot) = slot(channel, controller) {
            self.assignments[slot] = Some(parameter_id);
        }
    }
}

fn slot(channel: u8, controller: i16) -> Option<usize> {
    let controller = usize::try_from(controller).ok()?;
    if usize::from(channel) >= MIDI_MAPPING_CHANNELS || controller >= MIDI_MAPPING_CONTROLLER_COUNT
    {
        return None;
    }

    Some(usize::from(channel) * MIDI_MAPPING_CONTROLLER_COUNT + controller)
}

#[allow(dead_code)]
const _: () = {
    assert!((VST3_MIDI_CONTROLLER_AFTERTOUCH as usize) < MIDI_MAPPING_CONTROLLER_COUNT);
    assert!((VST3_MIDI_CONTROLLER_PITCH_BEND as usize) < MIDI_MAPPING_CONTROLLER_COUNT);
};

fn processing_config(params: &InstanceCreateParams) -> Result<Vst3ProcessingConfig, String> {
    let input_channels = u16::try_from(params.input_channels)
        .map_err(|_| format!("inputChannels exceeds u16: {}", params.input_channels))?;
    let output_channels = u16::try_from(params.output_channels)
        .map_err(|_| format!("outputChannels exceeds u16: {}", params.output_channels))?;

    Vst3ProcessingConfig::new(
        params.sample_rate,
        params.max_block_frames,
        input_channels,
        output_channels,
    )
    .map_err(error_message)
}

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
