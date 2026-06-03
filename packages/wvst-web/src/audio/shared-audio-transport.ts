import type {
  SharedAudioRingCursorState,
  SharedAudioRingLayout,
  SharedAudioTransportLayout,
} from "../control/instances.js";

export type WVSTSharedAudioRingRole = "input" | "output";

export interface WVSTSharedAudioTransportView {
  memory: ArrayBufferLike;
  input: WVSTSharedAudioRingView;
  output: WVSTSharedAudioRingView;
}

export interface WVSTSharedAudioRingView {
  role: WVSTSharedAudioRingRole;
  layout: SharedAudioRingLayout;
  cursor: DataView;
  samples: Float32Array;
}

export const WVST_SHARED_AUDIO_CURSOR_BYTES = 64;

const CURSOR_READ_FRAME_OFFSET = 0;
const CURSOR_WRITE_FRAME_OFFSET = 8;
const CURSOR_DROPPED_FRAMES_OFFSET = 16;
const CURSOR_UNDERRUN_FRAMES_OFFSET = 24;
const CURSOR_OVERRUN_FRAMES_OFFSET = 32;
const CURSOR_GENERATION_OFFSET = 40;
const CURSOR_FLAGS_OFFSET = 48;
const F32_BYTES = 4;

export function createWVSTSharedAudioTransportView(
  layout: SharedAudioTransportLayout,
  memory?: ArrayBufferLike,
): WVSTSharedAudioTransportView {
  const transportMemory = memory ?? createSharedAudioBuffer(layout.totalBytes);
  validateMemoryLength(transportMemory, layout.totalBytes);

  return {
    memory: transportMemory,
    input: createRingView(transportMemory, layout.input),
    output: createRingView(transportMemory, layout.output),
  };
}

export function readWVSTSharedAudioCursor(
  ring: WVSTSharedAudioRingView,
): SharedAudioRingCursorState {
  return {
    readFrame: readU64(ring.cursor, CURSOR_READ_FRAME_OFFSET),
    writeFrame: readU64(ring.cursor, CURSOR_WRITE_FRAME_OFFSET),
    droppedFrames: readU64(ring.cursor, CURSOR_DROPPED_FRAMES_OFFSET),
    underrunFrames: readU64(ring.cursor, CURSOR_UNDERRUN_FRAMES_OFFSET),
    overrunFrames: readU64(ring.cursor, CURSOR_OVERRUN_FRAMES_OFFSET),
    generation: readU64(ring.cursor, CURSOR_GENERATION_OFFSET),
    flags: ring.cursor.getUint32(CURSOR_FLAGS_OFFSET, true),
  };
}

export function writeWVSTSharedAudioCursor(
  ring: WVSTSharedAudioRingView,
  state: SharedAudioRingCursorState,
): void {
  if (state.writeFrame < state.readFrame) {
    throw new Error(
      `WVST shared audio cursor order invalid: read ${state.readFrame}, write ${state.writeFrame}`,
    );
  }

  writeU64(ring.cursor, CURSOR_READ_FRAME_OFFSET, state.readFrame);
  writeU64(ring.cursor, CURSOR_WRITE_FRAME_OFFSET, state.writeFrame);
  writeU64(ring.cursor, CURSOR_DROPPED_FRAMES_OFFSET, state.droppedFrames);
  writeU64(ring.cursor, CURSOR_UNDERRUN_FRAMES_OFFSET, state.underrunFrames);
  writeU64(ring.cursor, CURSOR_OVERRUN_FRAMES_OFFSET, state.overrunFrames);
  writeU64(ring.cursor, CURSOR_GENERATION_OFFSET, state.generation);
  ring.cursor.setUint32(CURSOR_FLAGS_OFFSET, state.flags, true);
}

export function wvstSharedAudioReadableFrames(
  ring: WVSTSharedAudioRingView,
  cursor = readWVSTSharedAudioCursor(ring),
): number {
  return Math.min(pendingFrames(cursor), ring.layout.capacityFrames);
}

export function wvstSharedAudioWritableFrames(
  ring: WVSTSharedAudioRingView,
  cursor = readWVSTSharedAudioCursor(ring),
): number {
  return ring.layout.capacityFrames - wvstSharedAudioReadableFrames(ring, cursor);
}

export function writeWVSTSharedAudioInterleaved(
  ring: WVSTSharedAudioRingView,
  startFrame: number,
  frames: number,
  source: Float32Array,
): void {
  validateFrameRequest(ring, startFrame, frames, source.length);
  copyIntoRing(ring, startFrame, frames, source);
}

export function readWVSTSharedAudioInterleaved(
  ring: WVSTSharedAudioRingView,
  startFrame: number,
  frames: number,
  destination: Float32Array,
): void {
  validateFrameRequest(ring, startFrame, frames, destination.length);
  copyFromRing(ring, startFrame, frames, destination);
}

function createSharedAudioBuffer(totalBytes: number): SharedArrayBuffer {
  if (typeof globalThis.SharedArrayBuffer !== "function") {
    throw new Error("WVST shared audio transport view requires SharedArrayBuffer");
  }
  return new SharedArrayBuffer(totalBytes);
}

