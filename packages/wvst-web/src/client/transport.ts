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
  private readonly eventListeners = new Set<BridgeEventListener>();

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

  onBridgeEvent(listener: BridgeEventListener): () => void {
    this.eventListeners.add(listener);
    return () => {
      this.eventListeners.delete(listener);
    };
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
    let response: RpcResponse | BridgeEventNotification;

    try {
      response = JSON.parse(text) as RpcResponse | BridgeEventNotification;
    } catch {
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
      pending.reject(error);
    }
    this.pendingRpc.clear();

    for (const pending of this.pendingBinary.splice(0)) {
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
