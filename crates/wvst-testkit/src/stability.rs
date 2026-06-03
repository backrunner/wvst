use std::time::{Duration, Instant};

use wvst_protocol::AudioFrameFlags;

use crate::latency::{LatencyHarness, LatencyHarnessConfig, LatencyObservation, LatencySnapshot};

pub const DEFAULT_STABILITY_RUN_DURATION: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct StabilityRunConfig {
    latency: LatencyHarnessConfig,
    duration: Duration,
    max_blocks: Option<u64>,
    per_block_timeout: Option<Duration>,
    start_sequence: u64,
    start_frame_time: u64,
}

impl StabilityRunConfig {
    pub const fn new(latency: LatencyHarnessConfig) -> Self {
        Self {
            latency,
            duration: DEFAULT_STABILITY_RUN_DURATION,
            max_blocks: None,
            per_block_timeout: None,
            start_sequence: 0,
            start_frame_time: 0,
        }
    }

    pub const fn latency(self) -> LatencyHarnessConfig {
        self.latency
    }

    pub const fn duration(self) -> Duration {
        self.duration
    }

    pub const fn max_blocks(self) -> Option<u64> {
        self.max_blocks
    }

    pub const fn per_block_timeout(self) -> Option<Duration> {
        self.per_block_timeout
    }

    pub const fn start_sequence(self) -> u64 {
        self.start_sequence
    }

