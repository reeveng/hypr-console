//! Deflate, undone: RFC 1951 read back into the bytes it was made from.
//!
//! This is the whole of what a zip file on this device asks for. A book is a
//! zip of pages, a comic is a zip of pictures, and every one of them is stored
//! or deflated and nothing else; the other methods the format names are ones
//! no book has been written in. So this is the decoder zlib's own `puff` is,
//! said again in this tree's words: canonical codes built from their lengths,
//! read a bit at a time, with nothing in it that is fast and nothing in it that
//! is clever. A page of a book is tens of kilobytes and is inflated once, when
//! it is opened, and a table-driven decoder is a larger thing to trust for a
//! difference nobody holding the device would see.
//!
//! What it will not do is make more than it was told it would. A zip says how
//! large each thing in it is, and a stream that runs past that is either
//! damaged or written to fill the disk, so the size said is the most that is
//! ever made.

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::ZipError;

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];

const LENGTH_EXTRA: [u8; 29] =
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];

const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

const LENGTHS_IN_ORDER: [u16; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

const END_OF_BLOCK: u16 = 256;

const FIRST_LENGTH: u16 = 257;

struct BitReader<'a> {
    bytes: &'a [u8],
    at: u32,
    buffer: u32,
    buffered: u32,
}

impl BitReader<'_> {
    fn read_bits(&mut self, count: u32) -> Result<u32, ZipError> {
        while self.buffered < count {
            let Ok(at) = index(self.at);

            let byte = match self.bytes.get(at) {
                Some(byte) => *byte,
                None => return Err(ZipError::Truncated),
            };

            self.buffer |= u32::from(byte).wrapping_shl(self.buffered);
            self.buffered = self.buffered.saturating_add(8);
            self.at = self.at.saturating_add(1);
        }

        let value = self.buffer & 1u32.wrapping_shl(count).wrapping_sub(1);

        self.buffer = self.buffer.wrapping_shr(count);
        self.buffered = self.buffered.saturating_sub(count);

        Ok(value)
    }

    fn align_to_byte(&mut self) -> Result<(), Never> {
        self.buffer = 0;
        self.buffered = 0;

        Ok(())
    }

    fn read_u16(&mut self) -> Result<u16, ZipError> {
        let Ok(at) = index(self.at);
        let Ok(past) = index(self.at.saturating_add(2));

        let taken = match self.bytes.get(at..past) {
            Some(taken) => taken,
            None => return Err(ZipError::Truncated),
        };

        self.at = self.at.saturating_add(2);

        match <[u8; 2]>::try_from(taken) {
            Ok(taken) => Ok(u16::from_le_bytes(taken)),
            Err(_fault) => Err(ZipError::Truncated),
        }
    }
}

struct BackReference {
    distance: u32,
    length: u32,
}

struct Output {
    bytes: Vec<u8>,
    written: u32,
    most: u32,
}

impl Output {
    fn push(&mut self, byte: u8) -> Result<(), ZipError> {
        match self.written < self.most {
            true => {},
            false => return Err(ZipError::LargerThanDeclared),
        }

        self.bytes.push(byte);
        self.written = self.written.saturating_add(1);

        Ok(())
    }

    fn copy_back(&mut self, reference: BackReference) -> Result<(), ZipError> {
        let BackReference { distance: back, length: long } = reference;

        let from = match self.written.checked_sub(back) {
            Some(from) => from,
            None => return Err(ZipError::Corrupt),
        };

        for step in 0..long {
            let Ok(at) = index(from.saturating_add(step));

            let byte = match self.bytes.get(at) {
                Some(byte) => *byte,
                None => return Err(ZipError::Corrupt),
            };

            self.push(byte)?;
        }

        Ok(())
    }
}

struct Huffman {
    count: [u16; 16],
    symbol: Vec<u16>,
}

fn huffman(lengths: &[u16]) -> Result<Huffman, ZipError> {
    let mut count = [0u16; 16];

    for length in lengths {
        let Ok(at) = index(*length);

        match count.get_mut(at) {
            Some(counted) => *counted = counted.saturating_add(1),
            None => return Err(ZipError::Corrupt),
        }
    }

    let mut next = [0u16; 16];
    let mut running = 0u16;

    for (offset, counted) in next.iter_mut().zip(count.iter()).skip(1) {
        *offset = running;
        running = running.saturating_add(*counted);
    }

    let mut symbol = vec![0u16; lengths.len()];

    for (named, length) in (0u16..).zip(lengths) {
        let Ok(at) = index(*length);

        match (*length, next.get_mut(at)) {
            (0, _) => {},
            (_, Some(offset)) => {
                let Ok(slot) = index(*offset);

                match symbol.get_mut(slot) {
                    Some(slot) => *slot = named,
                    None => return Err(ZipError::Corrupt),
                }

                *offset = offset.saturating_add(1);
            },
            (_, None) => return Err(ZipError::Corrupt),
        }
    }

    Ok(Huffman { count, symbol })
}

