import {
  AUDIO_FRAME_HEADER_BYTES,
  AUDIO_FRAME_VERSION,
  MIDI_EVENT_BYTES,
  PARAMETER_AUTOMATION_EVENT_BYTES,
  VST3_OUTPUT_EVENT_BYTES,
  AudioSampleFormat,
  type AudioFrameHeader,
} from "../protocol/index.js";

export interface WVSTAudioFrameHeaderWasmView {
  readonly streamIdLow: number;
  readonly streamIdHigh: number;
  readonly sequenceLow: number;
  readonly sequenceHigh: number;
  readonly sentFrameTimeLow: number;
  readonly sentFrameTimeHigh: number;
  readonly sampleRate: number;
  readonly payloadBytes: number;
  readonly frames: number;
  readonly channels: number;
  readonly format: number;
  readonly flags: number;
  readonly eventCount: number;
  readonly parameterEventCount: number;
  readonly vst3OutputEventCount: number;
}

export interface WVSTProtocolWasmModule {
  audioFrameHeaderBytes(): number;
  audioFrameVersion(): number;
  midiEventBytes(): number;
  parameterAutomationEventBytes(): number;
  vst3OutputEventBytes(): number;
  expectedAudioFramePayloadBytes(
    frames: number,
    channels: number,
    midiEventCount: number,
    parameterEventCount: number,
    vst3OutputEventCount: number,
  ): number;
  encodeAudioFrameHeader(
    streamIdLow: number,
    streamIdHigh: number,
    sequenceLow: number,
    sequenceHigh: number,
    sentFrameTimeLow: number,
    sentFrameTimeHigh: number,
    sampleRate: number,
    frames: number,
    channels: number,
    flags: number,
    midiEventCount: number,
    parameterEventCount: number,
    vst3OutputEventCount: number,
  ): Uint8Array;
  decodeAudioFrameHeader(bytes: Uint8Array): WVSTAudioFrameHeaderWasmView;
}

export interface WVSTProtocolWasm {
  expectedAudioFramePayloadBytes(
    frames: number,
    channels: number,
    midiEventCount?: number,
    parameterEventCount?: number,
    vst3OutputEventCount?: number,
  ): number;
  encodeAudioFrameHeader(header: AudioFrameHeader): Uint8Array;
  decodeAudioFrameHeader(buffer: ArrayBufferLike): AudioFrameHeader;
}

export function createWVSTProtocolWasm(
  module: WVSTProtocolWasmModule,
): WVSTProtocolWasm {
  validateWVSTProtocolWasm(module);

  return {
    expectedAudioFramePayloadBytes: (
      frames,
      channels,
      midiEventCount = 0,
      parameterEventCount = 0,
      vst3OutputEventCount = 0,
    ) =>
      module.expectedAudioFramePayloadBytes(
        frames,
        channels,
        midiEventCount,
        parameterEventCount,
        vst3OutputEventCount,
      ),
    encodeAudioFrameHeader: (header) => encodeHeaderWithWasm(module, header),
    decodeAudioFrameHeader: (buffer) => decodeHeaderWithWasm(module, buffer),
  };
}

export function validateWVSTProtocolWasm(module: WVSTProtocolWasmModule): void {
  assertWasmConstant(
    "audio frame header bytes",
    module.audioFrameHeaderBytes(),
    AUDIO_FRAME_HEADER_BYTES,
  );
  assertWasmConstant("audio frame version", module.audioFrameVersion(), AUDIO_FRAME_VERSION);
  assertWasmConstant("MIDI event bytes", module.midiEventBytes(), MIDI_EVENT_BYTES);
  assertWasmConstant(
    "parameter automation event bytes",
    module.parameterAutomationEventBytes(),
    PARAMETER_AUTOMATION_EVENT_BYTES,
  );
  assertWasmConstant(
    "VST3 output event bytes",
    module.vst3OutputEventBytes(),
    VST3_OUTPUT_EVENT_BYTES,
  );
}

function encodeHeaderWithWasm(
  module: WVSTProtocolWasmModule,
  header: AudioFrameHeader,
): Uint8Array {
  if (header.format !== AudioSampleFormat.F32Le) {
    throw new Error(`WVST WASM protocol only supports f32-le audio frames`);
  }

  const parameterEventCount = header.parameterEventCount ?? 0;
  const vst3OutputEventCount = header.vst3OutputEventCount ?? 0;
  const expectedPayloadBytes = module.expectedAudioFramePayloadBytes(
    header.frames,
    header.channels,
    header.eventCount,
    parameterEventCount,
    vst3OutputEventCount,
  );
  if (expectedPayloadBytes !== header.payloadBytes) {
    throw new Error(
      `WVST WASM payload length mismatch: expected ${expectedPayloadBytes}, got ${header.payloadBytes}`,
    );
  }

  const [streamIdLow, streamIdHigh] = splitU64(header.streamId, "streamId");
  const [sequenceLow, sequenceHigh] = splitU64(header.sequence, "sequence");
  const [sentFrameTimeLow, sentFrameTimeHigh] = splitU64(
    header.sentFrameTime,
    "sentFrameTime",
  );

  return module.encodeAudioFrameHeader(
    streamIdLow,
    streamIdHigh,
    sequenceLow,
    sequenceHigh,
    sentFrameTimeLow,
    sentFrameTimeHigh,
    header.sampleRate,
    header.frames,
    header.channels,
    header.flags,
    header.eventCount,
    parameterEventCount,
    vst3OutputEventCount,
  );
}

function decodeHeaderWithWasm(
  module: WVSTProtocolWasmModule,
  buffer: ArrayBufferLike,
): AudioFrameHeader {
  const view = module.decodeAudioFrameHeader(new Uint8Array(buffer));

  return {
    streamId: joinU64(view.streamIdLow, view.streamIdHigh),
    sequence: joinU64(view.sequenceLow, view.sequenceHigh),
    sentFrameTime: joinU64(view.sentFrameTimeLow, view.sentFrameTimeHigh),
    sampleRate: view.sampleRate,
    payloadBytes: view.payloadBytes,
    frames: view.frames,
    channels: view.channels,
    format: view.format,
    flags: view.flags,
    eventCount: view.eventCount,
    parameterEventCount: view.parameterEventCount,
    vst3OutputEventCount: view.vst3OutputEventCount,
  };
}

function assertWasmConstant(label: string, actual: number, expected: number): void {
  if (actual !== expected) {
    throw new Error(`WVST WASM ${label} mismatch: expected ${expected}, got ${actual}`);
  }
}

function splitU64(value: bigint, label: string): [number, number] {
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new Error(`WVST ${label} must fit in an unsigned 64-bit integer`);
  }

  const low = Number(value & 0xffff_ffffn);
  const high = Number((value >> 32n) & 0xffff_ffffn);
  return [low, high];
}

function joinU64(low: number, high: number): bigint {
  return (BigInt(high >>> 0) << 32n) | BigInt(low >>> 0);
}
