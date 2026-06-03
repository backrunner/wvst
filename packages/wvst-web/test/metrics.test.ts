import { describe, expect, it } from "vitest";
import { createWVSTWebAudioMetricsSnapshot } from "../src/index.js";

describe("WVST WebAudio metrics evidence", () => {
  it("normalizes loopback metrics into stability-budget JSON shape", () => {
    expect(
      createWVSTWebAudioMetricsSnapshot(
        {
          inputFrames: 384,
          outputFrames: 384,
          underflows: 0,
          overflows: 0,
          droppedInputQuanta: 0,
          droppedOutputQuanta: 0,
          droppedMidiEvents: 0,
          droppedParameterEvents: 0,
          lateMidiEvents: 0,
          lateParameterEvents: 0,
          transportFailures: 0,
          inputSequence: 3,
          inputConsumedSequence: 3,
          outputSequence: 3,
          outputConsumedSequence: 3,
          pendingInputQuanta: 0,
          pendingOutputQuanta: 0,
        },
        {
          endToEndRoundTripUs: {
            count: 3,
            p50: 1_000,
            p95: 1_500,
            p99: 1_750,
          },
        },
      ),
    ).toEqual({
      inputFrames: 384,
      outputFrames: 384,
      underflows: 0,
      overflows: 0,
      droppedInputQuanta: 0,
      droppedOutputQuanta: 0,
      droppedMidiEvents: 0,
      droppedParameterEvents: 0,
      lateMidiEvents: 0,
      lateParameterEvents: 0,
      transportFailures: 0,
      inputSequence: 3,
      inputConsumedSequence: 3,
      outputSequence: 3,
      outputConsumedSequence: 3,
      pendingInputQuanta: 0,
      pendingOutputQuanta: 0,
      endToEndRoundTripUs: {
        count: 3,
        p50: 1_000,
        p95: 1_500,
        p99: 1_750,
      },
    });
  });

  it("keeps percentile fields explicit when browser round-trip samples are absent", () => {
    expect(
      createWVSTWebAudioMetricsSnapshot({
        inputFrames: 0,
        outputFrames: 0,
        underflows: 0,
        overflows: 0,
        droppedInputQuanta: 0,
        droppedOutputQuanta: 0,
        droppedMidiEvents: 0,
        droppedParameterEvents: 0,
        lateMidiEvents: 0,
        lateParameterEvents: 0,
        transportFailures: 0,
        inputSequence: 0,
        inputConsumedSequence: 0,
        outputSequence: 0,
        outputConsumedSequence: 0,
        pendingInputQuanta: 0,
        pendingOutputQuanta: 0,
      }).endToEndRoundTripUs,
    ).toEqual({
      count: 0,
      p50: null,
      p95: null,
      p99: null,
    });
  });

  it("rejects non-integer counters before writing evidence", () => {
    expect(() =>
      createWVSTWebAudioMetricsSnapshot({
        inputFrames: 1.5,
        outputFrames: 0,
        underflows: 0,
        overflows: 0,
        droppedInputQuanta: 0,
        droppedOutputQuanta: 0,
        droppedMidiEvents: 0,
        droppedParameterEvents: 0,
        lateMidiEvents: 0,
        lateParameterEvents: 0,
        transportFailures: 0,
        inputSequence: 0,
        inputConsumedSequence: 0,
        outputSequence: 0,
        outputConsumedSequence: 0,
        pendingInputQuanta: 0,
        pendingOutputQuanta: 0,
      }),
    ).toThrow(/inputFrames/);
  });
});
