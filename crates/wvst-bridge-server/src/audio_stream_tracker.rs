use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

use wvst_protocol::{AudioFrameFlags, AudioFrameHeader};

#[derive(Debug, Default)]
pub struct AudioStreamTracker {
    streams: Mutex<BTreeMap<u64, StreamAudioState>>,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct AudioStreamObservation {
    pub sequence_gap: u64,
    pub duplicate: bool,
    pub out_of_order: bool,
    pub late: bool,
    pub interarrival_jitter_us: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default)]
struct StreamAudioState {
    expected_sequence: Option<u64>,
    last_arrival: Option<Instant>,
}

impl AudioStreamTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&self, header: AudioFrameHeader) -> AudioStreamObservation {
        self.observe_at(header, Instant::now())
    }

    pub fn reset(&self, stream_id: u64) {
        if let Ok(mut streams) = self.streams.lock() {
            streams.remove(&stream_id);
        }
    }

    fn observe_at(&self, header: AudioFrameHeader, arrived_at: Instant) -> AudioStreamObservation {
        let Ok(mut streams) = self.streams.lock() else {
            return AudioStreamObservation {
                late: header.flags.contains(AudioFrameFlags::LATE),
                ..AudioStreamObservation::default()
            };
        };
        let state = streams.entry(header.stream_id.get()).or_default();
        let mut observation = AudioStreamObservation {
            late: header.flags.contains(AudioFrameFlags::LATE),
            interarrival_jitter_us: state
                .last_arrival
                .map(|last_arrival| interarrival_jitter_us(header, arrived_at, last_arrival)),
            ..AudioStreamObservation::default()
        };

        if let Some(expected) = state.expected_sequence {
            if header.sequence == expected {
                state.expected_sequence = header.sequence.checked_add(1);
            } else if header.sequence > expected {
                observation.sequence_gap = header.sequence - expected;
                state.expected_sequence = header.sequence.checked_add(1);
            } else if header.sequence == expected.saturating_sub(1) {
                observation.duplicate = true;
            } else {
                observation.out_of_order = true;
            }
        } else {
            state.expected_sequence = header.sequence.checked_add(1);
        }

        state.last_arrival = Some(arrived_at);
        observation
    }
}

fn interarrival_jitter_us(
    header: AudioFrameHeader,
    arrived_at: Instant,
    last_arrival: Instant,
) -> u64 {
    let actual_us = arrived_at.duration_since(last_arrival).as_micros() as u64;
    let expected_us = expected_block_duration_us(header);

    actual_us.abs_diff(expected_us)
}

fn expected_block_duration_us(header: AudioFrameHeader) -> u64 {
    u64::from(header.frames.get())
        .saturating_mul(1_000_000)
        .checked_div(u64::from(header.sample_rate.get()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};

    fn header(sequence: u64, flags: AudioFrameFlags) -> AudioFrameHeader {
        AudioFrameHeader::new_f32(
            StreamId::new(7),
            sequence,
            0,
            SampleRate::new(48_000).expect("sample rate"),
            FrameCount::new(48).expect("frames"),
            ChannelCount::new(2).expect("channels"),
            flags,
        )
        .expect("header")
    }

    #[test]
    fn reports_sequence_gap_duplicate_out_of_order_and_late() {
        let tracker = AudioStreamTracker::new();
        let start = Instant::now();

        assert_eq!(
            tracker.observe_at(header(10, AudioFrameFlags::empty()), start),
            AudioStreamObservation::default()
        );

        let gap = tracker.observe_at(
            header(12, AudioFrameFlags::empty()),
            start + Duration::from_micros(1_000),
        );
        assert_eq!(gap.sequence_gap, 1);
        assert_eq!(gap.interarrival_jitter_us, Some(0));

        let duplicate = tracker.observe_at(
            header(12, AudioFrameFlags::LATE),
            start + Duration::from_micros(2_200),
        );
        assert!(duplicate.duplicate);
        assert!(duplicate.late);
        assert_eq!(duplicate.interarrival_jitter_us, Some(200));

        let out_of_order = tracker.observe_at(
            header(11, AudioFrameFlags::empty()),
            start + Duration::from_micros(3_200),
        );
        assert!(out_of_order.out_of_order);
    }

    #[test]
    fn reset_drops_previous_sequence_state() {
        let tracker = AudioStreamTracker::new();
        let start = Instant::now();

        let _ = tracker.observe_at(header(10, AudioFrameFlags::empty()), start);
        tracker.reset(7);

        assert_eq!(
            tracker.observe_at(header(0, AudioFrameFlags::empty()), start),
            AudioStreamObservation::default()
        );
    }
}
