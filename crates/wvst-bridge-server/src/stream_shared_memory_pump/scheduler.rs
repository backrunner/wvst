use serde::Serialize;

use crate::stream_shared_memory::SharedMemoryStreamStatus;

use super::SharedMemoryPumpTickOutcome;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SharedMemoryPumpSchedulingMode {
    Fixed,
    Adaptive,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpSchedulingConfig {
    pub mode: SharedMemoryPumpSchedulingMode,
    pub base_interval_micros: u64,
    pub min_interval_micros: u64,
    pub max_interval_micros: u64,
    pub idle_backoff_micros: u64,
    pub target_input_frames: u64,
    pub target_output_headroom_frames: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpScheduleStatus {
    pub config: SharedMemoryPumpSchedulingConfig,
    pub current_interval_micros: u64,
    pub last_delay_micros: u64,
    pub last_ring_status: Option<SharedMemoryPumpRingStatus>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpRingStatus {
    pub input_channels: u16,
    pub output_channels: u16,
    pub input_readable_frames: u64,
    pub input_writable_frames: u64,
    pub output_readable_frames: u64,
    pub output_writable_frames: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SharedMemoryPumpPreflightSkip {
    pub outcome: SharedMemoryPumpTickOutcome,
    pub requested_frames: u64,
    pub available_frames: u64,
}

impl SharedMemoryPumpSchedulingConfig {
    pub const fn fixed(base_interval_micros: u64, frames: u16) -> Self {
        Self {
            mode: SharedMemoryPumpSchedulingMode::Fixed,
            base_interval_micros,
            min_interval_micros: base_interval_micros,
            max_interval_micros: base_interval_micros,
            idle_backoff_micros: base_interval_micros,
            target_input_frames: frames as u64,
            target_output_headroom_frames: frames as u64,
        }
    }

    pub const fn adaptive(
        base_interval_micros: u64,
        min_interval_micros: u64,
        max_interval_micros: u64,
        idle_backoff_micros: u64,
        frames: u16,
    ) -> Self {
        Self {
            mode: SharedMemoryPumpSchedulingMode::Adaptive,
            base_interval_micros,
            min_interval_micros,
            max_interval_micros,
            idle_backoff_micros,
            target_input_frames: frames as u64,
            target_output_headroom_frames: frames as u64,
        }
    }

    pub fn next_delay_micros(
        self,
        outcome: SharedMemoryPumpTickOutcome,
        ring: Option<&SharedMemoryPumpRingStatus>,
    ) -> u64 {
        if self.mode == SharedMemoryPumpSchedulingMode::Fixed {
            return self.base_interval_micros;
        }

        let desired = match outcome {
            SharedMemoryPumpTickOutcome::InputUnderrun => self.idle_backoff_micros,
            SharedMemoryPumpTickOutcome::OutputBackpressure => self.max_interval_micros,
            SharedMemoryPumpTickOutcome::WorkerError => self.base_interval_micros,
            SharedMemoryPumpTickOutcome::Success => self.delay_from_ring(ring),
        };
        desired.clamp(self.min_interval_micros, self.max_interval_micros)
    }

    pub fn preflight_skip(
        self,
        ring: &SharedMemoryPumpRingStatus,
    ) -> Option<SharedMemoryPumpPreflightSkip> {
        if self.mode == SharedMemoryPumpSchedulingMode::Fixed {
            return None;
        }
        if ring.output_writable_frames < self.target_output_headroom_frames {
            return Some(SharedMemoryPumpPreflightSkip {
                outcome: SharedMemoryPumpTickOutcome::OutputBackpressure,
                requested_frames: self.target_output_headroom_frames,
                available_frames: ring.output_writable_frames,
            });
        }

        if ring.input_channels > 0 && ring.input_readable_frames < self.target_input_frames {
            return Some(SharedMemoryPumpPreflightSkip {
                outcome: SharedMemoryPumpTickOutcome::InputUnderrun,
                requested_frames: self.target_input_frames,
                available_frames: ring.input_readable_frames,
            });
        }

        None
    }

    fn delay_from_ring(self, ring: Option<&SharedMemoryPumpRingStatus>) -> u64 {
        let Some(ring) = ring else {
            return self.base_interval_micros;
        };
        if ring.output_writable_frames < self.target_output_headroom_frames {
            return self.max_interval_micros;
        }
        if ring.input_channels == 0 {
            return self.base_interval_micros;
        }
        if ring.input_readable_frames < self.target_input_frames {
            return self.idle_backoff_micros;
        }
        if ring.input_readable_frames >= self.target_input_frames.saturating_mul(2)
            && ring.output_writable_frames >= self.target_output_headroom_frames.saturating_mul(2)
        {
            return self.min_interval_micros;
        }
        self.base_interval_micros
    }
}

impl From<&SharedMemoryStreamStatus> for SharedMemoryPumpRingStatus {
    fn from(status: &SharedMemoryStreamStatus) -> Self {
        Self {
            input_channels: status.descriptor.layout.config.input_channels,
            output_channels: status.descriptor.layout.config.output_channels,
            input_readable_frames: status.input.readable_frames,
            input_writable_frames: status.input.writable_frames,
            output_readable_frames: status.output.readable_frames,
            output_writable_frames: status.output.writable_frames,
        }
    }
}
