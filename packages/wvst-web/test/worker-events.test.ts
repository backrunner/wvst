import { describe, expect, it } from "vitest";
import { BridgeWorkerEventQueue } from "../src/audio/bridge-worker-events.js";
import { createLoopbackSharedBuffers, LoopbackCounter, MidiEventKind } from "../src/index.js";

function setup() {
  const { counters } = createLoopbackSharedBuffers({ frames: 128, channels: 2 });
  let sequence = 1;
  return { counters, advance: () => { sequence += 1; }, queue: new BridgeWorkerEventQueue({ frames: 128, counters, nextInputTargetSequence: () => sequence }) };
}

describe("MIDI during audio overload", () => {
  it("preserves overdue note and pedal order across skipped blocks", () => {
    const { queue, counters, advance } = setup();
    const on = { sampleOffset: 100, kind: MidiEventKind.NoteOn, channel: 0, data1: 60, data2: 100 };
    queue.enqueueMidiEvents([on]);
    advance();
    const off = { ...on, sampleOffset: 0, kind: MidiEventKind.NoteOff, data2: 0 };
    const pedal = { ...off, sampleOffset: 1, kind: MidiEventKind.ControlChange, data1: 64 };
    queue.enqueueMidiEvents([pedal, off]);
    expect(queue.takeMidiEvents(8)).toEqual([on, off, pedal].map((event) => ({ ...event, sampleOffset: 0 })));
    expect(counters[LoopbackCounter.LateMidiEvents]).toBe(3);
    expect(counters[LoopbackCounter.DroppedMidiEvents]).toBe(0);
    expect(queue.takeMidiEvents(9)).toEqual([]);
  });

  it("rejects an invalid batch atomically so a lone note-on cannot remain queued", () => {
    const { queue } = setup();
    const on = { sampleOffset: 0, kind: MidiEventKind.NoteOn, channel: 0, data1: 60, data2: 100 };
    expect(() => queue.enqueueMidiEvents([on, { ...on, channel: 16 }])).toThrow();
    expect(queue.takeMidiEvents(1)).toEqual([]);
    expect(() => queue.enqueueParameterEvents([
      { sampleOffset: 0, parameterId: 1, valueNormalized: 0.5 },
      { sampleOffset: 0, parameterId: 1, valueNormalized: NaN },
    ])).toThrow();
    expect(queue.takeParameterEvents(1)).toEqual([]);
  });
});
