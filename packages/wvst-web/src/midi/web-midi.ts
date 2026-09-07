import { MidiEventKind, type MidiEvent } from "../protocol/index.js";
import {
  resolveWVSTMidiSampleOffset, validateWVSTMidiChannel, type WVSTMidiEventTarget,
} from "./core.js";
import { webMidiMessageToWVSTEvents } from "./messages.js";

export interface WVSTWebMidiRequestOptions {
  sysex?: boolean;
  software?: boolean;
}

export interface WVSTWebMidiInput {
  id?: string;
  state?: string;
  addEventListener(type: "midimessage", listener: (event: WVSTWebMidiMessageEvent) => void): void;
  removeEventListener(type: "midimessage", listener: (event: WVSTWebMidiMessageEvent) => void): void;
}

export interface WVSTWebMidiInputCollection {
  [Symbol.iterator]?(): Iterator<WVSTWebMidiInput>;
  values?(): IterableIterator<WVSTWebMidiInput>;
}

export interface WVSTWebMidiAccess {
  inputs: WVSTWebMidiInputCollection;
  addEventListener?(type: "statechange", listener: () => void): void;
  removeEventListener?(type: "statechange", listener: () => void): void;
}

export interface WVSTWebMidiMessageEvent {
  data: ArrayLike<number>;
  timeStamp?: number;
}

export interface WVSTWebMidiAdapterOptions {
  channel?: number;
  /** Bind only these input IDs; omitted means all connected inputs. */
  inputIds?: readonly string[];
  /** Offset within the next input block; DOM timestamps are not audio-clock timestamps. */
  sampleOffset?: number;
  rawFallback?: boolean;
  midiAccess?: WVSTWebMidiAccess;
  requestOptions?: WVSTWebMidiRequestOptions;
  onError?: (error: Error) => void;
}

export interface WVSTWebMidiAdapter {
  access: WVSTWebMidiAccess;
  /** Enqueue releases for tracked notes and pedals; completion follows the target's send acknowledgement. */
  panic(): Promise<void>;
  /** Detach listeners, then enqueue releases. The worker target acknowledges enqueue, not native processing. */
  stop(): Promise<void>;
}

interface InputBinding {
  listener: (event: WVSTWebMidiMessageEvent) => void;
  notes: Set<number>;
  pedals: Set<number>;
}

export async function createWVSTWebMidiAdapter(
  target: WVSTMidiEventTarget,
  options: WVSTWebMidiAdapterOptions = {},
): Promise<WVSTWebMidiAdapter> {
  resolveWVSTMidiSampleOffset(target, options.sampleOffset);
  if (options.channel !== undefined) validateWVSTMidiChannel(options.channel);
  if (options.requestOptions?.sysex) throw new Error("WVST Web MIDI does not support SysEx");
  const access = options.midiAccess ?? await requestWebMidiAccess(options.requestOptions);
  const bindings = new Map<WVSTWebMidiInput, InputBinding>();
  let stopped = false;
  let stopPromise: Promise<void> | undefined;

  function report(error: unknown): void {
    options.onError?.(error instanceof Error ? error : new Error(String(error)));
  }

  async function send(events: MidiEvent[]): Promise<void> {
    if (events.length === 0) return;
    try {
      await target.sendMidiEvents(events);
    } catch (error) {
      report(error);
    }
  }

  function release(binding: InputBinding): Promise<void> {
    const events: MidiEvent[] = [];
    for (const key of binding.notes) {
      if (Array.from(bindings.values()).some((other) => other !== binding && other.notes.has(key))) continue;
      events.push({ sampleOffset: 0, kind: MidiEventKind.NoteOff, channel: key >> 7, data1: key & 127, data2: 0 });
    }
    for (const key of binding.pedals) {
      if (Array.from(bindings.values()).some((other) => other !== binding && other.pedals.has(key))) continue;
      events.push({ sampleOffset: 0, kind: MidiEventKind.ControlChange, channel: key >> 7, data1: key & 127, data2: 0 });
    }
    binding.notes.clear();
    binding.pedals.clear();
    return send(events);
  }

  function syncInputs(): void {
    if (stopped) return;
    try {
      const inputs = new Set(webMidiInputs(access).filter((input) =>
        input.state !== "disconnected" && (!options.inputIds || (input.id !== undefined && options.inputIds.includes(input.id)))));
      for (const [input, binding] of bindings) {
        if (!inputs.has(input)) {
          input.removeEventListener("midimessage", binding.listener);
          bindings.delete(input);
          void release(binding);
        }
      }
      for (const input of inputs) {
        if (bindings.has(input)) continue;
        const binding: InputBinding = {
          notes: new Set(), pedals: new Set(),
          listener: (event) => {
            if (stopped || !bindings.has(input)) return;
            try {
              const events = webMidiMessageToWVSTEvents(event.data, target, options);
              for (const midi of events) trackEvent(binding, midi);
              void send(events);
            } catch (error) {
              report(error);
            }
          },
        };
        bindings.set(input, binding);
        input.addEventListener("midimessage", binding.listener);
      }
    } catch (error) {
      report(error);
    }
  }

  // Validate before installing listeners so malformed access rejects setup.
  webMidiInputs(access);
  access.addEventListener?.("statechange", syncInputs);
  syncInputs();
  return {
    access,
    panic: async () => { await Promise.all(Array.from(bindings.values(), release)); },
    stop: () => {
      if (stopPromise) return stopPromise;
      stopped = true;
      access.removeEventListener?.("statechange", syncInputs);
      for (const [input, binding] of bindings) input.removeEventListener("midimessage", binding.listener);
      stopPromise = Promise.all(Array.from(bindings.values(), release)).then(() => { bindings.clear(); });
      return stopPromise;
    },
  };
}

function trackEvent(binding: InputBinding, event: MidiEvent): void {
  const key = (event.channel << 7) | event.data1;
  if (event.kind === MidiEventKind.NoteOn) binding.notes.add(key);
  if (event.kind === MidiEventKind.NoteOff) binding.notes.delete(key);
  if (event.kind === MidiEventKind.ControlChange && [64, 66, 69].includes(event.data1)) {
    if (event.data2 >= 64) binding.pedals.add(key);
    else binding.pedals.delete(key);
  }
}

async function requestWebMidiAccess(options: WVSTWebMidiRequestOptions | undefined): Promise<WVSTWebMidiAccess> {
  if (typeof navigator === "undefined" || typeof navigator.requestMIDIAccess !== "function") {
    throw new Error("WVST Web MIDI adapter requires navigator.requestMIDIAccess");
  }
  // DOM.Iterable is optional for SDK consumers; validate the input collection at runtime.
  return navigator.requestMIDIAccess(options) as unknown as Promise<WVSTWebMidiAccess>;
}

function webMidiInputs(access: WVSTWebMidiAccess): WVSTWebMidiInput[] {
  if (typeof access.inputs.values === "function") return Array.from(access.inputs.values());
  if (typeof access.inputs[Symbol.iterator] === "function") return Array.from(access.inputs as Iterable<WVSTWebMidiInput>);
  throw new Error("WVST Web MIDI access did not expose iterable inputs");
}
