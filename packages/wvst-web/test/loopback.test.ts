import { describe, expect, it } from "vitest";
import {
  LoopbackCounter,
  createLoopbackSharedBuffers,
  readLoopbackMetrics,
  readLoopbackOutput,
  writeLoopbackInput,
} from "../src/index.js";

describe("WVST loopback shared buffers", () => {
  it("writes input quanta into ring slots and reports pending input", () => {
    const buffers = createLoopbackSharedBuffers({
      frames: 2,
      inputChannels: 2,
      outputChannels: 2,
      capacityQuanta: 2,
    });

    writeLoopbackInput(buffers, new Float32Array([1, 2, 3, 4]));
    writeLoopbackInput(buffers, new Float32Array([5, 6, 7, 8]));

    expect(Array.from(buffers.inputSamples)).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
    expect(readLoopbackMetrics(buffers)).toMatchObject({
      inputFrames: 4,
      inputSequence: 2,
      inputConsumedSequence: 0,
      pendingInputQuanta: 2,
      overflows: 0,
    });
  });

  it("drops stale input quanta instead of growing beyond capacity", () => {
    const buffers = createLoopbackSharedBuffers({
      frames: 1,
      inputChannels: 1,
      outputChannels: 1,
      capacityQuanta: 2,
    });

    writeLoopbackInput(buffers, new Float32Array([1]));
    writeLoopbackInput(buffers, new Float32Array([2]));
    writeLoopbackInput(buffers, new Float32Array([3]));

    expect(readLoopbackMetrics(buffers)).toMatchObject({
      inputSequence: 3,
      inputConsumedSequence: 1,
      droppedInputQuanta: 1,
      overflows: 1,
      pendingInputQuanta: 2,
    });
  });

  it("reads output quanta and increments underflow counters for missing output", () => {
    const buffers = createLoopbackSharedBuffers({
      frames: 2,
      inputChannels: 0,
      outputChannels: 2,
      capacityQuanta: 2,
    });
    buffers.outputSamples.set([0.25, 0.5, 0.75, 1], 0);
    Atomics.store(buffers.counters, LoopbackCounter.OutputSequence, 1);
    const output = new Float32Array(4);

    readLoopbackOutput(buffers, output);

    expect(Array.from(output)).toEqual([0.25, 0.5, 0.75, 1]);
    expect(readLoopbackMetrics(buffers)).toMatchObject({
      outputFrames: 2,
      outputConsumedSequence: 1,
      pendingOutputQuanta: 0,
    });
    expect(() => readLoopbackOutput(buffers, output)).toThrow(/not ready/);
    expect(readLoopbackMetrics(buffers).underflows).toBe(1);
  });

  it("allocates zero-byte input rings for instrument sessions", () => {
    const buffers = createLoopbackSharedBuffers({
      frames: 128,
      inputChannels: 0,
      outputChannels: 2,
    });

    expect(buffers.inputBuffer.byteLength).toBe(0);
    expect(buffers.inputSamplesPerQuantum).toBe(0);
    expect(buffers.outputSamplesPerQuantum).toBe(256);
  });
});
