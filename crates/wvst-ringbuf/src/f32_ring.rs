use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RingBufferError {
    ZeroCapacity,
}

impl Display for RingBufferError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroCapacity => {
                formatter.write_str("ring buffer capacity must be greater than 0")
            }
        }
    }
}

impl Error for RingBufferError {}

#[derive(Debug, Clone)]
pub struct F32RingBuffer {
    storage: Vec<f32>,
    read: usize,
    write: usize,
    len: usize,
}

impl F32RingBuffer {
    pub fn new(capacity: usize) -> Result<Self, RingBufferError> {
        if capacity == 0 {
            return Err(RingBufferError::ZeroCapacity);
        }

        Ok(Self {
            storage: vec![0.0; capacity],
            read: 0,
            write: 0,
            len: 0,
        })
    }

    pub fn capacity(&self) -> usize {
        self.storage.len()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn available_write(&self) -> usize {
        self.capacity() - self.len
    }

    pub fn clear(&mut self) {
        self.read = 0;
        self.write = 0;
        self.len = 0;
    }

    pub fn push_slice(&mut self, input: &[f32]) -> usize {
        let count = input.len().min(self.available_write());

        for sample in &input[..count] {
            self.storage[self.write] = *sample;
            self.write = self.next_index(self.write);
        }

        self.len += count;
        count
    }

    pub fn pop_slice(&mut self, output: &mut [f32]) -> usize {
        let count = output.len().min(self.len);

        for sample in &mut output[..count] {
            *sample = self.storage[self.read];
            self.read = self.next_index(self.read);
        }

        self.len -= count;
        count
    }

    fn next_index(&self, index: usize) -> usize {
        let next = index + 1;
        if next == self.capacity() { 0 } else { next }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_capacity() {
        assert_eq!(
            F32RingBuffer::new(0).err(),
            Some(RingBufferError::ZeroCapacity)
        );
    }

    #[test]
    fn pushes_and_pops_in_order() {
        let mut buffer = F32RingBuffer::new(4).expect("valid capacity");
        let mut output = [0.0; 3];

        assert_eq!(buffer.push_slice(&[1.0, 2.0, 3.0]), 3);
        assert_eq!(buffer.pop_slice(&mut output), 3);
        assert_eq!(output, [1.0, 2.0, 3.0]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn wraps_without_reordering() {
        let mut buffer = F32RingBuffer::new(4).expect("valid capacity");
        let mut first = [0.0; 2];
        let mut second = [0.0; 4];

        assert_eq!(buffer.push_slice(&[1.0, 2.0, 3.0]), 3);
        assert_eq!(buffer.pop_slice(&mut first), 2);
        assert_eq!(buffer.push_slice(&[4.0, 5.0, 6.0]), 3);
        assert_eq!(buffer.pop_slice(&mut second), 4);
        assert_eq!(first, [1.0, 2.0]);
        assert_eq!(second, [3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn caps_write_to_available_space() {
        let mut buffer = F32RingBuffer::new(2).expect("valid capacity");

        assert_eq!(buffer.push_slice(&[1.0, 2.0, 3.0]), 2);
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.available_write(), 0);
    }
}
