export interface LoopbackBufferOptions {
  frames: number;
  channels?: number;
  inputChannels?: number;
  outputChannels?: number;
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
  inputChannels: number;
  outputChannels: number;
  capacityQuanta: number;
  samplesPerQuantum: number;
  inputSamplesPerQuantum: number;
  outputSamplesPerQuantum: number;
  inputBuffer: SharedArrayBuffer;
  outputBuffer: SharedArrayBuffer;
  countersBuffer: SharedArrayBuffer;
  inputSamples: Float32Array;
  outputSamples: Float32Array;
  counters: Int32Array;
}

export interface LoopbackMetrics {
  inputFrames: number;
  outputFrames: number;
  underflows: number;
  overflows: number;
  droppedInputQuanta: number;
  droppedOutputQuanta: number;
  droppedMidiEvents: number;
  droppedParameterEvents: number;
  lateMidiEvents: number;
  lateParameterEvents: number;
  transportFailures: number;
  inputSequence: number;
  inputConsumedSequence: number;
  outputSequence: number;
  outputConsumedSequence: number;
  pendingInputQuanta: number;
  pendingOutputQuanta: number;
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
  DroppedInputQuanta = 8,
  DroppedOutputQuanta = 9,
  DroppedMidiEvents = 10,
  DroppedParameterEvents = 11,
  LateMidiEvents = 12,
  LateParameterEvents = 13,
  TransportFailures = 14,
}

const COUNTER_COUNT = 15;
const F32_BYTES = 4;
const I32_BYTES = 4;

