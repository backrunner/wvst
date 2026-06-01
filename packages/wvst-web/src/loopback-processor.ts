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

class WVSTLoopbackProcessor extends AudioWorkletProcessor {
  process(inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
    const input = inputs[0] ?? [];
    const output = outputs[0] ?? [];

    for (let channel = 0; channel < output.length; channel += 1) {
      const source = input[channel];
      const destination = output[channel];

      if (source) {
        destination.set(source.subarray(0, destination.length));
      } else {
        destination.fill(0);
      }
    }

    return true;
  }
}

registerProcessor("wvst-loopback", WVSTLoopbackProcessor);

