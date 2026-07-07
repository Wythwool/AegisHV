use aegishv_hypervisor_core::error::{CoreError, CoreErrorKind};
use core::fmt;

pub const COM1_BASE: u16 = 0x3f8;
const UART_DATA: u16 = 0;
const UART_INTERRUPT_ENABLE: u16 = 1;
const UART_FIFO_CONTROL: u16 = 2;
const UART_LINE_CONTROL: u16 = 3;
const UART_MODEM_CONTROL: u16 = 4;
const UART_LINE_STATUS: u16 = 5;
const UART_TRANSMIT_EMPTY: u8 = 0x20;

pub trait PortIo {
    /// # Safety
    ///
    /// The caller must ensure the port exists and that the current privilege
    /// level allows port I/O. Calling this from the host userspace sensor is
    /// invalid; it is only for the boot/runtime environment that owns the CPU.
    unsafe fn read_u8(&mut self, port: u16) -> u8;

    /// # Safety
    ///
    /// The caller must ensure the port exists and that writing it cannot corrupt
    /// unrelated hardware state. The COM1 logger only uses the legacy UART port
    /// range selected when the boot path opts into serial output.
    unsafe fn write_u8(&mut self, port: u16, value: u8);
}

#[cfg(all(
    feature = "serial-io",
    any(target_arch = "x86", target_arch = "x86_64")
))]
pub struct X86PortIo;

#[cfg(all(
    feature = "serial-io",
    any(target_arch = "x86", target_arch = "x86_64")
))]
impl PortIo for X86PortIo {
    unsafe fn read_u8(&mut self, port: u16) -> u8 {
        let value: u8;
        // SAFETY: the trait contract requires the caller to run at a privilege
        // level where the selected port is valid for byte input.
        unsafe {
            core::arch::asm!(
                "in al, dx",
                in("dx") port,
                out("al") value,
                options(nomem, nostack, preserves_flags)
            );
        }
        value
    }

    unsafe fn write_u8(&mut self, port: u16, value: u8) {
        // SAFETY: the trait contract requires the caller to run at a privilege
        // level where the selected port is valid for byte output.
        unsafe {
            core::arch::asm!(
                "out dx, al",
                in("dx") port,
                in("al") value,
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}

pub struct SerialPort<I> {
    io: I,
    base: u16,
    poll_limit: u32,
}

impl<I: PortIo> SerialPort<I> {
    pub fn new(io: I, base: u16) -> Result<Self, CoreError> {
        if base == 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "serial port base must not be zero",
            ));
        }
        Ok(Self {
            io,
            base,
            poll_limit: 1024,
        })
    }

    pub fn set_poll_limit(&mut self, poll_limit: u32) -> Result<(), CoreError> {
        if poll_limit == 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "serial poll limit must be positive",
            ));
        }
        self.poll_limit = poll_limit;
        Ok(())
    }

    pub fn init_8n1(&mut self) {
        // SAFETY: SerialPort owns the selected UART base and writes only the
        // standard 16550 configuration registers relative to that base.
        unsafe {
            self.io.write_u8(self.base + UART_INTERRUPT_ENABLE, 0x00);
            self.io.write_u8(self.base + UART_LINE_CONTROL, 0x80);
            self.io.write_u8(self.base + UART_DATA, 0x03);
            self.io.write_u8(self.base + UART_INTERRUPT_ENABLE, 0x00);
            self.io.write_u8(self.base + UART_LINE_CONTROL, 0x03);
            self.io.write_u8(self.base + UART_FIFO_CONTROL, 0xc7);
            self.io.write_u8(self.base + UART_MODEM_CONTROL, 0x0b);
        }
    }

    pub fn write_byte(&mut self, byte: u8) -> Result<(), CoreError> {
        self.wait_for_transmitter()?;
        // SAFETY: wait_for_transmitter observed the UART ready bit for this
        // port before the byte write.
        unsafe {
            self.io.write_u8(self.base + UART_DATA, byte);
        }
        Ok(())
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        for &byte in bytes {
            self.write_byte(byte)?;
        }
        Ok(())
    }

    fn wait_for_transmitter(&mut self) -> Result<(), CoreError> {
        let mut polls = 0;
        while polls < self.poll_limit {
            // SAFETY: SerialPort owns the selected UART base and reads only the
            // line-status register relative to that base.
            let status = unsafe { self.io.read_u8(self.base + UART_LINE_STATUS) };
            if status & UART_TRANSMIT_EMPTY != 0 {
                return Ok(());
            }
            polls += 1;
        }
        Err(CoreError::new(
            CoreErrorKind::SerialTimeout,
            "serial transmitter did not become ready before the poll limit",
        ))
    }
}

impl<I: PortIo> fmt::Write for SerialPort<I> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes()).map_err(|_| fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;

    #[derive(Clone, Copy)]
    struct WriteRecord {
        port: u16,
        value: u8,
    }

    struct MockIo {
        ready_after: u32,
        reads: u32,
        writes: [WriteRecord; 32],
        write_len: usize,
    }

    impl MockIo {
        fn new(ready_after: u32) -> Self {
            Self {
                ready_after,
                reads: 0,
                writes: [WriteRecord { port: 0, value: 0 }; 32],
                write_len: 0,
            }
        }
    }

    impl PortIo for MockIo {
        unsafe fn read_u8(&mut self, _port: u16) -> u8 {
            self.reads += 1;
            if self.reads > self.ready_after {
                UART_TRANSMIT_EMPTY
            } else {
                0
            }
        }

        unsafe fn write_u8(&mut self, port: u16, value: u8) {
            self.writes[self.write_len] = WriteRecord { port, value };
            self.write_len += 1;
        }
    }

    #[test]
    fn serial_logger_writes_after_transmit_ready() {
        let mut serial = SerialPort::new(MockIo::new(2), COM1_BASE).unwrap();

        serial.write_byte(b'A').unwrap();

        assert_eq!(serial.io.write_len, 1);
        assert_eq!(serial.io.writes[0].port, COM1_BASE);
        assert_eq!(serial.io.writes[0].value, b'A');
        assert_eq!(serial.io.reads, 3);
    }

    #[test]
    fn serial_logger_reports_timeout_instead_of_spinning_forever() {
        let mut serial = SerialPort::new(MockIo::new(99), COM1_BASE).unwrap();
        serial.set_poll_limit(3).unwrap();

        assert_eq!(
            serial.write_byte(b'X').unwrap_err().kind,
            CoreErrorKind::SerialTimeout
        );
    }

    #[test]
    fn serial_logger_implements_fmt_write_without_allocating() {
        let mut serial = SerialPort::new(MockIo::new(0), COM1_BASE).unwrap();

        write!(&mut serial, "ok").unwrap();

        assert_eq!(serial.io.write_len, 2);
        assert_eq!(serial.io.writes[0].value, b'o');
        assert_eq!(serial.io.writes[1].value, b'k');
    }
}
