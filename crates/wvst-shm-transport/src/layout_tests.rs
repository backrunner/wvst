use super::*;

#[test]
fn computes_aligned_stereo_layout() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
        .layout()
        .expect("layout");

    assert_eq!(layout.input.cursor_offset, 192);
    assert_eq!(layout.input.audio_offset, 256);
    assert_eq!(layout.input.capacity_frames, 512);
    assert_eq!(layout.input.audio_bytes, 4_096);
    assert_eq!(layout.output.cursor_offset, 4_352);
    assert_eq!(layout.output.audio_offset, 4_416);
    assert_eq!(layout.output.audio_bytes, 4_096);
    assert_eq!(layout.total_bytes, 8_512);
    assert_eq!(layout.input.cursor_offset % CACHE_LINE_BYTES, 0);
    assert_eq!(layout.output.audio_offset % CACHE_LINE_BYTES, 0);
}

#[test]
fn supports_zero_input_instrument_layout() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 0, 2)
        .layout()
        .expect("layout");

    assert_eq!(layout.input.capacity_samples, 0);
    assert_eq!(layout.input.audio_bytes, 0);
    assert_eq!(layout.output.cursor_offset, layout.input.audio_offset);
    assert!(layout.total_bytes > layout.output.audio_offset);
}

#[test]
fn descriptor_encodes_header_and_ring_offsets() {
    let layout = SharedAudioTransportConfig::new(44_100, 64, 3, 1, 2)
        .layout()
        .expect("layout");
    let descriptor = layout.descriptor_bytes();

    assert_eq!(read_u32(&descriptor, 0), SHARED_AUDIO_LAYOUT_MAGIC);
    assert_eq!(read_u16(&descriptor, 4), SHARED_AUDIO_LAYOUT_VERSION);
    assert_eq!(read_u16(&descriptor, 8), SHARED_AUDIO_DESCRIPTOR_BYTES);
    assert_eq!(read_u16(&descriptor, 10), SHARED_AUDIO_RING_COUNT);
    assert_eq!(read_u32(&descriptor, 12), 44_100);
    assert_eq!(read_u16(&descriptor, 16), 64);
    assert_eq!(read_u32(&descriptor, 20), 3);
    assert_eq!(read_u64(&descriptor, 24), layout.total_bytes);
    assert_eq!(read_u16(&descriptor, 64), SharedAudioRingRole::Input as u16);
    assert_eq!(read_u16(&descriptor, 66), 1);
    assert_eq!(read_u64(&descriptor, 80), layout.input.cursor_offset);
    assert_eq!(
        read_u16(&descriptor, 128),
        SharedAudioRingRole::Output as u16
    );
    assert_eq!(read_u16(&descriptor, 130), 2);
    assert_eq!(read_u64(&descriptor, 152), layout.output.audio_offset);
}

#[test]
fn descriptor_decodes_canonical_layout() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 0, 2)
        .layout()
        .expect("layout");
    let descriptor = layout.descriptor_bytes();

    assert_eq!(
        SharedAudioTransportLayout::from_descriptor_bytes(&descriptor),
        Ok(layout)
    );
}

#[test]
fn descriptor_decode_rejects_bad_header() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
        .layout()
        .expect("layout");
    let mut descriptor = layout.descriptor_bytes();

    assert_eq!(
        SharedAudioTransportLayout::from_descriptor_bytes(&descriptor[..8]),
        Err(SharedAudioLayoutError::DescriptorTooShort {
            min: usize::from(SHARED_AUDIO_DESCRIPTOR_BYTES),
            actual: 8,
        })
    );

    descriptor[0] = 0;
    assert_eq!(
        SharedAudioTransportLayout::from_descriptor_bytes(&descriptor),
        Err(SharedAudioLayoutError::InvalidDescriptorMagic {
            actual: SHARED_AUDIO_LAYOUT_MAGIC & !0xff,
        })
    );

    let mut descriptor = layout.descriptor_bytes();
    descriptor[4..6].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        SharedAudioTransportLayout::from_descriptor_bytes(&descriptor),
        Err(SharedAudioLayoutError::UnsupportedDescriptorVersion { actual: 99 })
    );
}

#[test]
fn descriptor_decode_rejects_noncanonical_ring_layout() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
        .layout()
        .expect("layout");
    let mut descriptor = layout.descriptor_bytes();

    descriptor[80..88].copy_from_slice(&(layout.input.cursor_offset + 64).to_le_bytes());
    assert_eq!(
        SharedAudioTransportLayout::from_descriptor_bytes(&descriptor),
        Err(SharedAudioLayoutError::InvalidDescriptorField {
            field: "input.cursorOffset",
            expected: layout.input.cursor_offset,
            actual: layout.input.cursor_offset + 64,
        })
    );

    let mut descriptor = layout.descriptor_bytes();
    descriptor[128..130].copy_from_slice(&9_u16.to_le_bytes());
    assert_eq!(
        SharedAudioTransportLayout::from_descriptor_bytes(&descriptor),
        Err(SharedAudioLayoutError::InvalidDescriptorField {
            field: "ring.role",
            expected: SharedAudioRingRole::Output as u64,
            actual: 9,
        })
    );
}

