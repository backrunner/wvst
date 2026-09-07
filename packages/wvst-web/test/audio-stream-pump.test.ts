import { afterEach, describe, expect, it, vi } from "vitest";
import { AudioStreamPump } from "../src/audio/audio-stream-pump.js";
import {
  AudioFrameFlags, createLoopbackSharedBuffers, decodeAudioFrame, encodeAudioFrame,
  readLoopbackMetrics, writeLoopbackInput,
} from "../src/index.js";

afterEach(() => { vi.useRealTimers(); });

function setup(send: (frame: ArrayBuffer) => Promise<ArrayBuffer>) {
  vi.useFakeTimers();
  const buffers = createLoopbackSharedBuffers({ frames: 2, channels: 1, capacityQuanta: 2 });
  const pump = new AudioStreamPump({
    id: 1, type: "startAudioStream", streamId: 1, sampleRate: 48000, ...buffers,
  }, send);
  const write = () => writeLoopbackInput(buffers, new Float32Array([0.5, 0.5]));
  write();
  pump.start();
  return { buffers, pump, write };
}

describe("audio pump overload and replacement", () => {
  it("does not publish an old in-flight response after stopping", async () => {
    let reply!: () => void;
    const { buffers, pump } = setup((frame) => new Promise((resolve) => { reply = () => resolve(frame); }));
    await vi.advanceTimersByTimeAsync(0);
    pump.stop();
    reply();
    await vi.advanceTimersByTimeAsync(0);
    expect(readLoopbackMetrics(buffers)).toMatchObject({ outputSequence: 0, transportFailures: 0 });
  });

  it("drops results older than the configured buffer window", async () => {
    let reply!: () => void;
    const send = vi.fn<(frame: ArrayBuffer) => Promise<ArrayBuffer>>()
      .mockImplementationOnce((frame) => new Promise((resolve) => { reply = () => resolve(frame); }))
      .mockImplementation(() => new Promise(() => {}));
    const { buffers, pump, write } = setup(send);
    await vi.advanceTimersByTimeAsync(0);
    write(); write(); write();
    reply();
    await vi.advanceTimersByTimeAsync(0);
    expect(readLoopbackMetrics(buffers)).toMatchObject({ outputSequence: 0, droppedOutputQuanta: 1, underflows: 1 });
    pump.stop();
  });

  it("catches up to the newest block when the input ring is full", async () => {
    const sent: number[][] = [];
    const { buffers, pump, write } = setup(async (frame) => {
      sent.push(Array.from(new Float32Array(decodeAudioFrame(frame).audioPayload)));
      return frame;
    });
    write();
    writeLoopbackInput(buffers, new Float32Array([0.8, 0.8]));
    await vi.advanceTimersByTimeAsync(0);
    expect(sent).toHaveLength(1);
    expect(sent[0][0]).toBeCloseTo(0.8);
    expect(readLoopbackMetrics(buffers)).toMatchObject({ inputConsumedSequence: 3, droppedInputQuanta: 2, outputSequence: 1 });
    pump.stop();
  });

  it("surfaces native process errors as failures and outputs silence", async () => {
    const { buffers, pump } = setup(async (frame) => {
      const decoded = decodeAudioFrame(frame);
      return encodeAudioFrame({ ...decoded.header, flags: AudioFrameFlags.ProcessError }, decoded.audioPayload);
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(readLoopbackMetrics(buffers)).toMatchObject({ outputSequence: 1, transportFailures: 1, underflows: 1 });
    expect(Array.from(buffers.outputSamples)).toEqual([0, 0, 0, 0]);
    pump.stop();
  });

  it("mutes non-finite plugin output before it reaches WebAudio", async () => {
    const { buffers, pump } = setup(async (frame) => {
      const decoded = decodeAudioFrame(frame);
      return encodeAudioFrame(decoded.header, new Float32Array([NaN, Infinity]).buffer);
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(readLoopbackMetrics(buffers).transportFailures).toBe(1);
    expect(Array.from(buffers.outputSamples)).toEqual([0, 0, 0, 0]);
    pump.stop();
  });

  it("rejects a response from another request even with matching channel layout", async () => {
    const { buffers, pump } = setup(async (frame) => {
      const decoded = decodeAudioFrame(frame);
      return encodeAudioFrame({ ...decoded.header, sequence: 99n }, decoded.audioPayload);
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(readLoopbackMetrics(buffers)).toMatchObject({ transportFailures: 1, underflows: 1 });
    expect(Array.from(buffers.outputSamples)).toEqual([0, 0, 0, 0]);
    pump.stop();
  });

  it("rejects an unsupported sample format instead of interpreting it as float audio", async () => {
    const { buffers, pump } = setup(async (frame) => {
      new DataView(frame).setUint8(44, 255);
      return frame;
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(readLoopbackMetrics(buffers)).toMatchObject({ transportFailures: 1, underflows: 1 });
    expect(Array.from(buffers.outputSamples)).toEqual([0, 0, 0, 0]);
    pump.stop();
  });
});
