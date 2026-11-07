//! MQ arithmetic decoding shared by JBIG2 and JPEG 2000.
//!
//! The codecs choose contexts and interpret the decoded bits. This crate owns
//! the interval arithmetic, the 47 probability states from Table C.2 and byte stuffing. It
//! uses the Annex G form of the decoder carried by both Hayro codecs; the
//! original notices are in `LICENSE-MIT` and `LICENSE-APACHE`.
//!
//! An exhausted input supplies 0xFF bytes. A 0xFF followed by a byte above
//! 0x8F stops input consumption, while a following byte at most 0x8F supplies
//! seven bits. Neither case is an I/O failure: callers bound the number of
//! symbols they decode. The remaining input is a slice, so reaching its end
//! needs no advancing byte counter.
//!
//! Contexts retain the original one-byte representation. Their state index
//! is private, initialized to zero, reset through a checked API, and changed
//! only by the probability table. Arithmetic wraps at register width, as in
//! the original implementation; renormalization batches at most sixteen shifts.

#![no_std]
#![forbid(unsafe_code)]

use console_core_never::Never;

pub struct Decoder<'a> {
    data: &'a [u8],
    c: u32,
    a: u32,
    shift_count: u32,
}

impl<'a> Decoder<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Never> {
        let mut decoder = Self {
            data,
            c: 0,
            a: 0x8000,
            shift_count: 0,
        };
        let Ok(byte) = decoder.current_byte();

        decoder.c = (u32::from(byte) ^ 0xFF).wrapping_shl(16);

        let Ok(()) = decoder.read_byte();

        decoder.c = decoder.c.wrapping_shl(7);
        decoder.shift_count = decoder.shift_count.wrapping_sub(7);

        Ok(decoder)
    }

    #[inline(always)]
    pub fn read_bit(&mut self, context: &mut Context) -> Result<u32, Never> {
        let Ok(entry) = context.entry();
        let mps = u32::from(context.0.wrapping_shr(7));

        self.a = self.a.wrapping_sub(entry.qe);

        match self.c.wrapping_shr(16) < self.a && self.a & 0x8000 != 0 {
            true => return Ok(mps),
            false => {}
        }

        let is_lps = u32::from(self.c.wrapping_shr(16) >= self.a);
        let lps_mask = is_lps.wrapping_neg();

        self.c = self.c.wrapping_sub(self.a.wrapping_shl(16) & lps_mask);

        let exchange = u8::from(self.a < entry.qe) ^ u8::from(is_lps != 0);

        self.a = (self.a & !lps_mask) | (entry.qe & lps_mask);

        let bit = mps ^ u32::from(exchange);
        let pick_nlps = exchange.wrapping_neg();
        let next = entry.nmps ^ ((entry.nmps ^ entry.nlps) & pick_nlps);
        let switched = (exchange & u8::from(entry.switch)).wrapping_shl(7);

        context.0 = ((context.0 ^ switched) & 0x80) | next;

        let Ok(()) = self.renormalize();

        Ok(bit)
    }

    #[inline(always)]
    fn current_byte(&self) -> Result<u8, Never> {
        Ok(match self.data.first() {
            Some(byte) => *byte,
            None => 0xFF,
        })
    }

    #[inline(always)]
    fn read_byte(&mut self) -> Result<(), Never> {
        let remaining = match self.data.split_first() {
            Some((_current, remaining)) => remaining,
            None => &[],
        };
        let next = match remaining.first() {
            Some(byte) => u32::from(*byte),
            None => 0xFF,
        };
        let Ok(current) = self.current_byte();

        match (current == 0xFF, next > 0x8F) {
            (true, true) => self.shift_count = 8,
            (true, false) => {
                self.data = remaining;
                self.c = self
                    .c
                    .wrapping_add(0xFE00)
                    .wrapping_sub(next.wrapping_shl(9));
                self.shift_count = 7;
            }
            (false, _) => {
                self.data = remaining;
                self.c = self
                    .c
                    .wrapping_add(0xFF00)
                    .wrapping_sub(next.wrapping_shl(8));
                self.shift_count = 8;
            }
        }

        Ok(())
    }

    #[inline(always)]
    fn renormalize(&mut self) -> Result<(), Never> {
        for _shift in 0..16 {
            match self.a & 0x8000 != 0 {
                true => return Ok(()),
                false => {}
            }

            match self.shift_count {
                0 => {
                    let Ok(()) = self.read_byte();
                }
                1.. => {}
            }

            let shifts_needed = self.a.leading_zeros().wrapping_sub(16);
            let batch = shifts_needed.min(self.shift_count);

            self.a = self.a.wrapping_shl(batch);
            self.c = self.c.wrapping_shl(batch);
            self.shift_count = self.shift_count.wrapping_sub(batch);
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Context(u8);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InvalidState(pub u8);

impl core::fmt::Display for InvalidState {
    fn fmt(&self, to: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(to, "MQ probability state {} is outside 0..=46", self.0)
    }
}

impl core::error::Error for InvalidState {}

impl Context {
    pub fn reset(&mut self) -> Result<(), Never> {
        self.0 = 0;

        Ok(())
    }

    pub fn reset_with_index(&mut self, index: u8) -> Result<(), InvalidState> {
        match index {
            0..=46 => self.0 = index,
            47.. => return Err(InvalidState(index)),
        }

        Ok(())
    }

    #[inline(always)]
    #[cfg_attr(
        dylint_lib = "explicit014_no_index_slice",
        allow(
            explicit014_no_index_slice,
            reason = "the packed context is private; checked resets and the 47-state table keep its index within 0..=46"
        )
    )]
    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(
            explicit051_no_machine_width,
            reason = "the table lookup requires usize; the state remains a checked u8 and the shared decoder must stay no_std"
        )
    )]
    fn entry(&self) -> Result<&'static QeData, Never> {
        Ok(&QE_TABLE[usize::from(self.0 & 0x7F)])
    }
}

