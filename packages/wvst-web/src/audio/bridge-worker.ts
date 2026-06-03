import { WebSocketRpcTransport, type JsonValue } from "../client/transport.js";
import { LoopbackCounter } from "./loopback.js";
import {
  AudioSampleFormat,
  decodeAudioFrame,
  encodeAudioFrame,
  midiEventPayloadBytes,
  parameterAutomationEventPayloadBytes,
  type AudioFrameHeader,
  type MidiEvent,
  type ParameterAutomationEvent,
} from "../protocol/index.js";

type BridgeWorkerCommand =
  | {
      id: number;
      type: "connect";
      endpoint: string;
    }
  | {
      id: number;
      type: "request";
      method: string;
      params: unknown;
    }
  | {
      id: number;
      type: "sendBinary";
      frame: ArrayBuffer;
    }
  | {
      id: number;
      type: "startAudioStream";
      streamId: number;
      sampleRate: number;
      frames: number;
      capacityQuanta: number;
      inputChannels: number;
      outputChannels: number;
      inputBuffer: SharedArrayBuffer;
      outputBuffer: SharedArrayBuffer;
      countersBuffer: SharedArrayBuffer;
    }
  | {
      id: number;
      type: "stopAudioStream";
      streamId: number;
    }
  | {
      id: number;
      type: "sendMidiEvents";
      streamId: number;
      events: MidiEvent[];
    }
  | {
      id: number;
      type: "sendParameterEvents";
      streamId: number;
      events: ParameterAutomationEvent[];
    }
  | {
      id: number;
      type: "close";
    };

type BridgeWorkerResponse =
  | {
      id: number;
      ok: true;
      result: JsonValue | ArrayBuffer | null;
    }
  | {
      id: number;
      ok: false;
      error: string;
    };

interface DedicatedWorkerScope {
  postMessage(message: BridgeWorkerResponse, transfer?: Transferable[]): void;
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<BridgeWorkerCommand>) => void,
  ): void;
}

const workerScope = globalThis as unknown as DedicatedWorkerScope;
let transport: WebSocketRpcTransport | undefined;
const audioPumps = new Map<number, AudioStreamPump>();

workerScope.addEventListener("message", (event) => {
  void handleCommand(event.data);
});

async function handleCommand(command: BridgeWorkerCommand): Promise<void> {
  try {
    switch (command.type) {
      case "connect":
        transport = await WebSocketRpcTransport.connect(command.endpoint);
        postResult(command.id, null);
        return;
      case "request":
        postResult(command.id, await requireTransport().request(command.method, command.params));
        return;
      case "sendBinary": {
        const echoed = await requireTransport().sendBinary(command.frame);
        postResult(command.id, echoed, [echoed]);
        return;
      }
      case "startAudioStream":
        startAudioStream(command);
        postResult(command.id, null);
        return;
      case "stopAudioStream":
        stopAudioStream(command.streamId);
        postResult(command.id, null);
        return;
      case "sendMidiEvents":
        enqueueMidiEvents(command.streamId, command.events);
        postResult(command.id, null);
        return;
      case "sendParameterEvents":
        enqueueParameterEvents(command.streamId, command.events);
        postResult(command.id, null);
        return;
      case "close":
        stopAllAudioStreams();
        requireTransport().close();
        transport = undefined;
        postResult(command.id, null);
        return;
    }
  } catch (error) {
    postError(command.id, error);
  }
}

function requireTransport(): WebSocketRpcTransport {
  if (!transport) {
    throw new Error("WVST bridge worker is not connected");
  }

  return transport;
}

function postResult(id: number, result: JsonValue | ArrayBuffer | null, transfer?: Transferable[]) {
  workerScope.postMessage({ id, ok: true, result }, transfer);
}

function postError(id: number, error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  workerScope.postMessage({ id, ok: false, error: message });
}

function startAudioStream(command: Extract<BridgeWorkerCommand, { type: "startAudioStream" }>) {
  stopAudioStream(command.streamId);
  const pump = new AudioStreamPump(command);
  audioPumps.set(command.streamId, pump);
  pump.start();
}

function stopAudioStream(streamId: number) {
  const pump = audioPumps.get(streamId);
  if (pump) {
    pump.stop();
    audioPumps.delete(streamId);
  }
}

function enqueueMidiEvents(streamId: number, events: MidiEvent[]) {
  const pump = audioPumps.get(streamId);
  if (!pump) {
    throw new Error(`WVST audio stream is not active: ${streamId}`);
  }

  pump.enqueueMidiEvents(events);
}

function enqueueParameterEvents(streamId: number, events: ParameterAutomationEvent[]) {
  const pump = audioPumps.get(streamId);
  if (!pump) {
    throw new Error(`WVST audio stream is not active: ${streamId}`);
  }

  pump.enqueueParameterEvents(events);
}

function stopAllAudioStreams() {
  for (const pump of audioPumps.values()) {
    pump.stop();
  }
  audioPumps.clear();
}

const AUDIO_POLL_INTERVAL_MS = 1;
const MAX_AUDIO_PUMP_QUANTA_PER_TICK = 8;
const MAX_PENDING_MIDI_EVENTS = 4_096;
const MAX_PENDING_PARAMETER_EVENTS = 4_096;

interface QueuedAudioEvent<TEvent> {
  event: TEvent;
  targetSequence: number;
}

class AudioStreamPump {
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
  private pendingMidiEvents: QueuedAudioEvent<MidiEvent>[] = [];
  private pendingParameterEvents: QueuedAudioEvent<ParameterAutomationEvent>[] = [];

