import { describe, expect, it, vi } from "vitest";
import {
  createWVSTWebMidiAdapter, MidiEventKind, type WVSTWebMidiAccess,
  type WVSTWebMidiInput, type WVSTWebMidiMessageEvent,
} from "../src/index.js";

class Input implements WVSTWebMidiInput {
  state = "connected";
  listeners = new Set<(event: WVSTWebMidiMessageEvent) => void>();
  constructor(public id: string) {}
  addEventListener(_type: "midimessage", listener: (event: WVSTWebMidiMessageEvent) => void) { this.listeners.add(listener); }
  removeEventListener(_type: "midimessage", listener: (event: WVSTWebMidiMessageEvent) => void) { this.listeners.delete(listener); }
  send(data: number[]) { for (const listener of this.listeners) listener({ data }); }
}

function setup() {
  const inputs = new Map<string, Input>();
  const changes = new Set<() => void>();
  const access: WVSTWebMidiAccess = {
    inputs,
    addEventListener: (_, listener) => { changes.add(listener); },
    removeEventListener: (_, listener) => { changes.delete(listener); },
  };
  const target = { buffers: { frames: 128 }, sendMidiEvents: vi.fn(async (_events: unknown[]) => {}) };
  return { inputs, changes, access, target, change: () => { for (const listener of changes) listener(); } };
}

describe("Web MIDI device lifecycle", () => {
  it("binds hot-plugged devices once and releases held notes and pedals on unplug", async () => {
    const env = setup();
    const adapter = await createWVSTWebMidiAdapter(env.target, { midiAccess: env.access });
    const input = new Input("keyboard");
    env.inputs.set(input.id, input);
    env.change();
    env.change();
    expect(input.listeners.size).toBe(1);
    input.send([0x92, 60, 100]);
    input.send([0xb2, 64, 127]);
    input.state = "disconnected";
    env.change();
    expect(input.listeners.size).toBe(0);
    expect(env.target.sendMidiEvents).toHaveBeenLastCalledWith([
      { sampleOffset: 0, kind: MidiEventKind.NoteOff, channel: 2, data1: 60, data2: 0 },
      { sampleOffset: 0, kind: MidiEventKind.ControlChange, channel: 2, data1: 64, data2: 0 },
    ]);
    input.state = "connected";
    env.change();
    expect(input.listeners.size).toBe(1);
    await adapter.stop();
    await adapter.stop();
    expect(input.listeners.size).toBe(0);
    expect(env.changes.size).toBe(0);
  });

  it("filters inputs and catches both parser errors and rejected sends", async () => {
    const env = setup();
    const input = new Input("selected");
    const other = new Input("other");
    env.inputs.set(input.id, input);
    env.inputs.set(other.id, other);
    const onError = vi.fn();
    const adapter = await createWVSTWebMidiAdapter(env.target, { midiAccess: env.access, inputIds: [input.id], onError });
    expect(other.listeners.size).toBe(0);
    expect(() => input.send([0x90, 60])).not.toThrow();
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: expect.stringMatching(/malformed/) }));
    env.target.sendMidiEvents.mockRejectedValueOnce(new Error("queue full"));
    input.send([0xb0, 1, 127]);
    await Promise.resolve();
    expect(onError).toHaveBeenLastCalledWith(expect.objectContaining({ message: "queue full" }));
    await adapter.stop();
  });

  it("stop waits for note release and prevents subsequent input", async () => {
    const env = setup();
    const input = new Input("keyboard");
    env.inputs.set(input.id, input);
    const adapter = await createWVSTWebMidiAdapter(env.target, { midiAccess: env.access });
    input.send([0x90, 60, 100]);
    let complete!: () => void;
    env.target.sendMidiEvents.mockImplementationOnce(() => new Promise<void>((resolve) => { complete = resolve; }));
    const stopping = adapter.stop();
    expect(input.listeners.size).toBe(0);
    input.send([0x90, 70, 100]);
    expect(env.target.sendMidiEvents).toHaveBeenCalledTimes(2);
    let done = false;
    void stopping.then(() => { done = true; });
    await Promise.resolve();
    expect(done).toBe(false);
    complete();
    await stopping;
    expect(done).toBe(true);
  });

  it("does not release a note still held by another device", async () => {
    const env = setup();
    const a = new Input("a"), b = new Input("b");
    env.inputs.set(a.id, a); env.inputs.set(b.id, b);
    const adapter = await createWVSTWebMidiAdapter(env.target, { midiAccess: env.access });
    a.send([0x90, 60, 100]); b.send([0x90, 60, 100]);
    env.inputs.delete(a.id); env.change();
    expect(env.target.sendMidiEvents).toHaveBeenCalledTimes(2);
    await adapter.panic();
    expect(env.target.sendMidiEvents).toHaveBeenLastCalledWith([
      { sampleOffset: 0, kind: MidiEventKind.NoteOff, channel: 0, data1: 60, data2: 0 },
    ]);
    await adapter.stop();
  });
});
