export const VST3_OUTPUT_EVENT_FIXED_FIELDS_BYTES = 40;
export const VST3_OUTPUT_EVENT_PAYLOAD_BYTES = 64;
export const VST3_OUTPUT_EVENT_BYTES =
  VST3_OUTPUT_EVENT_FIXED_FIELDS_BYTES + VST3_OUTPUT_EVENT_PAYLOAD_BYTES;

export enum Vst3OutputEventKind {
  Data = 1,
  NoteExpressionValue = 2,
  NoteExpressionText = 3,
  Chord = 4,
  Scale = 5,
  NoteExpressionIntValue = 6,
}

export enum Vst3OutputEventPayloadEncoding {
  None = 0,
  RawBytes = 1,
  Utf8 = 2,
}

export enum Vst3OutputEventPayloadFlags {
  Truncated = 1 << 0,
  Unavailable = 1 << 1,
  InvalidText = 1 << 2,
}

export interface Vst3OutputEvent {
  sampleOffset: number;
  kind: Vst3OutputEventKind;
  vst3EventType: number;
  busIndex: number;
  /** kind-specific signed field: noteId, chord root/bass, scale root/mask, or integer value. */
  data1: number;
  /** kind-specific signed field: chord bass, scale mask, or integer note-expression value. */
  data2: number;
  /** normalized note-expression value for NoteExpressionValue events. */
  value: number;
  /** Original Data byte count or UTF-16 text length reported by the VST3 event. */
  dataSize: number;
  /** Data event type or note-expression type id. */
  dataType: number;
  /** Bytes copied into the bounded WVST payload slot. Defaults to payloadBytes.length. */
  payloadSize?: number;
  /** Payload encoding for payloadBytes. Defaults to None for empty payloads, RawBytes otherwise. */
  payloadEncoding?: Vst3OutputEventPayloadEncoding;
  /** Bitmask of Vst3OutputEventPayloadFlags. */
  payloadFlags?: number;
  /** Bounded payload preview: raw Data bytes or UTF-8 text bytes. */
  payloadBytes?: Uint8Array;
}

export function encodeVst3OutputEvent(event: Vst3OutputEvent): ArrayBuffer {
  const payloadBytes = event.payloadBytes ?? new Uint8Array();
  const payloadSize = event.payloadSize ?? payloadBytes.byteLength;
  const payloadEncoding =
    event.payloadEncoding ??
    (payloadSize === 0
      ? Vst3OutputEventPayloadEncoding.None
      : Vst3OutputEventPayloadEncoding.RawBytes);
  validateVst3OutputEvent(event, payloadSize, payloadEncoding, payloadBytes);

  const buffer = new ArrayBuffer(VST3_OUTPUT_EVENT_BYTES);
  const view = new DataView(buffer);

  view.setUint16(0, event.sampleOffset, true);
  view.setUint8(2, event.kind);
  view.setUint16(4, event.vst3EventType, true);
  view.setInt32(8, event.busIndex, true);
  view.setInt32(12, event.data1, true);
  view.setInt32(16, event.data2, true);
  view.setFloat64(20, event.value, true);
  view.setUint32(28, event.dataSize, true);
  view.setUint32(32, event.dataType, true);
  view.setUint16(36, payloadSize, true);
  view.setUint8(38, payloadEncoding);
  view.setUint8(39, event.payloadFlags ?? 0);
  new Uint8Array(buffer, VST3_OUTPUT_EVENT_FIXED_FIELDS_BYTES, payloadSize).set(
    payloadBytes.subarray(0, payloadSize),
  );

  return buffer;
}

export function decodeVst3OutputEvent(buffer: ArrayBufferLike): Vst3OutputEvent {
  if (buffer.byteLength < VST3_OUTPUT_EVENT_BYTES) {
    throw new Error(`WVST VST3 output event requires ${VST3_OUTPUT_EVENT_BYTES} bytes`);
  }

  const view = new DataView(buffer, 0, VST3_OUTPUT_EVENT_BYTES);
  const payloadSize = view.getUint16(36, true);
  const payloadEncoding = view.getUint8(38);
  const payloadBytes = new Uint8Array(
    buffer,
    VST3_OUTPUT_EVENT_FIXED_FIELDS_BYTES,
    payloadSize,
  ).slice();
  const event: Vst3OutputEvent = {
    sampleOffset: view.getUint16(0, true),
    kind: view.getUint8(2),
    vst3EventType: view.getUint16(4, true),
    busIndex: view.getInt32(8, true),
    data1: view.getInt32(12, true),
    data2: view.getInt32(16, true),
    value: view.getFloat64(20, true),
    dataSize: view.getUint32(28, true),
    dataType: view.getUint32(32, true),
    payloadSize,
    payloadEncoding,
    payloadFlags: view.getUint8(39),
    payloadBytes,
  };
  validateVst3OutputEvent(event, payloadSize, payloadEncoding, payloadBytes);
  return event;
}

