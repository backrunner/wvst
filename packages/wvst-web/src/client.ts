import { AUDIO_FRAME_VERSION } from "./protocol.js";
import type {
  PluginApi,
  PluginFactoryInfo,
  PluginFactoryInfoOptions,
  PluginListOptions,
  PluginScanOptions,
  PluginScanReport,
} from "./plugins.js";
import {
  WebSocketRpcTransport,
  type BridgeMetrics,
  type JsonValue,
  type RpcTransport,
} from "./transport.js";

export interface ConnectOptions {
  endpoint?: string;
  clientName?: string;
  clientVersion?: string;
  requireLowLatency?: boolean;
  token?: string;
}

export interface ProtocolVersion {
  major: number;
  minor: number;
}

export interface HelloParams {
  clientName: string;
  clientVersion: string;
  protocolMin: ProtocolVersion;
  protocolMax: ProtocolVersion;
  audioFrameVersion: number;
  token?: string;
  origin?: string;
}

export interface HelloRequest {
  method: "bridge.hello";
  params: HelloParams;
}

export interface BridgeHelloResult {
  bridgeName: string;
  bridgeVersion: string;
  protocol: ProtocolVersion;
  audioFrameVersion: number;
  pairingRequired: boolean;
  origin?: string;
  client: {
    name: string;
    version: string;
  };
  lowLatency: {
    sharedArrayBufferRequired: boolean;
    crossOriginIsolationRequired: boolean;
  };
  allowedOrigins: string[];
  metrics: BridgeMetrics;
}

export interface LowLatencyPrerequisites {
  sharedArrayBuffer: boolean;
  crossOriginIsolated: boolean;
}

const DEFAULT_ENDPOINT = "ws://127.0.0.1:35876";

export class WVSTClient {
  public readonly plugins: PluginApi;

  private constructor(
    public readonly endpoint: string,
    public readonly clientName: string,
    public readonly clientVersion: string,
    private readonly transport: RpcTransport,
    public readonly hello: BridgeHelloResult,
  ) {
    this.plugins = {
      scan: (options?: PluginScanOptions) =>
        this.request<PluginScanReport>("plugin.scan", options ?? {}),
      list: (options?: PluginListOptions) =>
        this.request<PluginScanReport>("plugin.list", options ?? {}),
      factoryInfo: (options: PluginFactoryInfoOptions) =>
        this.request<PluginFactoryInfo>("plugin.factoryInfo", options),
    };
  }

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

    const endpoint = options.endpoint ?? DEFAULT_ENDPOINT;
    const clientName = options.clientName ?? "@wvst/web";
    const clientVersion = options.clientVersion ?? "0.1.0";
    const transport = await WebSocketRpcTransport.connect(endpoint);

    try {
      const params = createHelloParams(clientName, clientVersion, options.token);
      const hello = await transport.request<BridgeHelloResult>("bridge.hello", params);

      return new WVSTClient(endpoint, clientName, clientVersion, transport, hello);
    } catch (error) {
      transport.close();
      throw error;
    }
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
      params: createHelloParams(this.clientName, this.clientVersion),
    };
  }

  metrics(): Promise<BridgeMetrics> {
    return this.transport.request<BridgeMetrics>("bridge.metrics", {});
  }

  echoAudioFrame(frame: ArrayBuffer): Promise<ArrayBuffer> {
    return this.transport.sendBinary(frame);
  }

  request<T = JsonValue>(method: string, params: unknown): Promise<T> {
    return this.transport.request<T>(method, params);
  }

  close(): void {
    this.transport.close();
  }
}

function createHelloParams(
  clientName: string,
  clientVersion: string,
  token?: string,
): HelloParams {
  return {
    clientName,
    clientVersion,
    protocolMin: { major: 1, minor: 0 },
    protocolMax: { major: 1, minor: 0 },
    audioFrameVersion: AUDIO_FRAME_VERSION,
    token,
    origin: currentOrigin(),
  };
}

function currentOrigin(): string | undefined {
  return typeof globalThis.location === "object" ? globalThis.location.origin : undefined;
}
