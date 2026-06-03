import type { LoopbackMetrics } from "../audio/loopback.js";

export interface WVSTLatencyPercentiles {
  count: number;
  p50: number | null;
  p95: number | null;
  p99: number | null;
}

export interface WVSTWebAudioMetricsSnapshot extends LoopbackMetrics {
  endToEndRoundTripUs: WVSTLatencyPercentiles;
}

export interface WVSTWebAudioMetricsSnapshotOptions {
  endToEndRoundTripUs?: Partial<WVSTLatencyPercentiles>;
}

export function createWVSTWebAudioMetricsSnapshot(
  metrics: LoopbackMetrics,
  options: WVSTWebAudioMetricsSnapshotOptions = {},
): WVSTWebAudioMetricsSnapshot {
  return {
    inputFrames: nonNegativeInteger("inputFrames", metrics.inputFrames),
    outputFrames: nonNegativeInteger("outputFrames", metrics.outputFrames),
    underflows: nonNegativeInteger("underflows", metrics.underflows),
    overflows: nonNegativeInteger("overflows", metrics.overflows),
    droppedInputQuanta: nonNegativeInteger(
      "droppedInputQuanta",
      metrics.droppedInputQuanta,
    ),
    droppedOutputQuanta: nonNegativeInteger(
      "droppedOutputQuanta",
      metrics.droppedOutputQuanta,
    ),
    droppedMidiEvents: nonNegativeInteger(
      "droppedMidiEvents",
      metrics.droppedMidiEvents,
    ),
    droppedParameterEvents: nonNegativeInteger(
      "droppedParameterEvents",
      metrics.droppedParameterEvents,
    ),
    lateMidiEvents: nonNegativeInteger("lateMidiEvents", metrics.lateMidiEvents),
    lateParameterEvents: nonNegativeInteger(
      "lateParameterEvents",
      metrics.lateParameterEvents,
    ),
    transportFailures: nonNegativeInteger(
      "transportFailures",
      metrics.transportFailures,
    ),
    inputSequence: nonNegativeInteger("inputSequence", metrics.inputSequence),
    inputConsumedSequence: nonNegativeInteger(
      "inputConsumedSequence",
      metrics.inputConsumedSequence,
    ),
    outputSequence: nonNegativeInteger("outputSequence", metrics.outputSequence),
    outputConsumedSequence: nonNegativeInteger(
      "outputConsumedSequence",
      metrics.outputConsumedSequence,
    ),
    pendingInputQuanta: nonNegativeInteger(
      "pendingInputQuanta",
      metrics.pendingInputQuanta,
    ),
    pendingOutputQuanta: nonNegativeInteger(
      "pendingOutputQuanta",
      metrics.pendingOutputQuanta,
    ),
    endToEndRoundTripUs: latencyPercentiles(
      options.endToEndRoundTripUs,
      "endToEndRoundTripUs",
    ),
  };
}

function latencyPercentiles(
  percentiles: Partial<WVSTLatencyPercentiles> | undefined,
  label: string,
): WVSTLatencyPercentiles {
  return {
    count: nonNegativeInteger(`${label}.count`, percentiles?.count ?? 0),
    p50: optionalNonNegativeInteger(`${label}.p50`, percentiles?.p50 ?? null),
    p95: optionalNonNegativeInteger(`${label}.p95`, percentiles?.p95 ?? null),
    p99: optionalNonNegativeInteger(`${label}.p99`, percentiles?.p99 ?? null),
  };
}

function optionalNonNegativeInteger(label: string, value: number | null): number | null {
  return value === null ? null : nonNegativeInteger(label, value);
}

function nonNegativeInteger(label: string, value: number): number {
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(`WVST WebAudio metrics ${label} must be a non-negative integer`);
  }
  return value;
}