fn decode_symbol(bits: &mut BitReader<'_>, code: &Huffman) -> Result<u16, ZipError> {
    let mut read = 0u32;
    let mut first = 0u32;
    let mut passed = 0u32;

    for buffered in code.count.iter().skip(1) {
        let bit = bits.read_bits(1)?;

        read |= bit;

        let buffered = u32::from(*buffered);
        let past = first.saturating_add(buffered);

        match read < past {
            true => {
                let Ok(slot) = index(passed.saturating_add(read.saturating_sub(first)));

                return match code.symbol.get(slot) {
                    Some(symbol) => Ok(*symbol),
                    None => Err(ZipError::Corrupt),
                };
            },
            false => {},
        }

        passed = passed.saturating_add(buffered);
        first = past.wrapping_shl(1);
        read = read.wrapping_shl(1);
    }

    Err(ZipError::Corrupt)
}

fn read_extra(bits: &mut BitReader<'_>, bases: &[u16], extras: &[u8], at: u16) -> Result<u32, ZipError> {
    let Ok(at) = index(at);

    let (base, extra) = match (bases.get(at), extras.get(at)) {
        (Some(base), Some(extra)) => (*base, *extra),
        (None, _) | (_, None) => return Err(ZipError::Corrupt),
    };

    let more = bits.read_bits(u32::from(extra))?;

    Ok(u32::from(base).saturating_add(more))
}

struct Tables {
    lengths: Huffman,
    distances: Huffman,
}

fn compressed_block(bits: &mut BitReader<'_>, output: &mut Output, codes: &Tables) -> Result<(), ZipError> {
    loop {
        let symbol = decode_symbol(bits, &codes.lengths)?;

        match (symbol, u8::try_from(symbol)) {
            (_, Ok(byte)) => output.push(byte)?,
            (END_OF_BLOCK, Err(_)) => return Ok(()),
            (_, Err(_)) => {
                let long = read_extra(bits, &LENGTH_BASE, &LENGTH_EXTRA, symbol.saturating_sub(FIRST_LENGTH))?;
                let near = decode_symbol(bits, &codes.distances)?;
                let back = read_extra(bits, &DISTANCE_BASE, &DISTANCE_EXTRA, near)?;

                output.copy_back(BackReference { distance: back, length: long })?;
            },
        }
    }
}

fn stored_block(bits: &mut BitReader<'_>, output: &mut Output) -> Result<(), ZipError> {
    let Ok(()) = bits.align_to_byte();
    let long = bits.read_u16()?;
    let check = bits.read_u16()?;

    match long == !check {
        true => {},
        false => return Err(ZipError::Corrupt),
    }

    let Ok(from) = index(bits.at);
    let Ok(past) = index(bits.at.saturating_add(u32::from(long)));

    let taken = match bits.bytes.get(from..past) {
        Some(taken) => taken,
        None => return Err(ZipError::Truncated),
    };

    for byte in taken {
        output.push(*byte)?;
    }

    bits.at = bits.at.saturating_add(u32::from(long));

    Ok(())
}

fn fixed_tables() -> Result<Tables, ZipError> {
    let lengths: Vec<u16> = (0u16..288)
        .map(|symbol| match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        })
        .collect();

    let lengths = huffman(&lengths)?;
    let distances = huffman(&[5u16; 30])?;

    Ok(Tables { lengths, distances })
}

fn push_repeated(lengths: &mut Vec<u16>, value: u16, times: u32) -> Result<(), Never> {
    for _ in 0..times {
        lengths.push(value);
    }

    Ok(())
}

