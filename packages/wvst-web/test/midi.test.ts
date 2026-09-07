import { describe, expect, it, vi } from "vitest";
import {
  MidiEventKind,
  createWVSTVirtualKeyboard,
  webMidiMessageToWVSTEvents,
  type WVSTMidiEventTarget,
} from "../src/index.js";

function captureTarget(frames = 128) {
  const batches: unknown[][] = [];
  const target: WVSTMidiEventTarget = {
    buffers: { frames },
    sendMidiEvents: vi.fn(async (events) => {
      batches.push(events);
    }),
  };
  return { target, batches };
}

describe("WVST MIDI helpers", () => {
  it("maps Web MIDI channel voice messages to WVST MIDI events", () => {
    const { target } = captureTarget();

    expect(webMidiMessageToWVSTEvents([0x90, 60, 100], target)).toEqual([
      {
        sampleOffset: 0,
        kind: MidiEventKind.NoteOn,
        channel: 0,
        data1: 60,
        data2: 100,
      },
    ]);
    expect(webMidiMessageToWVSTEvents([0x90, 60, 0], target)[0].kind).toBe(
      MidiEventKind.NoteOff,
    );
    expect(webMidiMessageToWVSTEvents([0xe1, 0, 64], target)[0]).toMatchObject({
      kind: MidiEventKind.PitchBend,
      channel: 1,
    });
  });

  it("filters channel input and rejects unsupported long messages", () => {
    const { target } = captureTarget();

    expect(webMidiMessageToWVSTEvents([0xb2, 74, 50], target, { channel: 1 })).toEqual([]);
    expect(webMidiMessageToWVSTEvents([0xf8], target, { channel: 1 })).toEqual([
      {
        sampleOffset: 0,
        kind: MidiEventKind.RawMidi,
        channel: 8,
        data1: 248,
        data2: 0,
        data3: 0,
        dataLength: 1,
      },
    ]);
  });

  it.each([[0x90], [0x90, 60], [0x90, 60, 128], [60, 100], [0xf0, 1, 2, 0xf7]])("rejects malformed or unsupported MIDI %j", (...data) => {
    expect(() => webMidiMessageToWVSTEvents(data, captureTarget().target)).toThrow();
  });

  it("generates note ids and matching note-off events for the virtual keyboard", async () => {
    const { target, batches } = captureTarget();
    const keyboard = createWVSTVirtualKeyboard(target, { channel: 2, sampleOffset: 8 });

    await keyboard.noteOn(64, 0.5);
    await keyboard.noteOff(64);

    expect(batches).toEqual([
      [
        {
          sampleOffset: 8,
          kind: MidiEventKind.NoteOn,
          channel: 2,
          data1: 64,
          data2: 64,
          noteId: 1,
        },
      ],
      [
        {
          sampleOffset: 8,
          kind: MidiEventKind.NoteOff,
          channel: 2,
          data1: 64,
          data2: 0,
          noteId: 1,
        },
      ],
    ]);
  });

  it("rejects sample offsets outside the active WebAudio quantum", () => {
    const { target } = captureTarget(16);
    const keyboard = createWVSTVirtualKeyboard(target);

    expect(() => keyboard.noteOn(60, 1, { sampleOffset: 16 })).toThrow(/below block/);
  });
});
