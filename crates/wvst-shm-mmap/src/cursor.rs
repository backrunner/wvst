use std::marker::PhantomData;
use std::mem::{align_of, size_of};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use wvst_shm_transport::{
    SHARED_AUDIO_CURSOR_DROPPED_FRAMES_OFFSET, SHARED_AUDIO_CURSOR_FLAGS_OFFSET,
    SHARED_AUDIO_CURSOR_GENERATION_OFFSET, SHARED_AUDIO_CURSOR_OVERRUN_FRAMES_OFFSET,
    SHARED_AUDIO_CURSOR_READ_FRAME_OFFSET, SHARED_AUDIO_CURSOR_UNDERRUN_FRAMES_OFFSET,
    SHARED_AUDIO_CURSOR_WRITE_FRAME_OFFSET, SharedAudioLayoutError, SharedAudioRingCursorState,
    SharedAudioRingLayout,
};

#[derive(Debug)]
pub struct SharedAudioAtomicCursor<'a> {
    read_frame: &'a AtomicU64,
    write_frame: &'a AtomicU64,
    dropped_frames: &'a AtomicU64,
    underrun_frames: &'a AtomicU64,
    overrun_frames: &'a AtomicU64,
    generation: &'a AtomicU64,
    flags: &'a AtomicU32,
    _memory: PhantomData<&'a [u8]>,
}

impl<'a> SharedAudioAtomicCursor<'a> {
    pub fn from_memory(
        memory: &'a [u8],
        ring: SharedAudioRingLayout,
    ) -> Result<Self, SharedAudioLayoutError> {
        let range = ring.cursor_range().validate_within(memory.len() as u64)?;
        Ok(Self {
            read_frame: atomic_u64(memory, range.offset, SHARED_AUDIO_CURSOR_READ_FRAME_OFFSET)?,
            write_frame: atomic_u64(memory, range.offset, SHARED_AUDIO_CURSOR_WRITE_FRAME_OFFSET)?,
            dropped_frames: atomic_u64(
                memory,
                range.offset,
                SHARED_AUDIO_CURSOR_DROPPED_FRAMES_OFFSET,
            )?,
            underrun_frames: atomic_u64(
                memory,
                range.offset,
                SHARED_AUDIO_CURSOR_UNDERRUN_FRAMES_OFFSET,
            )?,
            overrun_frames: atomic_u64(
                memory,
                range.offset,
                SHARED_AUDIO_CURSOR_OVERRUN_FRAMES_OFFSET,
            )?,
            generation: atomic_u64(memory, range.offset, SHARED_AUDIO_CURSOR_GENERATION_OFFSET)?,
            flags: atomic_u32(memory, range.offset, SHARED_AUDIO_CURSOR_FLAGS_OFFSET)?,
            _memory: PhantomData,
        })
    }

    pub fn load(
        &self,
        ordering: Ordering,
    ) -> Result<SharedAudioRingCursorState, SharedAudioLayoutError> {
        SharedAudioRingCursorState {
            read_frame: self.read_frame.load(ordering),
            write_frame: self.write_frame.load(ordering),
            dropped_frames: self.dropped_frames.load(ordering),
            underrun_frames: self.underrun_frames.load(ordering),
            overrun_frames: self.overrun_frames.load(ordering),
            generation: self.generation.load(ordering),
            flags: self.flags.load(ordering),
        }
        .validate_order()
    }

    pub fn store(
        &self,
        state: SharedAudioRingCursorState,
        ordering: Ordering,
    ) -> Result<(), SharedAudioLayoutError> {
        state.validate_order()?;
        self.read_frame.store(state.read_frame, ordering);
        self.write_frame.store(state.write_frame, ordering);
        self.dropped_frames.store(state.dropped_frames, ordering);
        self.underrun_frames.store(state.underrun_frames, ordering);
        self.overrun_frames.store(state.overrun_frames, ordering);
        self.generation.store(state.generation, ordering);
        self.flags.store(state.flags, ordering);
        Ok(())
    }

