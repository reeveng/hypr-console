//! Deflate, undone: RFC 1951 read back into the bytes it was made from.
//!
//! A book is a zip of pages and a comic a zip of pictures, every one of them
//! stored or deflated, and a PNG is one deflate stream cut into chunks. A page
//! is tens of kilobytes; a PNG of a photograph is tens of megabytes, and read a
//! bit at a time, the way zlib's `puff` reads it, a twelve megapixel one took
//! more than half a second to inflate. So a code is looked up rather than
//! walked: the next ten bits index a table of a thousand and twenty-four that
//! says which symbol they begin with and how long its code is, and only a code
//! longer than ten is read a length at a time. The bits are held up to
//! sixty-four at once and topped up eight bytes at a time, which is more than
//! a length and a distance take with all their extra bits, so a symbol asks
//! for more once. The bits and the codes are `console-core-prefix-codes`',
//! which a lossless WebP is read with too.
//!
//! What it will not do is make more than it was told it would. A zip says how
//! large each thing in it is, and a stream that runs past that is either
//! damaged or written to fill the disk, so the size said is the most that is
//! ever made. Nor does it read past what it was handed: the bits held past
//! the end are zeros, and a block that took one of them is the stream cut
//! short, whatever it made of them.

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_prefix_codes::{Bits, Code, PastTheEnd};

use crate::ZipError;

pub(crate) const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];

pub(crate) const LENGTH_EXTRA: [u8; 29] =
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];

pub(crate) const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

pub(crate) const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

