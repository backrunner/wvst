use std::error::Error;
use std::fmt;

use serde::Deserialize;
use wvst_protocol::AudioFrameFlags;

use crate::latency::{
    LatencyHarness, LatencyHarnessConfig, LatencyObservation, LatencyPercentiles, LatencySnapshot,
};

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

impl LatencySnapshot {
    pub fn from_json_str(text: &str) -> Result<Self, LatencySnapshotParseError> {
        serde_json::from_str::<LatencySnapshotJsonInput>(text)
            .map_err(|error| LatencySnapshotParseError::InvalidJson(error.to_string()))?
            .into_snapshot()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LatencySnapshotParseError {
    InvalidJson(String),
    BridgeSmokeFailed { error: Option<String> },
    MissingBridgeSmokeMetrics,
}

impl fmt::Display for LatencySnapshotParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(formatter, "{error}"),
            Self::BridgeSmokeFailed { error } => {
                write!(formatter, "web bridge smoke report did not pass")?;
                if let Some(error) = error {
                    write!(formatter, ": {error}")?;
                }
                Ok(())
            }
            Self::MissingBridgeSmokeMetrics => {
                write!(formatter, "web bridge smoke report did not include metrics")
            }
        }
    }
}

impl Error for LatencySnapshotParseError {}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LatencySnapshotJsonInput {
    Snapshot(LatencySnapshot),
    WebBridgeSmoke(WebBridgeSmokeLatencyReport),
}