#[derive(Debug, Clone, Copy)]
struct QeData {
    qe: u32,
    nmps: u8,
    nlps: u8,
    switch: bool,
}

#[rustfmt::skip]
static QE_TABLE: [QeData; 47] = [
    QeData { qe: 0x5601, nmps: 1, nlps: 1, switch: true },
    QeData { qe: 0x3401, nmps: 2, nlps: 6, switch: false },
    QeData { qe: 0x1801, nmps: 3, nlps: 9, switch: false },
    QeData { qe: 0x0AC1, nmps: 4, nlps: 12, switch: false },
    QeData { qe: 0x0521, nmps: 5, nlps: 29, switch: false },
    QeData { qe: 0x0221, nmps: 38, nlps: 33, switch: false },
    QeData { qe: 0x5601, nmps: 7, nlps: 6, switch: true },
    QeData { qe: 0x5401, nmps: 8, nlps: 14, switch: false },
    QeData { qe: 0x4801, nmps: 9, nlps: 14, switch: false },
    QeData { qe: 0x3801, nmps: 10, nlps: 14, switch: false },
    QeData { qe: 0x3001, nmps: 11, nlps: 17, switch: false },
    QeData { qe: 0x2401, nmps: 12, nlps: 18, switch: false },
    QeData { qe: 0x1C01, nmps: 13, nlps: 20, switch: false },
    QeData { qe: 0x1601, nmps: 29, nlps: 21, switch: false },
    QeData { qe: 0x5601, nmps: 15, nlps: 14, switch: true },
    QeData { qe: 0x5401, nmps: 16, nlps: 14, switch: false },
    QeData { qe: 0x5101, nmps: 17, nlps: 15, switch: false },
    QeData { qe: 0x4801, nmps: 18, nlps: 16, switch: false },
    QeData { qe: 0x3801, nmps: 19, nlps: 17, switch: false },
    QeData { qe: 0x3401, nmps: 20, nlps: 18, switch: false },
    QeData { qe: 0x3001, nmps: 21, nlps: 19, switch: false },
    QeData { qe: 0x2801, nmps: 22, nlps: 19, switch: false },
    QeData { qe: 0x2401, nmps: 23, nlps: 20, switch: false },
    QeData { qe: 0x2201, nmps: 24, nlps: 21, switch: false },
    QeData { qe: 0x1C01, nmps: 25, nlps: 22, switch: false },
    QeData { qe: 0x1801, nmps: 26, nlps: 23, switch: false },
    QeData { qe: 0x1601, nmps: 27, nlps: 24, switch: false },
    QeData { qe: 0x1401, nmps: 28, nlps: 25, switch: false },
    QeData { qe: 0x1201, nmps: 29, nlps: 26, switch: false },
    QeData { qe: 0x1101, nmps: 30, nlps: 27, switch: false },
    QeData { qe: 0x0AC1, nmps: 31, nlps: 28, switch: false },
    QeData { qe: 0x09C1, nmps: 32, nlps: 29, switch: false },
    QeData { qe: 0x08A1, nmps: 33, nlps: 30, switch: false },
    QeData { qe: 0x0521, nmps: 34, nlps: 31, switch: false },
    QeData { qe: 0x0441, nmps: 35, nlps: 32, switch: false },
    QeData { qe: 0x02A1, nmps: 36, nlps: 33, switch: false },
    QeData { qe: 0x0221, nmps: 37, nlps: 34, switch: false },
    QeData { qe: 0x0141, nmps: 38, nlps: 35, switch: false },
    QeData { qe: 0x0111, nmps: 39, nlps: 36, switch: false },
    QeData { qe: 0x0085, nmps: 40, nlps: 37, switch: false },
    QeData { qe: 0x0049, nmps: 41, nlps: 38, switch: false },
    QeData { qe: 0x0025, nmps: 42, nlps: 39, switch: false },
    QeData { qe: 0x0015, nmps: 43, nlps: 40, switch: false },
    QeData { qe: 0x0009, nmps: 44, nlps: 41, switch: false },
    QeData { qe: 0x0005, nmps: 45, nlps: 42, switch: false },
    QeData { qe: 0x0001, nmps: 45, nlps: 43, switch: false },
    QeData { qe: 0x5601, nmps: 46, nlps: 46, switch: false },
];

#[cfg(test)]
mod tests;
