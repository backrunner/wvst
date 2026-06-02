import type { LoopbackSharedBuffers } from "./loopback.js";
import type { MidiEvent } from "./protocol.js";
import type { JsonValue } from "./transport.js";

export interface BridgeWorkerClientOptions {
  worker: Worker;
}

export interface BridgeWorkerAudioStreamOptions {
  streamId: number;
  sampleRate: number;
  frames: number;
  inputChannels: number;
  outputChannels: number;
  buffers: LoopbackSharedBuffers;
}

export interface BridgeWorkerMidiEventOptions {
  streamId: number;
  events: MidiEvent[];
}

type BridgeWorkerResult = JsonValue | ArrayBuffer | null;

interface BridgeWorkerResponse {
  id: number;
  ok: boolean;
  result?: BridgeWorkerResult;
  error?: string;
}

interface PendingCommand {
  resolve: (value: BridgeWorkerResult) => void;
  reject: (error: Error) => void;
}

export class WVSTBridgeWorkerClient {
  private nextId = 1;
  private readonly pending = new Map<number, PendingCommand>();

  constructor(private readonly options: BridgeWorkerClientOptions) {
    options.worker.addEventListener("message", (event: MessageEvent<BridgeWorkerResponse>) => {
      this.handleMessage(event.data);
    });
    options.worker.addEventListener("error", (event) => {
      this.rejectPending(new Error(event.message || "WVST bridge worker error"));
    });
  }

  connect(endpoint: string): Promise<void> {
    return this.command("connect", { endpoint }).then(() => undefined);
  }

  request<T = JsonValue>(method: string, params: unknown): Promise<T> {
    return this.command("request", { method, params }).then((value) => value as T);
  }

  sendBinary(frame: ArrayBuffer): Promise<ArrayBuffer> {
    return this.command("sendBinary", { frame }, [frame]).then((value) => {
      if (!(value instanceof ArrayBuffer)) {
        throw new Error("WVST bridge worker returned non-binary response");
      }

      return value;
    });
  }

  startAudioStream(options: BridgeWorkerAudioStreamOptions): Promise<void> {
    return this.command("startAudioStream", {
      streamId: options.streamId,
      sampleRate: options.sampleRate,
      frames: options.frames,
      capacityQuanta: options.buffers.capacityQuanta,
      inputChannels: options.inputChannels,
      outputChannels: options.outputChannels,
      inputBuffer: options.buffers.inputBuffer,
      outputBuffer: options.buffers.outputBuffer,
      countersBuffer: options.buffers.countersBuffer,
    }).then(() => undefined);
  }

  stopAudioStream(streamId: number): Promise<void> {
    return this.command("stopAudioStream", { streamId }).then(() => undefined);
  }

  sendMidiEvents(options: BridgeWorkerMidiEventOptions): Promise<void> {
    return this.command("sendMidiEvents", {
      streamId: options.streamId,
      events: options.events,
    }).then(() => undefined);
  }

  close(): Promise<void> {
    return this.command("close", {}).then(() => {
      this.rejectPending(new Error("WVST bridge worker closed"));
    });
  }

  private command(
    type: string,
    payload: Record<string, unknown>,
    transfer?: Transferable[],
  ): Promise<BridgeWorkerResult> {
    const id = this.nextId++;
    const message = { id, type, ...payload };

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.options.worker.postMessage(message, transfer ?? []);
    });
  }

  private handleMessage(response: BridgeWorkerResponse): void {
    const pending = this.pending.get(response.id);
    if (!pending) {
      return;
    }

    this.pending.delete(response.id);

    if (response.ok) {
      pending.resolve(response.result ?? null);
      return;
    }

    pending.reject(new Error(response.error ?? "WVST bridge worker command failed"));
  }

  private rejectPending(error: Error): void {
    for (const pending of this.pending.values()) {
      pending.reject(error);
    }
    this.pending.clear();
  }
}