impl LatencySnapshotJsonInput {
    fn into_snapshot(self) -> Result<LatencySnapshot, LatencySnapshotParseError> {
        match self {
            Self::Snapshot(snapshot) => Ok(snapshot),
            Self::WebBridgeSmoke(report) => report.into_snapshot(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebBridgeSmokeLatencyReport {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    sample_rate: u32,
    #[serde(default)]
    config: WebBridgeSmokeConfig,
    metrics: Option<WebBridgeSmokeMetrics>,
    #[serde(default)]
    bridge_metrics: Option<WebBridgeSmokeBridgeMetrics>,
}

impl WebBridgeSmokeLatencyReport {
    fn into_snapshot(self) -> Result<LatencySnapshot, LatencySnapshotParseError> {
        if !self.ok {
            return Err(LatencySnapshotParseError::BridgeSmokeFailed { error: self.error });
        }
        let metrics = self
            .metrics
            .ok_or(LatencySnapshotParseError::MissingBridgeSmokeMetrics)?;
        let bridge = self.bridge_metrics.unwrap_or_default();
        let route_observations = bridge.audio_route_latency.count.unwrap_or(0);
        let observations = if route_observations > 0 {
            route_observations
        } else {
            metrics.end_to_end_round_trip_us.count as u64
        };
        let dropped_frames = metrics
            .dropped_input_quanta
            .saturating_add(metrics.dropped_output_quanta)
            .saturating_add(metrics.transport_failures)
            .saturating_add(bridge.audio_frame_route_failures);

        Ok(LatencySnapshot {
            config: LatencyHarnessConfig::new(self.sample_rate, self.config.frames),
            observations: observations as usize,
            dropped_frames,
            sequence_gap_events: bridge.audio_sequence_gap_events,
            sequence_gap_frames: bridge.audio_sequence_gap_frames,
            duplicate_frames: bridge.audio_frames_duplicate,
            out_of_order_frames: bridge.audio_frames_out_of_order,
            late_frames: bridge.audio_frames_late,
            timeout_frames: metrics.transport_failures,
            silence_frames: 0,
            process_error_frames: bridge.audio_frame_route_failures,
            route_latency_us: bridge.audio_route_latency.into_latency_percentiles(),
            round_trip_frames: metrics
                .end_to_end_round_trip_us
                .to_frame_percentiles(self.sample_rate),
            round_trip_us: metrics.end_to_end_round_trip_us,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebBridgeSmokeConfig {
    frames: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebBridgeSmokeMetrics {
    #[serde(default)]
    dropped_input_quanta: u64,
    #[serde(default)]
    dropped_output_quanta: u64,
    #[serde(default)]
    transport_failures: u64,
    #[serde(default, rename = "endToEndRoundTripUs")]
    end_to_end_round_trip_us: LatencyPercentiles,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebBridgeSmokeBridgeMetrics {
    #[serde(default)]
    audio_frame_route_failures: u64,
    #[serde(default)]
    audio_sequence_gap_events: u64,
    #[serde(default)]
    audio_sequence_gap_frames: u64,
    #[serde(default)]
    audio_frames_duplicate: u64,
    #[serde(default)]
    audio_frames_out_of_order: u64,
    #[serde(default)]
    audio_frames_late: u64,
    #[serde(default)]
    audio_route_latency: WebBridgeSmokeLatencyPercentiles,
}

#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebBridgeSmokeLatencyPercentiles {
    #[serde(default)]
    count: Option<u64>,
    #[serde(default)]
    p50_us: Option<u64>,
    #[serde(default)]
    p95_us: Option<u64>,
    #[serde(default)]
    p99_us: Option<u64>,
}

impl WebBridgeSmokeLatencyPercentiles {
    fn into_latency_percentiles(self) -> LatencyPercentiles {
        LatencyPercentiles {
            count: self.count.unwrap_or(0) as usize,
            p50: self.p50_us,
            p95: self.p95_us,
            p99: self.p99_us,
        }
    }
}

trait RoundTripFramePercentiles {
    fn to_frame_percentiles(self, sample_rate_hz: u32) -> LatencyPercentiles;
}

impl RoundTripFramePercentiles for LatencyPercentiles {
    fn to_frame_percentiles(self, sample_rate_hz: u32) -> LatencyPercentiles {
        LatencyPercentiles {
            count: self.count,
            p50: self
                .p50
                .map(|micros| micros_to_frames(micros, sample_rate_hz)),
            p95: self
                .p95
                .map(|micros| micros_to_frames(micros, sample_rate_hz)),
            p99: self
                .p99
                .map(|micros| micros_to_frames(micros, sample_rate_hz)),
        }
    }
}

fn micros_to_frames(micros: u64, sample_rate_hz: u32) -> u64 {
    micros.saturating_mul(u64::from(sample_rate_hz)) / 1_000_000
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
    fn derives_snapshot_from_web_bridge_smoke_report() {
        let report = json!({
            "ok": true,
            "mode": "bridge-vst3",
            "sampleRate": 48000,
            "config": {
                "frames": 128
            },
            "metrics": {
                "droppedInputQuanta": 1,
                "droppedOutputQuanta": 2,
                "transportFailures": 3,
                "endToEndRoundTripUs": {
                    "count": 5,
                    "p50": 3000,
                    "p95": 4000,
                    "p99": 5000
                }
            },
            "bridgeMetrics": {
                "audioFrameRouteFailures": 4,
                "audioSequenceGapEvents": 5,
                "audioSequenceGapFrames": 6,
                "audioFramesDuplicate": 7,
                "audioFramesOutOfOrder": 8,
                "audioFramesLate": 9,
                "audioRouteLatency": {
                    "count": 10,
                    "p50Us": 100,
                    "p95Us": 200,
                    "p99Us": 300
                }
            }
        });

        let snapshot =
            LatencySnapshot::from_json_str(&report.to_string()).expect("web bridge smoke snapshot");

        assert_eq!(snapshot.config.sample_rate_hz(), 48_000);
        assert_eq!(snapshot.config.block_frames(), 128);
        assert_eq!(snapshot.observations, 10);
        assert_eq!(snapshot.dropped_frames, 10);
        assert_eq!(snapshot.timeout_frames, 3);
        assert_eq!(snapshot.process_error_frames, 4);
        assert_eq!(snapshot.sequence_gap_events, 5);
        assert_eq!(snapshot.duplicate_frames, 7);
        assert_eq!(snapshot.out_of_order_frames, 8);
        assert_eq!(snapshot.late_frames, 9);
        assert_eq!(snapshot.route_latency_us.p95, Some(200));
        assert_eq!(snapshot.round_trip_us.p95, Some(4_000));
        assert_eq!(snapshot.round_trip_frames.p95, Some(192));
    }

    #[test]
    fn rejects_failed_web_bridge_smoke_report_as_snapshot() {
        let report = json!({
            "ok": false,
            "error": "bridge worker failed"
        });

        let error = LatencySnapshot::from_json_str(&report.to_string())
            .expect_err("failed web bridge smoke report");

        assert_eq!(
            error.to_string(),
            "web bridge smoke report did not pass: bridge worker failed"
        );
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
