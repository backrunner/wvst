use crate::vst3_abi::{
    Event, VST3_EVENT_TYPE_CHORD, VST3_EVENT_TYPE_DATA, VST3_EVENT_TYPE_NOTE_EXPRESSION_TEXT,
    VST3_EVENT_TYPE_SCALE,
};

pub const VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES: usize = 64;
pub const VST3_OUTPUT_PAYLOAD_ENCODING_NONE: u8 = 0;
pub const VST3_OUTPUT_PAYLOAD_ENCODING_RAW_BYTES: u8 = 1;
pub const VST3_OUTPUT_PAYLOAD_ENCODING_UTF8: u8 = 2;
pub const VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED: u8 = 1 << 0;
pub const VST3_OUTPUT_PAYLOAD_FLAG_UNAVAILABLE: u8 = 1 << 1;
pub const VST3_OUTPUT_PAYLOAD_FLAG_INVALID_TEXT: u8 = 1 << 2;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct CapturedVst3OutputPayload {
    pub(crate) size: u16,
    pub(crate) encoding: u8,
    pub(crate) flags: u8,
    pub(crate) bytes: [u8; VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES],
}

impl CapturedVst3OutputPayload {
    pub(crate) const fn empty() -> Self {
        Self {
            size: 0,
            encoding: VST3_OUTPUT_PAYLOAD_ENCODING_NONE,
            flags: 0,
            bytes: [0; VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES],
        }
    }

    pub(crate) fn for_event(event: Event) -> Self {
        match event.event_type {
            VST3_EVENT_TYPE_DATA => {
                // SAFETY: The event type tag selects the VST3 DataEvent union arm.
                let data = unsafe { event.payload.data };
                Self::raw_bytes(data.bytes, data.size as usize)
            }
            VST3_EVENT_TYPE_NOTE_EXPRESSION_TEXT => {
                // SAFETY: The event type tag selects the note-expression text union arm.
                let text = unsafe { event.payload.note_expression_text };
                Self::utf16_text(text.text, text.text_len as usize)
            }
            VST3_EVENT_TYPE_CHORD => {
                // SAFETY: The event type tag selects the chord union arm.
                let chord = unsafe { event.payload.chord };
                Self::utf16_text(chord.text, usize::from(chord.text_len))
            }
            VST3_EVENT_TYPE_SCALE => {
                // SAFETY: The event type tag selects the scale union arm.
                let scale = unsafe { event.payload.scale };
                Self::utf16_text(scale.text, usize::from(scale.text_len))
            }
            _ => Self::empty(),
        }
    }

    fn raw_bytes(source: *const u8, total_len: usize) -> Self {
        let mut output = Self {
            encoding: VST3_OUTPUT_PAYLOAD_ENCODING_RAW_BYTES,
            ..Self::empty()
        };
        if total_len == 0 {
            return output;
        }
        if source.is_null() {
            output.flags |= VST3_OUTPUT_PAYLOAD_FLAG_UNAVAILABLE;
            return output;
        }

        let copy_len = total_len.min(VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES);
        // SAFETY: VST3 output event payload pointers are valid during addEvent().
        // WVST copies at that boundary, caps the length, and never stores the
        // plugin-owned pointer beyond this function.
        let bytes = unsafe { std::slice::from_raw_parts(source, copy_len) };
        output.bytes[..copy_len].copy_from_slice(bytes);
        output.size = copy_len as u16;
        if copy_len < total_len {
            output.flags |= VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED;
        }
        output
    }

    fn utf16_text(source: *const u16, total_units: usize) -> Self {
        let mut output = Self {
            encoding: VST3_OUTPUT_PAYLOAD_ENCODING_UTF8,
            ..Self::empty()
        };
        if total_units == 0 {
            return output;
        }
        if source.is_null() {
            output.flags |= VST3_OUTPUT_PAYLOAD_FLAG_UNAVAILABLE;
            return output;
        }

        let mut unit_index = 0;
        while unit_index < total_units {
            let (character, consumed, valid) =
                // SAFETY: The VST3 text pointer is valid for `total_units`
                // UTF-16 code units during addEvent(). We only read the current
                // code unit and, for surrogate pairs, one checked following unit.
                unsafe { decode_utf16_at(source, total_units, unit_index) };
            if !valid {
                output.flags |= VST3_OUTPUT_PAYLOAD_FLAG_INVALID_TEXT;
            }

            let mut encoded = [0; 4];
            let encoded = character.encode_utf8(&mut encoded).as_bytes();
            let next_size = usize::from(output.size) + encoded.len();
            if next_size > VST3_ADVANCED_OUTPUT_EVENT_PAYLOAD_BYTES {
                output.flags |= VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED;
                break;
            }

            let start = usize::from(output.size);
            output.bytes[start..next_size].copy_from_slice(encoded);
            output.size = next_size as u16;
            unit_index += consumed;
        }

        if unit_index < total_units {
            output.flags |= VST3_OUTPUT_PAYLOAD_FLAG_TRUNCATED;
        }
        output
    }
}

impl Default for CapturedVst3OutputPayload {
    fn default() -> Self {
        Self::empty()
    }
}

unsafe fn decode_utf16_at(source: *const u16, len: usize, index: usize) -> (char, usize, bool) {
    // SAFETY: The caller guarantees `index < len` and that `source` points to
    // `len` UTF-16 code units for the duration of this boundary copy.
    let first = unsafe { *source.add(index) };
    if (0xd800..=0xdbff).contains(&first) {
        if index + 1 >= len {
            return ('\u{fffd}', 1, false);
        }
        // SAFETY: `index + 1 < len` was checked above.
        let second = unsafe { *source.add(index + 1) };
        if !(0xdc00..=0xdfff).contains(&second) {
            return ('\u{fffd}', 1, false);
        }
        let high = u32::from(first) - 0xd800;
        let low = u32::from(second) - 0xdc00;
        let scalar = 0x1_0000 + ((high << 10) | low);
        return (char::from_u32(scalar).unwrap_or('\u{fffd}'), 2, true);
    }
    if (0xdc00..=0xdfff).contains(&first) {
        return ('\u{fffd}', 1, false);
    }
    (
        char::from_u32(u32::from(first)).unwrap_or('\u{fffd}'),
        1,
        true,
    )
}
