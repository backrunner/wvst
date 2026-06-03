use serde::{Deserialize, Serialize};

use crate::{HostError, HostResult};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Vst3LifecycleState {
    Created,
    Initialized,
    SetupDone,
    Activated,
    Processing,
    Stopped,
    Terminated,
}

impl Vst3LifecycleState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Initialized => "initialized",
            Self::SetupDone => "setup-done",
            Self::Activated => "activated",
            Self::Processing => "processing",
            Self::Stopped => "stopped",
            Self::Terminated => "terminated",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ProcessingConfig {
    pub sample_rate: u32,
    pub max_block_frames: u16,
    pub input_channels: u16,
    pub output_channels: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_bus_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_bus_index: Option<i32>,
}

impl Vst3ProcessingConfig {
    pub fn new(
        sample_rate: u32,
        max_block_frames: u16,
        input_channels: u16,
        output_channels: u16,
    ) -> HostResult<Self> {
        if !(8_000..=384_000).contains(&sample_rate) {
            return Err(HostError::InvalidSampleRate(sample_rate));
        }
        if max_block_frames == 0 {
            return Err(HostError::InvalidMaxBlockFrames(max_block_frames));
        }
        if output_channels == 0 {
            return Err(HostError::InvalidChannelCount {
                input: usize::from(input_channels),
                output: usize::from(output_channels),
            });
        }

        Ok(Self {
            sample_rate,
            max_block_frames,
            input_channels,
            output_channels,
            input_bus_index: None,
            output_bus_index: None,
        })
    }

    pub fn with_audio_bus_indices(
        mut self,
        input_bus_index: Option<i32>,
        output_bus_index: Option<i32>,
    ) -> HostResult<Self> {
        validate_bus_index("input", input_bus_index)?;
        validate_bus_index("output", output_bus_index)?;
        self.input_bus_index = input_bus_index;
        self.output_bus_index = output_bus_index;
        Ok(self)
    }
}

fn validate_bus_index(direction: &'static str, index: Option<i32>) -> HostResult<()> {
    if let Some(index) = index
        && index < 0
    {
        return Err(HostError::InvalidAudioBusIndex { direction, index });
    }
    Ok(())
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3Lifecycle {
    state: Vst3LifecycleState,
    processing_config: Option<Vst3ProcessingConfig>,
}

impl Vst3Lifecycle {
    pub fn created() -> Self {
        Self {
            state: Vst3LifecycleState::Created,
            processing_config: None,
        }
    }

    pub const fn state(&self) -> Vst3LifecycleState {
        self.state
    }

    pub const fn processing_config(&self) -> Option<Vst3ProcessingConfig> {
        self.processing_config
    }

    pub fn initialize(&mut self) -> HostResult<()> {
        self.transition("initialize", &[Vst3LifecycleState::Created])?;
        self.state = Vst3LifecycleState::Initialized;
        Ok(())
    }

    pub fn setup_processing(&mut self, config: Vst3ProcessingConfig) -> HostResult<()> {
        self.transition("setup-processing", &[Vst3LifecycleState::Initialized])?;
        self.processing_config = Some(config);
        self.state = Vst3LifecycleState::SetupDone;
        Ok(())
    }

    pub fn activate(&mut self) -> HostResult<()> {
        self.transition("activate", &[Vst3LifecycleState::SetupDone])?;
        self.state = Vst3LifecycleState::Activated;
        Ok(())
    }

    pub fn start_processing(&mut self) -> HostResult<()> {
        self.transition(
            "start-processing",
            &[Vst3LifecycleState::Activated, Vst3LifecycleState::Stopped],
        )?;
        self.state = Vst3LifecycleState::Processing;
        Ok(())
    }

    pub fn stop_processing(&mut self) -> HostResult<()> {
        self.transition("stop-processing", &[Vst3LifecycleState::Processing])?;
        self.state = Vst3LifecycleState::Stopped;
        Ok(())
    }

    pub fn terminate(&mut self) -> HostResult<()> {
        self.transition(
            "terminate",
            &[
                Vst3LifecycleState::Initialized,
                Vst3LifecycleState::SetupDone,
                Vst3LifecycleState::Activated,
                Vst3LifecycleState::Stopped,
            ],
        )?;
        self.state = Vst3LifecycleState::Terminated;
        Ok(())
    }

    fn transition(&self, action: &'static str, allowed: &[Vst3LifecycleState]) -> HostResult<()> {
        if allowed.contains(&self.state) {
            Ok(())
        } else {
            Err(HostError::InvalidLifecycleTransition {
                from: self.state.as_str(),
                action,
            })
        }
    }
}

impl Default for Vst3Lifecycle {
    fn default() -> Self {
        Self::created()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_effect_processing_sequence() {
        let mut lifecycle = Vst3Lifecycle::created();
        let config = Vst3ProcessingConfig::new(48_000, 128, 2, 2).expect("config");

        lifecycle.initialize().expect("initialize");
        lifecycle.setup_processing(config).expect("setup");
        lifecycle.activate().expect("activate");
        lifecycle.start_processing().expect("start");
        lifecycle.stop_processing().expect("stop");
        lifecycle.start_processing().expect("restart");
        lifecycle.stop_processing().expect("stop again");
        lifecycle.terminate().expect("terminate");

        assert_eq!(lifecycle.state(), Vst3LifecycleState::Terminated);
        assert_eq!(lifecycle.processing_config(), Some(config));
    }

    #[test]
    fn rejects_start_before_setup() {
        let mut lifecycle = Vst3Lifecycle::created();

        let error = lifecycle
            .start_processing()
            .expect_err("start before setup");

        assert!(matches!(
            error,
            HostError::InvalidLifecycleTransition {
                from: "created",
                action: "start-processing"
            }
        ));
    }

    #[test]
    fn validates_processing_config() {
        assert!(matches!(
            Vst3ProcessingConfig::new(1, 128, 2, 2),
            Err(HostError::InvalidSampleRate(1))
        ));
        assert!(matches!(
            Vst3ProcessingConfig::new(48_000, 0, 2, 2),
            Err(HostError::InvalidMaxBlockFrames(0))
        ));
    }
}
