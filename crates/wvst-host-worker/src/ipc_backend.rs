use serde::Serialize;
use wvst_scanner::PluginDescriptor;
use wvst_vst3_host::{
    HeadlessPluginInstance, HostError, VST3_MIDI_CONTROLLER_AFTERTOUCH,
    VST3_MIDI_CONTROLLER_PITCH_BEND, Vst3BusDirection, Vst3HostMessage, Vst3InputEvent,
    Vst3LifecycleState, Vst3LoadedComponent, Vst3ParameterChange, Vst3ParameterInfo,
    Vst3ProcessOutput, Vst3ProcessingConfig, Vst3UnitMetadata, create_vst3_component_instance,
};

use super::{
    InstanceCreateParams,
    ipc_backend_error::WorkerBackendError,
    ipc_capabilities::WorkerRuntimeCapabilities,
    ipc_connection::{DecodedMessageAttribute, DecodedMessageAttributeValue},
};

pub(super) enum WorkerBackend {
    Passthrough(PassthroughBackend),
    Vst3Runtime(Box<Vst3RuntimeBackend>),
}

pub(super) struct PassthroughBackend {
    plugin: HeadlessPluginInstance,
    reason: PassthroughFallbackReason,
}

pub(super) struct Vst3RuntimeBackend {
    component: Vst3LoadedComponent,
    midi_mapping: Vst3MidiMappingCache,
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
    Passthrough,
    Vst3Runtime,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkerBackendDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    passthrough_reason: Option<PassthroughFallbackReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    component_handler: Option<wvst_vst3_host::Vst3ComponentHandlerSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_context_requirements: Option<u32>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PassthroughFallbackReason {
    kind: PassthroughFallbackKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PassthroughFallbackKind {
    MissingClassId,
    NonBundlePath,
    InvalidClassId,
}

impl WorkerBackend {
    pub(super) fn from_create_params(
        params: &InstanceCreateParams,
        descriptor: &PluginDescriptor,
    ) -> Result<Self, WorkerBackendError> {
        let Some(class_id) = params.class_id.as_deref() else {
            return Self::passthrough(
                descriptor,
                params,
                PassthroughFallbackReason::missing_class_id(),
            );
        };

        if !std::path::Path::new(&params.plugin_path).is_dir() {
            return Self::passthrough(
                descriptor,
                params,
                PassthroughFallbackReason::non_bundle_path(&params.plugin_path),
            );
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
                component
                    .instance_mut()
                    .activate()
                    .map_err(|error| WorkerBackendError::vst3_init("component.activate", error))?;
                Ok(Self::Vst3Runtime(Box::new(Vst3RuntimeBackend {
                    component,
                    midi_mapping,
                    capabilities,
                })))
            }
            Err(HostError::InvalidClassId(class_id)) => Self::passthrough(
                descriptor,
                params,
                PassthroughFallbackReason::invalid_class_id(class_id),
            ),
            Err(error) => Err(WorkerBackendError::vst3_init("component.create", error)),
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

    pub(super) fn capabilities(&self) -> WorkerRuntimeCapabilities {
        match self {
            Self::Passthrough(_) => WorkerRuntimeCapabilities::passthrough(),
            Self::Vst3Runtime(runtime) => runtime.capabilities,
        }
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
                passthrough_reason: self.passthrough_reason(),
                component_handler: None,
                process_context_requirements: None,
            },
            Self::Vst3Runtime(runtime) => WorkerBackendDiagnostics {
                passthrough_reason: None,
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

    pub(super) fn parameters(&self) -> Result<Vec<Vst3ParameterInfo>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(Vec::new()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .parameters()
                .map_err(|error| WorkerBackendError::vst3_control("controller.parameters", error)),
        }
    }

    pub(super) fn unit_metadata(&self) -> Result<Option<Vst3UnitMetadata>, WorkerBackendError> {
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
                .map_err(|error| WorkerBackendError::vst3_control("controller.unit-info", error)),
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

    pub(super) fn param_string_by_value(
        &self,
        id: u32,
        value_normalized: f64,
    ) -> Result<Option<String>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("edit controller not available")),
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
        }
    }

    pub(super) fn param_value_by_string(
        &self,
        id: u32,
        value: &str,
    ) -> Result<f64, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("edit controller not available")),
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
        }
    }

    pub(super) fn normalized_param_to_plain(&self, id: u32, value_normalized: f64) -> Option<f64> {
        match self {
            Self::Passthrough(_) => None,
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.normalized_param_to_plain(id, value_normalized)),
        }
    }

    pub(super) fn plain_param_to_normalized(&self, id: u32, plain_value: f64) -> Option<f64> {
        match self {
            Self::Passthrough(_) => None,
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.plain_param_to_normalized(id, plain_value)),
        }
    }

    pub(super) fn set_param_normalized(
        &self,
        id: u32,
        value: f64,
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("edit controller not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.set_param_normalized(id, value).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.set-param-normalized", error)
                    })
                }),
        }
    }

    pub(super) fn begin_param_edit(&mut self, id: u32) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("edit controller not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.begin_edit(id).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.begin-edit", error)
                    })
                }),
        }
    }

    pub(super) fn perform_param_edit(
        &mut self,
        id: u32,
        value: f64,
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("edit controller not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.perform_edit(id, value).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.perform-edit", error)
                    })
                }),
        }
    }

    pub(super) fn end_param_edit(&mut self, id: u32) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("edit controller not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.end_edit(id).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.end-edit", error)
                    })
                }),
        }
    }

    pub(super) fn controller_state(&self) -> Result<Option<Vec<u8>>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_state()
                .map_err(|error| WorkerBackendError::vst3_control("controller.get-state", error)),
        }
    }

    pub(super) fn component_state(&self) -> Result<Option<Vec<u8>>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .component_state()
                .map(Some)
                .map_err(|error| WorkerBackendError::vst3_control("component.get-state", error)),
        }
    }

    pub(super) fn set_controller_state(&self, state: &[u8]) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("edit controller not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.set_state(state).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.set-state", error)
                    })
                }),
        }
    }

    pub(super) fn set_component_state(&mut self, state: &[u8]) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("component state not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_component_state(state)
                .map_err(|error| WorkerBackendError::vst3_control("component.set-state", error)),
        }
    }

    pub(super) fn notify_component(
        &mut self,
        message_id: &str,
        attributes: &[DecodedMessageAttribute],
    ) -> Result<Option<()>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => notify_connection_point(
                &mut runtime.component,
                ConnectionNotifyTarget::Component,
                message_id,
                attributes,
            ),
        }
    }

    pub(super) fn notify_controller(
        &mut self,
        message_id: &str,
        attributes: &[DecodedMessageAttribute],
    ) -> Result<Option<()>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => notify_connection_point(
                &mut runtime.component,
                ConnectionNotifyTarget::Controller,
                message_id,
                attributes,
            ),
        }
    }

    pub(super) fn select_unit(&self, unit_id: i32) -> Result<i32, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("unit info not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .select_unit(unit_id)
                .map_err(|error| WorkerBackendError::vst3_control("controller.select-unit", error))?
                .ok_or_else(|| WorkerBackendError::plain("unit info not available")),
        }
    }

    pub(super) fn unit_by_audio_bus(
        &self,
        direction: Vst3BusDirection,
        bus_index: i32,
        channel: i32,
    ) -> Result<Option<i32>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(None),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .unit_by_audio_bus(direction, bus_index, channel)
                .map_err(|error| WorkerBackendError::vst3_control("controller.unit-by-bus", error)),
        }
    }

    pub(super) fn set_unit_program_data(
        &self,
        list_or_unit_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("unit info not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_unit_program_data(list_or_unit_id, program_index, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("controller.set-unit-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit info not available")),
        }
    }

    pub(super) fn program_data_supported(&self, list_id: i32) -> Result<bool, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(false),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .program_data_supported(list_id)
                .map(|supported| supported.unwrap_or(false))
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.program-data-supported", error)
                }),
        }
    }

    pub(super) fn get_program_data(
        &self,
        list_id: i32,
        program_index: i32,
    ) -> Result<Vec<u8>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => {
                Err(WorkerBackendError::plain("program list data not available"))
            }
            Self::Vst3Runtime(runtime) => runtime
                .component
                .get_program_data(list_id, program_index)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.get-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("program list data not available")),
        }
    }

    pub(super) fn set_program_data(
        &self,
        list_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => {
                Err(WorkerBackendError::plain("program list data not available"))
            }
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_program_data(list_id, program_index, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.set-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("program list data not available")),
        }
    }

    pub(super) fn unit_data_supported(&self, unit_id: i32) -> Result<bool, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(false),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .unit_data_supported(unit_id)
                .map(|supported| supported.unwrap_or(false))
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.unit-data-supported", error)
                }),
        }
    }

    pub(super) fn get_unit_data(&self, unit_id: i32) -> Result<Vec<u8>, WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("unit data not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .get_unit_data(unit_id)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.get-unit-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit data not available")),
        }
    }

    pub(super) fn set_unit_data(
        &self,
        unit_id: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Err(WorkerBackendError::plain("unit data not available")),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_unit_data(unit_id, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.set-unit-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit data not available")),
        }
    }

    pub(super) fn start_processing(&mut self) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .start_processing()
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.start-processing", error)
                }),
        }
    }

    pub(super) fn stop_processing(&mut self) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(()),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .stop_processing()
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.stop-processing", error)
                }),
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
            Self::Passthrough(passthrough) => passthrough
                .plugin
                .process_interleaved_f32(frames, input, output)
                .map(|_| ())
                .map_err(WorkerBackendError::plain),
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
        }
    }

    pub(super) fn midi_controller_param_id(&self, channel: u8, controller: i16) -> Option<u32> {
        match self {
            Self::Passthrough(_) => None,
            Self::Vst3Runtime(runtime) => runtime.midi_mapping.get(channel, controller),
        }
    }

    pub(super) fn terminate(&mut self, processing: bool) -> Result<(), WorkerBackendError> {
        match self {
            Self::Passthrough(_) => Ok(()),
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
        }
    }

    fn passthrough(
        descriptor: &PluginDescriptor,
        params: &InstanceCreateParams,
        reason: PassthroughFallbackReason,
    ) -> Result<Self, WorkerBackendError> {
        HeadlessPluginInstance::new(descriptor, params.input_channels, params.output_channels)
            .map(|plugin| Self::Passthrough(PassthroughBackend { plugin, reason }))
            .map_err(WorkerBackendError::plain)
    }

    fn passthrough_reason(&self) -> Option<PassthroughFallbackReason> {
        match self {
            Self::Passthrough(passthrough) => Some(passthrough.reason.clone()),
            Self::Vst3Runtime(_) => None,
        }
    }
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

impl PassthroughFallbackReason {
    fn missing_class_id() -> Self {
        Self {
            kind: PassthroughFallbackKind::MissingClassId,
            message: Some("classId was not provided".to_string()),
        }
    }

    fn non_bundle_path(path: &str) -> Self {
        Self {
            kind: PassthroughFallbackKind::NonBundlePath,
            message: Some(format!("pluginPath is not a directory bundle: {path}")),
        }
    }

    fn invalid_class_id(class_id: String) -> Self {
        Self {
            kind: PassthroughFallbackKind::InvalidClassId,
            message: Some(format!("classId is not a valid VST3 FUID: {class_id}")),
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
