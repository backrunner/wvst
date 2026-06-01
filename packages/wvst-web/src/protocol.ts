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

export function encodeAudioFrame(header: AudioFrameHeader, payload: ArrayBuffer): ArrayBuffer {
  if (payload.byteLength !== header.payloadBytes) {
    throw new Error(
      `WVST payload length mismatch: expected ${header.payloadBytes}, got ${payload.byteLength}`,
    );
  }

  const frame = new Uint8Array(AUDIO_FRAME_HEADER_BYTES + payload.byteLength);
  frame.set(new Uint8Array(encodeAudioFrameHeader(header)), 0);
  frame.set(new Uint8Array(payload), AUDIO_FRAME_HEADER_BYTES);

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
}

export function decodeAudioFrame(buffer: ArrayBuffer): DecodedAudioFrame {
  const header = decodeAudioFrameHeader(buffer);
  const expectedBytes = AUDIO_FRAME_HEADER_BYTES + header.payloadBytes;

  if (buffer.byteLength !== expectedBytes) {
    throw new Error(
      `WVST audio frame length mismatch: expected ${expectedBytes}, got ${buffer.byteLength}`,
    );
  }

  return {
    header,
    payload: buffer.slice(AUDIO_FRAME_HEADER_BYTES),
  };
}