    pub fn add_dropped_frames(
        &self,
        frames: u64,
        ordering: Ordering,
    ) -> Result<u64, SharedAudioLayoutError> {
        fetch_add_checked(self.dropped_frames, frames, ordering)
    }

    pub fn add_underrun_frames(
        &self,
        frames: u64,
        ordering: Ordering,
    ) -> Result<u64, SharedAudioLayoutError> {
        fetch_add_checked(self.underrun_frames, frames, ordering)
    }

    pub fn add_overrun_frames(
        &self,
        frames: u64,
        ordering: Ordering,
    ) -> Result<u64, SharedAudioLayoutError> {
        fetch_add_checked(self.overrun_frames, frames, ordering)
    }

    pub fn bump_generation(&self, ordering: Ordering) -> Result<u64, SharedAudioLayoutError> {
        fetch_add_checked(self.generation, 1, ordering)
    }
}

fn fetch_add_checked(
    value: &AtomicU64,
    delta: u64,
    ordering: Ordering,
) -> Result<u64, SharedAudioLayoutError> {
    value
        .fetch_update(ordering, ordering, |current| current.checked_add(delta))
        .map_err(|_| SharedAudioLayoutError::LayoutOverflow)
}

fn atomic_u64(
    memory: &[u8],
    cursor_offset: u64,
    field_offset: usize,
) -> Result<&AtomicU64, SharedAudioLayoutError> {
    let offset = checked_offset(cursor_offset, field_offset)?;
    let pointer = checked_atomic_pointer(
        memory,
        offset,
        size_of::<AtomicU64>(),
        align_of::<AtomicU64>(),
    )?;
    Ok(unsafe {
        // SAFETY: `checked_atomic_pointer` validates that the computed field is
        // inside the cursor block memory and aligned for `AtomicU64`. The
        // returned reference is tied to the input mapping lifetime and atomic
        // interior mutability is the only mutation exposed by this view.
        &*(pointer.cast::<AtomicU64>())
    })
}

fn atomic_u32(
    memory: &[u8],
    cursor_offset: u64,
    field_offset: usize,
) -> Result<&AtomicU32, SharedAudioLayoutError> {
    let offset = checked_offset(cursor_offset, field_offset)?;
    let pointer = checked_atomic_pointer(
        memory,
        offset,
        size_of::<AtomicU32>(),
        align_of::<AtomicU32>(),
    )?;
    Ok(unsafe {
        // SAFETY: `checked_atomic_pointer` validates that the computed field is
        // inside the cursor block memory and aligned for `AtomicU32`. The
        // returned reference is tied to the input mapping lifetime and atomic
        // interior mutability is the only mutation exposed by this view.
        &*(pointer.cast::<AtomicU32>())
    })
}

fn checked_atomic_pointer(
    memory: &[u8],
    offset: usize,
    size: usize,
    alignment: usize,
) -> Result<*const u8, SharedAudioLayoutError> {
    let end = offset
        .checked_add(size)
        .ok_or(SharedAudioLayoutError::LayoutOverflow)?;
    if end > memory.len() {
        return Err(SharedAudioLayoutError::MemoryTooShort {
            min: end as u64,
            actual: memory.len() as u64,
        });
    }
    let pointer = unsafe {
        // SAFETY: `end <= memory.len()` was checked above, so `offset` is within
        // the same slice allocation.
        memory.as_ptr().add(offset)
    };
    let actual = pointer as usize % alignment;
    if actual != 0 {
        return Err(SharedAudioLayoutError::InvalidDescriptorField {
            field: "cursor.atomicAlignment",
            expected: 0,
            actual: actual as u64,
        });
    }
    Ok(pointer)
}

fn checked_offset(
    cursor_offset: u64,
    field_offset: usize,
) -> Result<usize, SharedAudioLayoutError> {
    let cursor_offset =
        usize::try_from(cursor_offset).map_err(|_| SharedAudioLayoutError::LayoutOverflow)?;
    cursor_offset
        .checked_add(field_offset)
        .ok_or(SharedAudioLayoutError::LayoutOverflow)
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
