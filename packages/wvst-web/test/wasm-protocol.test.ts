import { describe, expect, it } from "vitest";
import {
  AUDIO_FRAME_HEADER_BYTES,
  AUDIO_FRAME_VERSION,
  AudioFrameFlags,
  AudioSampleFormat,
  MIDI_EVENT_BYTES,
  PARAMETER_AUTOMATION_EVENT_BYTES,
  VST3_OUTPUT_EVENT_BYTES,
  decodeAudioFrameHeader,
  encodeAudioFrameHeader,
  type AudioFrameHeader,
} from "../src/protocol/index.js";
import {
  createWVSTProtocolWasm,
  type WVSTAudioFrameHeaderWasmView,
  type WVSTProtocolWasmModule,
} from "../src/wasm/protocol.js";

describe("WVST protocol WASM adapter", () => {
  it("round-trips audio frame headers through WASM-shaped exports", () => {
    const wasm = createWVSTProtocolWasm(fakeProtocolWasmModule());
    const header: AudioFrameHeader = {
      streamId: 0x1_0000_0002n,
      sequence: 0x3_0000_0004n,
      sentFrameTime: 0x5_0000_0006n,
      sampleRate: 48_000,
      payloadBytes: 8 * 4 + MIDI_EVENT_BYTES + PARAMETER_AUTOMATION_EVENT_BYTES,
      frames: 4,
      channels: 2,
      format: AudioSampleFormat.F32Le,
      flags: AudioFrameFlags.ProcessError,
      eventCount: 1,
      parameterEventCount: 1,
      vst3OutputEventCount: 0,
    };

    expect(wasm.decodeAudioFrameHeader(wasm.encodeAudioFrameHeader(header))).toEqual(
      header,
    );
  });

  it("rejects a WASM module with mismatched protocol constants", () => {
    const module = fakeProtocolWasmModule({
      audioFrameVersion: () => AUDIO_FRAME_VERSION + 1,
    });

    expect(() => createWVSTProtocolWasm(module)).toThrow(/version mismatch/);
  });

  it("checks payload size before encoding headers", () => {
    const wasm = createWVSTProtocolWasm(fakeProtocolWasmModule());

    expect(() =>
      wasm.encodeAudioFrameHeader({
        streamId: 1n,
        sequence: 1n,
        sentFrameTime: 0n,
        sampleRate: 48_000,
        payloadBytes: 7,
        frames: 1,
        channels: 1,
        format: AudioSampleFormat.F32Le,
        flags: 0,
        eventCount: 0,
      }),
    ).toThrow(/payload length mismatch/);
  });
});

function fakeProtocolWasmModule(
  overrides: Partial<WVSTProtocolWasmModule> = {},
): WVSTProtocolWasmModule {
  return {
    audioFrameHeaderBytes: () => AUDIO_FRAME_HEADER_BYTES,
    audioFrameVersion: () => AUDIO_FRAME_VERSION,
    midiEventBytes: () => MIDI_EVENT_BYTES,
    parameterAutomationEventBytes: () => PARAMETER_AUTOMATION_EVENT_BYTES,
    vst3OutputEventBytes: () => VST3_OUTPUT_EVENT_BYTES,
    expectedAudioFramePayloadBytes: (
      frames,
      channels,
      midiEventCount,
      parameterEventCount,
      vst3OutputEventCount,
    ) =>
      frames * channels * 4 +
      midiEventCount * MIDI_EVENT_BYTES +
      parameterEventCount * PARAMETER_AUTOMATION_EVENT_BYTES +
      vst3OutputEventCount * VST3_OUTPUT_EVENT_BYTES,
    encodeAudioFrameHeader: (
      streamIdLow,
      streamIdHigh,
      sequenceLow,
      sequenceHigh,
      sentFrameTimeLow,
      sentFrameTimeHigh,
      sampleRate,
      frames,
      channels,
      flags,
      eventCount,
      parameterEventCount,
      vst3OutputEventCount,
    ) =>
      new Uint8Array(
        encodeAudioFrameHeader({
          streamId: joinU64(streamIdLow, streamIdHigh),
          sequence: joinU64(sequenceLow, sequenceHigh),
          sentFrameTime: joinU64(sentFrameTimeLow, sentFrameTimeHigh),
          sampleRate,
          payloadBytes:
            frames * channels * 4 +
            eventCount * MIDI_EVENT_BYTES +
            parameterEventCount * PARAMETER_AUTOMATION_EVENT_BYTES +
            vst3OutputEventCount * VST3_OUTPUT_EVENT_BYTES,
          frames,
          channels,
          format: AudioSampleFormat.F32Le,
          flags,
          eventCount,
          parameterEventCount,
          vst3OutputEventCount,
        }),
      ),
    decodeAudioFrameHeader: (bytes) => wasmHeaderView(decodeBytes(bytes)),
    ...overrides,
  };
}

function wasmHeaderView(header: AudioFrameHeader): WVSTAudioFrameHeaderWasmView {
  const [streamIdLow, streamIdHigh] = splitU64(header.streamId);
  const [sequenceLow, sequenceHigh] = splitU64(header.sequence);
  const [sentFrameTimeLow, sentFrameTimeHigh] = splitU64(header.sentFrameTime);

  return {
    streamIdLow,
    streamIdHigh,
    sequenceLow,
    sequenceHigh,
    sentFrameTimeLow,
    sentFrameTimeHigh,
    sampleRate: header.sampleRate,
    payloadBytes: header.payloadBytes,
    frames: header.frames,
    channels: header.channels,
    format: header.format,
    flags: header.flags,
    eventCount: header.eventCount,
    parameterEventCount: header.parameterEventCount ?? 0,
    vst3OutputEventCount: header.vst3OutputEventCount ?? 0,
  };
}

function decodeBytes(bytes: Uint8Array): AudioFrameHeader {
  const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  return decodeAudioFrameHeader(buffer);
}

function splitU64(value: bigint): [number, number] {
  return [Number(value & 0xffff_ffffn), Number((value >> 32n) & 0xffff_ffffn)];
}

function joinU64(low: number, high: number): bigint {
  return (BigInt(high >>> 0) << 32n) | BigInt(low >>> 0);
}
