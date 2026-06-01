import { AUDIO_FRAME_VERSION } from "./protocol.js";

export interface ConnectOptions {
  endpoint?: string;
  clientName?: string;
  clientVersion?: string;
  requireLowLatency?: boolean;
}

export interface HelloRequest {
  method: "bridge.hello";
  params: {
    clientName: string;
    clientVersion: string;
    protocolMin: { major: number; minor: number };
    protocolMax: { major: number; minor: number };
    audioFrameVersion: number;
  };
}

export interface LowLatencyPrerequisites {
  sharedArrayBuffer: boolean;
  crossOriginIsolated: boolean;
}

const DEFAULT_ENDPOINT = "ws://127.0.0.1:35876";

export class WVSTClient {
  private constructor(
    public readonly endpoint: string,
    public readonly clientName: string,
    public readonly clientVersion: string,
  ) {}

  static async connect(options: ConnectOptions = {}): Promise<WVSTClient> {
    const requireLowLatency = options.requireLowLatency ?? true;

    if (requireLowLatency) {
      const prerequisites = WVSTClient.lowLatencyPrerequisites();

      if (!prerequisites.sharedArrayBuffer || !prerequisites.crossOriginIsolated) {
        throw new Error(
          "WVST low-latency mode requires SharedArrayBuffer and cross-origin isolation",
        );
      }
    }

    return new WVSTClient(
      options.endpoint ?? DEFAULT_ENDPOINT,
      options.clientName ?? "@wvst/web",
      options.clientVersion ?? "0.1.0",
    );
  }

  static lowLatencyPrerequisites(): LowLatencyPrerequisites {
    return {
      sharedArrayBuffer: typeof globalThis.SharedArrayBuffer === "function",
      crossOriginIsolated: globalThis.crossOriginIsolated === true,
    };
  }

  createHelloRequest(): HelloRequest {
    return {
      method: "bridge.hello",
      params: {
        clientName: this.clientName,
        clientVersion: this.clientVersion,
        protocolMin: { major: 1, minor: 0 },
        protocolMax: { major: 1, minor: 0 },
        audioFrameVersion: AUDIO_FRAME_VERSION,
      },
    };
  }
}

