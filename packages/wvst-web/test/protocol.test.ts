import { describe, expect, it } from "vitest";
import {
  AudioFrameFlags,
  AudioSampleFormat,
  MidiEventKind,
  Vst3OutputEventKind,
  Vst3OutputEventPayloadEncoding,
  Vst3OutputEventPayloadFlags,
  audioPayloadBytes,
  decodeAudioFrame,
  decodeAudioFrameHeader,
  decodeMidiEvent,
  decodeParameterAutomationEvent,
  decodeVst3OutputEventPayloadText,
  encodeAudioFrame,
  encodeAudioFrameHeader,
  encodeMidiEvent,
  encodeParameterAutomationEvent,
  type AudioFrameHeader,
} from "../src/protocol/index.js";

describe("WVST audio frame protocol", () => {
  it("round-trips a header with MIDI, parameter, and VST3 output event counts", () => {
    const header: AudioFrameHeader = {
      streamId: 7n,
      sequence: 9n,
      sentFrameTime: 128n,
      sampleRate: 48_000,
      payloadBytes: 16,
      frames: 2,
      channels: 2,
      format: AudioSampleFormat.F32Le,
      flags: AudioFrameFlags.ProcessError,
      eventCount: 1,
      parameterEventCount: 2,
      vst3OutputEventCount: 3,
    };

    expect(decodeAudioFrameHeader(encodeAudioFrameHeader(header))).toEqual(header);
  });

  it("round-trips audio, MIDI, automation, and advanced VST3 output sections", () => {
    const audio = new Float32Array([0.1, -0.2, 0.3, -0.4]);
    const midiEvent = {
      sampleOffset: 4,
      kind: MidiEventKind.NoteOn,
      channel: 1,
      data1: 60,
      data2: 100,
      noteId: 42,
    };
    const parameterEvent = {
      sampleOffset: 5,
      parameterId: 99,
      valueNormalized: 0.75,
    };
    const vst3OutputEvent = {
      sampleOffset: 6,
      kind: Vst3OutputEventKind.NoteExpressionText,
      vst3EventType: 3,
      busIndex: 0,
      data1: 123,
      data2: 0,
      value: 0,
      dataSize: 4,
      dataType: 77,
      payloadEncoding: Vst3OutputEventPayloadEncoding.Utf8,
      payloadFlags: Vst3OutputEventPayloadFlags.Truncated,
      payloadBytes: new TextEncoder().encode("WVST"),
    };
    const header: AudioFrameHeader = {
      streamId: 11n,
      sequence: 12n,
      sentFrameTime: 13n,
      sampleRate: 44_100,
      payloadBytes: audio.byteLength + 16 + 16 + 104,
      frames: 2,
      channels: 2,
      format: AudioSampleFormat.F32Le,
      flags: 0,
      eventCount: 1,
      parameterEventCount: 1,
      vst3OutputEventCount: 1,
    };

    const decoded = decodeAudioFrame(
      encodeAudioFrame(
        header,
        audio.buffer,
        [midiEvent],
        [parameterEvent],
        [vst3OutputEvent],
      ),
    );

    expect(decoded.header).toEqual(header);
    expect(Array.from(new Float32Array(decoded.audioPayload))).toEqual(Array.from(audio));
    expect(decoded.events).toEqual([
      {
        ...midiEvent,
        data3: 0,
        dataLength: 2,
      },
    ]);
    expect(decoded.parameterEvents).toEqual([parameterEvent]);
    expect(decodeVst3OutputEventPayloadText(decoded.vst3OutputEvents[0])).toBe("WVST");
    expect(decoded.vst3OutputEvents[0].payloadFlags).toBe(
      Vst3OutputEventPayloadFlags.Truncated,
    );
  });

  it("rejects malformed payload section sizes before consumers see split buffers", () => {
    const header: AudioFrameHeader = {
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
    };

    expect(() => decodeAudioFrame(encodeAudioFrameHeader(header))).toThrow(
      /length mismatch/,
    );
  });

  it("validates MIDI and parameter event ranges", () => {
    expect(() =>
      decodeMidiEvent(
        encodeMidiEvent({
          sampleOffset: 0,
          kind: MidiEventKind.ControlChange,
          channel: 16,
          data1: 1,
          data2: 2,
        }),
      ),
    ).toThrow(/channel/);
    expect(() =>
      decodeParameterAutomationEvent(
        encodeParameterAutomationEvent({
          sampleOffset: 0,
          parameterId: 1,
          valueNormalized: 1.5,
        }),
      ),
    ).toThrow(/normalized/);
  });

  it("supports zero-input instrument payload sizing", () => {
    expect(
      audioPayloadBytes({
        streamId: 1n,
        sequence: 1n,
        sentFrameTime: 0n,
        sampleRate: 48_000,
        payloadBytes: 0,
        frames: 128,
        channels: 0,
        format: AudioSampleFormat.F32Le,
        flags: AudioFrameFlags.MidiOnly,
        eventCount: 1,
      }),
    ).toBe(0);
  });
});