pub(crate) const LENGTHS_IN_ORDER: [u8; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

pub(crate) const END_OF_BLOCK: u16 = 256;

pub(crate) const FIRST_LENGTH: u16 = 257;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BackReference {
    pub(crate) distance: u32,
    pub(crate) length: u32,
}

struct Output {
    bytes: Vec<u8>,
    written: u32,
    most: u32,
}

impl Output {
    #[inline(always)]
    fn push(&mut self, byte: u8) -> Result<(), ZipError> {
        match self.written < self.most {
            true => {},
            false => return Err(ZipError::LargerThanDeclared),
        }

        self.bytes.push(byte);
        self.written = self.written.saturating_add(1);

        Ok(())
    }

    fn room(&self, long: u32) -> Result<u32, ZipError> {
        let past = match self.written.checked_add(long) {
            Some(past) => past,
            None => return Err(ZipError::LargerThanDeclared),
        };

        match past <= self.most {
            true => Ok(past),
            false => Err(ZipError::LargerThanDeclared),
        }
    }

    fn copy_back(&mut self, reference: BackReference) -> Result<(), ZipError> {
        let BackReference { distance: back, length: long } = reference;
        let past = self.room(long)?;

        let from = match self.written.checked_sub(back) {
            Some(from) => from,
            None => return Err(ZipError::Corrupt),
        };

        let Ok(start) = index(from);
        let Ok(end) = index(from.saturating_add(long));

        match back < long {
            false => self.bytes.extend_from_within(start..end),
            true => {
                for step in start..end {
                    let byte = match self.bytes.get(step) {
                        Some(byte) => *byte,
                        None => return Err(ZipError::Corrupt),
                    };

                    self.bytes.push(byte);
                }
            },
        }

        self.written = past;

        Ok(())
    }

    fn copy(&mut self, taken: &[u8]) -> Result<(), ZipError> {
        let long = match u32::try_from(taken.len()) {
            Ok(long) => long,
            Err(_fault) => return Err(ZipError::LargerThanDeclared),
        };

        let past = self.room(long)?;

        self.bytes.extend_from_slice(taken);
        self.written = past;

        Ok(())
    }
}

#[inline(always)]
fn read_extra(bits: &mut Bits<'_>, bases: &[u16], extras: &[u8], at: u16) -> Result<u32, ZipError> {
    let Ok(at) = index(at);

    let (base, extra) = match (bases.get(at), extras.get(at)) {
        (Some(base), Some(extra)) => (*base, *extra),
        (None, _) | (_, None) => return Err(ZipError::Corrupt),
    };

    let Ok(more) = bits.held(u32::from(extra));

    Ok(u32::from(base).saturating_add(more))
}

struct Tables {
    lengths: Code,
    distances: Code,
}

#[inline(always)]
fn decode_symbol(bits: &mut Bits<'_>, code: &Code) -> Result<u16, ZipError> {
    match code.decoded(bits) {
        Ok(symbol) => Ok(symbol),
        Err(_no_such_code) => Err(ZipError::Corrupt),
    }
}

fn taken(bits: &mut Bits<'_>, count: u32) -> Result<u32, ZipError> {
    match bits.take(count) {
        Ok(taken) => Ok(taken),
        Err(PastTheEnd) => Err(ZipError::Truncated),
    }
}

fn code(lengths: &[u8]) -> Result<Code, ZipError> {
    match Code::partial(lengths) {
        Ok(code) => Ok(code),
        Err(_not_a_code) => Err(ZipError::Corrupt),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Continues,
    Ended,
}

fn compressed_block(bits: &mut Bits<'_>, output: &mut Output, codes: &Tables) -> Result<(), ZipError> {
    let decoded = iterate((bits, output), |(bits, output)| {
        Ok(match decoded_symbol(bits, output, codes) {
            Ok(Block::Continues) => Step::Again((bits, output)),
            Ok(Block::Ended) => Step::Halt(Ok(())),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    match decoded {
        Ok(decoded) => decoded,
        Err(Endless) => Err(ZipError::Corrupt),
    }
}

#[inline(always)]
fn decoded_symbol(bits: &mut Bits<'_>, output: &mut Output, codes: &Tables) -> Result<Block, ZipError> {
    let Ok(()) = bits.fill();
    let symbol = decode_symbol(bits, &codes.lengths)?;

    match (symbol, u8::try_from(symbol)) {
        (_, Ok(byte)) => output.push(byte)?,
        (END_OF_BLOCK, Err(_past_a_byte)) => return Ok(Block::Ended),
        (_, Err(_past_a_byte)) => {
            let long = read_extra(bits, &LENGTH_BASE, &LENGTH_EXTRA, symbol.saturating_sub(FIRST_LENGTH))?;
            let near = decode_symbol(bits, &codes.distances)?;
            let back = read_extra(bits, &DISTANCE_BASE, &DISTANCE_EXTRA, near)?;

            output.copy_back(BackReference { distance: back, length: long })?;
        },
    }

    Ok(Block::Continues)
}

fn stored_block(bits: &mut Bits<'_>, output: &mut Output) -> Result<(), ZipError> {
    let Ok(here) = bits.aligned();
    let Ok(from) = index(here);
    let Ok(bytes) = bits.bytes();

    let (said, rest) = match bytes.get(from..).and_then(<[u8]>::split_first_chunk::<4>) {
        Some(split) => split,
        None => return Err(ZipError::Truncated),
    };

    let [low, high, check_low, check_high] = *said;
    let long = u16::from_le_bytes([low, high]);

    match long == !u16::from_le_bytes([check_low, check_high]) {
        true => {},
        false => return Err(ZipError::Corrupt),
    }

    let Ok(taken) = index(long);

    let taken = match rest.get(..taken) {
        Some(taken) => taken,
        None => return Err(ZipError::Truncated),
    };

    output.copy(taken)?;

    let whole = u32::from(long).saturating_add(4);
    let Ok(()) = bits.moved_to(here.saturating_add(whole));

    Ok(())
}

pub(crate) const FIXED_DISTANCES: [u8; 30] = [5; 30];

pub(crate) fn fixed_lengths() -> Result<Vec<u8>, Never> {
    Ok((0u16..288)
        .map(|symbol| match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        })
        .collect())
}

fn fixed_tables() -> Result<Tables, ZipError> {
    let Ok(lengths) = fixed_lengths();
    let lengths = code(&lengths)?;
    let distances = code(&FIXED_DISTANCES)?;

    Ok(Tables { lengths, distances })
}

fn push_repeated(lengths: &mut Vec<u8>, value: u8, times: u32) -> Result<(), Never> {
    for _ in 0..times {
        lengths.push(value);
    }

    Ok(())
}

fn repeated(bits: &mut Bits<'_>, short: &Code, lengths: &[u8]) -> Result<(u8, u32), ZipError> {
    let Ok(()) = bits.fill();
    let symbol = decode_symbol(bits, short)?;

    let (value, times) = match (symbol, u8::try_from(symbol)) {
        (0..=15, Ok(length)) => (length, 1),
        (16, _) => match lengths.last() {
            Some(last) => {
                let Ok(more) = bits.held(2);

                (*last, more.saturating_add(3))
            },
            None => return Err(ZipError::Corrupt),
        },
        (17, _) => {
            let Ok(more) = bits.held(3);

            (0, more.saturating_add(3))
        },
        (18, _) => {
            let Ok(more) = bits.held(7);

            (0, more.saturating_add(11))
        },
        (_, _) => return Err(ZipError::Corrupt),
    };

    Ok((value, times))
}

fn read_code_lengths(bits: &mut Bits<'_>, all: u32) -> Result<Vec<u8>, ZipError> {
    let told = taken(bits, 4)?;
    let mut short = [0u8; 19];

    let Ok(told) = index(told.saturating_add(4));

    for order in LENGTHS_IN_ORDER.iter().take(told) {
        let length = taken(bits, 3)?;
        let Ok(at) = index(*order);

        match (short.get_mut(at), u8::try_from(length)) {
            (Some(slot), Ok(length)) => *slot = length,
            (None, _) | (_, Err(_)) => return Err(ZipError::Corrupt),
        }
    }

    let short = code(&short)?;

    let read = iterate((bits, Vec::new(), 0u32), |(bits, mut lengths, had)| {
        match had < all {
            true => {},
            false => return Ok(Step::Halt(Ok((lengths, had)))),
        }

        Ok(match repeated(bits, &short, &lengths) {
            Ok((value, times)) => {
                let Ok(()) = push_repeated(&mut lengths, value, times);

                Step::Again((bits, lengths, had.saturating_add(times)))
            },
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    let read = match read {
        Ok(read) => read,
        Err(Endless) => Err(ZipError::Corrupt),
    };
    let (lengths, had) = read?;

    match had == all {
        true => Ok(lengths),
        false => Err(ZipError::Corrupt),
    }
}

fn dynamic_tables(bits: &mut Bits<'_>) -> Result<Tables, ZipError> {
    let literal = taken(bits, 5)?;
    let literal = literal.saturating_add(u32::from(FIRST_LENGTH));
    let distance = taken(bits, 5)?;
    let distance = distance.saturating_add(1);

    let lengths = read_code_lengths(bits, literal.saturating_add(distance))?;
    let Ok(split) = index(literal);

    let (literals, distances) = match lengths.split_at_checked(split) {
        Some(both) => both,
        None => return Err(ZipError::Corrupt),
    };

    let lengths = code(literals)?;
    let distances = code(distances)?;

    Ok(Tables { lengths, distances })
}

const DEFLATE_EXPANDS_AT_MOST: u32 = 1032;

pub fn inflate(packed: &[u8], size: u32) -> Result<Vec<u8>, ZipError> {
    let long = match u32::try_from(packed.len()) {
        Ok(long) => long,
        Err(_fault) => return Err(ZipError::TooLarge),
    };

    let bits = match Bits::new(packed) {
        Ok(bits) => bits,
        Err(_too_long) => return Err(ZipError::TooLarge),
    };

    let Ok(room) = index(size.min(long.saturating_mul(DEFLATE_EXPANDS_AT_MOST)));
    let output = Output { bytes: Vec::with_capacity(room), written: 0, most: size };

    let inflated = iterate((bits, output), |(mut bits, mut output)| {
        Ok(match (block(&mut bits, &mut output), bits.within()) {
            (Ok(Block::Continues), Ok(())) => Step::Again((bits, output)),
            (Ok(Block::Ended), Ok(())) => Step::Halt(Ok(output.bytes)),
            (Err(fault), Ok(())) => Step::Halt(Err(fault)),
            (_, Err(PastTheEnd)) => Step::Halt(Err(ZipError::Truncated)),
        })
    });

    match inflated {
        Ok(inflated) => inflated,
        Err(Endless) => Err(ZipError::Corrupt),
    }
}

fn block(bits: &mut Bits<'_>, output: &mut Output) -> Result<Block, ZipError> {
    let last = taken(bits, 1)?;
    let kind = taken(bits, 2)?;

    match kind {
        0 => stored_block(bits, output)?,
        1 => {
            let codes = fixed_tables()?;

            compressed_block(bits, output, &codes)?;
        },
        2 => {
            let codes = dynamic_tables(bits)?;

            compressed_block(bits, output, &codes)?;
        },
        _ => return Err(ZipError::Corrupt),
    }

    Ok(match last {
        1 => Block::Ended,
        _ => Block::Continues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn a_stored_block_is_its_own_bytes() {
        let packed = [0x01, 0x05, 0x00, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o'];

        assert_eq!(inflate(&packed, 5), Ok(b"hello".to_vec()));
    }

    #[test]
    fn a_fixed_block_with_a_repeat_in_it_comes_back() {
        let packed = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x90, 0x00];

        assert_eq!(inflate(&packed, 17), Ok(b"hello hello hello".to_vec()));
    }

    #[test]
    fn a_stream_that_runs_past_its_size_is_refused_rather_than_followed() {
        let packed = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x90, 0x00];

        assert_eq!(inflate(&packed, 8), Err(ZipError::LargerThanDeclared));
    }

    #[test]
    fn a_stream_cut_short_says_so() {
        let packed = [0xcb, 0x48, 0xcd];

        assert_eq!(inflate(&packed, 17), Err(ZipError::Truncated));
    }

    const MIXED: u32 = 39_330;

    #[test]
    fn long_codes_copies_over_themselves_and_stored_blocks_mid_stream_come_back() -> Result<(), Box<dyn Error>> {
        let packed = include_bytes!("../tests/mixed.deflate");
        let inflated = inflate(packed, MIXED)?;

        assert_eq!(u32::try_from(inflated.len()), Ok(MIXED));
        assert_eq!(console_core_checksums::crc32::of(&inflated), Ok(0xAB1E_EAEB));

        Ok(())
    }

    #[test]
    fn a_stream_cut_anywhere_says_it_was_cut() {
        let packed = include_bytes!("../tests/mixed.deflate");

        for cut in (0..packed.len()).step_by(97) {
            let (short, _) = packed.split_at(cut);

            assert_eq!(inflate(short, MIXED).map(|inflated| inflated.len()), Err(ZipError::Truncated), "cut at {cut}");
        }
    }

    #[test]
    fn a_block_that_carries_its_own_codes_comes_back() -> Result<(), Box<dyn Error>> {
        let packed = include_bytes!("../tests/words.deflate");
        let expected = include_bytes!("../tests/words.txt");
        let size = u32::try_from(expected.len())?;
        let inflated = inflate(packed, size)?;

        assert_eq!(inflated.as_slice(), expected.as_slice());

        Ok(())
    }
}
