import type {
  BridgeEvent,
  BridgeEventListener,
  JsonValue,
  RpcTransport,
} from "./transport-types.js";

export type {
  BridgeEvent,
  BridgeEventKind,
  BridgeEventListener,
  BridgeEventsOptions,
  BridgeEventsResult,
  BridgeLatencyBucket,
  BridgeLatencyMetrics,
  BridgeMetrics,
  JsonPrimitive,
  JsonValue,
  RpcTransport,
  Vst3ComponentHandlerEventKind,
  Vst3MetadataInvalidationReason,
  Vst3MetadataRefreshPolicy,
  Vst3RestartFlags,
  WorkerRecoveryMode,
} from "./transport-types.js";

interface RpcResponse {
  id: string | number | null;
  result?: JsonValue;
  error?: {
    code: number;
    message: string;
    data?: JsonValue;
  };
}

interface BridgeEventNotification {
  jsonrpc: "2.0";
  method: "bridge.event";
  params?: {
    event?: BridgeEvent;
  };
}

type PendingRpc = {
  resolve: (value: JsonValue) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};

type PendingBinary = {
  resolve: (value: ArrayBuffer) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
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

export interface WebSocketRpcTransportOptions {
  connectTimeoutMs?: number;
  requestTimeoutMs?: number;
  binaryTimeoutMs?: number;
  maxPendingRequests?: number;
}

const DEFAULT_OPTIONS: Required<WebSocketRpcTransportOptions> = {
  connectTimeoutMs: 10_000,
  requestTimeoutMs: 180_000,
  binaryTimeoutMs: 10_000,
  maxPendingRequests: 256,
};

export class WebSocketRpcTransport implements RpcTransport {
  private nextId = 1;
  private readonly pendingRpc = new Map<string, PendingRpc>();
  private readonly pendingBinary: PendingBinary[] = [];
  private readonly eventListeners = new Set<BridgeEventListener>();

  private closed = false;

  private constructor(
    private readonly socket: WebSocket,
    private readonly options: Required<WebSocketRpcTransportOptions>,
  ) {
    this.socket.binaryType = "arraybuffer";
    this.socket.addEventListener("message", (event) => {
      void this.handleMessage(event).catch((error: unknown) => {
        this.fail(error instanceof Error ? error : new Error(String(error)));
      });
    });
    this.socket.addEventListener("close", () => {
      this.closed = true;
      this.rejectPending(new Error("WVST bridge connection closed"));
    });
  }

  static connect(endpoint: string, options: WebSocketRpcTransportOptions = {}): Promise<WebSocketRpcTransport> {
    const config = { ...DEFAULT_OPTIONS, ...options };
    for (const [name, value] of Object.entries(config)) {
      if (!Number.isSafeInteger(value) || value <= 0 || value > 0x7fffffff) {
        return Promise.reject(new Error(`WVST ${name} must be a positive 32-bit integer`));
      }
    }
    return new Promise((resolve, reject) => {
      const socket = new WebSocket(endpoint);

      const handleOpen = (): void => {
        cleanup();
        resolve(new WebSocketRpcTransport(socket, config));
      };
      const handleError = (): void => {
        cleanup();
        socket.close();
        reject(new Error(`failed to connect to WVST bridge at ${endpoint}`));
      };
      const cleanup = (): void => {
        socket.removeEventListener("open", handleOpen);
        socket.removeEventListener("error", handleError);
        socket.removeEventListener("close", handleError);
        clearTimeout(timer);
      };

      const timer = setTimeout(() => {
        cleanup();
        socket.close();
        reject(new Error(`WVST bridge connection timed out after ${config.connectTimeoutMs}ms`));
      }, config.connectTimeoutMs);
      socket.addEventListener("close", handleError);
      socket.addEventListener("open", handleOpen);
      socket.addEventListener("error", handleError);
    });
  }

  request<T = JsonValue>(method: string, params: unknown): Promise<T> {
    if (this.closed || this.socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("WVST bridge connection closed"));
    }
    if (this.pendingRpc.size + this.pendingBinary.length >= this.options.maxPendingRequests) {
      return Promise.reject(new Error("WVST bridge request queue is full"));
    }
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
        timer: setTimeout(() => this.fail(new Error(`WVST ${method} request timed out`)), this.options.requestTimeoutMs),
      });
      try {
        this.socket.send(JSON.stringify(envelope));
      } catch (error) {
        clearTimeout(this.pendingRpc.get(String(id))?.timer);
        this.pendingRpc.delete(String(id));
        reject(error);
      }
    });
  }

  sendBinary(frame: ArrayBuffer): Promise<ArrayBuffer> {
    if (this.closed || this.socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("WVST bridge connection closed"));
    }
    if (this.pendingRpc.size + this.pendingBinary.length >= this.options.maxPendingRequests) {
      return Promise.reject(new Error("WVST bridge request queue is full"));
    }
    return new Promise((resolve, reject) => {
      const pending = {
        resolve, reject,
        // Binary responses are FIFO. Close on timeout so late responses cannot
        // be mistaken for a subsequent request after its queue entry expires.
        timer: setTimeout(() => this.fail(new Error("WVST audio response timed out")), this.options.binaryTimeoutMs),
      };
      this.pendingBinary.push(pending);
      try {
        this.socket.send(frame);
      } catch (error) {
        clearTimeout(pending.timer);
        const index = this.pendingBinary.indexOf(pending);
        if (index !== -1) this.pendingBinary.splice(index, 1);
        reject(error);
      }
    });
  }

  onBridgeEvent(listener: BridgeEventListener): () => void {
    this.eventListeners.add(listener);
    return () => {
      this.eventListeners.delete(listener);
    };
  }

  close(): void {
    this.fail(new Error("WVST bridge connection closed"));
  }

  private fail(error: Error): void {
    if (this.closed) return;
    this.closed = true;
    this.rejectPending(error);
    this.socket.close();
  }

  private async handleMessage(event: MessageEvent): Promise<void> {
    if (typeof event.data === "string") {
      this.handleText(event.data);
      return;
    }

    const frame = await messageDataToArrayBuffer(event.data);
    const pending = this.pendingBinary.shift();

    if (pending) {
      clearTimeout(pending.timer);
      pending.resolve(frame);
    }
  }

  private handleText(text: string): void {
    let response: RpcResponse | BridgeEventNotification;

    try {
      response = JSON.parse(text) as RpcResponse | BridgeEventNotification;
    } catch {
      return;
    }

    if (!response || typeof response !== "object" || Array.isArray(response)) {
      return;
    }

    if (isBridgeEventNotification(response)) {
      this.emitBridgeEvent(response.params?.event);
      return;
    }

    const pending = this.pendingRpc.get(String(response.id));
    if (!pending) {
      return;
    }

    clearTimeout(pending.timer);
    this.pendingRpc.delete(String(response.id));

    if (response.error) {
      pending.reject(
        new WVSTBridgeError(response.error.code, response.error.message, response.error.data),
      );
      return;
    }

    pending.resolve(response.result ?? null);
  }

  private emitBridgeEvent(event: BridgeEvent | undefined): void {
    if (!event) {
      return;
    }

    for (const listener of this.eventListeners) {
      listener(event);
    }
  }

  private rejectPending(error: Error): void {
    for (const pending of this.pendingRpc.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.pendingRpc.clear();

    for (const pending of this.pendingBinary.splice(0)) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
  }
}

function isBridgeEventNotification(
  value: RpcResponse | BridgeEventNotification,
): value is BridgeEventNotification {
  return "method" in value && value.method === "bridge.event";
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
