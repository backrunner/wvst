export const MIDI_EVENT_BYTES = 16;

export enum MidiEventKind {
  NoteOn = 1,
  NoteOff = 2,
  ControlChange = 3,
  PitchBend = 4,
  ChannelAftertouch = 5,
  PolyAftertouch = 6,
  RawMidi = 255,
}

export interface MidiEvent {
  sampleOffset: number;
  kind: MidiEventKind;
  channel: number;
  data1: number;
  data2: number;
  data3?: number;
  dataLength?: number;
  noteId?: number;
}

export function midiEventPayloadBytes(eventCount: number): number {
  return checkedU32(eventCount * MIDI_EVENT_BYTES, "MIDI event payload bytes");
}

export function encodeMidiEvent(event: MidiEvent): ArrayBuffer {
  validateMidiEvent(event);
  const buffer = new ArrayBuffer(MIDI_EVENT_BYTES);
  const view = new DataView(buffer);

  view.setUint16(0, event.sampleOffset, true);
  view.setUint8(2, event.kind);
  view.setUint8(3, event.channel);
  view.setUint8(4, event.data1);
  view.setUint8(5, event.data2);
  view.setUint8(6, event.data3 ?? 0);
  view.setUint8(7, event.dataLength ?? defaultMidiDataLength(event.kind));
  view.setUint32(8, event.noteId ?? 0, true);

  return buffer;
}

export function decodeMidiEvent(buffer: ArrayBufferLike): MidiEvent {
  if (buffer.byteLength < MIDI_EVENT_BYTES) {
    throw new Error(`WVST MIDI event requires ${MIDI_EVENT_BYTES} bytes`);
  }

  const view = new DataView(buffer, 0, MIDI_EVENT_BYTES);
  const event: MidiEvent = {
    sampleOffset: view.getUint16(0, true),
    kind: view.getUint8(2),
    channel: view.getUint8(3),
    data1: view.getUint8(4),
    data2: view.getUint8(5),
    data3: view.getUint8(6),
    dataLength: view.getUint8(7),
    noteId: view.getUint32(8, true),
  };
  validateMidiEvent(event);
  return event;
}

export function encodeMidiEvents(events: MidiEvent[]): ArrayBuffer {
  const payload = new Uint8Array(midiEventPayloadBytes(events.length));

  for (let index = 0; index < events.length; index += 1) {
    payload.set(new Uint8Array(encodeMidiEvent(events[index])), index * MIDI_EVENT_BYTES);
  }

  return payload.buffer;
}

export function decodeMidiEvents(
  payload: ArrayBufferLike,
  eventCount: number,
): MidiEvent[] {
  const expectedBytes = midiEventPayloadBytes(eventCount);
  if (payload.byteLength !== expectedBytes) {
    throw new Error(
      `WVST MIDI event payload length mismatch: expected ${expectedBytes}, got ${payload.byteLength}`,
    );
  }

  const events: MidiEvent[] = [];
  for (let offset = 0; offset < expectedBytes; offset += MIDI_EVENT_BYTES) {
    events.push(decodeMidiEvent(payload.slice(offset, offset + MIDI_EVENT_BYTES)));
  }

  return events;
}

export function validateMidiEvent(event: MidiEvent): void {
  if (!Number.isInteger(event.sampleOffset) || event.sampleOffset < 0 || event.sampleOffset > 0xffff) {
    throw new Error(`invalid WVST MIDI sample offset: ${event.sampleOffset}`);
  }
  if (!Object.values(MidiEventKind).includes(event.kind)) {
    throw new Error(`invalid WVST MIDI event kind: ${event.kind}`);
  }
  if (!Number.isInteger(event.channel) || event.channel < 0 || event.channel > 15) {
    throw new Error(`invalid WVST MIDI channel: ${event.channel}`);
  }
  validateMidiByte("data1", event.data1, event.kind === MidiEventKind.RawMidi);
  validateMidiByte("data2", event.data2, event.kind === MidiEventKind.RawMidi);
  validateMidiByte("data3", event.data3 ?? 0, event.kind === MidiEventKind.RawMidi);
  const dataLength = event.dataLength ?? defaultMidiDataLength(event.kind);
  if (!Number.isInteger(dataLength) || dataLength < 1 || dataLength > 3) {
    throw new Error(`invalid WVST MIDI data length: ${dataLength}`);
  }
}

function validateMidiByte(name: string, value: number, rawMidi: boolean): void {
  const max = rawMidi ? 0xff : 0x7f;
  if (!Number.isInteger(value) || value < 0 || value > max) {
    throw new Error(`invalid WVST MIDI ${name}: ${value}`);
  }
}

function defaultMidiDataLength(kind: MidiEventKind): number {
  return kind === MidiEventKind.RawMidi ? 3 : 2;
}

function checkedU32(value: number, label: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new Error(`invalid WVST ${label}: ${value}`);
  }

  return value;
}
