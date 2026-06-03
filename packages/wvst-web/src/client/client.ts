import { AUDIO_FRAME_VERSION } from "../protocol/index.js";
import type {
  InstanceApi,
  InstanceParameterGetResult,
  InstanceParametersResult,
  InstanceStatusResult,
  InstanceUnitsResult,
} from "../control/instances.js";
import type { PluginApi } from "../control/plugins.js";
import { createInstanceApi, createPluginApi } from "./apis.js";
import {
  WebSocketRpcTransport,
  type BridgeEvent,
  type BridgeEventListener,
  type BridgeEventKind,
  type BridgeEventsOptions,
  type BridgeEventsResult,
  type BridgeMetrics,
  type JsonValue,
  type RpcTransport,
  type Vst3MetadataInvalidationReason,
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

export interface MetadataInvalidationRefreshOptions {
  includeParameterValues?: boolean;
  maxParameterValues?: number;
}

export interface MetadataInvalidationRefreshResult {
  event: Extract<BridgeEventKind, { type: "vst3-metadata-invalidated" }>;
  status: InstanceStatusResult;
  parameters?: InstanceParametersResult;
  units?: InstanceUnitsResult;
  parameterValues?: InstanceParameterGetResult[];
  skippedParameterValues?: number;
  refreshed: Vst3MetadataInvalidationReason[];
}

export type MetadataInvalidationRefreshListener = (
  result: MetadataInvalidationRefreshResult,
) => void | Promise<void>;

export type MetadataInvalidationRefreshErrorListener = (
  event: Extract<BridgeEventKind, { type: "vst3-metadata-invalidated" }>,
  error: unknown,
) => void;

export interface MetadataInvalidationSubscriptionOptions
  extends MetadataInvalidationRefreshOptions {
  onError?: MetadataInvalidationRefreshErrorListener;
}

const DEFAULT_ENDPOINT = "ws://127.0.0.1:35876";
const DEFAULT_MAX_PARAMETER_VALUES = 128;

export class WVSTClient {
  public readonly instances: InstanceApi;
  public readonly plugins: PluginApi;

  private constructor(
    public readonly endpoint: string,
    public readonly clientName: string,
    public readonly clientVersion: string,
    private readonly transport: RpcTransport,
    public readonly hello: BridgeHelloResult,
  ) {
    this.instances = createInstanceApi(this);
    this.plugins = createPluginApi(this);
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

  events(options: BridgeEventsOptions = {}): Promise<BridgeEventsResult> {
    return this.transport.request<BridgeEventsResult>("bridge.events", options);
  }

  onEvent(listener: BridgeEventListener): () => void {
    return this.transport.onBridgeEvent(listener);
  }

  async refreshMetadataForInvalidation(
    event: Extract<BridgeEventKind, { type: "vst3-metadata-invalidated" }>,
    options: MetadataInvalidationRefreshOptions = {},
  ): Promise<MetadataInvalidationRefreshResult> {
    const status = await this.instances.status({ instanceId: event.instanceId });
    const result: MetadataInvalidationRefreshResult = {
      event,
      status,
      refreshed: [...event.reasons],
    };

    if (shouldRefreshParameters(event.reasons)) {
      const parametersResult = await this.instances.parameters({
        instanceId: event.instanceId,
      });
      result.parameters = parametersResult;
      if (options.includeParameterValues || event.reasons.includes("parameter-values")) {
        const maxParameterValues = options.maxParameterValues ?? DEFAULT_MAX_PARAMETER_VALUES;
        const parameters = parametersResult.parameters.slice(0, maxParameterValues);
        result.parameterValues = await Promise.all(
          parameters.map((parameter) =>
            this.instances.parameterGet({
              instanceId: event.instanceId,
              parameterId: parameter.id,
            }),
          ),
        );
        const skipped = parametersResult.parameters.length - parameters.length;
        if (skipped > 0) {
          result.skippedParameterValues = skipped;
        }
      }
    }

    if (shouldRefreshUnits(event.reasons)) {
      result.units = await this.instances.units({ instanceId: event.instanceId });
    }

    return result;
  }

  onMetadataInvalidated(
    listener: MetadataInvalidationRefreshListener,
    options: MetadataInvalidationSubscriptionOptions = {},
  ): () => void {
    return this.onEvent((event: BridgeEvent) => {
      if (event.kind.type !== "vst3-metadata-invalidated") {
        return;
      }
      const metadataEvent = event.kind;
      void this.refreshMetadataForInvalidation(metadataEvent, options)
        .then(listener)
        .catch((error: unknown) => {
          options.onError?.(metadataEvent, error);
        });
    });
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

function shouldRefreshParameters(reasons: readonly Vst3MetadataInvalidationReason[]): boolean {
  return reasons.some(
    (reason) =>
      reason === "parameter-info" ||
      reason === "parameter-values" ||
      reason === "reload-component",
  );
}

function shouldRefreshUnits(reasons: readonly Vst3MetadataInvalidationReason[]): boolean {
  return reasons.some(
    (reason) =>
      reason === "audio-io" ||
      reason === "routing-info" ||
      reason === "midi-mapping" ||
      reason === "note-expression" ||
      reason === "keyswitches" ||
      reason === "reload-component",
  );
}

function currentOrigin(): string | undefined {
  return typeof globalThis.location === "object" ? globalThis.location.origin : undefined;
}
