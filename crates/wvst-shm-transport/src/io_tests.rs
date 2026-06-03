use crate::{
    SharedAudioLayoutError, SharedAudioRingCursorState, SharedAudioTransportConfig,
    SharedAudioTransportLayout,
};

#[test]
fn initializes_memory_descriptor_and_cursors() {
    let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 2, 2)
        .layout()
        .expect("layout");
    let mut memory = vec![0xff; layout.total_bytes as usize];

    layout.initialize_memory(&mut memory).expect("initialize");

    assert_eq!(
        SharedAudioTransportLayout::from_memory_bytes(&memory),
        Ok(layout.clone())
    );
    assert_eq!(
        layout.input.cursor_state(&memory),
        Ok(SharedAudioRingCursorState::default())
    );
    assert_eq!(
        layout.output.cursor_state(&memory),
        Ok(SharedAudioRingCursorState::default())
    );
}

#[test]
fn writes_and_reads_interleaved_samples() {
    let layout = SharedAudioTransportConfig::new(48_000, 4, 2, 2, 2)
        .layout()
        .expect("layout");
    let mut memory = vec![0; layout.total_bytes as usize];
    layout.initialize_memory(&mut memory).expect("initialize");
    let input = [0.0, 0.1, 1.0, 1.1, 2.0, 2.1, 3.0, 3.1];

    let write = layout
        .input
        .write_interleaved_f32(&mut memory, &input, 4)
        .expect("write");

    assert_eq!(write.frames, 4);
    assert_eq!(write.samples, input.len());
    assert_eq!(write.cursor.write_frame, 4);
    assert_eq!(write.cursor.generation, 1);

    let mut output = [0.0; 8];
    let read = layout
        .input
        .read_interleaved_f32(&mut memory, &mut output, 4)
        .expect("read");

    assert_eq!(output, input);
    assert_eq!(read.cursor.read_frame, 4);
    assert_eq!(read.cursor.write_frame, 4);
    assert_eq!(read.cursor.generation, 2);
}

#[test]
fn wraps_interleaved_samples_across_ring_end() {
    let layout = SharedAudioTransportConfig::new(48_000, 4, 2, 1, 1)
        .layout()
        .expect("layout");
    let mut memory = vec![0; layout.total_bytes as usize];
    layout.initialize_memory(&mut memory).expect("initialize");
    let first = [10.0, 11.0, 12.0, 13.0, 14.0, 15.0];
    let second = [20.0, 21.0, 22.0, 23.0, 24.0, 25.0];
    let mut scratch = [0.0; 6];

    layout
        .input
        .write_interleaved_f32(&mut memory, &first, 6)
        .expect("first write");
    layout
        .input
        .read_interleaved_f32(&mut memory, &mut scratch, 6)
        .expect("first read");
    layout
        .input
        .write_interleaved_f32(&mut memory, &second, 6)
        .expect("wrapped write");

    let mut output = [0.0; 6];
    layout
        .input
        .read_interleaved_f32(&mut memory, &mut output, 6)
        .expect("wrapped read");

    assert_eq!(scratch, first);
    assert_eq!(output, second);
}

#[test]
fn rejects_sample_buffer_length_mismatch() {
    let layout = SharedAudioTransportConfig::new(48_000, 4, 2, 2, 2)
        .layout()
        .expect("layout");
    let mut memory = vec![0; layout.total_bytes as usize];
    layout.initialize_memory(&mut memory).expect("initialize");

    assert_eq!(
        layout
            .input
            .write_interleaved_f32(&mut memory, &[0.0; 7], 4),
        Err(SharedAudioLayoutError::SampleBufferLengthMismatch {
            expected: 8,
            actual: 7,
        })
    );
}

#[test]
fn records_underrun_and_overrun_counters() {
    let layout = SharedAudioTransportConfig::new(48_000, 4, 2, 1, 1)
        .layout()
        .expect("layout");
    let mut memory = vec![0; layout.total_bytes as usize];
    layout.initialize_memory(&mut memory).expect("initialize");
    let mut output = [0.0; 4];

    assert_eq!(
        layout
            .input
            .read_interleaved_f32(&mut memory, &mut output, 4),
        Err(SharedAudioLayoutError::InsufficientReadableFrames {
            requested: 4,
            available: 0,
        })
    );
    let cursor = layout.input.cursor_state(&memory).expect("cursor");
    assert_eq!(cursor.underrun_frames, 4);
    assert_eq!(cursor.generation, 1);

    layout
        .input
        .write_interleaved_f32(&mut memory, &[0.0; 8], 8)
        .expect("fill ring");
    assert_eq!(
        layout.input.write_interleaved_f32(&mut memory, &[1.0], 1),
        Err(SharedAudioLayoutError::InsufficientWritableFrames {
            requested: 1,
            available: 0,
        })
    );
    let cursor = layout.input.cursor_state(&memory).expect("cursor");
    assert_eq!(cursor.overrun_frames, 1);
    assert_eq!(cursor.generation, 3);
}

#[test]
fn advances_zero_channel_instrument_frames() {
    let layout = SharedAudioTransportConfig::new(48_000, 4, 2, 0, 2)
        .layout()
        .expect("layout");
    let mut memory = vec![0; layout.total_bytes as usize];
    layout.initialize_memory(&mut memory).expect("initialize");

    let write = layout
        .input
        .write_interleaved_f32(&mut memory, &[], 4)
        .expect("zero-channel write");
    let read = layout
        .input
        .read_interleaved_f32(&mut memory, &mut [], 4)
        .expect("zero-channel read");

    assert_eq!(write.samples, 0);
    assert_eq!(read.cursor.read_frame, 4);
    assert_eq!(read.cursor.write_frame, 4);
}
