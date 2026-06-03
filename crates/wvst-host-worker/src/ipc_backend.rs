use serde::Serialize;
use wvst_vst3_host::{
    VST3_MIDI_CONTROLLER_AFTERTOUCH, VST3_MIDI_CONTROLLER_PITCH_BEND, Vst3BusDirection,
    Vst3ControllerComponentStateSync, Vst3HostMessage, Vst3InputEvent, Vst3LifecycleState,
    Vst3LoadedComponent, Vst3ParameterChange, Vst3ParameterInfo, Vst3ProcessOutput,
    Vst3ProcessOutputDiagnostics, Vst3ProcessingConfig, Vst3TailSamples, Vst3UnitMetadata,
    create_vst3_component_instance,
};

use super::{
    InstanceCreateParams,
    ipc_backend_audio_bus::WorkerAudioBusDiagnostics,
    ipc_backend_error::WorkerBackendError,
    ipc_capabilities::WorkerRuntimeCapabilities,
    ipc_connection::{DecodedMessageAttribute, DecodedMessageAttributeValue},
};

pub(super) enum WorkerBackend {
    Vst3Runtime(Box<Vst3RuntimeBackend>),
    #[cfg(test)]
    TestAudioRuntime(TestAudioRuntimeBackend),
}

pub(super) struct Vst3RuntimeBackend {
    component: Vst3LoadedComponent,
    midi_mapping: Vst3MidiMappingCache,
    capabilities: WorkerRuntimeCapabilities,
    audio_buses: WorkerAudioBusDiagnostics,
}

#[cfg(test)]
pub(super) struct TestAudioRuntimeBackend {
    input_channels: usize,
    output_channels: usize,
    capabilities: WorkerRuntimeCapabilities,
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
    Vst3Runtime,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkerBackendDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    component_handler: Option<wvst_vst3_host::Vst3ComponentHandlerSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    controller_component_state_sync: Option<Vst3ControllerComponentStateSync>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connection_points: Option<WorkerConnectionPointDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_context_requirements: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_buses: Option<WorkerAudioBusDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_output: Option<Vst3ProcessOutputDiagnostics>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkerConnectionPointDiagnostics {
    connected: bool,
}

impl WorkerBackend {
    pub(super) fn from_create_params(
        params: &InstanceCreateParams,
    ) -> Result<Self, WorkerBackendError> {
        let Some(class_id) = params.class_id.as_deref() else {
            return Err(WorkerBackendError::plain(
                "classId is required for VST3 runtime instance create",
            ));
        };

        if !std::path::Path::new(&params.plugin_path).is_dir() {
            return Err(WorkerBackendError::plain(format!(
                "pluginPath must point to a VST3 bundle directory: {}",
                params.plugin_path
            )));
        }

        let config = processing_config(params).map_err(WorkerBackendError::plain)?;
        match create_vst3_component_instance(&params.plugin_path, class_id, config) {
            Ok(mut component) => {
                component.instance_mut().initialize().map_err(|error| {
                    WorkerBackendError::vst3_init("component.initialize", error)
                })?;
                component.initialize_controller().map_err(|error| {
                    WorkerBackendError::vst3_init("controller.initialize", error)
                })?;
                let midi_mapping = Vst3MidiMappingCache::from_component(&component);
                let capabilities = WorkerRuntimeCapabilities::from_vst3_component(
                    &component,
                    !midi_mapping.is_empty(),
                );
                component
                    .instance_mut()
                    .setup_processing()
                    .map_err(|error| {
                        WorkerBackendError::vst3_init("component.setup-processing", error)
                    })?;
                let audio_buses = component
                    .instance_mut()
                    .selected_audio_buses()
                    .map(WorkerAudioBusDiagnostics::from)
                    .map_err(|error| {
                        WorkerBackendError::vst3_init("component.audio-buses", error)
                    })?;
                component
                    .instance_mut()
                    .activate()
                    .map_err(|error| WorkerBackendError::vst3_init("component.activate", error))?;
                Ok(Self::Vst3Runtime(Box::new(Vst3RuntimeBackend {
                    component,
                    midi_mapping,
                    capabilities,
                    audio_buses,
                })))
            }
            Err(error) => Err(WorkerBackendError::vst3_init("component.create", error)),
        }
    }

    pub(super) fn kind(&self) -> WorkerBackendKind {
        match self {
            Self::Vst3Runtime(_) => WorkerBackendKind::Vst3Runtime,
            #[cfg(test)]
            Self::TestAudioRuntime(_) => WorkerBackendKind::Vst3Runtime,
        }
    }

