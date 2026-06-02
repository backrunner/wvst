export const AUDIO_FRAME_MAGIC = 0x54535657;
export const AUDIO_FRAME_VERSION = 1;
export const AUDIO_FRAME_HEADER_BYTES = 56;
export const MIDI_EVENT_BYTES = 16;

export enum AudioSampleFormat {
  F32Le = 1,
}

export enum AudioFrameFlags {
  Silence = 1 << 0,
  MidiOnly = 1 << 1,
  EndOfStream = 1 << 2,
  Late = 1 << 3,
  ProcessError = 1 << 4,
}

export interface AudioFrameHeader {
  streamId: bigint;
  sequence: bigint;
  sentFrameTime: bigint;
  sampleRate: number;
  payloadBytes: number;
  frames: number;
  channels: number;
  format: AudioSampleFormat;
  flags: number;
  eventCount: number;
}

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

export function encodeAudioFrameHeader(header: AudioFrameHeader): ArrayBuffer {
  const buffer = new ArrayBuffer(AUDIO_FRAME_HEADER_BYTES);
  const view = new DataView(buffer);

  view.setUint32(0, AUDIO_FRAME_MAGIC, true);
  view.setUint16(4, AUDIO_FRAME_VERSION, true);
  view.setUint16(6, AUDIO_FRAME_HEADER_BYTES, true);
  view.setBigUint64(8, header.streamId, true);
  view.setBigUint64(16, header.sequence, true);
  view.setBigUint64(24, header.sentFrameTime, true);
  view.setUint32(32, header.sampleRate, true);
  view.setUint32(36, header.payloadBytes, true);
  view.setUint16(40, header.frames, true);
  view.setUint16(42, header.channels, true);
  view.setUint8(44, header.format);
  view.setUint8(45, 0);
  view.setUint16(46, header.flags, true);
  view.setUint16(48, header.eventCount, true);

  return buffer;
}

export function encodeAudioFrame(
  header: AudioFrameHeader,
  audioPayload: ArrayBuffer,
  events: MidiEvent[] = [],
): ArrayBuffer {
  const eventPayload = encodeMidiEvents(events);
  const payloadBytes = audioPayload.byteLength + eventPayload.byteLength;
  if (payloadBytes !== header.payloadBytes) {
    throw new Error(
      `WVST payload length mismatch: expected ${header.payloadBytes}, got ${payloadBytes}`,
    );
  }
  if (events.length !== header.eventCount) {
    throw new Error(
      `WVST event count mismatch: expected ${header.eventCount}, got ${events.length}`,
    );
  }

  const frame = new Uint8Array(AUDIO_FRAME_HEADER_BYTES + payloadBytes);
  frame.set(new Uint8Array(encodeAudioFrameHeader(header)), 0);
  frame.set(new Uint8Array(audioPayload), AUDIO_FRAME_HEADER_BYTES);
  frame.set(
    new Uint8Array(eventPayload),
    AUDIO_FRAME_HEADER_BYTES + audioPayload.byteLength,
  );

  return frame.buffer;
}

export function decodeAudioFrameHeader(buffer: ArrayBufferLike): AudioFrameHeader {
  if (buffer.byteLength < AUDIO_FRAME_HEADER_BYTES) {
    throw new Error(
      `WVST audio frame header requires ${AUDIO_FRAME_HEADER_BYTES} bytes`,
    );
  }

  const view = new DataView(buffer, 0, AUDIO_FRAME_HEADER_BYTES);
  const magic = view.getUint32(0, true);
  const version = view.getUint16(4, true);
  const headerBytes = view.getUint16(6, true);

  if (magic !== AUDIO_FRAME_MAGIC) {
    throw new Error(`invalid WVST audio frame magic: ${magic}`);
  }

  if (version !== AUDIO_FRAME_VERSION) {
    throw new Error(`unsupported WVST audio frame version: ${version}`);
  }

  if (headerBytes !== AUDIO_FRAME_HEADER_BYTES) {
    throw new Error(`invalid WVST audio frame header length: ${headerBytes}`);
  }

  return {
    streamId: view.getBigUint64(8, true),
    sequence: view.getBigUint64(16, true),
    sentFrameTime: view.getBigUint64(24, true),
    sampleRate: view.getUint32(32, true),
    payloadBytes: view.getUint32(36, true),
    frames: view.getUint16(40, true),
    channels: view.getUint16(42, true),
    format: view.getUint8(44),
    flags: view.getUint16(46, true),
    eventCount: view.getUint16(48, true),
  };
}

export interface DecodedAudioFrame {
  header: AudioFrameHeader;
  payload: ArrayBuffer;
  audioPayload: ArrayBuffer;
  eventPayload: ArrayBuffer;
  events: MidiEvent[];
}

export function decodeAudioFrame(buffer: ArrayBuffer): DecodedAudioFrame {
  const header = decodeAudioFrameHeader(buffer);
  const expectedBytes = AUDIO_FRAME_HEADER_BYTES + header.payloadBytes;

  if (buffer.byteLength !== expectedBytes) {
    throw new Error(
      `WVST audio frame length mismatch: expected ${expectedBytes}, got ${buffer.byteLength}`,
    );
  }
  const payload = buffer.slice(AUDIO_FRAME_HEADER_BYTES);
  const audioBytes = audioPayloadBytes(header);
  const eventBytes = midiEventPayloadBytes(header.eventCount);

  if (header.payloadBytes !== audioBytes + eventBytes) {
    throw new Error(
      `WVST payload section mismatch: expected ${audioBytes + eventBytes}, got ${header.payloadBytes}`,
    );
  }

  return {
    header,
    payload,
    audioPayload: payload.slice(0, audioBytes),
    eventPayload: payload.slice(audioBytes),
    events: decodeMidiEvents(payload.slice(audioBytes), header.eventCount),
  };
}

export function audioPayloadBytes(header: AudioFrameHeader): number {
  return checkedU32(header.frames * header.channels * 4, "audio payload bytes");
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

function validateMidiEvent(event: MidiEvent): void {
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
