use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use memmap2::{MmapMut, MmapOptions};
use wvst_shm_transport::{
    SharedAudioLayoutError, SharedAudioRingIoReport, SharedAudioRingLayout,
    SharedAudioTransportLayout,
};

mod cursor;

pub use cursor::SharedAudioAtomicCursor;

#[derive(Debug)]
pub struct SharedAudioMmap {
    path: PathBuf,
    file: File,
    layout: SharedAudioTransportLayout,
    memory: MmapMut,
}

impl SharedAudioMmap {
    pub fn create(
        path: impl AsRef<Path>,
        layout: &SharedAudioTransportLayout,
    ) -> Result<Self, SharedAudioMmapError> {
        let path = path.as_ref().to_path_buf();
        let file = open_rw_create(&path)?;
        file.set_len(layout.total_bytes)
            .map_err(|source| SharedAudioMmapError::Io {
                path: path.clone(),
                source,
            })?;

        let mut memory = map_file_mut(&path, &file, layout.total_bytes)?;
        layout.initialize_memory(&mut memory)?;
        memory.flush().map_err(|source| SharedAudioMmapError::Io {
            path: path.clone(),
            source,
        })?;

        Ok(Self {
            path,
            file,
            layout: layout.clone(),
            memory,
        })
    }

    pub fn create_exclusive(
        path: impl AsRef<Path>,
        layout: &SharedAudioTransportLayout,
    ) -> Result<Self, SharedAudioMmapError> {
        let path = path.as_ref().to_path_buf();
        let file = open_rw_create_exclusive(&path)?;
        file.set_len(layout.total_bytes)
            .map_err(|source| SharedAudioMmapError::Io {
                path: path.clone(),
                source,
            })?;

        let mut memory = map_file_mut(&path, &file, layout.total_bytes)?;
        layout.initialize_memory(&mut memory)?;
        memory.flush().map_err(|source| SharedAudioMmapError::Io {
            path: path.clone(),
            source,
        })?;

        Ok(Self {
            path,
            file,
            layout: layout.clone(),
            memory,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, SharedAudioMmapError> {
        let path = path.as_ref().to_path_buf();
        let file = open_rw_existing(&path)?;
        let file_len = file_len(&path, &file)?;
        let memory = map_file_mut(&path, &file, file_len)?;
        let layout = SharedAudioTransportLayout::from_memory_bytes(&memory)?;

        Ok(Self {
            path,
            file,
            layout,
            memory,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn layout(&self) -> &SharedAudioTransportLayout {
        &self.layout
    }

    pub fn memory(&self) -> &[u8] {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut [u8] {
        &mut self.memory
    }

    pub fn flush(&mut self) -> Result<(), SharedAudioMmapError> {
        self.memory
            .flush()
            .map_err(|source| SharedAudioMmapError::Io {
                path: self.path.clone(),
                source,
            })
    }

    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn input_atomic_cursor(&self) -> Result<SharedAudioAtomicCursor<'_>, SharedAudioMmapError> {
        SharedAudioAtomicCursor::from_memory(&self.memory, self.layout.input).map_err(Into::into)
    }

    pub fn output_atomic_cursor(
        &self,
    ) -> Result<SharedAudioAtomicCursor<'_>, SharedAudioMmapError> {
        SharedAudioAtomicCursor::from_memory(&self.memory, self.layout.output).map_err(Into::into)
    }

    pub fn write_interleaved_f32_atomic(
        &mut self,
        ring: SharedAudioRingLayout,
        samples: &[f32],
        frames: u64,
    ) -> Result<SharedAudioRingIoReport, SharedAudioMmapError> {
        let memory_ptr = self.memory.as_mut_ptr();
        let memory_len = self.memory.len();
        let cursor_memory = unsafe {
            // SAFETY: The mmap owns `memory_ptr..memory_ptr + memory_len` for
            // the duration of this method and the cursor view is read/atomic
            // only. The mutable audio view below is disjoint from its fields.
            std::slice::from_raw_parts(memory_ptr.cast_const(), memory_len)
        };
        let cursor = SharedAudioAtomicCursor::from_memory(cursor_memory, ring)?;
        let state = cursor.load(Ordering::Acquire)?;
        let available = ring.writable_frames(state.cursor())?;
        if frames > available {
            cursor.add_overrun_frames(frames - available, Ordering::Relaxed)?;
            cursor.bump_generation(Ordering::Release)?;
            return Err(SharedAudioMmapError::Layout(
                SharedAudioLayoutError::InsufficientWritableFrames {
                    requested: frames,
                    available,
                },
            ));
        }

        let sample_count = unsafe {
            // SAFETY: The atomic cursor borrows only the ring cursor bytes. The
            // audio span is disjoint from that range, so this mutable slice is
            // used only for the non-atomic sample writes before publishing the
            // new write cursor with Release ordering.
            let memory = std::slice::from_raw_parts_mut(memory_ptr, memory_len);
            ring.write_interleaved_f32_at(memory, state.cursor(), samples, frames)?
        };
        let next_write =
            state
                .write_frame
                .checked_add(frames)
                .ok_or(SharedAudioMmapError::Layout(
                    SharedAudioLayoutError::LayoutOverflow,
                ))?;
        cursor.store_write_frame(next_write, Ordering::Release);
        cursor.bump_generation(Ordering::Release)?;
        let final_state = cursor.load(Ordering::Acquire)?;
        Ok(SharedAudioRingIoReport {
            frames,
            samples: sample_count,
            cursor: final_state,
        })
    }

    pub fn read_interleaved_f32_atomic(
        &mut self,
        ring: SharedAudioRingLayout,
        samples: &mut [f32],
        frames: u64,
    ) -> Result<SharedAudioRingIoReport, SharedAudioMmapError> {
        let cursor = SharedAudioAtomicCursor::from_memory(&self.memory, ring)?;
        let state = cursor.load(Ordering::Acquire)?;
        let available = ring.readable_frames(state.cursor())?;
        if frames > available {
            cursor.add_underrun_frames(frames - available, Ordering::Relaxed)?;
            cursor.bump_generation(Ordering::Release)?;
            return Err(SharedAudioMmapError::Layout(
                SharedAudioLayoutError::InsufficientReadableFrames {
                    requested: frames,
                    available,
                },
            ));
        }

        let sample_count = unsafe {
            // SAFETY: The atomic cursor borrows only the ring cursor bytes. The
            // audio span is disjoint from that range, so this immutable slice is
            // read only after the producer's Release cursor publication.
            let memory = std::slice::from_raw_parts(self.memory.as_ptr(), self.memory.len());
            ring.read_interleaved_f32_at(memory, state.cursor(), samples, frames)?
        };
        let next_read =
            state
                .read_frame
                .checked_add(frames)
                .ok_or(SharedAudioMmapError::Layout(
                    SharedAudioLayoutError::LayoutOverflow,
                ))?;
        cursor.store_read_frame(next_read, Ordering::Release);
        cursor.bump_generation(Ordering::Release)?;
        let final_state = cursor.load(Ordering::Acquire)?;
        Ok(SharedAudioRingIoReport {
            frames,
            samples: sample_count,
            cursor: final_state,
        })
    }
}

#[derive(Debug)]
pub enum SharedAudioMmapError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Layout(SharedAudioLayoutError),
    MappingTooLarge {
        path: PathBuf,
        bytes: u64,
    },
}

impl Display for SharedAudioMmapError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Layout(source) => write!(formatter, "{source}"),
            Self::MappingTooLarge { path, bytes } => write!(
                formatter,
                "{}: shared audio mmap size {bytes} does not fit in usize",
                path.display()
            ),
        }
    }
}

impl Error for SharedAudioMmapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Layout(source) => Some(source),
            Self::MappingTooLarge { .. } => None,
        }
    }
}