fn read_code_lengths(bits: &mut BitReader<'_>, all: u32) -> Result<Vec<u16>, ZipError> {
    let buffered = bits.read_bits(4)?;
    let mut short = [0u16; 19];

    let Ok(told) = index(buffered.saturating_add(4));

    for order in LENGTHS_IN_ORDER.iter().take(told) {
        let length = bits.read_bits(3)?;
        let Ok(at) = index(*order);

        match (short.get_mut(at), u16::try_from(length)) {
            (Some(buffer), Ok(length)) => *buffer = length,
            (None, _) | (_, Err(_)) => return Err(ZipError::Corrupt),
        }
    }

    let short = huffman(&short)?;
    let mut lengths: Vec<u16> = Vec::new();
    let mut had = 0u32;

    while had < all {
        let symbol = decode_symbol(bits, &short)?;

        let (value, times) = match symbol {
            0..=15 => (symbol, 1),
            16 => match lengths.last() {
                Some(last) => {
                    let more = bits.read_bits(2)?;

                    (*last, more.saturating_add(3))
                },
                None => return Err(ZipError::Corrupt),
            },
            17 => {
                let more = bits.read_bits(3)?;

                (0, more.saturating_add(3))
            },
            18 => {
                let more = bits.read_bits(7)?;

                (0, more.saturating_add(11))
            },
            _ => return Err(ZipError::Corrupt),
        };

        let Ok(()) = push_repeated(&mut lengths, value, times);

        had = had.saturating_add(times);
    }

    match had == all {
        true => Ok(lengths),
        false => Err(ZipError::Corrupt),
    }
}

fn dynamic_tables(bits: &mut BitReader<'_>) -> Result<Tables, ZipError> {
    let literal = bits.read_bits(5)?;
    let literal = literal.saturating_add(u32::from(FIRST_LENGTH));
    let distance = bits.read_bits(5)?;
    let distance = distance.saturating_add(1);

    let lengths = read_code_lengths(bits, literal.saturating_add(distance))?;
    let Ok(split) = index(literal);

    let (literals, distances) = match lengths.split_at_checked(split) {
        Some(both) => both,
        None => return Err(ZipError::Corrupt),
    };

    let lengths = huffman(literals)?;
    let distances = huffman(distances)?;

    Ok(Tables { lengths, distances })
}

const DEFLATE_EXPANDS_AT_MOST: u32 = 1032;

pub fn inflate(packed: &[u8], size: u32) -> Result<Vec<u8>, ZipError> {
    let mut bits = BitReader { bytes: packed, at: 0, buffer: 0, buffered: 0 };
    let long = match u32::try_from(packed.len()) {
        Ok(long) => long,
        Err(_fault) => return Err(ZipError::TooLarge),
    };

    let Ok(room) = index(size.min(long.saturating_mul(DEFLATE_EXPANDS_AT_MOST)));
    let mut output = Output { bytes: Vec::with_capacity(room), written: 0, most: size };

    loop {
        let last = bits.read_bits(1)?;
        let kind = bits.read_bits(2)?;

        match kind {
            0 => stored_block(&mut bits, &mut output)?,
            1 => {
                let codes = fixed_tables()?;

                compressed_block(&mut bits, &mut output, &codes)?;
            },
            2 => {
                let codes = dynamic_tables(&mut bits)?;

                compressed_block(&mut bits, &mut output, &codes)?;
            },
            _ => return Err(ZipError::Corrupt),
        }

        match last {
            1 => return Ok(output.bytes),
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_block_is_its_own_bytes() {
        let packed = [0x01, 0x05, 0x00, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o'];

        assert_eq!(inflate(&packed, 5).ok(), Some(b"hello".to_vec()));
    }

    #[test]
    fn a_fixed_block_with_a_repeat_in_it_comes_back() {
        let packed = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x90, 0x00];

        assert_eq!(inflate(&packed, 17).ok(), Some(b"hello hello hello".to_vec()));
    }

    #[test]
    fn a_stream_that_runs_past_its_size_is_refused_rather_than_followed() {
        let packed = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x90, 0x00];

        assert_eq!(inflate(&packed, 8).err(), Some(ZipError::LargerThanDeclared));
    }

    #[test]
    fn a_stream_cut_short_says_so() {
        let packed = [0xcb, 0x48, 0xcd];

        assert_eq!(inflate(&packed, 17).err(), Some(ZipError::Truncated));
    }

    #[test]
    fn a_block_that_carries_its_own_codes_comes_back() {
        let packed = include_bytes!("../tests/words.deflate");
        let expected = include_bytes!("../tests/words.txt");
        let size = u32::try_from(expected.len()).map_err(|_| ZipError::TooLarge);
        let inflated = size.and_then(|size| inflate(packed, size));

        assert_eq!(inflated.ok().as_deref(), Some(expected.as_slice()));
    }
}
