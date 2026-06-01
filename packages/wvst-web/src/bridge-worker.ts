import { WebSocketRpcTransport, type JsonValue } from "./transport.js";
import { LoopbackCounter } from "./loopback.js";
import {
  AudioSampleFormat,
  decodeAudioFrame,
  encodeAudioFrame,
  type AudioFrameHeader,
} from "./protocol.js";

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

function stopAllAudioStreams() {
  for (const pump of audioPumps.values()) {
    pump.stop();
  }
  audioPumps.clear();
}

const AUDIO_POLL_INTERVAL_MS = 1;

class AudioStreamPump {
  private readonly inputSamples: Float32Array;
  private readonly outputSamples: Float32Array;
  private readonly counters: Int32Array;
  private stopped = false;
  private timer: number | undefined;
  private lastInputSequence = 0;
  private outputSequence = 0;
  private sequence = 0n;
  private sentFrameTime = 0n;

  constructor(private readonly options: Extract<BridgeWorkerCommand, { type: "startAudioStream" }>) {
    this.inputSamples = new Float32Array(options.inputBuffer);
    this.outputSamples = new Float32Array(options.outputBuffer);
    this.counters = new Int32Array(options.countersBuffer);
    this.lastInputSequence = Atomics.load(this.counters, LoopbackCounter.InputSequence);
    Atomics.store(
      this.counters,
      LoopbackCounter.InputConsumedSequence,
      this.lastInputSequence,
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
      const inputSequence = Atomics.load(this.counters, LoopbackCounter.InputSequence);
      if (inputSequence !== this.lastInputSequence) {
        await this.processLatestInput(inputSequence);
      }
    } catch {
      Atomics.add(this.counters, LoopbackCounter.Overflows, 1);
    } finally {
      this.schedule();
    }
  }

  private async processLatestInput(inputSequence: number): Promise<void> {
    const inputCopy = new Float32Array(this.inputSamples.length);
    inputCopy.set(this.inputSamples);
    Atomics.store(this.counters, LoopbackCounter.InputConsumedSequence, inputSequence);
    this.lastInputSequence = inputSequence;

    const frame = encodeAudioFrame(this.header(inputCopy.byteLength), inputCopy.buffer);
    const response = await requireTransport().sendBinary(frame);
    const decoded = decodeAudioFrame(response);
    const output = new Float32Array(decoded.payload);

    if (
      decoded.header.streamId !== BigInt(this.options.streamId) ||
      decoded.header.frames !== this.options.frames ||
      output.length !== this.outputSamples.length
    ) {
      this.outputSamples.fill(0);
      Atomics.add(this.counters, LoopbackCounter.Underflows, 1);
    } else {
      this.outputSamples.set(output);
    }

    this.outputSequence = (this.outputSequence + 1) | 0;
    Atomics.add(this.counters, LoopbackCounter.OutputFrames, decoded.header.frames);
    Atomics.store(this.counters, LoopbackCounter.OutputSequence, this.outputSequence);
    this.sentFrameTime += BigInt(this.options.frames);
    this.sequence += 1n;
  }

  private header(payloadBytes: number): AudioFrameHeader {
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
      eventCount: 0,
    };
  }
}
