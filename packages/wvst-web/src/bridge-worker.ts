import { WebSocketRpcTransport, type JsonValue } from "./transport.js";

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
      case "close":
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

