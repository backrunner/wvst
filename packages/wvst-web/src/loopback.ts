export interface LoopbackBufferOptions {
  frames: number;
  channels: number;
}

export interface LoopbackNodeOptions {
  processorUrl: string;
  inputChannels?: number;
  outputChannels?: number;
}

export interface LoopbackSharedBuffers {
  frames: number;
  channels: number;
  inputBuffer: SharedArrayBuffer;
  outputBuffer: SharedArrayBuffer;
  countersBuffer: SharedArrayBuffer;
  inputSamples: Float32Array;
  outputSamples: Float32Array;
  counters: Int32Array;
}

export enum LoopbackCounter {
  InputFrames = 0,
  OutputFrames = 1,
  Underflows = 2,
  Overflows = 3,
}

const COUNTER_COUNT = 4;
const F32_BYTES = 4;
const I32_BYTES = 4;

export function createLoopbackSharedBuffers(
  options: LoopbackBufferOptions,
): LoopbackSharedBuffers {
  assertSharedArrayBufferAvailable();
  assertPositiveInteger("frames", options.frames);
  assertPositiveInteger("channels", options.channels);

  const sampleCount = options.frames * options.channels;
  const inputBuffer = new SharedArrayBuffer(sampleCount * F32_BYTES);
  const outputBuffer = new SharedArrayBuffer(sampleCount * F32_BYTES);
  const countersBuffer = new SharedArrayBuffer(COUNTER_COUNT * I32_BYTES);

  return {
    frames: options.frames,
    channels: options.channels,
    inputBuffer,
    outputBuffer,
    countersBuffer,
    inputSamples: new Float32Array(inputBuffer),
    outputSamples: new Float32Array(outputBuffer),
    counters: new Int32Array(countersBuffer),
  };
}

export function writeLoopbackInput(
  buffers: LoopbackSharedBuffers,
  interleavedSamples: Float32Array,
): void {
  if (interleavedSamples.length !== buffers.inputSamples.length) {
    Atomics.add(buffers.counters, LoopbackCounter.Overflows, 1);
    throw new Error(
      `WVST loopback input requires ${buffers.inputSamples.length} samples`,
    );
  }

  buffers.inputSamples.set(interleavedSamples);
  Atomics.add(buffers.counters, LoopbackCounter.InputFrames, buffers.frames);
}

export function readLoopbackOutput(
  buffers: LoopbackSharedBuffers,
  destination: Float32Array,
): void {
  if (destination.length !== buffers.outputSamples.length) {
    Atomics.add(buffers.counters, LoopbackCounter.Underflows, 1);
    throw new Error(
      `WVST loopback output requires ${buffers.outputSamples.length} samples`,
    );
  }

  destination.set(buffers.outputSamples);
  Atomics.add(buffers.counters, LoopbackCounter.OutputFrames, buffers.frames);
}

export async function createLoopbackAudioWorkletNode(
  context: BaseAudioContext,
  options: LoopbackNodeOptions,
): Promise<AudioWorkletNode> {
  const inputChannels = options.inputChannels ?? 2;
  const outputChannels = options.outputChannels ?? inputChannels;

  await context.audioWorklet.addModule(options.processorUrl);

  return new AudioWorkletNode(context, "wvst-loopback", {
    numberOfInputs: 1,
    numberOfOutputs: 1,
    outputChannelCount: [outputChannels],
    channelCount: inputChannels,
    channelCountMode: "explicit",
    channelInterpretation: "speakers",
  });
}

function assertSharedArrayBufferAvailable(): void {
  if (typeof globalThis.SharedArrayBuffer !== "function") {
    throw new Error("WVST loopback requires SharedArrayBuffer");
  }
}

function assertPositiveInteger(name: string, value: number): void {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
}