#[test]
fn rejects_invalid_config() {
    assert_eq!(
        SharedAudioTransportConfig::new(0, 128, 2, 2, 2).layout(),
        Err(SharedAudioLayoutError::ZeroSampleRate)
    );
    assert_eq!(
        SharedAudioTransportConfig::new(48_000, 0, 2, 2, 2).layout(),
        Err(SharedAudioLayoutError::ZeroBlockFrames)
    );
    assert_eq!(
        SharedAudioTransportConfig::new(48_000, 128, 0, 2, 2).layout(),
        Err(SharedAudioLayoutError::ZeroCapacityBlocks)
    );
    assert_eq!(
        SharedAudioTransportConfig::new(48_000, 128, 2, MAX_SHARED_AUDIO_CHANNELS + 1, 2,).layout(),
        Err(SharedAudioLayoutError::ChannelCountTooLarge {
            channels: MAX_SHARED_AUDIO_CHANNELS + 1,
            max: MAX_SHARED_AUDIO_CHANNELS,
        })
    );
}

#[test]
fn plans_contiguous_read_and_write_spans() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
        .layout()
        .expect("layout");
    let cursor = SharedAudioRingCursor::new(0, 128);

    assert_eq!(layout.input.readable_frames(cursor), Ok(128));
    assert_eq!(layout.input.writable_frames(cursor), Ok(384));

    let read = layout.input.read_plan(cursor, 128).expect("read");
    assert_eq!(read.first_byte_offset, layout.input.audio_offset);
    assert_eq!(read.first_bytes, 128 * 2 * F32_SAMPLE_BYTES);
    assert_eq!(read.second_bytes, 0);

    let write = layout.input.write_plan(cursor, 128).expect("write");
    assert_eq!(
        write.first_byte_offset,
        layout.input.audio_offset + 128 * 2 * F32_SAMPLE_BYTES
    );
    assert_eq!(write.first_frames, 128);
    assert_eq!(write.second_frames, 0);
}

#[test]
fn plans_wrapped_write_spans() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
        .layout()
        .expect("layout");
    let cursor = SharedAudioRingCursor::new(64, 448);

    let plan = layout.input.write_plan(cursor, 128).expect("write");

    assert_eq!(plan.frame_offset, 448);
    assert_eq!(plan.first_frames, 64);
    assert_eq!(plan.second_frames, 64);
    assert_eq!(
        plan.first_byte_offset,
        layout.input.audio_offset + 448 * 2 * F32_SAMPLE_BYTES
    );
    assert_eq!(plan.first_bytes, 64 * 2 * F32_SAMPLE_BYTES);
    assert_eq!(plan.second_byte_offset, layout.input.audio_offset);
    assert_eq!(plan.second_bytes, 64 * 2 * F32_SAMPLE_BYTES);
}

#[test]
fn rejects_invalid_cursor_and_backpressure() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 2, 2)
        .layout()
        .expect("layout");

    assert_eq!(
        layout
            .input
            .readable_frames(SharedAudioRingCursor::new(10, 4)),
        Err(SharedAudioLayoutError::CursorOrderInvalid {
            read_frame: 10,
            write_frame: 4,
        })
    );
    assert_eq!(
        layout
            .input
            .write_plan(SharedAudioRingCursor::new(0, 256), 128),
        Err(SharedAudioLayoutError::InsufficientWritableFrames {
            requested: 128,
            available: 0,
        })
    );
    assert_eq!(
        layout
            .input
            .read_plan(SharedAudioRingCursor::new(0, 64), 128),
        Err(SharedAudioLayoutError::InsufficientReadableFrames {
            requested: 128,
            available: 64,
        })
    );
}

#[test]
fn zero_channel_plan_keeps_frame_span_without_sample_bytes() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 0, 2)
        .layout()
        .expect("layout");
    let cursor = SharedAudioRingCursor::new(0, 0);

    let plan = layout.input.write_plan(cursor, 128).expect("write");

    assert_eq!(plan.frames, 128);
    assert_eq!(plan.channels, 0);
    assert_eq!(plan.first_frames, 128);
    assert_eq!(plan.second_frames, 0);
    assert_eq!(plan.first_bytes, 0);
    assert_eq!(plan.second_bytes, 0);
    assert_eq!(plan.first_byte_offset, layout.input.audio_offset);
}