  constructor(private readonly options: Extract<BridgeWorkerCommand, { type: "startAudioStream" }>) {
    this.inputSamples = new Float32Array(options.inputBuffer);
    this.outputSamples = new Float32Array(options.outputBuffer);
    this.counters = new Int32Array(options.countersBuffer);
    this.inputSamplesPerQuantum = options.frames * options.inputChannels;
    this.outputSamplesPerQuantum = options.frames * options.outputChannels;
    this.inputScratchBuffer = new ArrayBuffer(this.inputSamplesPerQuantum * 4);
    this.inputScratch = new Float32Array(this.inputScratchBuffer);
    this.silenceOutput = new Float32Array(this.outputSamplesPerQuantum);
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
    if (this.pendingMidiEvents.length + events.length > MAX_PENDING_MIDI_EVENTS) {
      Atomics.add(this.counters, LoopbackCounter.Overflows, 1);
      Atomics.add(this.counters, LoopbackCounter.DroppedMidiEvents, events.length);
      throw new Error("WVST MIDI event queue is full");
    }

    const targetSequence = this.nextInputTargetSequence();
    for (const event of events) {
      if (
        !Number.isInteger(event.sampleOffset) ||
        event.sampleOffset < 0 ||
        event.sampleOffset >= this.options.frames
      ) {
        throw new Error(
          `WVST MIDI sampleOffset must be an integer in [0, ${this.options.frames})`,
        );
      }
      this.pendingMidiEvents.push({ event, targetSequence });
    }
  }

  enqueueParameterEvents(events: ParameterAutomationEvent[]): void {
    if (this.pendingParameterEvents.length + events.length > MAX_PENDING_PARAMETER_EVENTS) {
      Atomics.add(this.counters, LoopbackCounter.Overflows, 1);
      Atomics.add(
        this.counters,
        LoopbackCounter.DroppedParameterEvents,
        events.length,
      );
      throw new Error("WVST parameter event queue is full");
    }

    const targetSequence = this.nextInputTargetSequence();
    for (const event of events) {
      if (
        !Number.isInteger(event.sampleOffset) ||
        event.sampleOffset < 0 ||
        event.sampleOffset >= this.options.frames
      ) {
        throw new Error(
          `WVST parameter sampleOffset must be an integer in [0, ${this.options.frames})`,
        );
      }
      if (
        !Number.isInteger(event.parameterId) ||
        event.parameterId < 0 ||
        event.parameterId > 0xffffffff
      ) {
        throw new Error(`invalid WVST parameter id: ${event.parameterId}`);
      }
      if (
        !Number.isFinite(event.valueNormalized) ||
        event.valueNormalized < 0 ||
        event.valueNormalized > 1
      ) {
        throw new Error(
          `invalid WVST normalized parameter value: ${event.valueNormalized}`,
        );
      }
      this.pendingParameterEvents.push({ event, targetSequence });
    }
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
    if (inputSequence - consumedSequence > this.options.capacityQuanta) {
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

    const events = this.takeMidiEvents(readSequence);
    const parameterEvents = this.takeParameterEvents(readSequence);
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
      response = await requireTransport().sendBinary(frame);
    } catch {
      Atomics.add(this.counters, LoopbackCounter.TransportFailures, 1);
      Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
      this.writeSilenceOutput();
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
      decoded.header.frames !== this.options.frames ||
      decoded.header.channels !== this.options.outputChannels ||
      output.length !== this.outputSamplesPerQuantum
    ) {
      Atomics.add(this.counters, LoopbackCounter.TransportFailures, 1);
      this.writeSilenceOutput();
      Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
    } else {
      this.writeOutput(output);
    }

    this.advanceTimeline();
  }

  private advanceTimeline(): void {
    this.sentFrameTime += BigInt(this.options.frames);
    this.sequence += 1n;
  }

  private takeMidiEvents(readSequence: number): MidiEvent[] {
    if (this.pendingMidiEvents.length === 0) {
      return [];
    }

    const { due, pending, late } = takeDueEvents(this.pendingMidiEvents, readSequence);
    this.pendingMidiEvents = pending;
    if (late > 0) {
      Atomics.add(this.counters, LoopbackCounter.LateMidiEvents, late);
      Atomics.add(this.counters, LoopbackCounter.DroppedMidiEvents, late);
    }
    due.sort((left, right) => left.sampleOffset - right.sampleOffset);
    return due;
  }

  private takeParameterEvents(readSequence: number): ParameterAutomationEvent[] {
    if (this.pendingParameterEvents.length === 0) {
      return [];
    }

    const { due, pending, late } = takeDueEvents(
      this.pendingParameterEvents,
      readSequence,
    );
    this.pendingParameterEvents = pending;
    if (late > 0) {
      Atomics.add(this.counters, LoopbackCounter.LateParameterEvents, late);
      Atomics.add(this.counters, LoopbackCounter.DroppedParameterEvents, late);
    }
    due.sort((left, right) => {
      const byOffset = left.sampleOffset - right.sampleOffset;
      return byOffset === 0 ? left.parameterId - right.parameterId : byOffset;
    });
    return due;
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

function takeDueEvents<TEvent>(
  events: QueuedAudioEvent<TEvent>[],
  readSequence: number,
): { due: TEvent[]; pending: QueuedAudioEvent<TEvent>[]; late: number } {
  const due: TEvent[] = [];
  const pending: QueuedAudioEvent<TEvent>[] = [];
  let late = 0;

  for (const item of events) {
    if (item.targetSequence < readSequence) {
      late += 1;
    } else if (item.targetSequence === readSequence) {
      due.push(item.event);
    } else {
      pending.push(item);
    }
  }

  return { due, pending, late };
}
