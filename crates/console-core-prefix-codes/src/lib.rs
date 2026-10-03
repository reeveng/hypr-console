//! Bits read from the lowest of each byte up, and the canonical prefix codes
//! written in them.
//!
//! Deflate and a lossless WebP are both written this way: every field is a
//! run of bits taken from the low end of what is left, and every symbol is a
//! canonical prefix code, the codes the length of each symbol's code makes,
//! read a bit at a time from the low end. `console-core-zip-files` inflates
//! with them and `console-core-webp-files` reads its pixels with them.
//!
//! [`Bits`] keeps up to sixty-four bits in a word, topped up eight bytes at a
//! time and shifted down as they are taken. Fifty-six are always there after
//! a top-up, which is enough for three codes of fifteen bits, or a code and
//! the most extra bits either format puts after one, so a decoder tops up
//! once or twice a symbol. Past the end the word is topped up with nothing.
//! Whether a bit that was never there has been taken is asked by the decoder
//! when it likes -- at a header field, at the end of a row or a block -- and
//! not once a code, and the answer is the stream cut short whatever was made
//! of the bits past its end.
//!
//! A [`Code`] is undone through a table indexed by its next ten bits, which
//! says the symbol and how long its code is; a code longer than that is
//! counted out from the lengths instead. The table is as large as the longest
//! code needs and no larger, so a code of a few symbols is a few entries.
//! Deflate takes any lengths that do not ask for more codes than there are
//! strings of bits; a WebP's code has to be complete, unless it has one
//! symbol, which is then read in no bits at all.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const FULL: u32 = 56;

const PAST_THE_END: u8 = 0;

const QUICK: u32 = 10;

const LONGEST: u32 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PastTheEnd;

impl std::fmt::Display for PastTheEnd {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(to, "the bits ran out before what they hold did")
    }
}

impl std::error::Error for PastTheEnd {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooLong;

impl std::fmt::Display for TooLong {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(to, "these are more bytes than a stream is counted in")
    }
}

impl std::error::Error for TooLong {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeError {
    Oversubscribed,
    Incomplete,
    Unused,
    Unknown,
}

impl std::fmt::Display for CodeError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeError::Oversubscribed => write!(to, "these lengths ask for more codes than there are"),
            CodeError::Incomplete => write!(to, "these lengths leave strings of bits that are no code"),
            CodeError::Unused => write!(to, "no symbol in this code has a length"),
            CodeError::Unknown => write!(to, "these bits are no code"),
        }
    }
}

impl std::error::Error for CodeError {}

pub struct Bits<'a> {
    bytes: &'a [u8],
    at: u32,
    word: u64,
    count: u32,
    there: u64,
}

impl<'a> Bits<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Bits<'a>, TooLong> {
        let long = match u32::try_from(bytes.len()) {
            Ok(long) => long,
            Err(_fault) => return Err(TooLong),
        };