    #[cfg(test)]
    pub(super) fn test_audio_runtime(input_channels: usize, output_channels: usize) -> Self {
        Self::TestAudioRuntime(TestAudioRuntimeBackend {
            input_channels,
            output_channels,
            capabilities: test_audio_runtime_capabilities(),
        })
    }

    pub(super) fn supports_binary_audio_process(&self) -> bool {
        true
    }

    pub(super) fn capabilities(&self) -> WorkerRuntimeCapabilities {
        match self {
            Self::Vst3Runtime(runtime) => runtime.capabilities.clone(),
            #[cfg(test)]
            Self::TestAudioRuntime(runtime) => runtime.capabilities.clone(),
        }
    }

    pub(super) fn latency_samples(&self) -> u32 {
        match self {
            Self::Vst3Runtime(runtime) => runtime.component.instance().latency_samples(),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => 0,
        }
    }

    pub(super) fn tail_samples(&self) -> u32 {
        match self {
            Self::Vst3Runtime(runtime) => runtime.component.instance().tail_samples(),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => 0,
        }
    }

    pub(super) fn tail_info(&self) -> Vst3TailSamples {
        match self {
            Self::Vst3Runtime(runtime) => runtime.component.instance().tail_info(),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Vst3TailSamples::from_raw(0),
        }
    }

    pub(super) fn diagnostics_with_process_output(
        &self,
        process_output: Option<&Vst3ProcessOutput>,
    ) -> WorkerBackendDiagnostics {
        match self {
            Self::Vst3Runtime(runtime) => WorkerBackendDiagnostics {
                component_handler: runtime.component.component_handler_snapshot(),
                controller_component_state_sync: runtime
                    .component
                    .controller_component_state_sync()
                    .cloned(),
                connection_points: Some(WorkerConnectionPointDiagnostics {
                    connected: runtime.component.connection_points_connected(),
                }),
                process_context_requirements: runtime
                    .component
                    .instance()
                    .process_context_requirements(),
                audio_buses: Some(runtime.audio_buses.clone()),
                process_output: process_output.map(|output| output.diagnostics),
            },
            #[cfg(test)]
            Self::TestAudioRuntime(_) => WorkerBackendDiagnostics {
                component_handler: None,
                controller_component_state_sync: None,
                connection_points: None,
                process_context_requirements: None,
                audio_buses: None,
                process_output: process_output.map(|output| output.diagnostics),
            },
        }
    }

    pub(super) fn controller_class_id(&self) -> Option<String> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance()
                .controller_class_id()
                .ok()
                .flatten(),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(super) fn parameters(&self) -> Result<Vec<Vst3ParameterInfo>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .parameters()
                .map_err(|error| WorkerBackendError::vst3_control("controller.parameters", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(Vec::new()),
        }
    }

