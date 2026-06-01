export interface LoopbackBufferOptions {
  frames: number;
  channels: number;
  capacityQuanta?: number;
}

export interface LoopbackNodeOptions {
  processorUrl: string;
  inputChannels?: number;
  outputChannels?: number;
  buffers?: LoopbackSharedBuffers;
}

export interface LoopbackSharedBuffers {
  frames: number;
  channels: number;
  capacityQuanta: number;
  samplesPerQuantum: number;
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
  InputSequence = 4,
  InputConsumedSequence = 5,
  OutputSequence = 6,
  OutputConsumedSequence = 7,
}

const COUNTER_COUNT = 8;
const F32_BYTES = 4;
const I32_BYTES = 4;

export function createLoopbackSharedBuffers(
  options: LoopbackBufferOptions,
): LoopbackSharedBuffers {
  assertSharedArrayBufferAvailable();
  assertPositiveInteger("frames", options.frames);
  assertPositiveInteger("channels", options.channels);
  const capacityQuanta = options.capacityQuanta ?? 4;
  assertPositiveInteger("capacityQuanta", capacityQuanta);

  const samplesPerQuantum = options.frames * options.channels;
  const sampleCount = samplesPerQuantum * capacityQuanta;
  const inputBuffer = new SharedArrayBuffer(sampleCount * F32_BYTES);
  const outputBuffer = new SharedArrayBuffer(sampleCount * F32_BYTES);
  const countersBuffer = new SharedArrayBuffer(COUNTER_COUNT * I32_BYTES);

  return {
    frames: options.frames,
    channels: options.channels,
    capacityQuanta,
    samplesPerQuantum,
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
  if (interleavedSamples.length !== buffers.samplesPerQuantum) {
    Atomics.add(buffers.counters, LoopbackCounter.Overflows, 1);
    throw new Error(
      `WVST loopback input requires ${buffers.samplesPerQuantum} samples`,
    );
  }

  const nextSequence = Atomics.load(buffers.counters, LoopbackCounter.InputSequence) + 1;
  const consumedSequence = Atomics.load(
    buffers.counters,
    LoopbackCounter.InputConsumedSequence,
  );
  if (nextSequence - consumedSequence > buffers.capacityQuanta) {
    Atomics.add(buffers.counters, LoopbackCounter.Overflows, 1);
    Atomics.store(
      buffers.counters,
      LoopbackCounter.InputConsumedSequence,
      nextSequence - buffers.capacityQuanta,
    );
  }

  buffers.inputSamples.set(interleavedSamples, slotOffset(buffers, nextSequence));
  Atomics.add(buffers.counters, LoopbackCounter.InputFrames, buffers.frames);
  Atomics.store(buffers.counters, LoopbackCounter.InputSequence, nextSequence);
}

export function readLoopbackOutput(
  buffers: LoopbackSharedBuffers,
  destination: Float32Array,
): void {
  if (destination.length !== buffers.samplesPerQuantum) {
    Atomics.add(buffers.counters, LoopbackCounter.Underflows, 1);
    throw new Error(
      `WVST loopback output requires ${buffers.samplesPerQuantum} samples`,
    );
  }

  const outputSequence = Atomics.load(buffers.counters, LoopbackCounter.OutputSequence);
  const consumedSequence = Atomics.load(
    buffers.counters,
    LoopbackCounter.OutputConsumedSequence,
  );
  if (outputSequence <= consumedSequence) {
    Atomics.add(buffers.counters, LoopbackCounter.Underflows, 1);
    throw new Error("WVST loopback output is not ready");
  }

  const readSequence = consumedSequence + 1;
  const offset = slotOffset(buffers, readSequence);
  destination.set(buffers.outputSamples.subarray(offset, offset + buffers.samplesPerQuantum));
  Atomics.add(buffers.counters, LoopbackCounter.OutputFrames, buffers.frames);
  Atomics.store(buffers.counters, LoopbackCounter.OutputConsumedSequence, readSequence);
}

export async function createLoopbackAudioWorkletNode(
  context: BaseAudioContext,
  options: LoopbackNodeOptions,
): Promise<AudioWorkletNode> {
  const inputChannels = options.inputChannels ?? 2;
  const outputChannels = options.outputChannels ?? inputChannels;

  await context.audioWorklet.addModule(options.processorUrl);

  const node = new AudioWorkletNode(context, "wvst-loopback", {
    numberOfInputs: 1,
    numberOfOutputs: 1,
    outputChannelCount: [outputChannels],
    channelCount: inputChannels,
    channelCountMode: "explicit",
    channelInterpretation: "speakers",
  });

  if (options.buffers) {
    configureLoopbackAudioWorkletNode(node, options.buffers);
  }

  return node;
}

export function configureLoopbackAudioWorkletNode(
  node: AudioWorkletNode,
  buffers: LoopbackSharedBuffers,
): void {
  node.port.postMessage({
    type: "configure",
    frames: buffers.frames,
    channels: buffers.channels,
    capacityQuanta: buffers.capacityQuanta,
    inputBuffer: buffers.inputBuffer,
    outputBuffer: buffers.outputBuffer,
    countersBuffer: buffers.countersBuffer,
  });
}

function slotOffset(buffers: LoopbackSharedBuffers, sequence: number): number {
  return ((sequence - 1) % buffers.capacityQuanta) * buffers.samplesPerQuantum;
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