export function createLoopbackSharedBuffers(
  options: LoopbackBufferOptions,
): LoopbackSharedBuffers {
  assertSharedArrayBufferAvailable();
  assertPositiveInteger("frames", options.frames);
  const inputChannels = options.inputChannels ?? options.channels;
  const outputChannels = options.outputChannels ?? options.channels ?? inputChannels;
  if (inputChannels === undefined || outputChannels === undefined) {
    throw new Error("channels or inputChannels/outputChannels must be provided");
  }
  assertNonNegativeInteger("inputChannels", inputChannels);
  assertPositiveInteger("outputChannels", outputChannels);
  const capacityQuanta = options.capacityQuanta ?? 4;
  assertPositiveInteger("capacityQuanta", capacityQuanta);

  const inputSamplesPerQuantum = options.frames * inputChannels;
  const outputSamplesPerQuantum = options.frames * outputChannels;
  const inputBuffer = new SharedArrayBuffer(
    inputSamplesPerQuantum * capacityQuanta * F32_BYTES,
  );
  const outputBuffer = new SharedArrayBuffer(
    outputSamplesPerQuantum * capacityQuanta * F32_BYTES,
  );
  const countersBuffer = new SharedArrayBuffer(COUNTER_COUNT * I32_BYTES);
  const channels = options.channels ?? inputChannels;

  return {
    frames: options.frames,
    channels,
    inputChannels,
    outputChannels,
    capacityQuanta,
    samplesPerQuantum: inputSamplesPerQuantum,
    inputSamplesPerQuantum,
    outputSamplesPerQuantum,
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
  if (interleavedSamples.length !== buffers.inputSamplesPerQuantum) {
    Atomics.add(buffers.counters, LoopbackCounter.Overflows, 1);
    throw new Error(
      `WVST loopback input requires ${buffers.inputSamplesPerQuantum} samples`,
    );
  }

  const nextSequence = Atomics.load(buffers.counters, LoopbackCounter.InputSequence) + 1;
  const consumedSequence = Atomics.load(
    buffers.counters,
    LoopbackCounter.InputConsumedSequence,
  );
  if (nextSequence - consumedSequence > buffers.capacityQuanta) {
    const droppedQuanta = nextSequence - buffers.capacityQuanta - consumedSequence;
    Atomics.add(buffers.counters, LoopbackCounter.Overflows, 1);
    if (droppedQuanta > 0) {
      Atomics.add(
        buffers.counters,
        LoopbackCounter.DroppedInputQuanta,
        droppedQuanta,
      );
    }
    Atomics.store(
      buffers.counters,
      LoopbackCounter.InputConsumedSequence,
      nextSequence - buffers.capacityQuanta,
    );
  }

  buffers.inputSamples.set(interleavedSamples, inputSlotOffset(buffers, nextSequence));
  Atomics.add(buffers.counters, LoopbackCounter.InputFrames, buffers.frames);
  Atomics.store(buffers.counters, LoopbackCounter.InputSequence, nextSequence);
}

export function readLoopbackOutput(
  buffers: LoopbackSharedBuffers,
  destination: Float32Array,
): void {
  if (destination.length !== buffers.outputSamplesPerQuantum) {
    Atomics.add(buffers.counters, LoopbackCounter.Underflows, 1);
    throw new Error(
      `WVST loopback output requires ${buffers.outputSamplesPerQuantum} samples`,
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
  const offset = outputSlotOffset(buffers, readSequence);
  destination.set(
    buffers.outputSamples.subarray(offset, offset + buffers.outputSamplesPerQuantum),
  );
  Atomics.add(buffers.counters, LoopbackCounter.OutputFrames, buffers.frames);
  Atomics.store(buffers.counters, LoopbackCounter.OutputConsumedSequence, readSequence);
}

export async function createLoopbackAudioWorkletNode(
  context: BaseAudioContext,
  options: LoopbackNodeOptions,
): Promise<AudioWorkletNode> {
  const inputChannels = options.inputChannels ?? options.buffers?.inputChannels ?? 2;
  const outputChannels =
    options.outputChannels ?? options.buffers?.outputChannels ?? inputChannels;

  await context.audioWorklet.addModule(options.processorUrl);

  const node = new AudioWorkletNode(context, "wvst-loopback", {
    numberOfInputs: inputChannels === 0 ? 0 : 1,
    numberOfOutputs: 1,
    outputChannelCount: [outputChannels],
    channelCount: Math.max(1, inputChannels),
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
    inputChannels: buffers.inputChannels,
    outputChannels: buffers.outputChannels,
    capacityQuanta: buffers.capacityQuanta,
    inputBuffer: buffers.inputBuffer,
    outputBuffer: buffers.outputBuffer,
    countersBuffer: buffers.countersBuffer,
  });
}

export function readLoopbackMetrics(buffers: LoopbackSharedBuffers): LoopbackMetrics {
  const inputSequence = Atomics.load(buffers.counters, LoopbackCounter.InputSequence);
  const inputConsumedSequence = Atomics.load(
    buffers.counters,
    LoopbackCounter.InputConsumedSequence,
  );
  const outputSequence = Atomics.load(buffers.counters, LoopbackCounter.OutputSequence);
  const outputConsumedSequence = Atomics.load(
    buffers.counters,
    LoopbackCounter.OutputConsumedSequence,
  );

  return {
    inputFrames: Atomics.load(buffers.counters, LoopbackCounter.InputFrames),
    outputFrames: Atomics.load(buffers.counters, LoopbackCounter.OutputFrames),
    underflows: Atomics.load(buffers.counters, LoopbackCounter.Underflows),
    overflows: Atomics.load(buffers.counters, LoopbackCounter.Overflows),
    droppedInputQuanta: Atomics.load(
      buffers.counters,
      LoopbackCounter.DroppedInputQuanta,
    ),
    droppedOutputQuanta: Atomics.load(
      buffers.counters,
      LoopbackCounter.DroppedOutputQuanta,
    ),
    droppedMidiEvents: Atomics.load(
      buffers.counters,
      LoopbackCounter.DroppedMidiEvents,
    ),
    droppedParameterEvents: Atomics.load(
      buffers.counters,
      LoopbackCounter.DroppedParameterEvents,
    ),
    lateMidiEvents: Atomics.load(buffers.counters, LoopbackCounter.LateMidiEvents),
    lateParameterEvents: Atomics.load(
      buffers.counters,
      LoopbackCounter.LateParameterEvents,
    ),
    transportFailures: Atomics.load(
      buffers.counters,
      LoopbackCounter.TransportFailures,
    ),
    inputSequence,
    inputConsumedSequence,
    outputSequence,
    outputConsumedSequence,
    pendingInputQuanta: Math.max(0, inputSequence - inputConsumedSequence),
    pendingOutputQuanta: Math.max(0, outputSequence - outputConsumedSequence),
  };
}

function inputSlotOffset(buffers: LoopbackSharedBuffers, sequence: number): number {
  return ((sequence - 1) % buffers.capacityQuanta) * buffers.inputSamplesPerQuantum;
}

function outputSlotOffset(buffers: LoopbackSharedBuffers, sequence: number): number {
  return ((sequence - 1) % buffers.capacityQuanta) * buffers.outputSamplesPerQuantum;
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

function assertNonNegativeInteger(name: string, value: number): void {
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(`${name} must be a non-negative integer`);
  }
}
