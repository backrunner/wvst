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

export { webMidiMessageToWVSTEvents } from "./messages.js";
export {
  createWVSTWebMidiAdapter,
  type WVSTWebMidiRequestOptions,
  type WVSTWebMidiInput,
  type WVSTWebMidiInputCollection,
  type WVSTWebMidiAccess,
  type WVSTWebMidiMessageEvent,
  type WVSTWebMidiAdapterOptions,
  type WVSTWebMidiAdapter,
} from "./web-midi.js";
