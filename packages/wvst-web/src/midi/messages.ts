import {
  MidiEventKind,
  type MidiEvent,
} from "../protocol/index.js";
import {
  createWVSTMidiEvent,
  createWVSTRawMidiEvent,
  resolveWVSTMidiSampleOffset,
  validateWVSTMidiChannel,
  wvstMidiBytes,
  type WVSTMidiEventTarget,
} from "./core.js";

import type { WVSTWebMidiAdapterOptions } from "./web-midi.js";

export function webMidiMessageToWVSTEvents(
  data: ArrayLike<number>,
  target: WVSTMidiEventTarget,
  options: WVSTWebMidiAdapterOptions = {},
): MidiEvent[] {
  const bytes = wvstMidiBytes(data);
  if (bytes.length === 0) {
    return [];
  }

  const status = bytes[0] ?? 0;
  const expectedLength = shortMessageLength(status);
  if (bytes.length !== expectedLength || bytes.slice(1).some((byte) => byte > 127)) {
    throw new Error("WVST received a malformed MIDI message");
  }
  const sampleOffset = resolveWVSTMidiSampleOffset(target, options.sampleOffset);
  const channel = status & 0x0f;
  if (options.channel !== undefined && status < 0xf0 && channel !== validateWVSTMidiChannel(options.channel)) {
    return [];
  }

  const data1 = bytes[1] ?? 0;
  const data2 = bytes[2] ?? 0;
  switch (status & 0xf0) {
    case 0x80:
      return [createWVSTMidiEvent(sampleOffset, MidiEventKind.NoteOff, channel, data1, data2)];
    case 0x90:
      return [
        createWVSTMidiEvent(
          sampleOffset,
          data2 === 0 ? MidiEventKind.NoteOff : MidiEventKind.NoteOn,
          channel,
          data1,
          data2,
        ),
      ];
    case 0xa0:
      return [
        createWVSTMidiEvent(sampleOffset, MidiEventKind.PolyAftertouch, channel, data1, data2),
      ];
    case 0xb0:
      return [
        createWVSTMidiEvent(sampleOffset, MidiEventKind.ControlChange, channel, data1, data2),
      ];
    case 0xd0:
      return [
        createWVSTMidiEvent(
          sampleOffset,
          MidiEventKind.ChannelAftertouch,
          channel,
          data1,
          0,
        ),
      ];
    case 0xe0:
      return [
        createWVSTMidiEvent(sampleOffset, MidiEventKind.PitchBend, channel, data1, data2),
      ];
    default:
      return options.rawFallback === false
        ? []
        : [createWVSTRawMidiEvent(sampleOffset, bytes)];
  }
}


function shortMessageLength(status: number): number {
  if (status >= 0x80 && status < 0xf0) {
    return (status & 0xf0) === 0xc0 || (status & 0xf0) === 0xd0 ? 2 : 3;
  }
  switch (status) {
    case 0xf1: case 0xf3: return 2;
    case 0xf2: return 3;
    case 0xf6: case 0xf8: case 0xfa: case 0xfb: case 0xfc: case 0xfe: case 0xff: return 1;
    default: throw new Error("WVST received an unsupported MIDI status (SysEx is unsupported)");
  }
}
