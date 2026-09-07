import { afterEach, describe, expect, it, vi } from "vitest";
import { WebSocketRpcTransport } from "../src/client/transport.js";

class MockSocket extends EventTarget {
  static OPEN = 1;
  static latest: MockSocket;
  readyState = 0;
  binaryType = "blob";
  send = vi.fn();

  constructor() {
    super();
    MockSocket.latest = this;
  }

  close() {
    this.readyState = 3;
    this.dispatchEvent(new Event("close"));
  }

  message(data: unknown) {
    this.dispatchEvent(new MessageEvent("message", { data }));
  }
}

async function connect() {
  vi.stubGlobal("WebSocket", MockSocket);
  const connected = WebSocketRpcTransport.connect("ws://localhost:35876");
  const socket = MockSocket.latest;
  socket.readyState = MockSocket.OPEN;
  socket.dispatchEvent(new Event("open"));
  return { transport: await connected, socket };
}

afterEach(() => vi.unstubAllGlobals());

describe("WebSocket transport lifecycle", () => {
  it("rejects requests submitted after the socket closes", async () => {
    const { transport, socket } = await connect();
    socket.close();
    await expect(transport.request("bridge.metrics", {})).rejects.toThrow(/closed/);
    await expect(transport.sendBinary(new ArrayBuffer(4))).rejects.toThrow(/closed/);
    expect(socket.send).not.toHaveBeenCalled();
  });

  it("does not let a failed binary send consume the next response", async () => {
    const { transport, socket } = await connect();
    socket.send.mockImplementationOnce(() => { throw new Error("send failed"); });
    await expect(transport.sendBinary(new ArrayBuffer(4))).rejects.toThrow("send failed");
    const response = new ArrayBuffer(8);
    const pending = transport.sendBinary(new ArrayBuffer(4));
    socket.message(response);
    await expect(pending).resolves.toBe(response);
  });

  it("ignores JSON primitives and still resolves the next valid RPC response", async () => {
    const { transport, socket } = await connect();
    const pending = transport.request("bridge.metrics", {});
    socket.message("null");
    socket.message("42");
    socket.message(JSON.stringify({ id: 1, result: { ok: true } }));
    await expect(pending).resolves.toEqual({ ok: true });
  });

  it("rejects in-flight control and audio requests on disconnect", async () => {
    const { transport, socket } = await connect();
    const rpc = expect(transport.request("bridge.metrics", {})).rejects.toThrow(/closed/);
    const binary = expect(transport.sendBinary(new ArrayBuffer(4))).rejects.toThrow(/closed/);
    socket.close();
    await Promise.all([rpc, binary]);
  });
});
