use crate::error::{DeviceError, DeviceErrorKind};

pub struct ConsoleQueue<const N: usize> {
    bytes: [u8; N],
    read: usize,
    len: usize,
}

impl<const N: usize> ConsoleQueue<N> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            read: 0,
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, byte: u8) -> Result<(), DeviceError> {
        if self.len == N {
            return Err(DeviceError::new(
                DeviceErrorKind::CapacityExceeded,
                "virtio-console diagnostic queue is full",
            ));
        }
        let write = (self.read + self.len) % N;
        self.bytes[write] = byte;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        let byte = self.bytes[self.read];
        self.read = (self.read + 1) % N;
        self.len -= 1;
        Some(byte)
    }
}

impl<const N: usize> Default for ConsoleQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_queue_is_bounded_and_fifo() {
        let mut queue = ConsoleQueue::<2>::new();

        queue.push(b'a').unwrap();
        queue.push(b'b').unwrap();
        assert_eq!(
            queue.push(b'c').unwrap_err().kind,
            DeviceErrorKind::CapacityExceeded
        );
        assert_eq!(queue.pop(), Some(b'a'));
        assert_eq!(queue.pop(), Some(b'b'));
        assert_eq!(queue.pop(), None);
    }
}
