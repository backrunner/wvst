use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::SharedAudioMmap;
use wvst_shm_transport::{
    SharedAudioLayoutError, SharedAudioRingCursorState, SharedAudioTransportConfig,
};

#[test]
fn atomic_cursor_store_and_load_round_trips() {
    let path = unique_path("roundtrip");
    let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 2, 2)
        .layout()
        .expect("layout");
    let mmap = SharedAudioMmap::create(&path, &layout).expect("mmap");
    let cursor = mmap.input_atomic_cursor().expect("cursor");
    let state = SharedAudioRingCursorState {
        read_frame: 128,
        write_frame: 384,
        dropped_frames: 3,
        underrun_frames: 5,
        overrun_frames: 7,
        generation: 11,
        flags: 0x03,
    };

    cursor.store(state, Ordering::SeqCst).expect("store");

    assert_eq!(cursor.load(Ordering::SeqCst), Ok(state));
    assert_eq!(layout.input.cursor_state(mmap.memory()), Ok(state));
    let _ = std::fs::remove_file(path);
}

#[test]
fn second_mapping_observes_atomic_cursor_updates() {
    let path = unique_path("visible");
    let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 2, 2)
        .layout()
        .expect("layout");
    let writer = SharedAudioMmap::create(&path, &layout).expect("writer");
    let writer_cursor = writer.input_atomic_cursor().expect("writer cursor");
    writer_cursor
        .store(SharedAudioRingCursorState::new(0, 256), Ordering::SeqCst)
        .expect("store");

    let reader = SharedAudioMmap::open(&path).expect("reader");
    let reader_cursor = reader.input_atomic_cursor().expect("reader cursor");

    assert_eq!(
        reader_cursor
            .load(Ordering::SeqCst)
            .expect("reader state")
            .write_frame,
        256
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn atomic_counter_updates_are_checked() {
    let path = unique_path("counters");
    let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 2, 2)
        .layout()
        .expect("layout");
    let mmap = SharedAudioMmap::create(&path, &layout).expect("mmap");
    let cursor = mmap.output_atomic_cursor().expect("cursor");

    assert_eq!(cursor.add_underrun_frames(12, Ordering::SeqCst), Ok(0));
    assert_eq!(cursor.add_overrun_frames(4, Ordering::SeqCst), Ok(0));
    assert_eq!(cursor.bump_generation(Ordering::SeqCst), Ok(0));

    let state = cursor.load(Ordering::SeqCst).expect("state");
    assert_eq!(state.underrun_frames, 12);
    assert_eq!(state.overrun_frames, 4);
    assert_eq!(state.generation, 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn atomic_cursor_rejects_invalid_order() {
    let path = unique_path("invalid");
    let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 2, 2)
        .layout()
        .expect("layout");
    let mmap = SharedAudioMmap::create(&path, &layout).expect("mmap");
    let cursor = mmap.input_atomic_cursor().expect("cursor");

    assert_eq!(
        cursor.store(SharedAudioRingCursorState::new(256, 128), Ordering::SeqCst),
        Err(SharedAudioLayoutError::CursorOrderInvalid {
            read_frame: 256,
            write_frame: 128,
        })
    );
    let _ = std::fs::remove_file(path);
}

fn unique_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "wvst-shm-mmap-cursor-{label}-{}-{nanos}",
        std::process::id()
    ))
}
