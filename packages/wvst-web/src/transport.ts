export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

export interface RpcTransport {
  request<T = JsonValue>(method: string, params: unknown): Promise<T>;
  sendBinary(frame: ArrayBuffer): Promise<ArrayBuffer>;
  close(): void;
}

export interface BridgeMetrics {
  uptimeMs: number;
  websocketConnections: number;
  controlMessages: number;
  binaryFrames: number;
  audioFramesRouted: number;
  audioFrameFallbacks: number;
  audioFrameRouteFailures: number;
  helloRequests: number;
  workerFailures: number;
  workerRestarts: number;
  workerAutoRestarts: number;
  audioSequenceGapEvents: number;
  audioSequenceGapFrames: number;
  audioFramesDuplicate: number;
  audioFramesOutOfOrder: number;
  audioFramesLate: number;
  audioRouteLatency: BridgeLatencyMetrics;
  audioInterarrivalJitter: BridgeLatencyMetrics;
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

export type BridgeEventKind =
  | { type: "server-starting" }
  | { type: "server-started"; localAddr: string }
  | { type: "server-stopping" }
  | { type: "server-stopped" }
  | { type: "worker-starting"; instanceId: number; pluginId: string }
  | { type: "worker-ready"; instanceId: number; pluginId: string }
  | { type: "worker-processing"; instanceId: number }
  | { type: "worker-stopped"; instanceId: number }
  | {
      type: "worker-failed";
      instanceId: number;
      pluginId: string;
      code: number;
      message: string;
    }
  | { type: "worker-recovering"; instanceId: number; pluginId: string }
  | {
      type: "worker-recovered";
      instanceId: number;
      pluginId: string;
      processingRestored: boolean;
    }
  | { type: "worker-quarantined"; pluginId: string; failures: number }
  | { type: "worker-quarantine-released"; pluginId: string };

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

interface RpcResponse {
  id: string | number | null;
  result?: JsonValue;
  error?: {
    code: number;
    message: string;
    data?: JsonValue;
  };
}

type PendingRpc = {
  resolve: (value: JsonValue) => void;
  reject: (error: Error) => void;
};

type PendingBinary = {
  resolve: (value: ArrayBuffer) => void;
  reject: (error: Error) => void;
};

export class WVSTBridgeError extends Error {
  constructor(
    public readonly code: number,
    message: string,
    public readonly data?: JsonValue,
  ) {
    super(message);
    this.name = "WVSTBridgeError";
  }
}

export class WebSocketRpcTransport implements RpcTransport {
  private nextId = 1;
  private readonly pendingRpc = new Map<string, PendingRpc>();
  private readonly pendingBinary: PendingBinary[] = [];

  private constructor(private readonly socket: WebSocket) {
    this.socket.binaryType = "arraybuffer";
    this.socket.addEventListener("message", (event) => {
      void this.handleMessage(event);
    });
    this.socket.addEventListener("close", () => {
      this.rejectPending(new Error("WVST bridge connection closed"));
    });
  }

  static connect(endpoint: string): Promise<WebSocketRpcTransport> {
    return new Promise((resolve, reject) => {
      const socket = new WebSocket(endpoint);

      const handleOpen = (): void => {
        cleanup();
        resolve(new WebSocketRpcTransport(socket));
      };
      const handleError = (): void => {
        cleanup();
        reject(new Error(`failed to connect to WVST bridge at ${endpoint}`));
      };
      const cleanup = (): void => {
        socket.removeEventListener("open", handleOpen);
        socket.removeEventListener("error", handleError);
      };

      socket.addEventListener("open", handleOpen);
      socket.addEventListener("error", handleError);
    });
  }

  request<T = JsonValue>(method: string, params: unknown): Promise<T> {
    const id = this.nextId++;
    const envelope = {
      jsonrpc: "2.0",
      id,
      method,
      params,
    };

    return new Promise((resolve, reject) => {
      this.pendingRpc.set(String(id), {
        resolve: (value) => resolve(value as T),
        reject,
      });
      this.socket.send(JSON.stringify(envelope));
    });
  }

  sendBinary(frame: ArrayBuffer): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) => {
      this.pendingBinary.push({ resolve, reject });
      this.socket.send(frame);
    });
  }

  close(): void {
    this.socket.close();
    this.rejectPending(new Error("WVST bridge connection closed"));
  }

  private async handleMessage(event: MessageEvent): Promise<void> {
    if (typeof event.data === "string") {
      this.handleText(event.data);
      return;
    }

    const frame = await messageDataToArrayBuffer(event.data);
    const pending = this.pendingBinary.shift();

    if (pending) {
      pending.resolve(frame);
    }
  }

  private handleText(text: string): void {
    let response: RpcResponse;

    try {
      response = JSON.parse(text) as RpcResponse;
    } catch {
      return;
    }

    const pending = this.pendingRpc.get(String(response.id));
    if (!pending) {
      return;
    }

    this.pendingRpc.delete(String(response.id));

    if (response.error) {
      pending.reject(
        new WVSTBridgeError(response.error.code, response.error.message, response.error.data),
      );
      return;
    }

    pending.resolve(response.result ?? null);
  }

  private rejectPending(error: Error): void {
    for (const pending of this.pendingRpc.values()) {
      pending.reject(error);
    }
    this.pendingRpc.clear();

    for (const pending of this.pendingBinary.splice(0)) {
      pending.reject(error);
    }
  }
}

async function messageDataToArrayBuffer(data: unknown): Promise<ArrayBuffer> {
  if (data instanceof ArrayBuffer) {
    return data;
  }

  if (ArrayBuffer.isView(data)) {
    const copy = new Uint8Array(data.byteLength);
    copy.set(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
    return copy.buffer;
  }

  if (data instanceof Blob) {
    return data.arrayBuffer();
  }

  throw new Error("unsupported WVST bridge binary response");
}
