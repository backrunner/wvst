import { LoopbackCounter } from "./loopback.js";
import {
  AudioSampleFormat,
  AudioFrameFlags,
  decodeAudioFrame,
  encodeAudioFrame,
  midiEventPayloadBytes,
  parameterAutomationEventPayloadBytes,
  type AudioFrameHeader,
  type MidiEvent,
  type ParameterAutomationEvent,
} from "../protocol/index.js";
import { BridgeWorkerEventQueue } from "./bridge-worker-events.js";
import type {
  BridgeWorkerCommand,
} from "./bridge-worker-messages.js";

const AUDIO_POLL_INTERVAL_MS = 1;
const MAX_AUDIO_PUMP_QUANTA_PER_TICK = 8;

export class AudioStreamPump {
  private readonly inputSamples: Float32Array;
  private readonly outputSamples: Float32Array;
  private readonly counters: Int32Array;
  private readonly inputSamplesPerQuantum: number;
  private readonly outputSamplesPerQuantum: number;
  private readonly inputScratchBuffer: ArrayBuffer;
  private readonly inputScratch: Float32Array;
  private readonly silenceOutput: Float32Array;
  private stopped = false;
  private timer: number | undefined;
  private sequence = 0n;
  private sentFrameTime = 0n;
  private readonly events: BridgeWorkerEventQueue;

  constructor(
    private readonly options: Extract<BridgeWorkerCommand, { type: "startAudioStream" }>,
    private readonly sendFrame: (frame: ArrayBuffer) => Promise<ArrayBuffer>,
  ) {
    this.inputSamples = new Float32Array(options.inputBuffer);
    this.outputSamples = new Float32Array(options.outputBuffer);
    this.counters = new Int32Array(options.countersBuffer);
    this.inputSamplesPerQuantum = options.frames * options.inputChannels;
    this.outputSamplesPerQuantum = options.frames * options.outputChannels;
    this.inputScratchBuffer = new ArrayBuffer(this.inputSamplesPerQuantum * 4);
    this.inputScratch = new Float32Array(this.inputScratchBuffer);
    this.silenceOutput = new Float32Array(this.outputSamplesPerQuantum);
    this.events = new BridgeWorkerEventQueue({
      frames: options.frames,
      counters: this.counters,
      nextInputTargetSequence: () => this.nextInputTargetSequence(),
    });
    const inputSequence = Atomics.load(this.counters, LoopbackCounter.InputSequence);
    Atomics.store(
      this.counters,
      LoopbackCounter.InputConsumedSequence,
      inputSequence,
    );
    const outputSequence = Atomics.load(this.counters, LoopbackCounter.OutputSequence);
    Atomics.store(
      this.counters,
      LoopbackCounter.OutputConsumedSequence,
      outputSequence,
    );
  }

  start(): void {
    this.schedule(0);
  }

  stop(): void {
    this.stopped = true;
    if (this.timer !== undefined) {
      globalThis.clearTimeout(this.timer);
      this.timer = undefined;
    }
  }

  enqueueMidiEvents(events: MidiEvent[]): void {
    this.events.enqueueMidiEvents(events);
  }

  enqueueParameterEvents(events: ParameterAutomationEvent[]): void {
    this.events.enqueueParameterEvents(events);
  }

  private schedule(delay = AUDIO_POLL_INTERVAL_MS): void {
    if (!this.stopped) {
      this.timer = globalThis.setTimeout(() => {
        void this.tick();
      }, delay);
    }
  }

  private async tick(): Promise<void> {
    if (this.stopped) {
      return;
    }

    try {
      let processed = 0;
      while (!this.stopped && processed < MAX_AUDIO_PUMP_QUANTA_PER_TICK) {
        const inputSequence = Atomics.load(this.counters, LoopbackCounter.InputSequence);
        const consumedSequence = Atomics.load(
          this.counters,
          LoopbackCounter.InputConsumedSequence,
        );
        if (inputSequence <= consumedSequence) {
          break;
        }
        await this.processNextInput(inputSequence, consumedSequence);
        processed += 1;
      }
    } catch {
      Atomics.add(this.counters, LoopbackCounter.Overflows, 1);
    } finally {
      const delay = this.pendingInputQuanta() > 0 ? 0 : AUDIO_POLL_INTERVAL_MS;
      this.schedule(delay);
    }
  }

  private pendingInputQuanta(): number {
    return Math.max(
      0,
      Atomics.load(this.counters, LoopbackCounter.InputSequence) -
        Atomics.load(this.counters, LoopbackCounter.InputConsumedSequence),
    );
  }