impl From<SharedAudioLayoutError> for SharedAudioMmapError {
    fn from(source: SharedAudioLayoutError) -> Self {
        Self::Layout(source)
    }
}

fn open_rw_create(path: &Path) -> Result<File, SharedAudioMmapError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(true);
    set_private_file_mode(&mut options);
    options
        .open(path)
        .map_err(|source| SharedAudioMmapError::Io {
            path: path.to_path_buf(),
            source,
        })
}

fn open_rw_create_exclusive(path: &Path) -> Result<File, SharedAudioMmapError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    set_private_file_mode(&mut options);
    options
        .open(path)
        .map_err(|source| SharedAudioMmapError::Io {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(unix)]
fn set_private_file_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_private_file_mode(_options: &mut OpenOptions) {}

fn open_rw_existing(path: &Path) -> Result<File, SharedAudioMmapError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|source| SharedAudioMmapError::Io {
            path: path.to_path_buf(),
            source,
        })
}

fn file_len(path: &Path, file: &File) -> Result<u64, SharedAudioMmapError> {
    file.metadata()
        .map(|metadata| metadata.len())
        .map_err(|source| SharedAudioMmapError::Io {
            path: path.to_path_buf(),
            source,
        })
}

fn map_file_mut(path: &Path, file: &File, bytes: u64) -> Result<MmapMut, SharedAudioMmapError> {
    let len = usize::try_from(bytes).map_err(|_| SharedAudioMmapError::MappingTooLarge {
        path: path.to_path_buf(),
        bytes,
    })?;

    unsafe {
        // SAFETY: WVST maps only regular read/write files opened by this module.
        // `create()` sets the file length before mapping; `open()` maps the
        // current file length and validates the WVST descriptor before exposing
        // audio accessors. The returned `MmapMut` owns the mapping and is only
        // exposed through Rust slice borrows tied to `SharedAudioMmap`.
        MmapOptions::new()
            .len(len)
            .map_mut(file)
            .map_err(|source| SharedAudioMmapError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wvst_shm_transport::{
        SharedAudioLayoutError, SharedAudioRingCursorState, SharedAudioTransportConfig,
    };

    #[test]
    fn create_initializes_descriptor_and_cursors() {
        let path = unique_path("create");
        let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 2, 2)
            .layout()
            .expect("layout");

        let mmap = SharedAudioMmap::create(&path, &layout).expect("mmap");

        assert_eq!(mmap.path(), path.as_path());
        assert_eq!(mmap.layout(), &layout);
        assert_eq!(
            file_len(&path, mmap.file()).expect("len"),
            layout.total_bytes
        );
        assert_eq!(
            SharedAudioTransportLayout::from_memory_bytes(mmap.memory()),
            Ok(layout.clone())
        );
        assert_eq!(
            layout.input.cursor_state(mmap.memory()),
            Ok(SharedAudioRingCursorState::default())
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn second_mapping_observes_ring_io() {
        let path = unique_path("roundtrip");
        let layout = SharedAudioTransportConfig::new(48_000, 4, 2, 2, 2)
            .layout()
            .expect("layout");
        let mut writer = SharedAudioMmap::create(&path, &layout).expect("writer");
        let input_ring = writer.layout().input;
        let input = [0.0, 0.1, 1.0, 1.1, 2.0, 2.1, 3.0, 3.1];

        writer
            .write_interleaved_f32_atomic(input_ring, &input, 4)
            .expect("write");
        writer.flush().expect("flush");

        let mut reader = SharedAudioMmap::open(&path).expect("reader");
        let input_ring = reader.layout().input;
        let mut output = [0.0; 8];
        reader
            .read_interleaved_f32_atomic(input_ring, &mut output, 4)
            .expect("read");

        assert_eq!(output, input);
        assert_eq!(
            input_ring
                .cursor_state(reader.memory())
                .expect("reader cursor")
                .read_frame,
            4
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn open_rejects_uninitialized_file() {
        let path = unique_path("bad");
        std::fs::write(&path, [0_u8; 256]).expect("write");

        let error = SharedAudioMmap::open(&path).expect_err("descriptor");

        assert!(matches!(
            error,
            SharedAudioMmapError::Layout(SharedAudioLayoutError::InvalidDescriptorMagic {
                actual: 0
            })
        ));
        let _ = std::fs::remove_file(path);
    }

    fn unique_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "wvst-shm-mmap-{label}-{}-{nanos}",
            std::process::id()
        ))
    }
}
