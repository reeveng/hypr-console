//! A partition's bits, each written against the chance that it is a nought.
//!
//! The other half of `booleans`, and RFC 6386's writer rather than libwebp's,
//! because it is the one the RFC proves the reader against: the bottom of the
//! range narrowed to so far and the range itself, a byte written out each time
//! eight bits of the bottom are settled, and a carry walked back into the bytes
//! already written when an addition overflows them. A run is finished with
//! thirty-two noughts at even chances, which settles every bit a reader can
//! still ask for.
//!
//! What a bit costs is the other thing kept here, in 256ths of a bit, because
//! choosing how to write a block is weighing what it costs against how far it
//! lands from the picture. A nought at a chance of `p` in 256 costs the
//! logarithm of `256 / p`, and a one costs the same of `256 / (256 - p)`.

use console_core_never::Never;
use console_core_number_conversion::{index, whole_u32};

const EVEN: u8 = 0x80;

const SETTLED: u32 = 24;

const TOP: u32 = 1 << 31;

const LOW_BITS: u32 = (1 << 24) - 1;

const FINISHING: u32 = 32;

const CERTAIN: u32 = 256;

const NEVER_SENT: u32 = 4096;

pub(crate) struct BooleanEncoder {
    bytes: Vec<u8>,
    range: u32,
    bottom: u32,
    count: u32,
}

impl BooleanEncoder {
    pub(crate) fn new() -> Result<BooleanEncoder, Never> {
        Ok(BooleanEncoder { bytes: Vec::new(), range: 255, bottom: 0, count: SETTLED })
    }

    pub(crate) fn bit(&mut self, sent: (u32, u8)) -> Result<(), Never> {
        let (bit, chance) = sent;
        let split = self.range.wrapping_sub(1).wrapping_mul(u32::from(chance)).wrapping_shr(8).wrapping_add(1);

        match bit {
            0 => self.range = split,
            _ => {
                self.bottom = self.bottom.wrapping_add(split);
                self.range = self.range.wrapping_sub(split);
            },
        }

        let shift = self.range.leading_zeros().saturating_sub(SETTLED);

        for _each in 0..shift {
            let Ok(()) = self.shifted();
        }

        Ok(())
    }

    fn shifted(&mut self) -> Result<(), Never> {
        self.range = self.range.wrapping_shl(1);

        match self.bottom & TOP {
            0 => {},
            _ => {
                let Ok(()) = self.carried();
            },
        }

        self.bottom = self.bottom.wrapping_shl(1);
        self.count = self.count.wrapping_sub(1);

        match self.count {
            0 => {
                let [_, _, _, settled] = self.bottom.to_le_bytes();

                self.bytes.push(settled);
                self.bottom &= LOW_BITS;
                self.count = 8;
            },
            _ => {},
        }

        Ok(())
    }

    fn carried(&mut self) -> Result<(), Never> {
        'back: for byte in self.bytes.iter_mut().rev() {
            match *byte {
                u8::MAX => *byte = 0,
                _ => {
                    *byte = byte.wrapping_add(1);

                    break 'back;
                },
            }
        }

        Ok(())
    }

    pub(crate) fn even(&mut self, bit: u32) -> Result<(), Never> {
        self.bit((bit, EVEN))
    }

    pub(crate) fn number(&mut self, sent: (u32, u32)) -> Result<(), Never> {
        let (value, bits) = sent;

        for at in (0..bits).rev() {
            let Ok(()) = self.even(value.wrapping_shr(at) & 1);
        }

        Ok(())
    }

    pub(crate) fn flagged(&mut self, sent: (i32, u32)) -> Result<(), Never> {
        let (value, bits) = sent;

        match value {
            0 => self.even(0),
            _ => {
                let Ok(()) = self.even(1);
                let Ok(()) = self.number((value.unsigned_abs(), bits));

                self.even(u32::from(value < 0))
            },
        }
    }

    pub(crate) fn finished(mut self) -> Result<Vec<u8>, Never> {
        for _each in 0..FINISHING {
            let Ok(()) = self.even(0);
        }

        Ok(self.bytes)
    }
}

pub(crate) struct Costs {
    bits: [u32; 257],
}

impl Costs {
    pub(crate) fn new() -> Result<Costs, Never> {
        let mut bits = [NEVER_SENT; 257];

        for (cost, chance) in bits.iter_mut().zip(0u32..).skip(1) {
            let Ok(spent) = whole_u32(-(f64::from(chance) / f64::from(CERTAIN)).log2() * f64::from(CERTAIN));

            *cost = spent;
        }

        Ok(Costs { bits })
    }

    #[inline(always)]
    pub(crate) fn of(&self, sent: (u32, u8)) -> Result<u32, Never> {
        let (bit, chance) = sent;

        let likely = match bit {
            0 => u32::from(chance),
            _ => CERTAIN.saturating_sub(u32::from(chance)),
        };

        let Ok(at) = index(likely);

        Ok(match self.bits.get(at) {
            Some(cost) => *cost,
            None => NEVER_SENT,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::booleans::Booleans;

    #[test]
    fn what_is_written_is_read_back_at_every_chance() {
        let Ok(mut writer) = BooleanEncoder::new();
        let sent: Vec<(u32, u8)> = (0u32..2000).map(|at| (at.wrapping_mul(7919).wrapping_shr(3) & 1, u8::try_from(at.wrapping_mul(31) & 0xFF).map_or(1, |chance| chance.max(1)))).collect();

        for one in &sent {
            let Ok(()) = writer.bit(*one);
        }

        let Ok(()) = writer.number((0x5A, 8));
        let Ok(()) = writer.flagged((-5, 4));
        let Ok(bytes) = writer.finished();
        let Ok(mut reader) = Booleans::new(&bytes);
        let mut read: Vec<(u32, u8)> = Vec::new();

        for (_, chance) in &sent {
            match reader.bit(*chance) {
                Ok(bit) => read.push((bit, *chance)),
                Err(_unread) => read.push((2, *chance)),
            }
        }

        assert_eq!(read, sent);
        assert_eq!(reader.number(8), Ok(0x5A));
        assert_eq!(reader.flagged(4), Ok(-5));
        assert_eq!(reader.within(), Ok(()));
    }

    #[test]
    fn a_likely_bit_costs_less_than_one_and_an_even_one_costs_one() -> Result<(), Never> {
        let Ok(costs) = Costs::new();
        let Ok(likely) = costs.of((0, 250));
        let Ok(unlikely) = costs.of((1, 250));

        assert_eq!(costs.of((0, 128)), Ok(256));
        assert_eq!(costs.of((1, 128)), Ok(256));
        assert!(likely < 10);
        assert!(unlikely > 1000);

        Ok(())
    }
}
