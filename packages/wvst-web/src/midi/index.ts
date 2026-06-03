import {
  MidiEventKind,
  type MidiEvent,
} from "../protocol/index.js";
import {
  createWVSTMidiEvent,
  createWVSTRawMidiEvent,
  resolveWVSTMidiSampleOffset,
  validateWVSTMidiChannel,
  wvstMidiBytes,
  type WVSTMidiEventTarget,
} from "./core.js";

export type { WVSTMidiEventTarget } from "./core.js";
export {
  WVST_DEFAULT_KEYBOARD_MIDI_MAP,
  createWVSTVirtualKeyboard,
  handleWVSTVirtualKeyboardEvent,
  type WVSTKeyboardEventMapOptions,
  type WVSTVirtualKeyboard,
  type WVSTVirtualKeyboardNoteOptions,
  type WVSTVirtualKeyboardOptions,
} from "./virtual-keyboard.js";

export interface WVSTWebMidiRequestOptions {
  sysex?: boolean;
  software?: boolean;
}

export interface WVSTWebMidiInput {
  addEventListener(
    type: "midimessage",
    listener: (event: WVSTWebMidiMessageEvent) => void,
  ): void;
  removeEventListener(
    type: "midimessage",
    listener: (event: WVSTWebMidiMessageEvent) => void,
  ): void;
}

export interface WVSTWebMidiInputCollection {
  [Symbol.iterator]?(): Iterator<WVSTWebMidiInput>;
  values?(): IterableIterator<WVSTWebMidiInput>;
}

export interface WVSTWebMidiAccess {
  inputs: WVSTWebMidiInputCollection;
}

export interface WVSTWebMidiMessageEvent {
  data: ArrayLike<number>;
  timeStamp?: number;
}

export interface WVSTWebMidiAdapterOptions {
  channel?: number;
  sampleOffset?: number;
  rawFallback?: boolean;
  midiAccess?: WVSTWebMidiAccess;
  requestOptions?: WVSTWebMidiRequestOptions;
  onError?: (error: Error) => void;
}

export interface WVSTWebMidiAdapter {
  access: WVSTWebMidiAccess;
  stop(): void;
}

export async function createWVSTWebMidiAdapter(
  target: WVSTMidiEventTarget,
  options: WVSTWebMidiAdapterOptions = {},
): Promise<WVSTWebMidiAdapter> {
  const access = options.midiAccess ?? (await requestWebMidiAccess(options.requestOptions));
  const listener = (event: WVSTWebMidiMessageEvent) => {
    const events = webMidiMessageToWVSTEvents(event.data, target, options);
    if (events.length === 0) {
      return;
    }

    void target.sendMidiEvents(events).catch((error: unknown) => {
      options.onError?.(error instanceof Error ? error : new Error(String(error)));
    });
  };
  const inputs = webMidiInputs(access);
  for (const input of inputs) {
    input.addEventListener("midimessage", listener);
  }

  return {
    access,
    stop: () => {
      for (const input of inputs) {
        input.removeEventListener("midimessage", listener);
      }
    },
  };
}

export function webMidiMessageToWVSTEvents(
  data: ArrayLike<number>,
  target: WVSTMidiEventTarget,
  options: WVSTWebMidiAdapterOptions = {},
): MidiEvent[] {
  const bytes = wvstMidiBytes(data);
  if (bytes.length === 0) {
    return [];
  }

  const sampleOffset = resolveWVSTMidiSampleOffset(target, options.sampleOffset);
  const status = bytes[0] ?? 0;
  const channel = status & 0x0f;
  if (options.channel !== undefined && channel !== validateWVSTMidiChannel(options.channel)) {
    return [];
  }

  const data1 = bytes[1] ?? 0;
  const data2 = bytes[2] ?? 0;
  switch (status & 0xf0) {
    case 0x80:
      return [createWVSTMidiEvent(sampleOffset, MidiEventKind.NoteOff, channel, data1, data2)];
    case 0x90:
      return [
        createWVSTMidiEvent(
          sampleOffset,
          data2 === 0 ? MidiEventKind.NoteOff : MidiEventKind.NoteOn,
          channel,
          data1,
          data2,
        ),
      ];
    case 0xa0:
      return [
        createWVSTMidiEvent(sampleOffset, MidiEventKind.PolyAftertouch, channel, data1, data2),
      ];
    case 0xb0:
      return [
        createWVSTMidiEvent(sampleOffset, MidiEventKind.ControlChange, channel, data1, data2),
      ];
    case 0xd0:
      return [
        createWVSTMidiEvent(
          sampleOffset,
          MidiEventKind.ChannelAftertouch,
          channel,
          data1,
          0,
        ),
      ];
    case 0xe0:
      return [
        createWVSTMidiEvent(sampleOffset, MidiEventKind.PitchBend, channel, data1, data2),
      ];
    default:
      return options.rawFallback === false
        ? []
        : [createWVSTRawMidiEvent(sampleOffset, bytes)];
  }
}

async function requestWebMidiAccess(
  options: WVSTWebMidiRequestOptions | undefined,
): Promise<WVSTWebMidiAccess> {
  const navigatorWithMidi = navigator as Navigator & {
    requestMIDIAccess?: (
      options?: WVSTWebMidiRequestOptions,
    ) => Promise<unknown>;
  };
  if (typeof navigatorWithMidi.requestMIDIAccess !== "function") {
    throw new Error("WVST Web MIDI adapter requires navigator.requestMIDIAccess");
  }

  return navigatorWithMidi.requestMIDIAccess(options) as Promise<WVSTWebMidiAccess>;
}

function webMidiInputs(access: WVSTWebMidiAccess): WVSTWebMidiInput[] {
  if (typeof access.inputs.values === "function") {
    return Array.from(access.inputs.values());
  }
  if (typeof access.inputs[Symbol.iterator] === "function") {
    return Array.from(access.inputs as Iterable<WVSTWebMidiInput>);
  }
  throw new Error("WVST Web MIDI access did not expose iterable inputs");
}
