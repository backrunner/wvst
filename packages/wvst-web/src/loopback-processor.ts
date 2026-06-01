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
  inputBuffer: SharedArrayBuffer;
  outputBuffer: SharedArrayBuffer;
  countersBuffer: SharedArrayBuffer;
}

const INPUT_FRAMES = 0;
const OUTPUT_FRAMES = 1;
const UNDERFLOWS = 2;
const OVERFLOWS = 3;
const INPUT_SEQUENCE = 4;
const INPUT_CONSUMED_SEQUENCE = 5;
const OUTPUT_SEQUENCE = 6;

class WVSTLoopbackProcessor extends AudioWorkletProcessor {
  private frames = 0;
  private channels = 0;
  private inputSamples?: Float32Array;
  private outputSamples?: Float32Array;
  private counters?: Int32Array;
  private lastOutputSequence = 0;

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

    if (Atomics.load(counters, INPUT_SEQUENCE) !== Atomics.load(counters, INPUT_CONSUMED_SEQUENCE)) {
      Atomics.add(counters, OVERFLOWS, 1);
    }

    writeInterleavedInput(input, inputSamples, this.frames, this.channels);
    Atomics.add(counters, INPUT_FRAMES, this.frames);
    Atomics.add(counters, INPUT_SEQUENCE, 1);

    const outputSequence = Atomics.load(counters, OUTPUT_SEQUENCE);
    if (outputSequence === this.lastOutputSequence) {
      Atomics.add(counters, UNDERFLOWS, 1);
      clearOutputs(output);
      return true;
    }

    readInterleavedOutput(output, outputSamples, this.frames, this.channels);
    this.lastOutputSequence = outputSequence;
    return true;
  }

  private configure(message: ConfigureMessage): void {
    this.frames = message.frames;
    this.channels = message.channels;
    this.inputSamples = new Float32Array(message.inputBuffer);
    this.outputSamples = new Float32Array(message.outputBuffer);
    this.counters = new Int32Array(message.countersBuffer);
    this.lastOutputSequence = Atomics.load(this.counters, OUTPUT_SEQUENCE);
  }
}

function writeInterleavedInput(
  input: Float32Array[],
  destination: Float32Array,
  frames: number,
  channels: number,
): void {
  for (let frame = 0; frame < frames; frame += 1) {
    for (let channel = 0; channel < channels; channel += 1) {
      const source = input[channel];
      destination[frame * channels + channel] = source ? source[frame] ?? 0 : 0;
    }
  }
}

function readInterleavedOutput(
  output: Float32Array[],
  source: Float32Array,
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
      destination[frame] = source[frame * channels + channel] ?? 0;
    }
  }
}

function clearOutputs(outputs: Float32Array[]): void {
  for (const output of outputs) {
    output.fill(0);
  }
}

registerProcessor("wvst-loopback", WVSTLoopbackProcessor);
