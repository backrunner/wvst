use serde::{Deserialize, Serialize};

use super::event_queue::{DEFAULT_MAX_QUEUED_EVENTS, MAX_QUEUED_EVENTS_LIMIT};
use super::{SharedMemoryPumpError, SharedMemoryPumpSchedulingConfig};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpConfig {
    pub instance_id: u64,
    pub frames: u16,
    pub interval_micros: u64,
    pub scheduling: SharedMemoryPumpSchedulingConfig,
    pub max_queued_events: u32,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpStartParams {
    pub instance_id: u64,
    #[serde(default)]
    pub frames: Option<u16>,
    #[serde(default)]
    pub interval_micros: Option<u64>,
    #[serde(default)]
    pub adaptive: bool,
    #[serde(default)]
    pub min_interval_micros: Option<u64>,
    #[serde(default)]
    pub max_interval_micros: Option<u64>,
    #[serde(default)]
    pub idle_backoff_micros: Option<u64>,
    #[serde(default)]
    pub max_queued_events: Option<u32>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryPumpInstanceParams {
    pub instance_id: u64,
}

pub fn default_interval_micros(frames: u16, sample_rate: u32) -> u64 {
    let micros = u64::from(frames)
        .saturating_mul(1_000_000)
        .checked_div(u64::from(sample_rate))
        .unwrap_or(0);
    micros.max(1)
}

pub fn pump_config_from_params(
    params: SharedMemoryPumpStartParams,
    max_block_frames: u16,
    sample_rate: u32,
) -> Result<SharedMemoryPumpConfig, SharedMemoryPumpError> {
    let frames = params.frames.unwrap_or(max_block_frames);
    if frames == 0 || frames > max_block_frames {
        return Err(SharedMemoryPumpError::InvalidFrames {
            frames,
            max_frames: max_block_frames,
        });
    }
    let interval_micros = match params.interval_micros {
        Some(0) => return Err(SharedMemoryPumpError::InvalidInterval),
        Some(value) => value,
        None => default_interval_micros(frames, sample_rate),
    };
    let scheduling = scheduling_config_from_params(&params, frames, interval_micros)?;
    let max_queued_events = max_queued_events_from_params(params.max_queued_events)?;
    Ok(SharedMemoryPumpConfig {
        instance_id: params.instance_id,
        frames,
        interval_micros,
        scheduling,
        max_queued_events,
    })
}

fn scheduling_config_from_params(
    params: &SharedMemoryPumpStartParams,
    frames: u16,
    interval_micros: u64,
) -> Result<SharedMemoryPumpSchedulingConfig, SharedMemoryPumpError> {
    if !params.adaptive {
        return Ok(SharedMemoryPumpSchedulingConfig::fixed(
            interval_micros,
            frames,
        ));
    }

    let min_interval_micros = validate_interval_option(params.min_interval_micros)?
        .unwrap_or_else(|| interval_micros.saturating_div(2).max(1));
    let max_interval_micros = validate_interval_option(params.max_interval_micros)?
        .unwrap_or_else(|| interval_micros.saturating_mul(4).max(interval_micros));
    if min_interval_micros > max_interval_micros {
        return Err(SharedMemoryPumpError::InvalidSchedulingWindow {
            min_micros: min_interval_micros,
            max_micros: max_interval_micros,
        });
    }
    let idle_backoff_micros =
        validate_interval_option(params.idle_backoff_micros)?.unwrap_or(interval_micros);

    Ok(SharedMemoryPumpSchedulingConfig::adaptive(
        interval_micros,
        min_interval_micros,
        max_interval_micros,
        idle_backoff_micros,
        frames,
    ))
}

fn validate_interval_option(value: Option<u64>) -> Result<Option<u64>, SharedMemoryPumpError> {
    match value {
        Some(0) => Err(SharedMemoryPumpError::InvalidInterval),
        value => Ok(value),
    }
}

fn max_queued_events_from_params(value: Option<u32>) -> Result<u32, SharedMemoryPumpError> {
    match value.unwrap_or(DEFAULT_MAX_QUEUED_EVENTS) {
        0 => Err(SharedMemoryPumpError::InvalidMaxQueuedEvents {
            max_queued_events: 0,
        }),
        value if value > MAX_QUEUED_EVENTS_LIMIT => {
            Err(SharedMemoryPumpError::InvalidMaxQueuedEvents {
                max_queued_events: value,
            })
        }
        value => Ok(value),
    }
}
