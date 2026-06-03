use std::error::Error;
use std::fmt;

use serde::Deserialize;
use wvst_protocol::AudioFrameFlags;

use crate::latency::{LatencyHarness, LatencyHarnessConfig, LatencyObservation, LatencySnapshot};

const SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencySnapshotInput {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub config: LatencyHarnessConfig,
    #[serde(default)]
    pub start_sequence: u64,
    #[serde(default)]
    pub start_frame_time: u64,
    #[serde(default)]
    pub observations: Vec<LatencySnapshotObservation>,
}

impl LatencySnapshotInput {
    pub fn into_snapshot(self) -> Result<LatencySnapshot, LatencySnapshotInputError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(LatencySnapshotInputError::UnsupportedSchemaVersion {
                actual: self.schema_version,
            });
        }

        let mut harness = LatencyHarness::new(self.config);
        for (index, observation) in self.observations.into_iter().enumerate() {
            let sequence = observation
                .sequence
                .unwrap_or_else(|| self.start_sequence.saturating_add(index as u64));
            let sent_frame_time = observation.sent_frame_time.unwrap_or_else(|| {
                self.start_frame_time.saturating_add(
                    (index as u64).saturating_mul(u64::from(self.config.block_frames())),
                )
            });

            match observation.outcome {
                LatencySnapshotOutcome::Observed => {
                    let route_latency_us = observation
                        .route_latency_us
                        .ok_or(LatencySnapshotInputError::MissingRouteLatency { index })?;
                    let observed_frame_time =
                        observation.observed_frame_time(index, sent_frame_time)?;
                    harness.record(LatencyObservation::new(
                        sequence,
                        sent_frame_time,
                        observed_frame_time,
                        route_latency_us,
                        observation.flags(),
                    ));
                }
                LatencySnapshotOutcome::Dropped => harness.record_drop(sequence),
                LatencySnapshotOutcome::TimedOut => harness.record_timeout(sequence),
            }
        }

        Ok(harness.snapshot())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencySnapshotObservation {
    #[serde(default)]
    pub outcome: LatencySnapshotOutcome,
    #[serde(default)]
    pub sequence: Option<u64>,
    #[serde(default)]
    pub sent_frame_time: Option<u64>,
    #[serde(default)]
    pub observed_frame_time: Option<u64>,
    #[serde(default)]
    pub round_trip_frames: Option<u64>,
    #[serde(default)]
    pub route_latency_us: Option<u64>,
    #[serde(default)]
    pub flags_bits: u16,
    #[serde(default)]
    pub late: bool,
    #[serde(default)]
    pub silence: bool,
    #[serde(default)]
    pub end_of_stream: bool,
    #[serde(default)]
    pub midi_only: bool,
    #[serde(default)]
    pub process_error: bool,
}

impl LatencySnapshotObservation {
    fn observed_frame_time(
        &self,
        index: usize,
        sent_frame_time: u64,
    ) -> Result<Option<u64>, LatencySnapshotInputError> {
        match (self.observed_frame_time, self.round_trip_frames) {
            (Some(observed_frame_time), Some(round_trip_frames)) => {
                let derived = sent_frame_time
                    .checked_add(round_trip_frames)
                    .ok_or(LatencySnapshotInputError::ObservedFrameTimeOverflow { index })?;
                if observed_frame_time != derived {
                    return Err(LatencySnapshotInputError::InconsistentObservedFrameTime {
                        index,
                        observed_frame_time,
                        derived_from_round_trip: derived,
                    });
                }
                Ok(Some(observed_frame_time))
            }
            (Some(observed_frame_time), None) => Ok(Some(observed_frame_time)),
            (None, Some(round_trip_frames)) => sent_frame_time
                .checked_add(round_trip_frames)
                .map(Some)
                .ok_or(LatencySnapshotInputError::ObservedFrameTimeOverflow { index }),
            (None, None) => Ok(None),
        }
    }