        Ok(Bits { bytes, at: 0, word: 0, count: 0, there: u64::from(long).saturating_mul(8) })
    }

    pub fn bytes(&self) -> Result<&'a [u8], Never> {
        Ok(self.bytes)
    }

    #[inline(always)]
    pub fn fill(&mut self) -> Result<(), Never> {
        let Ok(at) = index(self.at);

        match self.bytes.get(at..).and_then(<[u8]>::first_chunk::<8>) {
            Some(eight) => {
                self.word |= u64::from_le_bytes(*eight).wrapping_shl(self.count);
                self.at = self.at.saturating_add(63u32.saturating_sub(self.count).wrapping_shr(3));
                self.count |= FULL;
            },
            None => {
                let Ok(()) = self.near_the_end();
            },
        }

        Ok(())
    }

    #[inline(never)]
    fn near_the_end(&mut self) -> Result<(), Never> {
        for _byte in 0..FULL.saturating_sub(self.count).div_ceil(8) {
            let Ok(at) = index(self.at);

            let byte = match self.bytes.get(at) {
                Some(byte) => *byte,
                None => PAST_THE_END,
            };

            self.word |= u64::from(byte).wrapping_shl(self.count);
            self.count = self.count.saturating_add(8);
            self.at = self.at.saturating_add(1);
        }

        Ok(())
    }

    #[inline(always)]
    pub fn skip(&mut self, count: u32) -> Result<(), Never> {
        self.word = self.word.wrapping_shr(count);
        self.count = self.count.saturating_sub(count);

        Ok(())
    }

    #[inline(always)]
    pub fn held(&mut self, count: u32) -> Result<u32, Never> {
        let Ok(value) = fitted::<u64, u32>(self.word & 1u64.wrapping_shl(count).wrapping_sub(1));
        let Ok(()) = self.skip(count);

        Ok(value)
    }

    pub fn take(&mut self, count: u32) -> Result<u32, PastTheEnd> {
        let Ok(()) = self.fill();
        let Ok(value) = self.held(count);

        self.within()?;

        Ok(value)
    }

    pub fn within(&self) -> Result<(), PastTheEnd> {
        let taken = u64::from(self.at).saturating_mul(8).saturating_sub(u64::from(self.count));

        match taken <= self.there {
            true => Ok(()),
            false => Err(PastTheEnd),
        }
    }

    pub fn aligned(&mut self) -> Result<u32, Never> {
        let Ok(()) = self.skip(self.count & 7);
        let here = self.at.saturating_sub(self.count.wrapping_shr(3));
        let Ok(()) = self.moved_to(here);

        Ok(here)
    }

    pub fn moved_to(&mut self, at: u32) -> Result<(), Never> {
        self.at = at;
        self.word = 0;
        self.count = 0;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quick {
    Longer,
    Code { length: u8, symbol: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    quick: Vec<Quick>,
    mask: u64,
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Code {
    pub fn partial(lengths: &[u8]) -> Result<Code, CodeError> {
        let count = counted(lengths)?;
        let _strings_no_code_begins = unused(&count)?;

        built(lengths, count)
    }

    pub fn complete(lengths: &[u8]) -> Result<Code, CodeError> {
        let count = counted(lengths)?;
        let used = count.iter().skip(1).fold(0u32, |used, counted| used.saturating_add(u32::from(*counted)));

        match used {
            0 => return Err(CodeError::Unused),
            1 => return only(lengths),
            2.. => {},
        }

        let left = unused(&count)?;

        match left {
            0 => {},
            1.. => return Err(CodeError::Incomplete),
        }

        built(lengths, count)
    }

    pub fn only(&self) -> Result<Option<u16>, Never> {
        Ok(match (self.mask, self.quick.first()) {
            (0, Some(Quick::Code { symbol, .. })) => Some(*symbol),
            (_, _) => None,
        })
    }

    #[inline(always)]
    pub fn decoded(&self, bits: &mut Bits<'_>) -> Result<u16, CodeError> {
        let Ok(at) = index(bits.word & self.mask);

        match self.quick.get(at) {
            Some(Quick::Code { length, symbol }) => {
                let Ok(()) = bits.skip(u32::from(*length));

                Ok(*symbol)
            },
            Some(Quick::Longer) | None => self.longer(bits),
        }
    }

    #[inline(never)]
    fn longer(&self, bits: &mut Bits<'_>) -> Result<u16, CodeError> {
        let word = bits.word;
        let mut read = 0u32;
        let mut first = 0u32;
        let mut passed = 0u32;

        for (length, counted) in (1u32..).zip(self.count.iter().skip(1)) {
            let Ok(bit) = fitted::<u64, u32>(word.wrapping_shr(length.saturating_sub(1)) & 1);

            read |= bit;

            let counted = u32::from(*counted);
            let past = first.saturating_add(counted);

            match read < past {
                true => {
                    let Ok(slot) = index(passed.saturating_add(read.saturating_sub(first)));
                    let Ok(()) = bits.skip(length);

                    return match self.symbol.get(slot) {
                        Some(symbol) => Ok(*symbol),
                        None => Err(CodeError::Unknown),
                    };
                },
                false => {},
            }

            passed = passed.saturating_add(counted);
            first = past.wrapping_shl(1);
            read = read.wrapping_shl(1);
        }

        Err(CodeError::Unknown)
    }
}

fn counted(lengths: &[u8]) -> Result<[u16; 16], CodeError> {
    let mut count = [0u16; 16];

    for length in lengths {
        let Ok(at) = index(*length);

        match count.get_mut(at) {
            Some(counted) => *counted = counted.saturating_add(1),
            None => return Err(CodeError::Oversubscribed),
        }
    }

    Ok(count)
}

fn unused(count: &[u16; 16]) -> Result<u32, CodeError> {
    let left = count.iter().skip(1).try_fold(1u32, |left, counted| left.wrapping_shl(1).checked_sub(u32::from(*counted)));

    match left {
        Some(left) => Ok(left),
        None => Err(CodeError::Oversubscribed),
    }
}

fn only(lengths: &[u8]) -> Result<Code, CodeError> {
    match (0u16..).zip(lengths).find(|(_, length)| **length > 0) {
        Some((symbol, _)) => Ok(Code { quick: vec![Quick::Code { length: 0, symbol }], mask: 0, count: [0; 16], symbol: Vec::new() }),
        None => Err(CodeError::Unused),
    }
}

fn built(lengths: &[u8], count: [u16; 16]) -> Result<Code, CodeError> {
    let longest = (1u32..=LONGEST).zip(count.iter().skip(1)).fold(0u32, |longest, (length, counted)| match counted {
        0 => longest,
        1.. => length,
    });

    let looked = longest.min(QUICK);
    let symbol = in_order(lengths, &count)?;
    let Ok(quick) = quick(lengths, (&count, looked));

    Ok(Code { quick, mask: 1u64.wrapping_shl(looked).wrapping_sub(1), count, symbol })
}

fn in_order(lengths: &[u8], count: &[u16; 16]) -> Result<Vec<u16>, CodeError> {
    let mut next = [0u16; 16];
    let mut running = 0u16;

    for (offset, counted) in next.iter_mut().zip(count.iter()).skip(1) {
        *offset = running;
        running = running.saturating_add(*counted);
    }

    let Ok(used) = index(running);
    let mut symbol = vec![0u16; used];

    for (named, length) in (0u16..).zip(lengths) {
        let Ok(at) = index(*length);

        match (*length, next.get_mut(at)) {
            (0, _) => {},
            (_, Some(offset)) => {
                let Ok(slot) = index(*offset);

                match symbol.get_mut(slot) {
                    Some(slot) => *slot = named,
                    None => return Err(CodeError::Oversubscribed),
                }

                *offset = offset.saturating_add(1);
            },
            (_, None) => return Err(CodeError::Oversubscribed),
        }
    }

    Ok(symbol)
}

fn quick(lengths: &[u8], counted: (&[u16; 16], u32)) -> Result<Vec<Quick>, Never> {
    let (count, looked) = counted;
    let mut first = [0u32; 16];
    let mut code = 0u32;
    let shorter = std::iter::once(0u16).chain(count.iter().skip(1).copied());

    for (slot, counted) in first.iter_mut().skip(1).zip(shorter) {
        code = code.saturating_add(u32::from(counted)).wrapping_shl(1);
        *slot = code;
    }

    let Ok(entries) = index(1u32.wrapping_shl(looked));
    let mut quick = vec![Quick::Longer; entries];

    for (symbol, length) in (0u16..).zip(lengths) {
        let Ok(at) = index(*length);

        match (u32::from(*length) <= looked, *length, first.get_mut(at)) {
            (true, 1.., Some(next)) => {
                let Ok(()) = spread(&mut quick, *next, (*length, symbol));

                *next = next.saturating_add(1);
            },
            (_, _, _) => {},
        }
    }

    Ok(quick)
}

fn spread(quick: &mut [Quick], code: u32, coded: (u8, u16)) -> Result<(), Never> {
    let (length, symbol) = coded;
    let bits = u32::from(length);
    let Ok(from) = index(code.reverse_bits().wrapping_shr(32u32.saturating_sub(bits)));
    let Ok(step) = index(1u32.wrapping_shl(bits));

    for slot in quick.iter_mut().skip(from).step_by(step) {
        *slot = Quick::Code { length, symbol };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_taken_from_the_low_end_of_each_byte() -> Result<(), TooLong> {
        let mut bits = Bits::new(&[0b1011_0110, 0b0000_0001])?;

        assert_eq!(bits.take(1), Ok(0));
        assert_eq!(bits.take(2), Ok(0b11));
        assert_eq!(bits.take(6), Ok(0b11_0110));

        Ok(())
    }

    #[test]
    fn a_bit_past_the_end_is_the_stream_cut_short() -> Result<(), TooLong> {
        let mut bits = Bits::new(&[0xFF])?;

        assert_eq!(bits.take(8), Ok(0xFF));
        assert_eq!(bits.take(1), Err(PastTheEnd));

        Ok(())
    }

    #[test]
    fn aligning_drops_what_is_left_of_the_byte_and_says_where_the_next_one_is() -> Result<(), TooLong> {
        let mut bits = Bits::new(&[0xFF, 0x0F, 0xAA])?;

        assert_eq!(bits.take(3), Ok(0b111));
        assert_eq!(bits.aligned(), Ok(1));
        assert_eq!(bits.take(8), Ok(0x0F));

        Ok(())
    }

    #[test]
    fn a_complete_code_of_one_symbol_is_read_in_no_bits() -> Result<(), Box<dyn std::error::Error>> {
        let mut bits = Bits::new(&[0xFF])?;
        let only = Code::complete(&[0, 0, 3, 0])?;

        assert_eq!(only.decoded(&mut bits), Ok(2));
        assert_eq!(bits.take(8), Ok(0xFF));

        Ok(())
    }

    #[test]
    fn a_complete_code_leaves_no_string_of_bits_unused_and_a_partial_one_may() {
        assert_eq!(Code::complete(&[1, 2, 0, 0]), Err(CodeError::Incomplete));
        assert_eq!(Code::complete(&[0, 0, 0]), Err(CodeError::Unused));
        assert_eq!(Code::complete(&[1, 1, 1]), Err(CodeError::Oversubscribed));
        assert_eq!(Code::partial(&[1, 1, 1]), Err(CodeError::Oversubscribed));
        assert_eq!(Code::partial(&[1, 2, 0, 0]).map(|partial| partial.mask), Ok(0b11));
    }

    #[test]
    fn codes_longer_than_the_table_are_counted_out() -> Result<(), Box<dyn std::error::Error>> {
        let lengths = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15];
        let code = Code::complete(&lengths)?;
        let longest_but_one = [0xFF, 0xBF];
        let mut bits = Bits::new(&longest_but_one)?;

        let Ok(()) = bits.fill();

        assert_eq!(code.decoded(&mut bits), Ok(14));

        Ok(())
    }
}