    pub(super) fn unit_metadata(&self) -> Result<Option<Vst3UnitMetadata>, WorkerBackendError> {
        match self {
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
                .map_err(|error| WorkerBackendError::vst3_control("controller.unit-info", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(super) fn get_param_normalized(&self, id: u32) -> Option<f64> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.get_param_normalized(id)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(super) fn param_string_by_value(
        &self,
        id: u32,
        value_normalized: f64,
    ) -> Result<Option<String>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller
                        .param_string_by_value(id, value_normalized)
                        .map_err(|error| {
                            WorkerBackendError::vst3_control(
                                "controller.get-param-string-by-value",
                                error,
                            )
                        })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(super) fn param_value_by_string(
        &self,
        id: u32,
        value: &str,
    ) -> Result<f64, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller
                        .param_value_by_string(id, value)
                        .map_err(|error| {
                            WorkerBackendError::vst3_control(
                                "controller.get-param-value-by-string",
                                error,
                            )
                        })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(super) fn normalized_param_to_plain(&self, id: u32, value_normalized: f64) -> Option<f64> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.normalized_param_to_plain(id, value_normalized)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(super) fn plain_param_to_normalized(&self, id: u32, plain_value: f64) -> Option<f64> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.plain_param_to_normalized(id, plain_value)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(super) fn set_param_normalized(
        &self,
        id: u32,
        value: f64,
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.set_param_normalized(id, value).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.set-param-normalized", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(super) fn begin_param_edit(&mut self, id: u32) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.begin_edit(id).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.begin-edit", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(super) fn perform_param_edit(
        &mut self,
        id: u32,
        value: f64,
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.perform_edit(id, value).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.perform-edit", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(super) fn end_param_edit(&mut self, id: u32) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.end_edit(id).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.end-edit", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(super) fn controller_state(&self) -> Result<Option<Vec<u8>>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_state()
                .map_err(|error| WorkerBackendError::vst3_control("controller.get-state", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(super) fn component_state(&self) -> Result<Option<Vec<u8>>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .component_state()
                .map(Some)
                .map_err(|error| WorkerBackendError::vst3_control("component.get-state", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(super) fn set_controller_state(&self, state: &[u8]) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.set_state(state).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.set-state", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(super) fn set_component_state(&mut self, state: &[u8]) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_component_state(state)
                .map_err(|error| WorkerBackendError::vst3_control("component.set-state", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("component state not available"))
            }
        }
    }

    pub(super) fn notify_component(
        &mut self,
        message_id: &str,
        attributes: &[DecodedMessageAttribute],
    ) -> Result<Option<()>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => notify_connection_point(
                &mut runtime.component,
                ConnectionNotifyTarget::Component,
                message_id,
                attributes,
            ),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(super) fn notify_controller(
        &mut self,
        message_id: &str,
        attributes: &[DecodedMessageAttribute],
    ) -> Result<Option<()>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => notify_connection_point(
                &mut runtime.component,
                ConnectionNotifyTarget::Controller,
                message_id,
                attributes,
            ),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(super) fn select_unit(&self, unit_id: i32) -> Result<i32, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .select_unit(unit_id)
                .map_err(|error| WorkerBackendError::vst3_control("controller.select-unit", error))?
                .ok_or_else(|| WorkerBackendError::plain("unit info not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Err(WorkerBackendError::plain("unit info not available")),
        }
    }

    pub(super) fn unit_by_audio_bus(
        &self,
        direction: Vst3BusDirection,
        bus_index: i32,
        channel: i32,
    ) -> Result<Option<i32>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .unit_by_audio_bus(direction, bus_index, channel)
                .map_err(|error| WorkerBackendError::vst3_control("controller.unit-by-bus", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(super) fn set_unit_program_data(
        &self,
        list_or_unit_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_unit_program_data(list_or_unit_id, program_index, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("controller.set-unit-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit info not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Err(WorkerBackendError::plain("unit info not available")),
        }
    }

    pub(super) fn program_data_supported(&self, list_id: i32) -> Result<bool, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .program_data_supported(list_id)
                .map(|supported| supported.unwrap_or(false))
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.program-data-supported", error)
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(false),
        }
    }

    pub(super) fn get_program_data(
        &self,
        list_id: i32,
        program_index: i32,
    ) -> Result<Vec<u8>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .get_program_data(list_id, program_index)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.get-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("program list data not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("program list data not available"))
            }
        }
    }

    pub(super) fn set_program_data(
        &self,
        list_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_program_data(list_id, program_index, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.set-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("program list data not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("program list data not available"))
            }
        }
    }

    pub(super) fn unit_data_supported(&self, unit_id: i32) -> Result<bool, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .unit_data_supported(unit_id)
                .map(|supported| supported.unwrap_or(false))
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.unit-data-supported", error)
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(false),
        }
    }

    pub(super) fn get_unit_data(&self, unit_id: i32) -> Result<Vec<u8>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .get_unit_data(unit_id)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.get-unit-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit data not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Err(WorkerBackendError::plain("unit data not available")),
        }
    }

    pub(super) fn set_unit_data(
        &self,
        unit_id: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_unit_data(unit_id, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.set-unit-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit data not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Err(WorkerBackendError::plain("unit data not available")),
        }
    }

    pub(super) fn start_processing(&mut self) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .start_processing()
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.start-processing", error)
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(()),
        }
    }

    pub(super) fn stop_processing(&mut self) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .stop_processing()
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.stop-processing", error)
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(()),
        }
    }

