use serde::Serialize;
use wvst_scanner::PluginDescriptor;
use wvst_vst3_host::{
    HeadlessPluginInstance, HostError, Vst3InputEvent, Vst3LifecycleState, Vst3LoadedComponent,
    Vst3ProcessingConfig, create_vst3_component_instance,
};

use super::InstanceCreateParams;

pub(super) enum WorkerBackend {
    Passthrough(HeadlessPluginInstance),
    Vst3Runtime(Vst3LoadedComponent),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum WorkerBackendKind {
    Passthrough,
    Vst3Runtime,
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
                component
                    .instance_mut()
                    .setup_processing()
                    .map_err(error_message)?;
                component.instance_mut().activate().map_err(error_message)?;
                Ok(Self::Vst3Runtime(component))
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
            Self::Vst3Runtime(component) => component.instance().latency_samples(),
        }
    }

    pub(super) fn tail_samples(&self) -> u32 {
        match self {
            Self::Passthrough(_) => 0,
            Self::Vst3Runtime(component) => component.instance().tail_samples(),
        }
    }

    pub(super) fn controller_class_id(&self) -> Option<String> {
        match self {
            Self::Passthrough(_) => None,
            Self::Vst3Runtime(component) => {
                component.instance().controller_class_id().ok().flatten()
            }
        }
    }

    pub(super) fn start_processing(&mut self) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Ok(()),
            Self::Vst3Runtime(component) => component
                .instance_mut()
                .start_processing()
                .map_err(error_message),
        }
    }

    pub(super) fn stop_processing(&mut self) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Ok(()),
            Self::Vst3Runtime(component) => component
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
        output: &mut [f32],
    ) -> Result<(), String> {
        match self {
            Self::Passthrough(plugin) => plugin
                .process_interleaved_f32(frames, input, output)
                .map(|_| ())
                .map_err(error_message),
            Self::Vst3Runtime(component) => component
                .instance_mut()
                .process_interleaved_f32_with_events(frames, input, events, output)
                .map_err(error_message),
        }
    }

    pub(super) fn terminate(&mut self, processing: bool) -> Result<(), String> {
        match self {
            Self::Passthrough(_) => Ok(()),
            Self::Vst3Runtime(component) => {
                if processing {
                    component
                        .instance_mut()
                        .stop_processing()
                        .map_err(error_message)?;
                }
                if component.instance().state() != Vst3LifecycleState::Terminated {
                    component
                        .instance_mut()
                        .terminate()
                        .map_err(error_message)?;
                }
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