  private async processNextInput(
    inputSequence: number,
    consumedSequence: number,
  ): Promise<void> {
    let readSequence = consumedSequence + 1;
    // The producer advances its read cursor on overflow, so equality also means
    // the ring is full. Catch up to fresh input instead of repeatedly processing
    // its oldest block just as that block expires.
    if (inputSequence - consumedSequence >= this.options.capacityQuanta) {
      const droppedQuanta = inputSequence - consumedSequence - 1;
      readSequence = inputSequence;
      Atomics.add(this.counters, LoopbackCounter.Overflows, 1);
      if (droppedQuanta > 0) {
        Atomics.add(
          this.counters,
          LoopbackCounter.DroppedInputQuanta,
          droppedQuanta,
        );
      }
    }

    const inputOffset = slotOffset(
      readSequence,
      this.options.capacityQuanta,
      this.inputSamplesPerQuantum,
    );
    this.inputScratch.set(
      this.inputSamples.subarray(inputOffset, inputOffset + this.inputSamplesPerQuantum),
    );
    Atomics.store(this.counters, LoopbackCounter.InputConsumedSequence, readSequence);

    const events = this.events.takeMidiEvents(readSequence);
    const parameterEvents = this.events.takeParameterEvents(readSequence);
    const payloadBytes =
      this.inputScratch.byteLength +
      midiEventPayloadBytes(events.length) +
      parameterAutomationEventPayloadBytes(parameterEvents.length);
    const frame = encodeAudioFrame(
      this.header(payloadBytes, events.length, parameterEvents.length),
      this.inputScratchBuffer,
      events,
      parameterEvents,
    );
    let response: ArrayBuffer;
    try {
      response = await this.sendFrame(frame);
    } catch {
      if (this.stopped) return;
      Atomics.add(this.counters, LoopbackCounter.TransportFailures, 1);
      Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
      this.writeSilenceOutput();
      this.advanceTimeline();
      return;
    }
    // A replaced pump must never publish its in-flight response into the reused SAB.
    if (this.stopped) return;
    if (Atomics.load(this.counters, LoopbackCounter.InputSequence) - readSequence >= this.options.capacityQuanta) {
      Atomics.add(this.counters, LoopbackCounter.DroppedOutputQuanta, 1);
      Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
      this.advanceTimeline();
      return;
    }
    let decoded: ReturnType<typeof decodeAudioFrame>;
    try {
      decoded = decodeAudioFrame(response);
    } catch {
      Atomics.add(this.counters, LoopbackCounter.TransportFailures, 1);
      Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
      this.writeSilenceOutput();
      this.advanceTimeline();
      return;
    }
    const output = new Float32Array(decoded.audioPayload);

    if (
      decoded.header.streamId !== BigInt(this.options.streamId) ||
      decoded.header.sequence !== this.sequence ||
      decoded.header.sentFrameTime !== this.sentFrameTime ||
      decoded.header.sampleRate !== this.options.sampleRate ||
      decoded.header.frames !== this.options.frames ||
      decoded.header.channels !== this.options.outputChannels ||
      decoded.header.format !== AudioSampleFormat.F32Le ||
      output.length !== this.outputSamplesPerQuantum ||
      !output.every(Number.isFinite)
    ) {
      Atomics.add(this.counters, LoopbackCounter.TransportFailures, 1);
      this.writeSilenceOutput();
      Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
    } else {
      if (decoded.header.flags & (AudioFrameFlags.ProcessError | AudioFrameFlags.EndOfStream)) {
        Atomics.add(this.counters, LoopbackCounter.TransportFailures, 1);
        Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
        this.writeSilenceOutput();
      } else if (decoded.header.flags & AudioFrameFlags.Late) {
        Atomics.add(this.counters, LoopbackCounter.DroppedOutputQuanta, 1);
        Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
      } else this.writeOutput(output);
    }

    this.advanceTimeline();
  }

  private advanceTimeline(): void {
    this.sentFrameTime += BigInt(this.options.frames);
    this.sequence += 1n;
  }

  private writeOutput(output: Float32Array): void {
    const nextSequence = Atomics.load(this.counters, LoopbackCounter.OutputSequence) + 1;
    const consumedSequence = Atomics.load(
      this.counters,
      LoopbackCounter.OutputConsumedSequence,
    );
    if (nextSequence - consumedSequence > this.options.capacityQuanta) {
      const droppedQuanta = nextSequence - this.options.capacityQuanta - consumedSequence;
      Atomics.add(this.counters, LoopbackCounter.Overflows, 1);
      if (droppedQuanta > 0) {
        Atomics.add(
          this.counters,
          LoopbackCounter.DroppedOutputQuanta,
          droppedQuanta,
        );
      }
      Atomics.store(
        this.counters,
        LoopbackCounter.OutputConsumedSequence,
        nextSequence - this.options.capacityQuanta,
      );
    }

    this.outputSamples.set(
      output,
      slotOffset(nextSequence, this.options.capacityQuanta, this.outputSamplesPerQuantum),
    );
    Atomics.add(this.counters, LoopbackCounter.OutputFrames, this.options.frames);
    Atomics.store(this.counters, LoopbackCounter.OutputSequence, nextSequence);
  }

  private writeSilenceOutput(): void {
    this.writeOutput(this.silenceOutput);
  }

  private header(
    payloadBytes: number,
    eventCount: number,
    parameterEventCount: number,
  ): AudioFrameHeader {
    return {
      streamId: BigInt(this.options.streamId),
      sequence: this.sequence,
      sentFrameTime: this.sentFrameTime,
      sampleRate: this.options.sampleRate,
      payloadBytes,
      frames: this.options.frames,
      channels: this.options.inputChannels,
      format: AudioSampleFormat.F32Le,
      flags: 0,
      eventCount,
      parameterEventCount,
    };
  }

  private nextInputTargetSequence(): number {
    return Math.max(
      Atomics.load(this.counters, LoopbackCounter.InputSequence),
      Atomics.load(this.counters, LoopbackCounter.InputConsumedSequence),
    ) + 1;
  }
}

function slotOffset(sequence: number, capacityQuanta: number, samplesPerQuantum: number): number {
  return ((sequence - 1) % capacityQuanta) * samplesPerQuantum;
}