    pub(super) fn process_interleaved_f32(
        &mut self,
        frames: usize,
        input: &[f32],
        events: &[Vst3InputEvent],
        parameter_changes: &[Vst3ParameterChange],
        output: &mut [f32],
        process_output: &mut Vst3ProcessOutput,
    ) -> Result<(), WorkerBackendError> {
        process_output.clear();
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .process_interleaved_f32_with_io_events_and_parameters(
                    frames,
                    input,
                    events,
                    parameter_changes,
                    output,
                    process_output,
                )
                .map_err(|error| WorkerBackendError::vst3_process("component.process", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(runtime) => {
                process_test_audio_runtime(runtime, frames, input, output)
            }
        }
    }

    pub(super) fn midi_controller_param_id(&self, channel: u8, controller: i16) -> Option<u32> {
        match self {
            Self::Vst3Runtime(runtime) => runtime.midi_mapping.get(channel, controller),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(super) fn terminate(&mut self, processing: bool) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => {
                if processing {
                    runtime
                        .component
                        .instance_mut()
                        .stop_processing()
                        .map_err(|error| {
                            WorkerBackendError::vst3_control("component.stop-processing", error)
                        })?;
                }
                if runtime.component.instance().state() != Vst3LifecycleState::Terminated {
                    runtime
                        .component
                        .instance_mut()
                        .terminate()
                        .map_err(|error| {
                            WorkerBackendError::vst3_control("component.terminate", error)
                        })?;
                }
                runtime.component.terminate_controller().map_err(|error| {
                    WorkerBackendError::vst3_control("controller.terminate", error)
                })?;
                Ok(())
            }
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(()),
        }
    }
}

#[cfg(test)]
fn test_audio_runtime_capabilities() -> WorkerRuntimeCapabilities {
    WorkerRuntimeCapabilities {
        schema_version: 2,
        binary_audio_process: true,
        component_state: false,
        controller: false,
        controller_state: false,
        parameters: false,
        parameter_automation: true,
        units: false,
        unit_program_data: false,
        program_list_data: false,
        unit_data: false,
        midi_mapping: false,
        output_events: true,
        output_parameter_changes: true,
        component_handler_events: false,
        connection_points: false,
        process_context: false,
        unavailable: Vec::new(),
    }
}

#[cfg(test)]
fn process_test_audio_runtime(
    runtime: &TestAudioRuntimeBackend,
    frames: usize,
    input: &[f32],
    output: &mut [f32],
) -> Result<(), WorkerBackendError> {
    let expected_input = frames.saturating_mul(runtime.input_channels);
    let expected_output = frames.saturating_mul(runtime.output_channels);
    if input.len() != expected_input {
        return Err(WorkerBackendError::plain(format!(
            "invalid test audio input buffer length: expected {expected_input}, got {}",
            input.len()
        )));
    }
    if output.len() != expected_output {
        return Err(WorkerBackendError::plain(format!(
            "invalid test audio output buffer length: expected {expected_output}, got {}",
            output.len()
        )));
    }

    for frame in 0..frames {
        for channel in 0..runtime.output_channels {
            let output_index = frame * runtime.output_channels + channel;
            output[output_index] = if channel < runtime.input_channels {
                input[frame * runtime.input_channels + channel]
            } else {
                0.0
            };
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum ConnectionNotifyTarget {
    Component,
    Controller,
}

fn notify_connection_point(
    component: &mut Vst3LoadedComponent,
    target: ConnectionNotifyTarget,
    message_id: &str,
    attributes: &[DecodedMessageAttribute],
) -> Result<Option<()>, WorkerBackendError> {
    let mut message = Vst3HostMessage::new();
    message
        .set_id(message_id)
        .map_err(WorkerBackendError::plain)?;
    for attribute in attributes {
        match &attribute.value {
            DecodedMessageAttributeValue::Int(value) => {
                message.attributes_mut().set_int(&attribute.key, *value)
            }
            DecodedMessageAttributeValue::Float(value) => {
                message.attributes_mut().set_float(&attribute.key, *value)
            }
            DecodedMessageAttributeValue::String(value) => {
                message.attributes_mut().set_string(&attribute.key, value)
            }
            DecodedMessageAttributeValue::Binary(value) => {
                message.attributes_mut().set_binary(&attribute.key, value)
            }
        }
        .map_err(WorkerBackendError::plain)?;
    }
    match target {
        ConnectionNotifyTarget::Component => {
            component.notify_component(&mut message).map_err(|error| {
                WorkerBackendError::vst3_control("connection-point.notify-component", error)
            })
        }
        ConnectionNotifyTarget::Controller => {
            component.notify_controller(&mut message).map_err(|error| {
                WorkerBackendError::vst3_control("connection-point.notify-controller", error)
            })
        }
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

    fn is_empty(&self) -> bool {
        self.assignments.iter().all(Option::is_none)
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
