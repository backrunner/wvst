import { MidiEventKind, type MidiEvent } from "../protocol/index.js";
import {
  createWVSTMidiEvent,
  createWVSTPitchBendEvent,
  normalizedToWVSTMidiValue,
  resolveWVSTMidiSampleOffset,
  validateWVSTMidiChannel,
  validateWVSTMidiData7,
  type WVSTMidiEventTarget,
} from "./core.js";

export interface WVSTVirtualKeyboardOptions {
  channel?: number;
  sampleOffset?: number;
}

export interface WVSTVirtualKeyboardNoteOptions {
  channel?: number;
  sampleOffset?: number;
  noteId?: number;
}

export interface WVSTVirtualKeyboard {
  noteOn(
    note: number,
    velocity?: number,
    options?: WVSTVirtualKeyboardNoteOptions,
  ): Promise<void>;
  noteOff(
    note: number,
    velocity?: number,
    options?: WVSTVirtualKeyboardNoteOptions,
  ): Promise<void>;
  allNotesOff(options?: WVSTVirtualKeyboardNoteOptions): Promise<void>;
  controlChange(
    controller: number,
    value: number,
    options?: WVSTVirtualKeyboardNoteOptions,
  ): Promise<void>;
  pitchBend(value: number, options?: WVSTVirtualKeyboardNoteOptions): Promise<void>;
  channelAftertouch(
    value: number,
    options?: WVSTVirtualKeyboardNoteOptions,
  ): Promise<void>;
  polyAftertouch(
    note: number,
    value: number,
    options?: WVSTVirtualKeyboardNoteOptions,
  ): Promise<void>;
}

export interface WVSTKeyboardEventMapOptions {
  keyMap?: Record<string, number>;
  velocity?: number;
}

export const WVST_DEFAULT_KEYBOARD_MIDI_MAP: Record<string, number> = {
  KeyA: 60,
  KeyW: 61,
  KeyS: 62,
  KeyE: 63,
  KeyD: 64,
  KeyF: 65,
  KeyT: 66,
  KeyG: 67,
  KeyY: 68,
  KeyH: 69,
  KeyU: 70,
  KeyJ: 71,
  KeyK: 72,
};

export function createWVSTVirtualKeyboard(
  target: WVSTMidiEventTarget,
  options: WVSTVirtualKeyboardOptions = {},
): WVSTVirtualKeyboard {
  const activeNotes = new Map<string, number>();
  let nextNoteId = 1;

  return {
    noteOn: (note, velocity = 1, eventOptions = {}) => {
      const event = noteEvent(
        target,
        MidiEventKind.NoteOn,
        note,
        velocity,
        options,
        eventOptions,
      );
      const noteId = eventOptions.noteId ?? nextNoteId++;
      event.noteId = noteId;
      activeNotes.set(noteKey(event.channel, note), noteId);
      return target.sendMidiEvents([event]);
    },
    noteOff: (note, velocity = 0, eventOptions = {}) => {
      const event = noteEvent(
        target,
        MidiEventKind.NoteOff,
        note,
        velocity,
        options,
        eventOptions,
      );
      const key = noteKey(event.channel, note);
      event.noteId = eventOptions.noteId ?? activeNotes.get(key) ?? 0;
      activeNotes.delete(key);
      return target.sendMidiEvents([event]);
    },
    allNotesOff: (eventOptions = {}) => {
      const events: MidiEvent[] = [];
      for (const [key, noteId] of activeNotes) {
        const [channel, note] = key.split(":").map(Number);
        events.push({
          sampleOffset: sampleOffset(target, options, eventOptions),
          kind: MidiEventKind.NoteOff,
          channel: validateWVSTMidiChannel(eventOptions.channel ?? channel),
          data1: validateWVSTMidiData7("note", note),
          data2: 0,
          noteId,
        });
      }
      activeNotes.clear();
      return events.length === 0 ? Promise.resolve() : target.sendMidiEvents(events);
    },
    controlChange: (controller, value, eventOptions = {}) =>
      target.sendMidiEvents([
        createWVSTMidiEvent(
          sampleOffset(target, options, eventOptions),
          MidiEventKind.ControlChange,
          channel(options, eventOptions),
          validateWVSTMidiData7("controller", controller),
          normalizedToWVSTMidiValue(value),
        ),
      ]),
    pitchBend: (value, eventOptions = {}) =>
      target.sendMidiEvents([
        createWVSTPitchBendEvent(
          sampleOffset(target, options, eventOptions),
          channel(options, eventOptions),
          value,
        ),
      ]),
    channelAftertouch: (value, eventOptions = {}) =>
      target.sendMidiEvents([
        createWVSTMidiEvent(
          sampleOffset(target, options, eventOptions),
          MidiEventKind.ChannelAftertouch,
          channel(options, eventOptions),
          normalizedToWVSTMidiValue(value),
          0,
        ),
      ]),
    polyAftertouch: (note, value, eventOptions = {}) =>
      target.sendMidiEvents([
        createWVSTMidiEvent(
          sampleOffset(target, options, eventOptions),
          MidiEventKind.PolyAftertouch,
          channel(options, eventOptions),
          validateWVSTMidiData7("note", note),
          normalizedToWVSTMidiValue(value),
        ),
      ]),
  };
}

export function handleWVSTVirtualKeyboardEvent(
  keyboard: WVSTVirtualKeyboard,
  event: KeyboardEvent,
  options: WVSTKeyboardEventMapOptions = {},
): boolean {
  if (event.repeat) {
    return false;
  }

  const note = (options.keyMap ?? WVST_DEFAULT_KEYBOARD_MIDI_MAP)[event.code];
  if (note === undefined) {
    return false;
  }

  if (event.type === "keydown") {
    void keyboard.noteOn(note, options.velocity ?? 0.8);
    return true;
  }
  if (event.type === "keyup") {
    void keyboard.noteOff(note);
    return true;
  }
  return false;
}

function noteEvent(
  target: WVSTMidiEventTarget,
  kind: MidiEventKind.NoteOn | MidiEventKind.NoteOff,
  note: number,
  velocity: number,
  defaults: WVSTVirtualKeyboardOptions,
  options: WVSTVirtualKeyboardNoteOptions,
): MidiEvent {
  return createWVSTMidiEvent(
    sampleOffset(target, defaults, options),
    kind,
    channel(defaults, options),
    validateWVSTMidiData7("note", note),
    normalizedToWVSTMidiValue(velocity),
  );
}

function sampleOffset(
  target: WVSTMidiEventTarget,
  defaults: WVSTVirtualKeyboardOptions,
  options: WVSTVirtualKeyboardNoteOptions,
): number {
  return resolveWVSTMidiSampleOffset(
    target,
    options.sampleOffset ?? defaults.sampleOffset,
  );
}

function channel(
  defaults: WVSTVirtualKeyboardOptions,
  options: WVSTVirtualKeyboardNoteOptions,
): number {
  return validateWVSTMidiChannel(options.channel ?? defaults.channel ?? 0);
}

function noteKey(channel: number, note: number): string {
  return `${channel}:${validateWVSTMidiData7("note", note)}`;
}
