import { afterEach, describe, expect, it, vi } from "vitest";
import { WebSocketRpcTransport, type WebSocketRpcTransportOptions } from "../src/client/transport.js";

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

async function connect(options?: WebSocketRpcTransportOptions) {
  vi.stubGlobal("WebSocket", MockSocket);
  const connected = WebSocketRpcTransport.connect("ws://localhost:35876", options);
  const socket = MockSocket.latest;
  socket.readyState = MockSocket.OPEN;
  socket.dispatchEvent(new Event("open"));
  return { transport: await connected, socket };
}

afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); });

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


describe("WebSocket stalled peers and backpressure", () => {
  it("bounds the handshake when neither open nor error arrives", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("WebSocket", MockSocket);
    const pending = expect(WebSocketRpcTransport.connect("ws://localhost", { connectTimeoutMs: 25 }))
      .rejects.toThrow(/connection timed out/);
    await vi.advanceTimersByTimeAsync(25);
    await pending;
    expect(MockSocket.latest.readyState).toBe(3);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("closes a stalled audio connection without misassigning late FIFO responses", async () => {
    vi.useFakeTimers();
    const { transport, socket } = await connect({ binaryTimeoutMs: 25 });
    const first = expect(transport.sendBinary(new ArrayBuffer(4))).rejects.toThrow(/audio response timed out/);
    const second = expect(transport.sendBinary(new ArrayBuffer(4))).rejects.toThrow(/audio response timed out/);
    const control = expect(transport.request("bridge.metrics", {})).rejects.toThrow(/audio response timed out/);
    await vi.advanceTimersByTimeAsync(25);
    await Promise.all([first, second, control]);
    socket.message(new ArrayBuffer(8));
    await expect(transport.sendBinary(new ArrayBuffer(4))).rejects.toThrow(/closed/);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps the configurable control budget independent from audio", async () => {
    vi.useFakeTimers();
    const { transport, socket } = await connect({ requestTimeoutMs: 500, binaryTimeoutMs: 25 });
    const loading = transport.request("instance.create", {});
    await vi.advanceTimersByTimeAsync(100);
    socket.message(JSON.stringify({ id: 1, result: { instanceId: 1 } }));
    await expect(loading).resolves.toEqual({ instanceId: 1 });
    const stalled = expect(transport.request("instance.create", {})).rejects.toThrow(/request timed out/);
    await vi.advanceTimersByTimeAsync(500);
    await stalled;
    expect(vi.getTimerCount()).toBe(0);
  });

  it("rejects excess pending requests and releases their timers on close", async () => {
    vi.useFakeTimers();
    const { transport } = await connect({ maxPendingRequests: 1 });
    const pending = expect(transport.request("instance.create", {})).rejects.toThrow(/closed/);
    await expect(transport.sendBinary(new ArrayBuffer(4))).rejects.toThrow(/queue is full/);
    await expect(transport.request("bridge.metrics", {})).rejects.toThrow(/queue is full/);
    transport.close();
    await pending;
    expect(vi.getTimerCount()).toBe(0);
  });
});