export function encodeVst3OutputEvents(events: Vst3OutputEvent[]): ArrayBuffer {
  const payload = new Uint8Array(vst3OutputEventPayloadBytes(events.length));

  for (let index = 0; index < events.length; index += 1) {
    payload.set(
      new Uint8Array(encodeVst3OutputEvent(events[index])),
      index * VST3_OUTPUT_EVENT_BYTES,
    );
  }

  return payload.buffer;
}

export function decodeVst3OutputEvents(
  payload: ArrayBufferLike,
  eventCount: number,
): Vst3OutputEvent[] {
  const expectedBytes = vst3OutputEventPayloadBytes(eventCount);
  if (payload.byteLength !== expectedBytes) {
    throw new Error(
      `WVST VST3 output event payload length mismatch: expected ${expectedBytes}, got ${payload.byteLength}`,
    );
  }

  const events: Vst3OutputEvent[] = [];
  for (let offset = 0; offset < expectedBytes; offset += VST3_OUTPUT_EVENT_BYTES) {
    events.push(
      decodeVst3OutputEvent(payload.slice(offset, offset + VST3_OUTPUT_EVENT_BYTES)),
    );
  }

  return events;
}

export function vst3OutputEventPayloadBytes(eventCount: number): number {
  return checkedU32(
    eventCount * VST3_OUTPUT_EVENT_BYTES,
    "VST3 output event payload bytes",
  );
}

export function decodeVst3OutputEventPayloadText(
  event: Vst3OutputEvent,
): string | undefined {
  if (
    event.payloadEncoding !== Vst3OutputEventPayloadEncoding.Utf8 ||
    (event.payloadSize ?? 0) === 0 ||
    event.payloadBytes === undefined
  ) {
    return undefined;
  }
  return new TextDecoder("utf-8", { fatal: false }).decode(event.payloadBytes);
}

function validateVst3OutputEvent(
  event: Vst3OutputEvent,
  payloadSize: number,
  payloadEncoding: number,
  payloadBytes: Uint8Array,
): void {
  if (!Number.isInteger(event.sampleOffset) || event.sampleOffset < 0 || event.sampleOffset > 0xffff) {
    throw new Error(`invalid WVST VST3 output sample offset: ${event.sampleOffset}`);
  }
  if (!Object.values(Vst3OutputEventKind).includes(event.kind)) {
    throw new Error(`invalid WVST VST3 output event kind: ${event.kind}`);
  }
  validateU16("VST3 event type", event.vst3EventType);
  validateI32("VST3 bus index", event.busIndex);
  validateI32("VST3 data1", event.data1);
  validateI32("VST3 data2", event.data2);
  if (!Number.isFinite(event.value)) {
    throw new Error(`invalid WVST VST3 output value: ${event.value}`);
  }
  validateU32("VST3 data size", event.dataSize);
  validateU32("VST3 data type", event.dataType);
  if (
    !Object.values(Vst3OutputEventPayloadEncoding).includes(payloadEncoding)
  ) {
    throw new Error(`invalid WVST VST3 output payload encoding: ${payloadEncoding}`);
  }
  if (
    !Number.isInteger(payloadSize) ||
    payloadSize < 0 ||
    payloadSize > VST3_OUTPUT_EVENT_PAYLOAD_BYTES
  ) {
    throw new Error(`invalid WVST VST3 output payload size: ${payloadSize}`);
  }
  if (payloadBytes.byteLength < payloadSize) {
    throw new Error(
      `WVST VST3 output payload bytes shorter than payload size: ${payloadBytes.byteLength} < ${payloadSize}`,
    );
  }
  validateU8("VST3 payload flags", event.payloadFlags ?? 0);
}

function checkedU32(value: number, label: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new Error(`invalid WVST ${label}: ${value}`);
  }

  return value;
}

function validateU8(label: string, value: number): void {
  if (!Number.isInteger(value) || value < 0 || value > 0xff) {
    throw new Error(`invalid WVST ${label}: ${value}`);
  }
}

function validateU16(label: string, value: number): void {
  if (!Number.isInteger(value) || value < 0 || value > 0xffff) {
    throw new Error(`invalid WVST ${label}: ${value}`);
  }
}

function validateU32(label: string, value: number): void {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new Error(`invalid WVST ${label}: ${value}`);
  }
}

function validateI32(label: string, value: number): void {
  if (!Number.isInteger(value) || value < -0x80000000 || value > 0x7fffffff) {
    throw new Error(`invalid WVST ${label}: ${value}`);
  }
}
