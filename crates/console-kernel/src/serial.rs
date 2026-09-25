//! COM1, the 16550 every PC-compatible machine and QEMU put at 0x3F8.
//!
//! The firmware's console is a protocol in boot services, and it goes when
//! they do, so the kernel says its lines through the port itself: eight bits,
//! no parity, one stop bit at 115200 baud, the FIFO on and interrupts off.
//! Nothing here allocates, which is what lets a line be said while the
//! allocator is being changed underneath it.

use console_core_never::Never;

const DATA: u16 = 0x3f8;

const INTERRUPT_ENABLE: u16 = 0x3f9;

const FIFO_CONTROL: u16 = 0x3fa;

const LINE_CONTROL: u16 = 0x3fb;

const MODEM_CONTROL: u16 = 0x3fc;

const LINE_STATUS: u16 = 0x3fd;

const DIVISOR_LATCH: u8 = 0x80;

const EIGHT_BITS_NO_PARITY_ONE_STOP: u8 = 0x03;

const DIVISOR_115200: u8 = 1;

const FIFO_ON_AND_CLEARED: u8 = 0xc7;

const READY_TO_SEND: u8 = 0x03;

const TRANSMITTER_EMPTY: u8 = 0x20;

const NOTHING: u8 = 0;

pub struct Serial;

#[must_use]
pub fn open() -> Result<Serial, Never> {
    let Ok(()) = write(INTERRUPT_ENABLE, NOTHING);
    let Ok(()) = write(LINE_CONTROL, DIVISOR_LATCH);
    let Ok(()) = write(DATA, DIVISOR_115200);
    let Ok(()) = write(INTERRUPT_ENABLE, NOTHING);
    let Ok(()) = write(LINE_CONTROL, EIGHT_BITS_NO_PARITY_ONE_STOP);
    let Ok(()) = write(FIFO_CONTROL, FIFO_ON_AND_CLEARED);
    let Ok(()) = write(MODEM_CONTROL, READY_TO_SEND);

    Ok(Serial)
}

impl Serial {
    pub fn print(&self, line: &str) -> Result<(), Never> {
        for byte in line.bytes().chain(*b"\r\n") {
            let Ok(()) = send(byte);
        }

        Ok(())
    }
}

fn send(byte: u8) -> Result<(), Never> {
    loop {
        let Ok(status) = read(LINE_STATUS);

        match status & TRANSMITTER_EMPTY == NOTHING {
            true => {},
            false => break,
        }
    }

    write(DATA, byte)
}

fn write(port: u16, value: u8) -> Result<(), Never> {
    // SAFETY: every port written here is one of COM1's registers, which the kernel owns alone; an `out` touches no memory.
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };

    Ok(())
}

#[must_use]
fn read(port: u16) -> Result<u8, Never> {
    let value: u8;

    // SAFETY: reading COM1's line status has no effect but the answer; an `in` touches no memory.
    unsafe { core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags)) };

    Ok(value)
}
