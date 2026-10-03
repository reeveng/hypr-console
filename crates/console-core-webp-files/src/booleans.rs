//! A partition's bits, each read against the chance that it is a nought.
//!
//! Everything in a lossy bitstream after its first ten bytes is written with
//! a boolean entropy coder. Each bit comes with the chance, out of 256, that
//! it is a nought, and the coder spends less than a bit on one that went the
//! likely way. Reading is splitting a range at that chance and asking which
//! side of the split the value read so far falls on; the range is kept
//! between 128 and 255 by shifting it, and the value with it, a bit at a
//! time.
//!
//! This is libwebp's reader, which is the RFC's with the range held less one
//! and seven bytes taken at a time while there are eight left. Past the end
//! of its bytes it reads noughts and remembers having done so, and a picture
//! that needed them is the bitstream cut short.

use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_prefix_codes::PastTheEnd;

const TAKEN: u32 = 56;

const WINDOW: u32 = 8;

const EVEN: u8 = 0x80;

pub(crate) struct Booleans<'a> {
    bytes: &'a [u8],
    at: u32,
    value: u64,
    count: u32,
    range: u32,
    past: u32,
}

impl<'a> Booleans<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Result<Booleans<'a>, Never> {
        let mut booleans = Booleans { bytes, at: 0, value: 0, count: 0, range: 254, past: 0 };
        let Ok(()) = booleans.loaded();

        Ok(booleans)
    }

    #[inline(always)]
    fn loaded(&mut self) -> Result<(), Never> {
        let Ok(at) = index(self.at);

        match self.bytes.get(at..).and_then(<[u8]>::first_chunk::<8>) {
            Some(eight) => {
                self.value = self.value.wrapping_shl(TAKEN) | u64::from_be_bytes(*eight).wrapping_shr(WINDOW);
                self.count = self.count.wrapping_add(TAKEN);
                self.at = self.at.wrapping_add(7);

                Ok(())
            },
            None => self.last_loaded(),
        }
    }

    #[inline(never)]
    fn last_loaded(&mut self) -> Result<(), Never> {
        let Ok(at) = index(self.at);

        match (self.bytes.get(at), self.past) {
            (Some(byte), _) => {
                self.value = self.value.wrapping_shl(WINDOW) | u64::from(*byte);
                self.count = self.count.wrapping_add(WINDOW);
                self.at = self.at.wrapping_add(1);
            },
            (None, 0) => {
                self.value = self.value.wrapping_shl(WINDOW);
                self.count = self.count.wrapping_add(WINDOW);
                self.past = 1;
            },
            (None, _) => self.count = WINDOW,
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn bit(&mut self, chance: u8) -> Result<u32, Never> {
        let mut range = self.range;

        match self.count < WINDOW {
            true => {
                let Ok(()) = self.loaded();
            },
            false => {},
        }

        let position = self.count.wrapping_sub(WINDOW);
        let split = range.wrapping_mul(u32::from(chance)).wrapping_shr(8);

        let bit = match self.value.wrapping_shr(position) > u64::from(split) {
            true => {
                range = range.wrapping_sub(split);
                self.value = self.value.wrapping_sub(u64::from(split.wrapping_add(1)).wrapping_shl(position));

                1
            },
            false => {
                range = split.wrapping_add(1);

                0
            },
        };

        let shift = range.leading_zeros().wrapping_sub(24);

        self.range = range.wrapping_shl(shift).wrapping_sub(1);
        self.count = self.count.wrapping_sub(shift);

        Ok(bit)
    }

    pub(crate) fn even(&mut self) -> Result<u32, Never> {
        self.bit(EVEN)
    }

    pub(crate) fn number(&mut self, bits: u32) -> Result<u32, Never> {
        let mut number = 0u32;

        for _bit in 0..bits {
            let Ok(bit) = self.even();

            number = number.wrapping_shl(1) | bit;
        }

        Ok(number)
    }

    pub(crate) fn signed(&mut self, bits: u32) -> Result<i32, Never> {
        let Ok(magnitude) = self.number(bits);
        let Ok(negative) = self.even();
        let magnitude = i32::from_le_bytes(magnitude.to_le_bytes());

        Ok(match negative {
            0 => magnitude,
            _ => magnitude.wrapping_neg(),
        })
    }

    pub(crate) fn flagged(&mut self, bits: u32) -> Result<i32, Never> {
        let Ok(flag) = self.even();

        match flag {
            0 => Ok(0),
            _ => self.signed(bits),
        }
    }

    pub(crate) fn within(&self) -> Result<(), PastTheEnd> {
        match self.past {
            0 => Ok(()),
            _ => Err(PastTheEnd),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bit_certain_to_be_a_nought_is_one_only_when_the_value_is_at_the_top() {
        let Ok(mut booleans) = Booleans::new(&[0xFF, 0xFF]);

        assert_eq!(booleans.bit(1), Ok(1));
    }

    #[test]
    fn reading_past_the_end_is_remembered() {
        let Ok(mut booleans) = Booleans::new(&[0x00]);

        for _bit in 0..16 {
            let _bit = booleans.even();
        }

        assert_eq!(booleans.within(), Err(PastTheEnd));
    }

    #[test]
    fn eight_even_bits_read_back_a_byte_written_plainly() {
        let Ok(mut booleans) = Booleans::new(&[0x00, 0x00, 0x00]);

        assert_eq!(booleans.number(8), Ok(0));
        assert_eq!(booleans.within(), Ok(()));
    }
}
