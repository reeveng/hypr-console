//! CABAC interval arithmetic shared by AVC and HEVC.
//!
//! The codecs own their context arrays, initialization tables and syntax. A
//! context is the original packed byte, probability state << 1 | MPS; decision
//! decoding masks its high bit as the carried engines did. This crate owns
//! range transitions, buffered input, bypass bins and termination. It needs
//! neither allocation nor std. The FilmCraft notices are carried alongside
//! this port in LICENSE-MIT and LICENSE-APACHE.
//!
//! Starts and restarts name byte positions; consumed positions name bits.
//! Input is padded with zeros, and Exhaustion reports the original sixteen
//! byte read-ahead allowance. Codecs still bound the number of bins they read.
//! Position counters saturate so even a position beyond addressable memory
//! supplies padding without overflowing. Bypass runs accept up to 255 bins,
//! in batches of at most sixteen, retaining the low 32 bits of long runs.
//! A terminating bin of one ends the arithmetic stream. Callers set the next
//! byte position and initialize the engine before decoding another stream.

#![no_std]
#![forbid(unsafe_code)]

use console_core_never::Never;

mod tables;

use tables::{NEXT_STATE, RANGE_TAB_LPS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytePosition(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitPosition(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exhaustion {
    WithinAllowance,
    Overrun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidOffset;

impl core::fmt::Display for InvalidOffset {
    fn fmt(&self, to: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        to.write_str("invalid CABAC offset at init")
    }
}

impl core::error::Error for InvalidOffset {}

pub struct Decoder<'a> {
    data: &'a [u8],
    next_byte: u64,
    range: u32,
    value: u64,
    bits: u32,
}

impl<'a> Decoder<'a> {
    pub fn new(data: &'a [u8], start: BytePosition) -> Result<Self, InvalidOffset> {
        let mut decoder = Self {
            data,
            next_byte: start.0,
            range: 0,
            value: 0,
            bits: 0,
        };

        decoder.init_engine()?;

        Ok(decoder)
    }

    pub fn init_engine(&mut self) -> Result<(), InvalidOffset> {
        self.range = 510;
        self.value = 0;
        self.bits = 0;

        let Ok(()) = self.refill();

        self.bits = self.bits.wrapping_sub(9);

        match self.value.wrapping_shr(self.bits) < 510 {
            true => Ok(()),
            false => Err(InvalidOffset),
        }
    }

    pub fn set_byte_position(&mut self, position: BytePosition) -> Result<(), Never> {
        self.next_byte = position.0;
        self.value = 0;
        self.bits = 0;

        Ok(())
    }

    pub fn bit_position(&self) -> Result<BitPosition, Never> {
        Ok(BitPosition(
            self.next_byte
                .saturating_mul(8)
                .saturating_sub(u64::from(self.bits)),
        ))
    }

    pub fn data(&self) -> Result<&'a [u8], Never> {
        Ok(self.data)
    }

    pub fn exhaustion(&self) -> Result<Exhaustion, Never> {
        let length = match u64::try_from(self.data.len()) {
            Ok(length) => length,
            Err(_beyond_counter) => u64::MAX,
        };

        Ok(match self.next_byte > length.saturating_add(16) {
            true => Exhaustion::Overrun,
            false => Exhaustion::WithinAllowance,
        })
    }

    #[inline(always)]
    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(
            explicit051_no_machine_width,
            reason = "only the slice boundary uses the machine width; positions stay u64 and the decoder stays no_std"
        )
    )]
    fn refill(&mut self) -> Result<(), Never> {
        let available = match usize::try_from(self.next_byte) {
            Ok(at) => match self.data.get(at..) {
                Some(bytes) => bytes,
                None => &[],
            },
            Err(_beyond_memory) => &[],
        };
        let mut word = [0u8; 4];

        for (to, from) in word.iter_mut().zip(available.iter()) {
            *to = *from;
        }

        self.next_byte = self.next_byte.saturating_add(4);
        self.value = self.value.wrapping_shl(32) | u64::from(u32::from_be_bytes(word));
        self.bits = self.bits.wrapping_add(32);

        Ok(())
    }

    #[inline(always)]
    fn renormalize(&mut self) -> Result<(), Never> {
        let shift = self.range.leading_zeros().wrapping_sub(23);

        self.range = self.range.wrapping_shl(shift);

        match self.bits < shift {
            true => {
                let Ok(()) = self.refill();
            }
            false => {}
        }

        self.bits = self.bits.wrapping_sub(shift);

        Ok(())
    }

    #[inline(always)]
    #[cfg_attr(
        dylint_lib = "explicit014_no_index_slice",
        allow(
            explicit014_no_index_slice,
            reason = "the packed context is masked to 0..127 and the range column to 0..3, keeping every table index in bounds"
        )
    )]
    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(
            explicit051_no_machine_width,
            reason = "table lookup widths are local conversions of masked u8 states in a no_std decoder"
        )
    )]
    pub fn decode_decision(&mut self, context: &mut u8) -> Result<u32, Never> {
        let state = *context & 127;
        let column = (self.range.wrapping_shr(6) & 3).to_le_bytes()[0];
        let lps = u32::from(RANGE_TAB_LPS[usize::from(state.wrapping_shr(1))][usize::from(column)]);
        let mps_range = self.range.wrapping_sub(lps);
        let scaled = u64::from(mps_range).wrapping_shl(self.bits);
        let is_lps = self.value >= scaled;

        self.value = self
            .value
            .wrapping_sub(scaled & u64::from(is_lps).wrapping_neg());
        self.range = match is_lps {
            true => lps,
            false => mps_range,
        };
        *context = NEXT_STATE[usize::from(state)][usize::from(is_lps)];

        let Ok(()) = self.renormalize();

        Ok(u32::from(state & 1) ^ u32::from(is_lps))
    }

    #[inline(always)]
    pub fn decode_bypass(&mut self) -> Result<u32, Never> {
        match self.bits {
            0 => {
                let Ok(()) = self.refill();
            }
            1.. => {}
        }

        self.bits = self.bits.wrapping_sub(1);

        let scaled = u64::from(self.range).wrapping_shl(self.bits);
        let one = self.value >= scaled;

        self.value = self
            .value
            .wrapping_sub(scaled & u64::from(one).wrapping_neg());

        Ok(u32::from(one))
    }

    #[inline(always)]
    pub fn decode_bypass_bits(&mut self, count: u8) -> Result<u32, Never> {
        let mut result = 0u32;

        for start in (0..u32::from(count)).step_by(16) {
            let batch = u32::from(count).wrapping_sub(start).min(16);

            match self.bits < batch {
                true => {
                    let Ok(()) = self.refill();
                }
                false => {}
            }

            self.bits = self.bits.wrapping_sub(batch);

            let quotient = self
                .value
                .wrapping_shr(self.bits)
                .wrapping_div(u64::from(self.range));
            let [a, b, c, d, _e, _f, _g, _h] = quotient.to_le_bytes();

            self.value = self.value.wrapping_sub(
                quotient
                    .wrapping_mul(u64::from(self.range))
                    .wrapping_shl(self.bits),
            );
            result = result.wrapping_shl(batch) | u32::from_le_bytes([a, b, c, d]);
        }

        Ok(result)
    }

    pub fn decode_terminate(&mut self) -> Result<u32, Never> {
        self.range = self.range.wrapping_sub(2);

        let scaled = u64::from(self.range).wrapping_shl(self.bits);

        match self.value >= scaled {
            true => Ok(1),
            false => {
                let Ok(()) = self.renormalize();

                Ok(0)
            }
        }
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
