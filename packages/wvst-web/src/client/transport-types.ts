export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

export interface RpcTransport {
  request<T = JsonValue>(method: string, params: unknown): Promise<T>;
  sendBinary(frame: ArrayBuffer): Promise<ArrayBuffer>;
  onBridgeEvent(listener: BridgeEventListener): () => void;
  close(): void;
}

export interface BridgeMetrics {
  uptimeMs: number;
  websocketConnections: number;
  controlMessages: number;
  binaryFrames: number;
  audioFramesRouted: number;
  audioFrameRouteFailures: number;
  audioFrameInvalidHeaders: number;
  audioFrameInvalidLengths: number;
  audioFrameUnmatchedStreams: number;
  helloRequests: number;
  workerFailures: number;
  workerRestarts: number;
  workerAutoRestarts: number;
  workerShutdowns: number;
  workerKillRequests: number;
  workerTreeKillRequests: number;
  workerForcedKillRequests: number;
  workerWaitSuccesses: number;
  workerWaitTimeouts: number;
  audioSequenceGapEvents: number;
  audioSequenceGapFrames: number;
  audioFramesDuplicate: number;
  audioFramesOutOfOrder: number;
  audioFramesLate: number;
  audioBackpressureDrops: number;
  sharedMemoryProcessBlocks: number;
  sharedMemoryProcessFrames: number;
  sharedMemoryProcessFailures: number;
  sharedMemoryPumpPreflightSkips: number;
  sharedMemoryPumpOverruns: number;
  sharedMemoryPumpInputUnderruns: number;
  sharedMemoryPumpOutputBackpressure: number;
  sharedMemoryPumpWorkerErrors: number;
  sharedMemoryPumpEventsEnqueued: number;
  sharedMemoryPumpEventsDrained: number;
  sharedMemoryPumpEventsLate: number;
  sharedMemoryPumpEventsDropped: number;
  sharedMemoryPumpEventsCleared: number;
  audioRouteLatency: BridgeLatencyMetrics;
  audioInterarrivalJitter: BridgeLatencyMetrics;
  sharedMemoryProcessLatency: BridgeLatencyMetrics;
}

export interface BridgeEventsOptions {
  afterSequence?: number;
}

export interface BridgeEventsResult {
  events: BridgeEvent[];
  lastSequence: number | null;
}

export interface BridgeEvent {
  sequence: number;
  kind: BridgeEventKind;
}

export type BridgeEventListener = (event: BridgeEvent) => void;

export type BridgeEventKind =
  | { type: "server-starting" }
  | { type: "server-started"; localAddr: string }
  | { type: "server-stopping" }
  | { type: "server-stopped" }
  | { type: "worker-starting"; instanceId: number; pluginId: string }
  | { type: "worker-ready"; instanceId: number; pluginId: string }
  | { type: "worker-processing"; instanceId: number }
  | { type: "worker-processing-starting"; instanceId: number; pluginId: string }
  | { type: "worker-processing-stopping"; instanceId: number; pluginId: string }
  | { type: "worker-stopped"; instanceId: number }
  | {
      type: "worker-destroying";
      instanceId: number;
      pluginId: string;
      streamId: number;
    }
  | {
      type: "worker-destroyed";
      instanceId: number;
      pluginId: string;
      streamId: number;
    }
  | { type: "stream-opened"; instanceId: number; pluginId: string; streamId: number }
  | { type: "stream-closing"; instanceId: number; pluginId: string; streamId: number }
  | {
      type: "stream-closed";
      instanceId: number;
      pluginId: string;
      streamId: number;
      drainTimedOut: boolean;
    }
  | {
      type: "worker-failed";
      instanceId: number;
      pluginId: string;
      code: number;
      message: string;
      errorData?: JsonValue;
    }
  | {
      type: "worker-recovering";
      instanceId: number;
      pluginId: string;
      mode: WorkerRecoveryMode;
      reason: string;
      errorData?: JsonValue;
    }
  | {
      type: "worker-recovered";
      instanceId: number;
      pluginId: string;
      processingRestored: boolean;
      mode: WorkerRecoveryMode;
    }
  | {
      type: "worker-recovery-failed";
      instanceId: number;
      pluginId: string;
      mode: WorkerRecoveryMode;
      reason: string;
      errorData?: JsonValue;
    }
  | {
      type: "worker-quarantined";
      pluginId: string;
      failures: number;
      releaseAfterMs: number;
    }
  | { type: "worker-quarantine-released"; pluginId: string }
  | {
      type: "worker-policy-decision";
      instanceId?: number;
      pluginId?: string;
      policy: string;
      decision: string;
      reason: string;
      data?: JsonValue;
    }
  | {
      type: "vst3-component-handler-event";
      instanceId: number;
      pluginId: string;
      streamId: number;
      handlerSequence: number;
      handlerKind: Vst3ComponentHandlerEventKind;
      parameterId?: number;
      valueNormalized?: number;
      flags?: number;
      restartFlags?: Vst3RestartFlags;
      dirty?: boolean;
      editorName?: string;
    }
  | {
      type: "vst3-component-handler-events-lost";
      instanceId: number;
      pluginId: string;
      streamId: number;
      fromSequence: number;
      toSequence: number;
      lostCount: number;
    }
  | {
      type: "vst3-metadata-invalidated";
      instanceId: number;
      pluginId: string;
      streamId: number;
      handlerSequence: number;
      reasons: Vst3MetadataInvalidationReason[];
      refreshPolicy: Vst3MetadataRefreshPolicy;
      restartFlags: Vst3RestartFlags;
    };

export type WorkerRecoveryMode = "manual-restart" | "auto-heartbeat";

export type Vst3MetadataInvalidationReason =
  | "reload-component"
  | "audio-io"
  | "parameter-values"
  | "parameter-info"
  | "latency"
  | "midi-mapping"
  | "note-expression"
  | "routing-info"
  | "prefetchable-support"
  | "keyswitches";

export type Vst3MetadataRefreshPolicy =
  | "refresh-metadata"
  | "rebuild-audio-graph"
  | "reload-component";

export type Vst3ComponentHandlerEventKind =
  | "begin-edit"
  | "perform-edit"
  | "end-edit"
  | "restart-component"
  | "set-dirty"
  | "request-open-editor"
  | "start-group-edit"
  | "finish-group-edit";

export interface Vst3RestartFlags {
  raw: number;
  reloadComponent: boolean;
  ioChanged: boolean;
  paramValuesChanged: boolean;
  latencyChanged: boolean;
  paramTitlesChanged: boolean;
  midiCcAssignmentChanged: boolean;
  noteExpressionChanged: boolean;
  ioTitlesChanged: boolean;
  prefetchableSupportChanged: boolean;
  routingInfoChanged: boolean;
  keyswitchChanged: boolean;
  paramIdMappingChanged: boolean;
  unknownBits: number;
}

export interface BridgeLatencyMetrics {
  count: number;
  p50Us: number | null;
  p95Us: number | null;
  p99Us: number | null;
  buckets: BridgeLatencyBucket[];
}

export interface BridgeLatencyBucket {
  leUs: number | null;
  count: number;
}
