//! A Huffman table, and a symbol read out of the bits with it.
//!
//! A table arrives as how many codes there are of each length and the symbols
//! in order, and the codes themselves are implied: canonical, counting up
//! from zero and doubling at each length. Nearly every symbol in a photograph
//! is coded in ten bits or fewer, so those are found by looking the next ten
//! bits up in a table of a thousand and twenty-four, and only a longer code is
//! read the way the standard describes, a length at a time.
//!
//! An AC symbol is followed by the bits of its value, and most of the time the
//! code and those bits are ten together, so a second table says for each ten
//! bits the whole step they spell: how far along the block it moves, what the
//! coefficient it lands on is, and how many bits it took. That is one look
//! where there would be two reads and a sign to work out, for nearly every
//! coefficient there is. Sixteen zeros and the end of a block are steps like
//! any other, one sixteen along with nothing to put down and the other past the
//! end, so reading a block never asks which kind of symbol it was given.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::JpegError;
use crate::bits::{Bits, Extra, signed};

const QUICK: u32 = 10;

const LONGEST: u32 = 16;

const ZERO_RUN: u32 = 15;

const SIXTEEN: u8 = 16;

const PAST_THE_END: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Step {
    pub(crate) advance: u32,
    pub(crate) value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fast {
    Slow,
    Known { length: u8, advance: u8, value: i16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quick {
    Longer,
    Code { length: u8, symbol: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    first: u32,
    last: u32,
    at: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Huffman {
    quick: Box<[Quick; 1024]>,
    fast: Box<[Fast; 1024]>,
    runs: [Option<Run>; 17],
    symbols: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Coded<'a> {
    length: u32,
    symbols: &'a [u8],
}

pub(crate) fn huffman(counts: &[u8; 16], symbols: &[u8]) -> Result<Huffman, JpegError> {
    let mut quick = Box::new([Quick::Longer; 1024]);
    let mut runs = [None; 17];
    let mut code = 0u32;
    let mut at = 0u32;

    for (length, counted) in (1u32..).zip(counts) {
        let counted = u32::from(*counted);
        let last = code.saturating_add(counted).saturating_sub(1);
        let run = Run { first: code, last, at };

        match (counted, last < 1u32.wrapping_shl(length)) {
            (0, _) => {},
            (_, false) => return Err(JpegError::Corrupt),
            (_, true) => {
                let Ok(slot) = index(length);

                match runs.get_mut(slot) {
                    Some(slot) => *slot = Some(run),
                    None => return Err(JpegError::Corrupt),
                }

                match length <= QUICK {
                    true => quickly(quick.as_mut_slice(), run, Coded { length, symbols })?,
                    false => {},
                }
            },
        }

        at = at.saturating_add(counted);
        code = code.saturating_add(counted).wrapping_shl(1);
    }

    let Ok(kept) = index(at);

    let Ok(fast) = fast(&quick);

    match symbols.get(..kept) {
        Some(kept) => Ok(Huffman { quick, fast, runs, symbols: kept.to_vec() }),
        None => Err(JpegError::Truncated),
    }
}

fn fast(quick: &[Quick; 1024]) -> Result<Box<[Fast; 1024]>, Never> {
    let mut fast = Box::new([Fast::Slow; 1024]);

    for ((at, entry), step) in (0u32..).zip(quick).zip(fast.iter_mut()) {
        *step = match entry {
            Quick::Code { length, symbol } => {
                let Ok(step) = stepped(at, (u32::from(*length), *symbol));

                step
            },
            Quick::Longer => Fast::Slow,
        };
    }

    Ok(fast)
}

fn stepped(at: u32, code: (u32, u8)) -> Result<Fast, Never> {
    let (length, symbol) = code;
    let size = u32::from(symbol & 0x0F);
    let whole = length.saturating_add(size);
    let Ok(short) = fitted::<u32, u8>(length);

    Ok(match (symbol.wrapping_shr(4), size, whole <= QUICK) {
        (0x0F, 0, _) => Fast::Known { length: short, advance: SIXTEEN, value: 0 },
        (_, 0, _) => Fast::Known { length: short, advance: PAST_THE_END, value: 0 },
        (run, 1.., true) => {
            let bits = at.wrapping_shr(QUICK.saturating_sub(whole)) & 1u32.wrapping_shl(size).wrapping_sub(1);
            let Ok(value) = signed(Extra { bits, count: size });
            let Ok(value) = fitted::<i32, i16>(value);
            let Ok(length) = fitted::<u32, u8>(whole);

            Fast::Known { length, advance: run.saturating_add(1), value }
        },
        (_, _, _) => Fast::Slow,
    })
}

fn quickly(quick: &mut [Quick], run: Run, coded: Coded<'_>) -> Result<(), JpegError> {
    let spread = QUICK.saturating_sub(coded.length);
    let Ok(length) = fitted::<u32, u8>(coded.length);
    let Ok(from) = index(run.at);
    let Ok(past) = index(run.at.saturating_add(run.last.saturating_sub(run.first)).saturating_add(1));

    let symbols = match coded.symbols.get(from..past) {
        Some(symbols) => symbols,
        None => return Err(JpegError::Truncated),
    };

    for (code, symbol) in (run.first..).zip(symbols) {
        let Ok(start) = index(code.wrapping_shl(spread));
        let Ok(end) = index(code.saturating_add(1).wrapping_shl(spread));

        match quick.get_mut(start..end) {
            Some(slots) => slots.fill(Quick::Code { length, symbol: *symbol }),
            None => return Err(JpegError::Corrupt),
        }
    }

    Ok(())
}

impl Huffman {
    #[inline(always)]
    pub(crate) fn ac(&self, bits: &mut Bits<'_>) -> Result<Step, JpegError> {
        let Ok(peeked) = bits.peek();
        let Ok(at) = index(peeked.wrapping_shr(LONGEST.saturating_sub(QUICK)));

        match self.fast.get(at) {
            Some(Fast::Known { length, advance, value }) => {
                let Ok(()) = bits.skip(u32::from(*length));

                Ok(Step { advance: u32::from(*advance), value: i32::from(*value) })
            },
            Some(Fast::Slow) | None => {
                let (after, step) = self.slowly(*bits)?;

                *bits = after;

                Ok(step)
            },
        }
    }

    #[inline(never)]
    fn slowly<'b>(&self, bits: Bits<'b>) -> Result<(Bits<'b>, Step), JpegError> {
        let mut bits = bits;
        let symbol = self.decode(&mut bits)?;
        let run = u32::from(symbol.wrapping_shr(4));
        let size = u32::from(symbol & 0x0F);

        let step = match (run, size) {
            (ZERO_RUN, 0) => Step { advance: u32::from(SIXTEEN), value: 0 },
            (_, 0) => Step { advance: u32::from(PAST_THE_END), value: 0 },
            (_, _) => {
                let Ok(value) = bits.extended(size);

                Step { advance: run.saturating_add(1), value }
            },
        };

        Ok((bits, step))
    }

    #[inline(always)]
    pub(crate) fn decode(&self, bits: &mut Bits<'_>) -> Result<u8, JpegError> {
        let Ok(peeked) = bits.peek();
        let Ok(at) = index(peeked.wrapping_shr(LONGEST.saturating_sub(QUICK)));

        match self.quick.get(at) {
            Some(Quick::Code { length, symbol }) => {
                let Ok(()) = bits.skip(u32::from(*length));

                Ok(*symbol)
            },
            Some(Quick::Longer) | None => {
                let (length, symbol) = self.longer(peeked)?;
                let Ok(()) = bits.skip(length);

                Ok(symbol)
            },
        }
    }

    #[inline(never)]
    fn longer(&self, peeked: u32) -> Result<(u32, u8), JpegError> {
        for (length, run) in (0u32..).zip(&self.runs).skip(10) {
            let code = peeked.wrapping_shr(LONGEST.saturating_sub(length));

            let found = match run {
                Some(run) => match (code <= run.last, code.checked_sub(run.first)) {
                    (true, Some(past)) => Some(run.at.saturating_add(past)),
                    (true, None) => return Err(JpegError::Corrupt),
                    (false, _) => None,
                },
                None => None,
            };

            match found {
                Some(at) => {
                    let Ok(at) = index(at);

                    return match self.symbols.get(at) {
                        Some(symbol) => Ok((length, *symbol)),
                        None => Err(JpegError::Corrupt),
                    };
                },
                None => {},
            }
        }

        Err(JpegError::Corrupt)
    }
}