function createRingView(
  memory: ArrayBufferLike,
  layout: SharedAudioRingLayout,
): WVSTSharedAudioRingView {
  if (layout.cursorBytes !== WVST_SHARED_AUDIO_CURSOR_BYTES) {
    throw new Error(`WVST shared audio cursor bytes mismatch: ${layout.cursorBytes}`);
  }
  validateMemoryRange(memory, layout.cursorOffset, layout.cursorBytes);
  validateMemoryRange(memory, layout.audioOffset, layout.audioBytes);
  if (layout.audioBytes % F32_BYTES !== 0) {
    throw new Error(`WVST shared audio ring audioBytes must be f32 aligned`);
  }

  return {
    role: layout.role,
    layout,
    cursor: new DataView(memory, layout.cursorOffset, layout.cursorBytes),
    samples: new Float32Array(
      memory,
      layout.audioOffset,
      layout.audioBytes / F32_BYTES,
    ),
  };
}

function validateFrameRequest(
  ring: WVSTSharedAudioRingView,
  startFrame: number,
  frames: number,
  sampleLength: number,
): void {
  assertNonNegativeInteger("startFrame", startFrame);
  assertNonNegativeInteger("frames", frames);
  if (frames > ring.layout.capacityFrames) {
    throw new Error(
      `WVST shared audio request exceeds ring capacity: ${frames} > ${ring.layout.capacityFrames}`,
    );
  }
  const expectedSamples = frames * ring.layout.channels;
  if (sampleLength !== expectedSamples) {
    throw new Error(
      `WVST shared audio sample length mismatch: expected ${expectedSamples}, got ${sampleLength}`,
    );
  }
}

function copyIntoRing(
  ring: WVSTSharedAudioRingView,
  startFrame: number,
  frames: number,
  source: Float32Array,
): void {
  const spans = ringSpans(ring.layout, startFrame, frames);
  ring.samples.set(source.subarray(0, spans.firstSamples), spans.firstSampleOffset);
  if (spans.secondSamples > 0) {
    ring.samples.set(source.subarray(spans.firstSamples), 0);
  }
}

function copyFromRing(
  ring: WVSTSharedAudioRingView,
  startFrame: number,
  frames: number,
  destination: Float32Array,
): void {
  const spans = ringSpans(ring.layout, startFrame, frames);
  destination.set(
    ring.samples.subarray(
      spans.firstSampleOffset,
      spans.firstSampleOffset + spans.firstSamples,
    ),
    0,
  );
  if (spans.secondSamples > 0) {
    destination.set(ring.samples.subarray(0, spans.secondSamples), spans.firstSamples);
  }
}

function ringSpans(
  layout: SharedAudioRingLayout,
  startFrame: number,
  frames: number,
): { firstSampleOffset: number; firstSamples: number; secondSamples: number } {
  const frameOffset = startFrame % layout.capacityFrames;
  const firstFrames = Math.min(frames, layout.capacityFrames - frameOffset);
  const secondFrames = frames - firstFrames;

  return {
    firstSampleOffset: frameOffset * layout.channels,
    firstSamples: firstFrames * layout.channels,
    secondSamples: secondFrames * layout.channels,
  };
}

function pendingFrames(cursor: SharedAudioRingCursorState): number {
  if (cursor.writeFrame < cursor.readFrame) {
    throw new Error(
      `WVST shared audio cursor order invalid: read ${cursor.readFrame}, write ${cursor.writeFrame}`,
    );
  }
  return cursor.writeFrame - cursor.readFrame;
}

function validateMemoryLength(memory: ArrayBufferLike, totalBytes: number): void {
  validateSafeInteger("totalBytes", totalBytes);
  if (memory.byteLength < totalBytes) {
    throw new Error(
      `WVST shared audio memory is too short: expected ${totalBytes}, got ${memory.byteLength}`,
    );
  }
}

function validateMemoryRange(
  memory: ArrayBufferLike,
  offset: number,
  bytes: number,
): void {
  validateSafeInteger("offset", offset);
  validateSafeInteger("bytes", bytes);
  if (offset + bytes > memory.byteLength) {
    throw new Error(
      `WVST shared audio range exceeds memory: ${offset}+${bytes} > ${memory.byteLength}`,
    );
  }
}

function readU64(view: DataView, offset: number): number {
  const value = view.getBigUint64(offset, true);
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`WVST shared audio u64 exceeds JS safe integer: ${value}`);
  }
  return Number(value);
}

function writeU64(view: DataView, offset: number, value: number): void {
  validateSafeInteger("u64", value);
  view.setBigUint64(offset, BigInt(value), true);
}

function validateSafeInteger(name: string, value: number): void {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`WVST shared audio ${name} must be a safe non-negative integer`);
  }
}

function assertNonNegativeInteger(name: string, value: number): void {
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(`WVST shared audio ${name} must be a non-negative integer`);
  }
}
