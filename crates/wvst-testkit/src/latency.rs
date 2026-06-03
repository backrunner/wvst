use std::collections::BTreeSet;

use wvst_protocol::AudioFrameFlags;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct LatencyHarnessConfig {
    sample_rate_hz: u32,
    block_frames: u16,
}

impl LatencyHarnessConfig {
    pub const fn new(sample_rate_hz: u32, block_frames: u16) -> Self {
        Self {
            sample_rate_hz,
            block_frames,
        }
    }

    pub const fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }

    pub const fn block_frames(self) -> u16 {
        self.block_frames
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct LatencyObservation {
    pub sequence: u64,
    pub sent_frame_time: u64,
    pub observed_frame_time: Option<u64>,
    pub route_latency_us: u64,
    pub flags: AudioFrameFlags,
}

impl LatencyObservation {
    pub const fn new(
        sequence: u64,
        sent_frame_time: u64,
        observed_frame_time: Option<u64>,
        route_latency_us: u64,
        flags: AudioFrameFlags,
    ) -> Self {
        Self {
            sequence,
            sent_frame_time,
            observed_frame_time,
            route_latency_us,
            flags,
        }
    }

    pub fn round_trip_frames(self) -> Option<u64> {
        self.observed_frame_time
            .and_then(|observed| observed.checked_sub(self.sent_frame_time))
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct LatencyPercentiles {
    pub count: usize,
    pub p50: Option<u64>,
    pub p95: Option<u64>,
    pub p99: Option<u64>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LatencySnapshot {
    pub config: LatencyHarnessConfig,
    pub observations: usize,
    pub dropped_frames: u64,
    pub sequence_gap_events: u64,
    pub sequence_gap_frames: u64,
    pub duplicate_frames: u64,
    pub out_of_order_frames: u64,
    pub late_frames: u64,
    pub timeout_frames: u64,
    pub silence_frames: u64,
    pub process_error_frames: u64,
    pub route_latency_us: LatencyPercentiles,
    pub round_trip_frames: LatencyPercentiles,
    pub round_trip_us: LatencyPercentiles,
}

#[derive(Debug, Clone)]
pub struct LatencyHarness {
    config: LatencyHarnessConfig,
    observations: Vec<LatencyObservation>,
    seen_sequences: BTreeSet<u64>,
    next_sequence: Option<u64>,
    dropped_frames: u64,
    sequence_gap_events: u64,
    sequence_gap_frames: u64,
    duplicate_frames: u64,
    out_of_order_frames: u64,
    late_frames: u64,
    timeout_frames: u64,
    silence_frames: u64,
    process_error_frames: u64,
}

impl LatencyHarness {
    pub fn new(config: LatencyHarnessConfig) -> Self {
        Self {
            config,
            observations: Vec::new(),
            seen_sequences: BTreeSet::new(),
            next_sequence: None,
            dropped_frames: 0,
            sequence_gap_events: 0,
            sequence_gap_frames: 0,
            duplicate_frames: 0,
            out_of_order_frames: 0,
            late_frames: 0,
            timeout_frames: 0,
            silence_frames: 0,
            process_error_frames: 0,
        }
    }

    pub fn record(&mut self, observation: LatencyObservation) {
        self.record_sequence(observation.sequence);
        self.record_flags(observation.flags);
        self.observations.push(observation);
    }

    pub fn record_drop(&mut self, sequence: u64) {
        self.record_sequence(sequence);
        self.dropped_frames = self.dropped_frames.saturating_add(1);
    }

    pub fn record_timeout(&mut self, sequence: u64) {
        self.record_sequence(sequence);
        self.timeout_frames = self.timeout_frames.saturating_add(1);
        self.dropped_frames = self.dropped_frames.saturating_add(1);
    }

    pub fn snapshot(&self) -> LatencySnapshot {
        let route_latency_us = percentiles(
            self.observations
                .iter()
                .map(|observation| observation.route_latency_us),
        );
        let round_trip_frames = percentiles(
            self.observations
                .iter()
                .filter_map(|observation| observation.round_trip_frames()),
        );
        let round_trip_us = percentiles(self.observations.iter().filter_map(|observation| {
            observation
                .round_trip_frames()
                .map(|frames| frames_to_us(frames, self.config.sample_rate_hz))
        }));

        LatencySnapshot {
            config: self.config,
            observations: self.observations.len(),
            dropped_frames: self.dropped_frames,
            sequence_gap_events: self.sequence_gap_events,
            sequence_gap_frames: self.sequence_gap_frames,
            duplicate_frames: self.duplicate_frames,
            out_of_order_frames: self.out_of_order_frames,
            late_frames: self.late_frames,
            timeout_frames: self.timeout_frames,
            silence_frames: self.silence_frames,
            process_error_frames: self.process_error_frames,
            route_latency_us,
            round_trip_frames,
            round_trip_us,
        }
    }

    fn record_sequence(&mut self, sequence: u64) {
        if !self.seen_sequences.insert(sequence) {
            self.duplicate_frames = self.duplicate_frames.saturating_add(1);
            return;
        }

        match self.next_sequence {
            None => self.next_sequence = sequence.checked_add(1),
            Some(expected) if sequence == expected => {
                self.next_sequence = sequence.checked_add(1);
            }
            Some(expected) if sequence > expected => {
                self.sequence_gap_events = self.sequence_gap_events.saturating_add(1);
                self.sequence_gap_frames = self
                    .sequence_gap_frames
                    .saturating_add(sequence.saturating_sub(expected));
                self.next_sequence = sequence.checked_add(1);
            }
            Some(_) => {
                self.out_of_order_frames = self.out_of_order_frames.saturating_add(1);
            }
        }
    }

    fn record_flags(&mut self, flags: AudioFrameFlags) {
        if flags.contains(AudioFrameFlags::LATE) {
            self.late_frames = self.late_frames.saturating_add(1);
        }
        if flags.contains(AudioFrameFlags::SILENCE) {
            self.silence_frames = self.silence_frames.saturating_add(1);
        }
        if flags.contains(AudioFrameFlags::PROCESS_ERROR) {
            self.process_error_frames = self.process_error_frames.saturating_add(1);
        }
    }
}

fn frames_to_us(frames: u64, sample_rate_hz: u32) -> u64 {
    if sample_rate_hz == 0 {
        return 0;
    }

    frames.saturating_mul(1_000_000) / u64::from(sample_rate_hz)
}

fn percentiles(values: impl Iterator<Item = u64>) -> LatencyPercentiles {
    let mut values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return LatencyPercentiles::default();
    }
    values.sort_unstable();

    LatencyPercentiles {
        count: values.len(),
        p50: Some(percentile(&values, 50)),
        p95: Some(percentile(&values, 95)),
        p99: Some(percentile(&values, 99)),
    }
}

fn percentile(values: &[u64], percentile: usize) -> u64 {
    let percentile = percentile.min(100);
    let index = values
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1);
    values[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_latency_percentiles_and_flags() {
        let mut harness = LatencyHarness::new(LatencyHarnessConfig::new(48_000, 128));

        harness.record(LatencyObservation::new(
            10,
            1_000,
            Some(1_128),
            700,
            AudioFrameFlags::empty(),
        ));
        harness.record(LatencyObservation::new(
            11,
            1_128,
            Some(1_384),
            900,
            AudioFrameFlags::LATE | AudioFrameFlags::SILENCE,
        ));
        harness.record(LatencyObservation::new(
            12,
            1_256,
            Some(1_640),
            1_100,
            AudioFrameFlags::PROCESS_ERROR,
        ));

        let snapshot = harness.snapshot();

        assert_eq!(snapshot.config.block_frames(), 128);
        assert_eq!(snapshot.observations, 3);
        assert_eq!(snapshot.late_frames, 1);
        assert_eq!(snapshot.timeout_frames, 0);
        assert_eq!(snapshot.silence_frames, 1);
        assert_eq!(snapshot.process_error_frames, 1);
        assert_eq!(snapshot.route_latency_us.p95, Some(1_100));
        assert_eq!(snapshot.round_trip_frames.p50, Some(256));
        assert_eq!(snapshot.round_trip_us.p50, Some(5_333));
    }

    #[test]
    fn reports_sequence_gaps_duplicates_out_of_order_and_drops() {
        let mut harness = LatencyHarness::new(LatencyHarnessConfig::new(48_000, 128));

        harness.record(LatencyObservation::new(
            1,
            0,
            None,
            100,
            AudioFrameFlags::empty(),
        ));
        harness.record(LatencyObservation::new(
            3,
            256,
            None,
            100,
            AudioFrameFlags::empty(),
        ));
        harness.record(LatencyObservation::new(
            2,
            128,
            None,
            100,
            AudioFrameFlags::empty(),
        ));
        harness.record_drop(3);
        harness.record_timeout(4);

        let snapshot = harness.snapshot();

        assert_eq!(snapshot.sequence_gap_events, 1);
        assert_eq!(snapshot.sequence_gap_frames, 1);
        assert_eq!(snapshot.out_of_order_frames, 1);
        assert_eq!(snapshot.duplicate_frames, 1);
        assert_eq!(snapshot.timeout_frames, 1);
        assert_eq!(snapshot.dropped_frames, 2);
    }
}
