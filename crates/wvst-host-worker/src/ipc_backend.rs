use serde::Serialize;
use wvst_vst3_host::{
    VST3_MIDI_CONTROLLER_AFTERTOUCH, VST3_MIDI_CONTROLLER_PITCH_BEND,
    Vst3ControllerComponentStateSync, Vst3HostMessage, Vst3LoadedComponent, Vst3ProcessOutput,
    Vst3ProcessOutputDiagnostics, Vst3ProcessingConfig, Vst3TailSamples,
    create_vst3_component_instance,
};

use super::{
    InstanceCreateParams,
    ipc_backend_audio_bus::WorkerAudioBusDiagnostics,
    ipc_backend_error::WorkerBackendError,
    ipc_capabilities::WorkerRuntimeCapabilities,
    ipc_connection::{DecodedMessageAttribute, DecodedMessageAttributeValue},
};

#[path = "ipc_backend/ops.rs"]
mod ops;

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
    .and_then(|config| {
        config.with_audio_bus_indices(params.input_bus_index, params.output_bus_index)
    })
    .map_err(error_message)
}

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processing_config_carries_explicit_audio_bus_indices() {
        let config = processing_config(&InstanceCreateParams {
            instance_id: 7,
            stream_id: 9,
            plugin_path: "/tmp/Test.vst3".to_string(),
            class_id: Some("class-a".to_string()),
            sample_rate: 48_000,
            max_block_frames: 128,
            input_channels: 2,
            output_channels: 2,
            input_bus_index: Some(1),
            output_bus_index: Some(3),
        })
        .expect("processing config");

        assert_eq!(config.input_bus_index, Some(1));
        assert_eq!(config.output_bus_index, Some(3));
    }
}
