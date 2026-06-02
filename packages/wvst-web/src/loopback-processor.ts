interface AudioWorkletProcessorLike {
  readonly port: MessagePort;
  process(inputs: Float32Array[][], outputs: Float32Array[][]): boolean;
}

declare const AudioWorkletProcessor: {
  prototype: AudioWorkletProcessorLike;
  new (): AudioWorkletProcessorLike;
};

declare function registerProcessor(
  name: string,
  processorCtor: typeof AudioWorkletProcessor,
): void;

interface ConfigureMessage {
  type: "configure";
  frames: number;
  channels: number;
  inputChannels?: number;
  outputChannels?: number;
  capacityQuanta: number;
  inputBuffer: SharedArrayBuffer;
  outputBuffer: SharedArrayBuffer;
  countersBuffer: SharedArrayBuffer;
}

const INPUT_FRAMES = 0;
const UNDERFLOWS = 2;
const OVERFLOWS = 3;
const INPUT_SEQUENCE = 4;
const INPUT_CONSUMED_SEQUENCE = 5;
const OUTPUT_SEQUENCE = 6;
const OUTPUT_CONSUMED_SEQUENCE = 7;

class WVSTLoopbackProcessor extends AudioWorkletProcessor {
  private frames = 0;
  private inputChannels = 0;
  private outputChannels = 0;
  private capacityQuanta = 1;
  private inputSamplesPerQuantum = 0;
  private outputSamplesPerQuantum = 0;
  private inputSamples?: Float32Array;
  private outputSamples?: Float32Array;
  private counters?: Int32Array;

  constructor() {
    super();
    this.port.onmessage = (event: MessageEvent<ConfigureMessage>) => {
      if (event.data.type === "configure") {
        this.configure(event.data);
      }
    };
  }

  process(inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
    const output = outputs[0] ?? [];
    const counters = this.counters;
    const inputSamples = this.inputSamples;
    const outputSamples = this.outputSamples;

    if (!counters || !inputSamples || !outputSamples) {
      clearOutputs(output);
      return true;
    }

    const input = inputs[0] ?? [];
    const frameCount = output[0]?.length ?? this.frames;
    if (frameCount !== this.frames) {
      Atomics.add(counters, UNDERFLOWS, 1);
      clearOutputs(output);
      return true;
    }

    const nextInputSequence = Atomics.load(counters, INPUT_SEQUENCE) + 1;
    const inputConsumedSequence = Atomics.load(counters, INPUT_CONSUMED_SEQUENCE);
    if (nextInputSequence - inputConsumedSequence > this.capacityQuanta) {
      Atomics.add(counters, OVERFLOWS, 1);
      Atomics.store(counters, INPUT_CONSUMED_SEQUENCE, nextInputSequence - this.capacityQuanta);
    }

    writeInterleavedInput(
      input,
      inputSamples,
      slotOffset(nextInputSequence, this.capacityQuanta, this.inputSamplesPerQuantum),
      this.frames,
      this.inputChannels,
    );
    Atomics.add(counters, INPUT_FRAMES, this.frames);
    Atomics.store(counters, INPUT_SEQUENCE, nextInputSequence);

    const outputSequence = Atomics.load(counters, OUTPUT_SEQUENCE);
    const outputConsumedSequence = Atomics.load(counters, OUTPUT_CONSUMED_SEQUENCE);
    if (outputSequence <= outputConsumedSequence) {
      Atomics.add(counters, UNDERFLOWS, 1);
      clearOutputs(output);
      return true;
    }

    const readSequence = outputConsumedSequence + 1;
    readInterleavedOutput(
      output,
      outputSamples,
      slotOffset(readSequence, this.capacityQuanta, this.outputSamplesPerQuantum),
      this.frames,
      this.outputChannels,
    );
    Atomics.store(counters, OUTPUT_CONSUMED_SEQUENCE, readSequence);
    return true;
  }

  private configure(message: ConfigureMessage): void {
    this.frames = message.frames;
    this.inputChannels = message.inputChannels ?? message.channels;
    this.outputChannels = message.outputChannels ?? message.channels;
    this.capacityQuanta = message.capacityQuanta;
    this.inputSamplesPerQuantum = message.frames * this.inputChannels;
    this.outputSamplesPerQuantum = message.frames * this.outputChannels;
    this.inputSamples = new Float32Array(message.inputBuffer);
    this.outputSamples = new Float32Array(message.outputBuffer);
    this.counters = new Int32Array(message.countersBuffer);
  }
}

function writeInterleavedInput(
  input: Float32Array[],
  destination: Float32Array,
  offset: number,
  frames: number,
  channels: number,
): void {
  for (let frame = 0; frame < frames; frame += 1) {
    for (let channel = 0; channel < channels; channel += 1) {
      const source = input[channel];
      destination[offset + frame * channels + channel] = source ? source[frame] ?? 0 : 0;
    }
  }
}

function readInterleavedOutput(
  output: Float32Array[],
  source: Float32Array,
  offset: number,
  frames: number,
  channels: number,
): void {
  for (let channel = 0; channel < output.length; channel += 1) {
    const destination = output[channel];
    if (!destination) {
      continue;
    }

    if (channel >= channels) {
      destination.fill(0);
      continue;
    }

    for (let frame = 0; frame < frames; frame += 1) {
      destination[frame] = source[offset + frame * channels + channel] ?? 0;
    }
  }
}

function slotOffset(sequence: number, capacityQuanta: number, samplesPerQuantum: number): number {
  return ((sequence - 1) % capacityQuanta) * samplesPerQuantum;
}

function clearOutputs(outputs: Float32Array[]): void {
  for (const output of outputs) {
    output.fill(0);
  }
}

registerProcessor("wvst-loopback", WVSTLoopbackProcessor);
