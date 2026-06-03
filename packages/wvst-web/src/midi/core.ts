import {
  MidiEventKind,
  type MidiEvent,
} from "../protocol/index.js";

export interface WVSTMidiEventTarget {
  buffers?: {
    frames: number;
  };
  sendMidiEvents(events: MidiEvent[]): Promise<void>;
}

export function resolveWVSTMidiSampleOffset(
  target: WVSTMidiEventTarget,
  sampleOffset: number | undefined,
): number {
  const offset = sampleOffset ?? 0;
  if (!Number.isInteger(offset) || offset < 0) {
    throw new Error(`WVST MIDI sampleOffset must be a non-negative integer: ${offset}`);
  }
  if (target.buffers && offset >= target.buffers.frames) {
    throw new Error(
      `WVST MIDI sampleOffset must be below block frames ${target.buffers.frames}: ${offset}`,
    );
  }
  return offset;
}

export function createWVSTMidiEvent(
  sampleOffset: number,
  kind: MidiEventKind,
  channel: number,
  data1: number,
  data2: number,
): MidiEvent {
  return {
    sampleOffset,
    kind,
    channel: validateWVSTMidiChannel(channel),
    data1: validateWVSTMidiData7("data1", data1),
    data2: validateWVSTMidiData7("data2", data2),
  };
}

export function createWVSTPitchBendEvent(
  sampleOffset: number,
  channel: number,
  value: number,
): MidiEvent {
  if (!Number.isFinite(value) || value < -1 || value > 1) {
    throw new Error(`WVST pitch bend value must be normalized in [-1, 1]: ${value}`);
  }

  const bend = Math.round(((value + 1) / 2) * 16_383);
  return createWVSTMidiEvent(
    sampleOffset,
    MidiEventKind.PitchBend,
    channel,
    bend & 0x7f,
    (bend >> 7) & 0x7f,
  );
}

export function createWVSTRawMidiEvent(
  sampleOffset: number,
  bytes: number[],
): MidiEvent {
  return {
    sampleOffset,
    kind: MidiEventKind.RawMidi,
    channel: (bytes[0] ?? 0) & 0x0f,
    data1: validateWVSTMidiByte("status", bytes[0] ?? 0),
    data2: validateWVSTMidiByte("data1", bytes[1] ?? 0),
    data3: validateWVSTMidiByte("data2", bytes[2] ?? 0),
    dataLength: Math.min(3, Math.max(1, bytes.length)),
  };
}

export function wvstMidiBytes(data: ArrayLike<number>): number[] {
  const bytes: number[] = [];
  for (let index = 0; index < Math.min(data.length, 3); index += 1) {
    bytes.push(validateWVSTMidiByte(`byte ${index}`, data[index] ?? 0));
  }
  return bytes;
}

export function normalizedToWVSTMidiValue(value: number): number {
  if (!Number.isFinite(value) || value < 0 || value > 1) {
    throw new Error(`WVST MIDI value must be normalized in [0, 1]: ${value}`);
  }
  return Math.round(value * 127);
}

export function validateWVSTMidiChannel(channel: number): number {
  if (!Number.isInteger(channel) || channel < 0 || channel > 15) {
    throw new Error(`WVST MIDI channel must be an integer in [0, 15]: ${channel}`);
  }
  return channel;
}

export function validateWVSTMidiData7(name: string, value: number): number {
  if (!Number.isInteger(value) || value < 0 || value > 127) {
    throw new Error(`WVST MIDI ${name} must be an integer in [0, 127]: ${value}`);
  }
  return value;
}

function validateWVSTMidiByte(name: string, value: number): number {
  if (!Number.isInteger(value) || value < 0 || value > 255) {
    throw new Error(`WVST MIDI ${name} must be an integer in [0, 255]: ${value}`);
  }
  return value;
}
