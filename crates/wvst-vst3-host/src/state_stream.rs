use std::ffi::c_void;
use std::ptr;

use crate::vst3_abi::{
    IBStream, IBStreamVTable, K_RESULT_FALSE, K_RESULT_OK, TUid, VST3_FUNKNOWN_IID,
    VST3_IBSTREAM_IID, VST3_STREAM_SEEK_CUR, VST3_STREAM_SEEK_END, VST3_STREAM_SEEK_SET,
    parse_tuid_hex,
};
use crate::{HostError, HostResult};

pub const DEFAULT_MAX_VST3_STATE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug)]
pub struct Vst3StateStream {
    object: Box<StateStreamObject>,
}

impl Vst3StateStream {
    pub fn writable() -> Self {
        Self::from_bytes(Vec::new())
    }

    pub fn bounded_writable(max_bytes: usize) -> Self {
        Self::with_bytes_and_limit(Vec::new(), Some(max_bytes))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::with_bytes_and_limit(bytes, None)
    }

    fn with_bytes_and_limit(bytes: Vec<u8>, max_len: Option<usize>) -> Self {
        Self {
            object: Box::new(StateStreamObject {
                iface: IBStream {
                    vtable: &STATE_STREAM_VTABLE,
                },
                data: bytes,
                max_len,
                position: 0,
                write_limit_exceeded: None,
            }),
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut IBStream {
        &mut self.object.iface
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.object.data
    }

    pub fn into_bytes_checked(self) -> HostResult<Vec<u8>> {
        self.check_write_limit()?;

        Ok(self.object.data)
    }

    pub fn check_write_limit(&self) -> HostResult<()> {
        if let (Some(max), Some(actual)) = (self.object.max_len, self.object.write_limit_exceeded) {
            return Err(HostError::StateStreamWriteLimitExceeded { max, actual });
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Debug)]
struct StateStreamObject {
    iface: IBStream,
    data: Vec<u8>,
    max_len: Option<usize>,
    position: usize,
    write_limit_exceeded: Option<usize>,
}

const STATE_STREAM_VTABLE: IBStreamVTable = IBStreamVTable {
    query_interface: stream_query_interface,
    add_ref: stream_add_ref,
    release: stream_release,
    read: stream_read,
    write: stream_write,
    seek: stream_seek,
    tell: stream_tell,
};

unsafe extern "system" fn stream_query_interface(
    this: *mut IBStream,
    iid: *const i8,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return K_RESULT_FALSE;
    }

    let Some(iid) = read_tuid(iid) else {
        unsafe { *obj = ptr::null_mut() };
        return K_RESULT_FALSE;
    };

    if iid == tuid(VST3_FUNKNOWN_IID) || iid == tuid(VST3_IBSTREAM_IID) {
        unsafe { *obj = this.cast::<c_void>() };
        K_RESULT_OK
    } else {
        unsafe { *obj = ptr::null_mut() };
        K_RESULT_FALSE
    }
}

unsafe extern "system" fn stream_add_ref(_this: *mut IBStream) -> u32 {
    1
}

unsafe extern "system" fn stream_release(_this: *mut IBStream) -> u32 {
    1
}

unsafe extern "system" fn stream_read(
    this: *mut IBStream,
    buffer: *mut c_void,
    num_bytes: i32,
    num_bytes_read: *mut i32,
) -> i32 {
    if num_bytes < 0 || (buffer.is_null() && num_bytes > 0) {
        return K_RESULT_FALSE;
    }

    let Some(stream) = stream_object_mut(this) else {
        return K_RESULT_FALSE;
    };
    let requested = num_bytes as usize;
    let available = stream.data.len().saturating_sub(stream.position);
    let count = requested.min(available);

    if count > 0 {
        unsafe {
            ptr::copy_nonoverlapping(
                stream.data[stream.position..].as_ptr(),
                buffer.cast::<u8>(),
                count,
            );
        }
    }
    stream.position += count;
    write_i32(num_bytes_read, count);
    K_RESULT_OK
}

unsafe extern "system" fn stream_write(
    this: *mut IBStream,
    buffer: *mut c_void,
    num_bytes: i32,
    num_bytes_written: *mut i32,
) -> i32 {
    if num_bytes < 0 || (buffer.is_null() && num_bytes > 0) {
        return K_RESULT_FALSE;
    }

    let Some(stream) = stream_object_mut(this) else {
        return K_RESULT_FALSE;
    };
    let count = num_bytes as usize;
    let end = match stream.position.checked_add(count) {
        Some(value) => value,
        None => return K_RESULT_FALSE,
    };
    if let Some(max_len) = stream.max_len
        && end > max_len
    {
        stream.write_limit_exceeded = Some(end);
        write_i32(num_bytes_written, 0);
        return K_RESULT_FALSE;
    }
    if end > stream.data.len() {
        stream.data.resize(end, 0);
    }
    if count > 0 {
        unsafe {
            ptr::copy_nonoverlapping(
                buffer.cast::<u8>(),
                stream.data[stream.position..].as_mut_ptr(),
                count,
            );
        }
    }
    stream.position = end;
    write_i32(num_bytes_written, count);
    K_RESULT_OK
}

unsafe extern "system" fn stream_seek(
    this: *mut IBStream,
    pos: i64,
    mode: i32,
    result: *mut i64,
) -> i32 {
    let Some(stream) = stream_object_mut(this) else {
        return K_RESULT_FALSE;
    };
    let base = match mode {
        VST3_STREAM_SEEK_SET => 0,
        VST3_STREAM_SEEK_CUR => stream.position as i64,
        VST3_STREAM_SEEK_END => stream.data.len() as i64,
        _ => return K_RESULT_FALSE,
    };
    let Some(new_position) = base.checked_add(pos) else {
        return K_RESULT_FALSE;
    };
    if new_position < 0 {
        return K_RESULT_FALSE;
    }

    stream.position = new_position as usize;
    if !result.is_null() {
        unsafe { *result = new_position };
    }
    K_RESULT_OK
}

unsafe extern "system" fn stream_tell(_this: *mut IBStream, pos: *mut i64) -> i32 {
    if pos.is_null() {
        return K_RESULT_FALSE;
    }
    let Some(stream) = stream_object_mut(_this) else {
        return K_RESULT_FALSE;
    };
    unsafe { *pos = stream.position as i64 };
    K_RESULT_OK
}

fn stream_object_mut(this: *mut IBStream) -> Option<&'static mut StateStreamObject> {
    if this.is_null() {
        return None;
    }

    Some(unsafe { &mut *this.cast::<StateStreamObject>() })
}

fn write_i32(output: *mut i32, value: usize) {
    if output.is_null() {
        return;
    }

    let value = i32::try_from(value).unwrap_or(i32::MAX);
    unsafe { *output = value };
}

fn read_tuid(iid: *const i8) -> Option<TUid> {
    if iid.is_null() {
        return None;
    }

    let mut value = [0; 16];
    unsafe {
        value.copy_from_slice(std::slice::from_raw_parts(iid.cast::<u8>(), 16));
    }
    Some(value)
}

fn tuid(value: &str) -> TUid {
    parse_tuid_hex(value).expect("built-in VST3 interface id is valid")
}

pub fn ensure_seek_position(position: i64) -> HostResult<usize> {
    usize::try_from(position).map_err(|_| HostError::InvalidStateStreamSeek { position })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_memory_stream() {
        let mut stream = Vst3StateStream::writable();
        let raw = stream.as_mut_ptr();
        let input = [1_u8, 2, 3];
        let mut written = 0;

        let result = unsafe {
            ((*(*raw).vtable).write)(raw, input.as_ptr().cast_mut().cast(), 3, &mut written)
        };

        assert_eq!(result, K_RESULT_OK);
        assert_eq!(written, 3);
        assert_eq!(stream.into_bytes(), input);
    }

    #[test]
    fn seeks_before_reading() {
        let mut stream = Vst3StateStream::from_bytes(vec![1, 2, 3]);
        let raw = stream.as_mut_ptr();
        let mut position = 0;
        let mut output = [0_u8; 1];

        unsafe {
            ((*(*raw).vtable).seek)(raw, 1, VST3_STREAM_SEEK_SET, &mut position);
            ((*(*raw).vtable).read)(raw, output.as_mut_ptr().cast(), 1, ptr::null_mut());
        }

        assert_eq!(position, 1);
        assert_eq!(output[0], 2);
    }

    #[test]
    fn bounded_writable_reports_write_limit() {
        let mut stream = Vst3StateStream::bounded_writable(2);
        let raw = stream.as_mut_ptr();
        let input = [1_u8, 2, 3];
        let mut written = 0;

        let result = unsafe {
            ((*(*raw).vtable).write)(raw, input.as_ptr().cast_mut().cast(), 3, &mut written)
        };
        assert_eq!(result, K_RESULT_FALSE);
        assert_eq!(written, 0);
        assert!(stream.check_write_limit().is_err());
        let error = stream
            .into_bytes_checked()
            .expect_err("write limit should be reported");
        assert!(matches!(
            error,
            HostError::StateStreamWriteLimitExceeded { max: 2, actual: 3 }
        ));
    }
}