    pub const fn start_frame_time(self) -> u64 {
        self.start_frame_time
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_max_blocks(mut self, max_blocks: u64) -> Self {
        self.max_blocks = Some(max_blocks);
        self
    }

    pub fn with_per_block_timeout(mut self, timeout: Duration) -> Self {
        self.per_block_timeout = Some(timeout);
        self
    }

    pub const fn with_start_sequence(mut self, sequence: u64) -> Self {
        self.start_sequence = sequence;
        self
    }

    pub const fn with_start_frame_time(mut self, frame_time: u64) -> Self {
        self.start_frame_time = frame_time;
        self
    }

    fn target_blocks(self) -> u64 {
        let sample_rate_hz = u64::from(self.latency.sample_rate_hz());
        let block_frames = u64::from(self.latency.block_frames());
        if sample_rate_hz == 0 || block_frames == 0 || self.duration.is_zero() {
            return 0;
        }

        let duration_frames = self
            .duration
            .as_secs()
            .saturating_mul(sample_rate_hz)
            .saturating_add(
                u64::from(self.duration.subsec_nanos()).saturating_mul(sample_rate_hz)
                    / 1_000_000_000,
            );
        let blocks = duration_frames.div_ceil(block_frames);
        self.max_blocks
            .map_or(blocks, |max_blocks| blocks.min(max_blocks))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct StabilityStep {
    pub sequence: u64,
    pub sent_frame_time: u64,
    pub sample_rate_hz: u32,
    pub block_frames: u16,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct StabilityObservation {
    pub observed_frame_time: Option<u64>,
    pub route_latency_us: Option<u64>,
    pub flags: AudioFrameFlags,
}

impl StabilityObservation {
    pub const fn new(
        observed_frame_time: Option<u64>,
        route_latency_us: Option<u64>,
        flags: AudioFrameFlags,
    ) -> Self {
        Self {
            observed_frame_time,
            route_latency_us,
            flags,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StabilityStepOutcome {
    Observed(StabilityObservation),
    Dropped,
    TimedOut,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StabilityRunReport {
    pub config: StabilityRunConfig,
    pub executed_blocks: u64,
    pub elapsed_audio_frames: u64,
    pub snapshot: LatencySnapshot,
}

pub struct StabilityRunner {
    config: StabilityRunConfig,
}

impl StabilityRunner {
    pub const fn new(config: StabilityRunConfig) -> Self {
        Self { config }
    }

    pub fn run<F>(&self, mut process_block: F) -> StabilityRunReport
    where
        F: FnMut(StabilityStep) -> StabilityStepOutcome,
    {
        let mut harness = LatencyHarness::new(self.config.latency());
        let mut sequence = self.config.start_sequence();
        let mut frame_time = self.config.start_frame_time();
        let block_frames = u64::from(self.config.latency().block_frames());
        let target_blocks = self.config.target_blocks();
        let mut executed_blocks = 0_u64;

        for _ in 0..target_blocks {
            let step = StabilityStep {
                sequence,
                sent_frame_time: frame_time,
                sample_rate_hz: self.config.latency().sample_rate_hz(),
                block_frames: self.config.latency().block_frames(),
            };
            let started_at = Instant::now();
            let outcome = process_block(step);
            let route_latency_us = started_at.elapsed().as_micros() as u64;

            if self
                .config
                .per_block_timeout()
                .is_some_and(|timeout| started_at.elapsed() > timeout)
            {
                harness.record_timeout(sequence);
            } else {
                record_outcome(&mut harness, step, outcome, route_latency_us);
            }

            executed_blocks += 1;
            sequence = sequence.saturating_add(1);
            frame_time = frame_time.saturating_add(block_frames);
        }

        let elapsed_audio_frames = executed_blocks.saturating_mul(block_frames);
        let mut snapshot = harness.snapshot();
        snapshot.run_duration_millis = Some(frames_to_millis(
            elapsed_audio_frames,
            self.config.latency().sample_rate_hz(),
        ));

        StabilityRunReport {
            config: self.config,
            executed_blocks,
            elapsed_audio_frames,
            snapshot,
        }
    }
}

fn frames_to_millis(frames: u64, sample_rate_hz: u32) -> u64 {
    if sample_rate_hz == 0 {
        return 0;
    }
    frames.saturating_mul(1_000) / u64::from(sample_rate_hz)
}

fn record_outcome(
    harness: &mut LatencyHarness,
    step: StabilityStep,
    outcome: StabilityStepOutcome,
    measured_route_latency_us: u64,
) {
    match outcome {
        StabilityStepOutcome::Observed(observation) => {
            harness.record(LatencyObservation::new(
                step.sequence,
                step.sent_frame_time,
                observation.observed_frame_time,
                observation
                    .route_latency_us
                    .unwrap_or(measured_route_latency_us),
                observation.flags,
            ));
        }
        StabilityStepOutcome::Dropped => harness.record_drop(step.sequence),
        StabilityStepOutcome::TimedOut => harness.record_timeout(step.sequence),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_thirty_minute_duration() {
        let config = StabilityRunConfig::new(LatencyHarnessConfig::new(48_000, 128));

        assert_eq!(config.duration(), DEFAULT_STABILITY_RUN_DURATION);
    }

    #[test]
    fn runs_fixed_block_budget_and_records_observations() {
        let config = StabilityRunConfig::new(LatencyHarnessConfig::new(48_000, 128))
            .with_duration(Duration::from_secs(1))
            .with_max_blocks(3)
            .with_start_sequence(10)
            .with_start_frame_time(1_000);
        let report = StabilityRunner::new(config).run(|step| {
            StabilityStepOutcome::Observed(StabilityObservation::new(
                Some(step.sent_frame_time + u64::from(step.block_frames)),
                Some(500 + step.sequence),
                AudioFrameFlags::empty(),
            ))
        });

        assert_eq!(report.executed_blocks, 3);
        assert_eq!(report.elapsed_audio_frames, 384);
        assert_eq!(report.snapshot.run_duration_millis, Some(8));
        assert_eq!(report.snapshot.observations, 3);
        assert_eq!(report.snapshot.route_latency_us.p50, Some(511));
        assert_eq!(report.snapshot.round_trip_frames.p99, Some(128));
    }

    #[test]
    fn records_drops_and_timeouts() {
        let config = StabilityRunConfig::new(LatencyHarnessConfig::new(48_000, 128))
            .with_max_blocks(4)
            .with_per_block_timeout(Duration::from_millis(1));
        let report = StabilityRunner::new(config).run(|step| match step.sequence {
            0 => StabilityStepOutcome::Dropped,
            1 => StabilityStepOutcome::TimedOut,
            2 => {
                std::thread::sleep(Duration::from_millis(2));
                StabilityStepOutcome::Observed(StabilityObservation::new(
                    None,
                    None,
                    AudioFrameFlags::empty(),
                ))
            }
            _ => StabilityStepOutcome::Observed(StabilityObservation::new(
                Some(step.sent_frame_time + 128),
                Some(250),
                AudioFrameFlags::LATE,
            )),
        });

        assert_eq!(report.snapshot.dropped_frames, 3);
        assert_eq!(report.snapshot.timeout_frames, 2);
        assert_eq!(report.snapshot.late_frames, 1);
        assert_eq!(report.snapshot.observations, 1);
    }
}
