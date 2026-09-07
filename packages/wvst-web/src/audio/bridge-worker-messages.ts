import type { JsonValue, WebSocketRpcTransportOptions } from "../client/transport.js";
import type { MidiEvent, ParameterAutomationEvent } from "../protocol/index.js";

export type BridgeWorkerCommand =
  | {
      id: number;
      type: "connect";
      endpoint: string;
      transportOptions?: WebSocketRpcTransportOptions;
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

export type BridgeWorkerResponse =
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

export interface DedicatedWorkerScope {
  postMessage(message: BridgeWorkerResponse, transfer?: Transferable[]): void;
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<BridgeWorkerCommand>) => void,
  ): void;
}
