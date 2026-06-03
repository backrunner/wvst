import {
  VST3_OUTPUT_EVENT_BYTES,
  decodeVst3OutputEvents,
  encodeVst3OutputEvents,
  vst3OutputEventPayloadBytes,
  type Vst3OutputEvent,
} from "./vst3-output-events.js";
import {
  MIDI_EVENT_BYTES,
  decodeMidiEvents,
  encodeMidiEvents,
  midiEventPayloadBytes,
  type MidiEvent,
} from "./midi-events.js";
import {
  PARAMETER_AUTOMATION_EVENT_BYTES,
  decodeParameterAutomationEvents,
  encodeParameterAutomationEvents,
  parameterAutomationEventPayloadBytes,
  type ParameterAutomationEvent,
} from "./parameter-events.js";

export {
  MIDI_EVENT_BYTES,
  MidiEventKind,
  decodeMidiEvent,
  decodeMidiEvents,
  encodeMidiEvent,
  encodeMidiEvents,
  midiEventPayloadBytes,
} from "./midi-events.js";
export type { MidiEvent } from "./midi-events.js";
export {
  PARAMETER_AUTOMATION_EVENT_BYTES,
  decodeParameterAutomationEvent,
  decodeParameterAutomationEvents,
  encodeParameterAutomationEvent,
  encodeParameterAutomationEvents,
  parameterAutomationEventPayloadBytes,
} from "./parameter-events.js";
export type { ParameterAutomationEvent } from "./parameter-events.js";
export {
  VST3_OUTPUT_EVENT_BYTES,
  VST3_OUTPUT_EVENT_FIXED_FIELDS_BYTES,
  VST3_OUTPUT_EVENT_PAYLOAD_BYTES,
  Vst3OutputEventKind,
  Vst3OutputEventPayloadEncoding,
  Vst3OutputEventPayloadFlags,
  decodeVst3OutputEvent,
  decodeVst3OutputEventPayloadText,
  decodeVst3OutputEvents,
  encodeVst3OutputEvent,
  encodeVst3OutputEvents,
  vst3OutputEventPayloadBytes,
} from "./vst3-output-events.js";
export type { Vst3OutputEvent } from "./vst3-output-events.js";

export const AUDIO_FRAME_MAGIC = 0x54535657;
export const AUDIO_FRAME_VERSION = 1;
export const AUDIO_FRAME_HEADER_BYTES = 56;

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
  parameterEventCount?: number;
  vst3OutputEventCount?: number;
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
  view.setUint16(50, header.parameterEventCount ?? 0, true);
  view.setUint16(52, header.vst3OutputEventCount ?? 0, true);

  return buffer;
}

export function encodeAudioFrame(
  header: AudioFrameHeader,
  audioPayload: ArrayBuffer,
  events: MidiEvent[] = [],
  parameterEvents: ParameterAutomationEvent[] = [],
  vst3OutputEvents: Vst3OutputEvent[] = [],
): ArrayBuffer {
  const eventPayload = encodeMidiEvents(events);
  const parameterEventPayload = encodeParameterAutomationEvents(parameterEvents);
  const vst3OutputEventPayload = encodeVst3OutputEvents(vst3OutputEvents);
  const payloadBytes =
    audioPayload.byteLength +
    eventPayload.byteLength +
    parameterEventPayload.byteLength +
    vst3OutputEventPayload.byteLength;
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
  const parameterEventCount = header.parameterEventCount ?? 0;
  if (parameterEvents.length !== parameterEventCount) {
    throw new Error(
      `WVST parameter event count mismatch: expected ${parameterEventCount}, got ${parameterEvents.length}`,
    );
  }
  const vst3OutputEventCount = header.vst3OutputEventCount ?? 0;
  if (vst3OutputEvents.length !== vst3OutputEventCount) {
    throw new Error(
      `WVST VST3 output event count mismatch: expected ${vst3OutputEventCount}, got ${vst3OutputEvents.length}`,
    );
  }

  const frame = new Uint8Array(AUDIO_FRAME_HEADER_BYTES + payloadBytes);
  frame.set(new Uint8Array(encodeAudioFrameHeader(header)), 0);
  frame.set(new Uint8Array(audioPayload), AUDIO_FRAME_HEADER_BYTES);
  frame.set(
    new Uint8Array(eventPayload),
    AUDIO_FRAME_HEADER_BYTES + audioPayload.byteLength,
  );
  frame.set(
    new Uint8Array(parameterEventPayload),
    AUDIO_FRAME_HEADER_BYTES + audioPayload.byteLength + eventPayload.byteLength,
  );
  frame.set(
    new Uint8Array(vst3OutputEventPayload),
    AUDIO_FRAME_HEADER_BYTES +
      audioPayload.byteLength +
      eventPayload.byteLength +
      parameterEventPayload.byteLength,
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
    parameterEventCount: view.getUint16(50, true),
    vst3OutputEventCount: view.getUint16(52, true),
  };
}

export interface DecodedAudioFrame {
  header: AudioFrameHeader;
  payload: ArrayBuffer;
  audioPayload: ArrayBuffer;
  eventPayload: ArrayBuffer;
  parameterEventPayload: ArrayBuffer;
  vst3OutputEventPayload: ArrayBuffer;
  events: MidiEvent[];
  parameterEvents: ParameterAutomationEvent[];
  vst3OutputEvents: Vst3OutputEvent[];
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
  const parameterEventBytes = parameterAutomationEventPayloadBytes(
    header.parameterEventCount ?? 0,
  );
  const vst3OutputEventBytes = vst3OutputEventPayloadBytes(
    header.vst3OutputEventCount ?? 0,
  );

  if (
    header.payloadBytes !==
    audioBytes + eventBytes + parameterEventBytes + vst3OutputEventBytes
  ) {
    throw new Error(
      `WVST payload section mismatch: expected ${audioBytes + eventBytes + parameterEventBytes + vst3OutputEventBytes}, got ${header.payloadBytes}`,
    );
  }
  const parameterOffset = audioBytes + eventBytes;
  const vst3OutputOffset = parameterOffset + parameterEventBytes;

  return {
    header,
    payload,
    audioPayload: payload.slice(0, audioBytes),
    eventPayload: payload.slice(audioBytes, audioBytes + eventBytes),
    parameterEventPayload: payload.slice(parameterOffset, vst3OutputOffset),
    vst3OutputEventPayload: payload.slice(vst3OutputOffset),
    events: decodeMidiEvents(
      payload.slice(audioBytes, audioBytes + eventBytes),
      header.eventCount,
    ),
    parameterEvents: decodeParameterAutomationEvents(
      payload.slice(parameterOffset, vst3OutputOffset),
      header.parameterEventCount ?? 0,
    ),
    vst3OutputEvents: decodeVst3OutputEvents(
      payload.slice(vst3OutputOffset),
      header.vst3OutputEventCount ?? 0,
    ),
  };
}

export function audioPayloadBytes(header: AudioFrameHeader): number {
  return checkedU32(header.frames * header.channels * 4, "audio payload bytes");
}

function checkedU32(value: number, label: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new Error(`invalid WVST ${label}: ${value}`);
  }

  return value;
}