    fn flags(&self) -> AudioFrameFlags {
        let mut flags = AudioFrameFlags::from_bits(self.flags_bits);
        if self.late {
            flags |= AudioFrameFlags::LATE;
        }
        if self.silence {
            flags |= AudioFrameFlags::SILENCE;
        }
        if self.end_of_stream {
            flags |= AudioFrameFlags::END_OF_STREAM;
        }
        if self.midi_only {
            flags |= AudioFrameFlags::MIDI_ONLY;
        }
        if self.process_error {
            flags |= AudioFrameFlags::PROCESS_ERROR;
        }
        flags
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LatencySnapshotOutcome {
    #[default]
    Observed,
    Dropped,
    TimedOut,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LatencySnapshotInputError {
    UnsupportedSchemaVersion {
        actual: u16,
    },
    MissingRouteLatency {
        index: usize,
    },
    ObservedFrameTimeOverflow {
        index: usize,
    },
    InconsistentObservedFrameTime {
        index: usize,
        observed_frame_time: u64,
        derived_from_round_trip: u64,
    },
}

impl fmt::Display for LatencySnapshotInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { actual } => {
                write!(
                    formatter,
                    "unsupported latency snapshot schema version {actual}"
                )
            }
            Self::MissingRouteLatency { index } => {
                write!(formatter, "observation {index} is missing routeLatencyUs")
            }
            Self::ObservedFrameTimeOverflow { index } => {
                write!(
                    formatter,
                    "observation {index} roundTripFrames overflows frame time"
                )
            }
            Self::InconsistentObservedFrameTime {
                index,
                observed_frame_time,
                derived_from_round_trip,
            } => write!(
                formatter,
                "observation {index} observedFrameTime {observed_frame_time} does not match roundTripFrames-derived value {derived_from_round_trip}"
            ),
        }
    }
}

impl Error for LatencySnapshotInputError {}

fn default_schema_version() -> u16 {
    SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_snapshot_from_scripted_observations() {
        let input = serde_json::from_value::<LatencySnapshotInput>(json!({
            "schemaVersion": 1,
            "config": {
                "sampleRateHz": 48000,
                "blockFrames": 128
            },
            "startSequence": 10,
            "startFrameTime": 1024,
            "observations": [
                {
                    "roundTripFrames": 128,
                    "routeLatencyUs": 400
                },
                {
                    "roundTripFrames": 256,
                    "routeLatencyUs": 800,
                    "late": true,
                    "silence": true
                },
                {
                    "outcome": "timedOut"
                }
            ]
        }))
        .expect("input");

        let snapshot = input.into_snapshot().expect("snapshot");

        assert_eq!(snapshot.observations, 2);
        assert_eq!(snapshot.dropped_frames, 1);
        assert_eq!(snapshot.timeout_frames, 1);
        assert_eq!(snapshot.late_frames, 1);
        assert_eq!(snapshot.silence_frames, 1);
        assert_eq!(snapshot.route_latency_us.p95, Some(800));
        assert_eq!(snapshot.round_trip_frames.p50, Some(128));
    }

    #[test]
    fn records_drop_and_explicit_sequences() {
        let input = serde_json::from_value::<LatencySnapshotInput>(json!({
            "config": {
                "sampleRateHz": 48000,
                "blockFrames": 64
            },
            "observations": [
                {
                    "sequence": 1,
                    "routeLatencyUs": 100
                },
                {
                    "sequence": 3,
                    "outcome": "dropped"
                }
            ]
        }))
        .expect("input");

        let snapshot = input.into_snapshot().expect("snapshot");

        assert_eq!(snapshot.sequence_gap_events, 1);
        assert_eq!(snapshot.sequence_gap_frames, 1);
        assert_eq!(snapshot.dropped_frames, 1);
    }

    #[test]
    fn rejects_missing_observed_route_latency() {
        let input = serde_json::from_value::<LatencySnapshotInput>(json!({
            "config": {
                "sampleRateHz": 48000,
                "blockFrames": 128
            },
            "observations": [
                {
                    "roundTripFrames": 128
                }
            ]
        }))
        .expect("input");

        assert_eq!(
            input.into_snapshot().expect_err("missing route latency"),
            LatencySnapshotInputError::MissingRouteLatency { index: 0 }
        );
    }

    #[test]
    fn rejects_inconsistent_observed_frame_time() {
        let input = serde_json::from_value::<LatencySnapshotInput>(json!({
            "config": {
                "sampleRateHz": 48000,
                "blockFrames": 128
            },
            "observations": [
                {
                    "sentFrameTime": 1000,
                    "observedFrameTime": 1200,
                    "roundTripFrames": 128,
                    "routeLatencyUs": 400
                }
            ]
        }))
        .expect("input");

        assert_eq!(
            input.into_snapshot().expect_err("inconsistent frame time"),
            LatencySnapshotInputError::InconsistentObservedFrameTime {
                index: 0,
                observed_frame_time: 1200,
                derived_from_round_trip: 1128,
            }
        );
    }
}
