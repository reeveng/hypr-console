//! The prefix codes a lossless WebP is written in, read from the stream.
//!
//! A code is sent as the length of each symbol's code, and the codes are the
//! canonical ones those lengths make, as in deflate. The lengths come one of
//! two ways. A simple code names one or two symbols, which then take one bit
//! each, or none at all when there is one. A normal code sends the lengths
//! themselves, coded with a small prefix code of their own whose lengths come
//! first, three bits each, in an order that puts the rarest last so they can
//! be left off: zero to fifteen are a length, sixteen repeats the last length
//! that was not zero three to six times, and seventeen and eighteen are a run
//! of zeros. It may also say how many of those it sends, and the symbols past
//! them are unused.
//!
//! A code with one symbol takes no bits to read. Any other code has to be
//! complete, every string of bits the start of exactly one code, and a code
//! that is not is damage. Building and undoing the codes is
//! `console-core-prefix-codes`, which deflate shares.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_prefix_codes::{Bits, Code};

use crate::WebpError;

const ORDER: [u8; 19] = [17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

const FIRST_LENGTH: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Repeat {
    length: u8,
    times: u32,
}

pub(crate) fn read(bits: &mut Bits<'_>, alphabet: u32) -> Result<Code, WebpError> {
    let Ok(symbols) = index(alphabet);
    let mut lengths = vec![0u8; symbols];
    let kind = bits.take(1)?;

    match kind {
        1 => simple(bits, &mut lengths)?,
        _ => normal(bits, (&mut lengths, alphabet))?,
    }

    let code = Code::complete(&lengths)?;

    Ok(code)
}

fn simple(bits: &mut Bits<'_>, lengths: &mut [u8]) -> Result<(), WebpError> {
    let two = bits.take(1)?;
    let eight_bits = bits.take(1)?;
    let first = bits.take(eight_bits.saturating_mul(7).saturating_add(1))?;
    let Ok(()) = marked(lengths, first);

    match two {
        1 => {
            let second = bits.take(8)?;
            let Ok(()) = marked(lengths, second);
        },
        _ => {},
    }

    Ok(())
}

fn marked(lengths: &mut [u8], symbol: u32) -> Result<(), Never> {
    let Ok(at) = index(symbol);

    match lengths.get_mut(at) {
        Some(length) => *length = 1,
        None => {},
    }

    Ok(())
}

fn normal(bits: &mut Bits<'_>, lengths: (&mut [u8], u32)) -> Result<(), WebpError> {
    let (lengths, alphabet) = lengths;
    let sent = bits.take(4)?;
    let Ok(sent) = index(sent.saturating_add(4));
    let mut short = [0u8; 19];

    for at in ORDER.iter().take(sent) {
        let length = bits.take(3)?;
        let Ok(at) = index(*at);

        match short.get_mut(at) {
            Some(slot) => {
                let Ok(length) = fitted::<u32, u8>(length);

                *slot = length;
            },
            None => return Err(WebpError::Corrupt),
        }
    }

    let short = Code::complete(&short)?;
    let limited = bits.take(1)?;

    let most = match limited {
        1 => {
            let long = bits.take(3)?;
            let most = bits.take(long.saturating_mul(2).saturating_add(2))?;

            most.saturating_add(2)
        },
        _ => alphabet,
    };

    match most <= alphabet {
        true => {},
        false => return Err(WebpError::Corrupt),
    }

    let mut previous = FIRST_LENGTH;
    let mut filled = 0u32;

    for _sent in 0..most {
        match filled < alphabet {
            true => {},
            false => return Ok(()),
        }

        let Ok(()) = bits.fill();
        let symbol = short.decoded(bits)?;
        let repeat = repeated(bits, symbol, previous)?;
        let past = filled.saturating_add(repeat.times);

        match past <= alphabet {
            true => {},
            false => return Err(WebpError::Corrupt),
        }

        let Ok(from) = index(filled);
        let Ok(to) = index(past);

        match lengths.get_mut(from..to) {
            Some(run) => run.fill(repeat.length),
            None => return Err(WebpError::Corrupt),
        }

        previous = match (symbol, repeat.length) {
            (0..=15, 1..) => repeat.length,
            (_, _) => previous,
        };

        filled = past;
    }

    Ok(())
}

fn repeated(bits: &mut Bits<'_>, symbol: u16, previous: u8) -> Result<Repeat, WebpError> {
    Ok(match symbol {
        0..=15 => {
            let Ok(length) = fitted::<u16, u8>(symbol);

            Repeat { length, times: 1 }
        },
        16 => {
            let Ok(more) = bits.held(2);

            Repeat { length: previous, times: more.saturating_add(3) }
        },
        17 => {
            let Ok(more) = bits.held(3);

            Repeat { length: 0, times: more.saturating_add(3) }
        },
        18 => {
            let Ok(more) = bits.held(7);

            Repeat { length: 0, times: more.saturating_add(11) }
        },
        _ => return Err(WebpError::Corrupt),
    })
}
