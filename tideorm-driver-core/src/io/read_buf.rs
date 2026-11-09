use bytes::{BufMut, BytesMut};

/// An extension for [`BufMut`] for getting a writeable buffer in safe code.
pub trait ReadBuf: BufMut {
    /// Get the full capacity of this buffer as a safely initialized slice.
    fn init_mut(&mut self) -> &mut [u8];
}

impl ReadBuf for &'_ mut [u8] {
    #[inline(always)]
    fn init_mut(&mut self) -> &mut [u8] {
        self
    }
}

impl ReadBuf for BytesMut {
    #[inline(always)]
    fn init_mut(&mut self) -> &mut [u8] {
        // `self.remaining_mut()` returns `usize::MAX - self.len()`
        let remaining = self.capacity() - self.len();

        self.put_bytes(0, remaining);

        self
    }
}

#[cfg(test)]
#[path = "../../tests/unit/io_read_buf_tests.rs"]
mod tests;
