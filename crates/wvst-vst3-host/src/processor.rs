use serde::{Deserialize, Serialize};
use wvst_scanner::PluginDescriptor;

use crate::{HeadlessPluginMetadata, HostError, HostResult};

#[derive(Debug, Clone)]
pub struct HeadlessPluginInstance {
    metadata: HeadlessPluginMetadata,
    input_channels: usize,
    output_channels: usize,
}

impl HeadlessPluginInstance {
    pub fn new(
        descriptor: &PluginDescriptor,
        input_channels: usize,
        output_channels: usize,
    ) -> HostResult<Self> {
        if output_channels == 0 {
            return Err(HostError::InvalidChannelCount {
                input: input_channels,
                output: output_channels,
            });
        }

        Ok(Self {
            metadata: descriptor.into(),
            input_channels,
            output_channels,
        })
    }

    pub fn metadata(&self) -> &HeadlessPluginMetadata {
        &self.metadata
    }

    pub fn process_interleaved_f32(
        &self,
        frames: usize,
        input: &[f32],
        output: &mut [f32],
    ) -> HostResult<ProcessStats> {
        let expected_input = frames * self.input_channels;
        let expected_output = frames * self.output_channels;

        if input.len() != expected_input {
            return Err(HostError::InvalidBufferLength {
                expected: expected_input,
                actual: input.len(),
            });
        }

        if output.len() != expected_output {
            return Err(HostError::InvalidBufferLength {
                expected: expected_output,
                actual: output.len(),
            });
        }

        for frame in 0..frames {
            for channel in 0..self.output_channels {
                let output_index = frame * self.output_channels + channel;
                output[output_index] = if channel < self.input_channels {
                    input[frame * self.input_channels + channel]
                } else {
                    0.0
                };
            }
        }

        Ok(ProcessStats {
            frames,
            input_channels: self.input_channels,
            output_channels: self.output_channels,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessStats {
    pub frames: usize,
    pub input_channels: usize,
    pub output_channels: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wvst_scanner::{MetadataSource, PluginClass, PluginDescriptor, PluginFormat};

    #[test]
    fn passthrough_processes_two_channels() {
        let descriptor = descriptor();
        let instance = HeadlessPluginInstance::new(&descriptor, 2, 2).expect("instance");
        let input = [0.1, 0.2, 0.3, 0.4];
        let mut output = [0.0; 4];

        let stats = instance
            .process_interleaved_f32(2, &input, &mut output)
            .expect("process");

        assert_eq!(stats.frames, 2);
        assert_eq!(output, input);
    }

    #[test]
    fn passthrough_supports_zero_input_instruments() {
        let descriptor = descriptor();
        let instance = HeadlessPluginInstance::new(&descriptor, 0, 2).expect("instance");
        let mut output = [1.0; 4];

        let stats = instance
            .process_interleaved_f32(2, &[], &mut output)
            .expect("process");

        assert_eq!(stats.input_channels, 0);
        assert_eq!(stats.output_channels, 2);
        assert_eq!(output, [0.0; 4]);
    }

    fn descriptor() -> PluginDescriptor {
        PluginDescriptor {
            plugin_id: "vst3:test".to_string(),
            format: PluginFormat::Vst3,
            name: "Test".to_string(),
            vendor: None,
            version: None,
            path: "/tmp/Test.vst3".to_string(),
            classes: vec![PluginClass {
                class_id: None,
                name: "Test".to_string(),
                category: Some("Fx".to_string()),
                subcategories: Vec::new(),
            }],
            metadata_source: MetadataSource::BundleName,
        }
    }
}
