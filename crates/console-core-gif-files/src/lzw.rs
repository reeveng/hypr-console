//! A frame's codes undone into the palette indices they stand for.
//!
//! LZW starts from a table with one entry for each index and two more: one
//! that clears the table and one that ends the frame. Every code read after
//! the first adds an entry, the string the code before it stood for and the
//! first index of its own, so the table is never sent; it is rebuilt as the
//! codes arrive. A code is as wide as the table needs, one bit more each time
//! the table reaches the next power of two, and never more than twelve. A
//! table that is full stays as it is until a code clears it.
//!
//! Every string an entry stands for has already been written out: it is the
//! string the code before it wrote, and the index after that, which is the
//! first of the next. So an entry is where in what has been written its
//! string is and how long it is, and a code is one copy from earlier in the
//! frame. The frame is its width times its height in indices, and what the
//! codes say past that is not read.

use console_core_geometry::Size;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::GifError;

const WIDEST: u32 = 12;

const ENTRIES: u32 = 1 << WIDEST;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Written {
    start: u32,
    long: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Code {
    Read(u32),
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Meaning {
    Clear,
    End,
    Known(u32),
    Next,
    Beyond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Progress {
    Going,
    Finished,
}

struct Codes<'a> {
    bytes: &'a [u8],
    at: u32,
    held: u32,
    count: u32,
}

impl Codes<'_> {
    #[inline(always)]
    fn read(&mut self, wide: u32) -> Result<Code, Never> {
        for _byte in 0..wide.saturating_sub(self.count).div_ceil(8) {
            let Ok(at) = index(self.at);

            let byte = match self.bytes.get(at) {
                Some(byte) => *byte,
                None => return Ok(Code::Ended),
            };

            self.held |= u32::from(byte).wrapping_shl(self.count);
            self.count = self.count.saturating_add(8);
            self.at = self.at.saturating_add(1);
        }

        let code = self.held & 1u32.wrapping_shl(wide).wrapping_sub(1);

        self.held = self.held.wrapping_shr(wide);
        self.count = self.count.saturating_sub(wide);

        Ok(Code::Read(code))
    }
}

struct Unpacking<'a> {
    codes: Codes<'a>,
    entries: Vec<Written>,
    out: Vec<u8>,
    had: u32,
    covered: u32,
    clear: u32,
    next: u32,
    wide: u32,
    before: Option<Written>,
}

pub(crate) fn unpacked(packed: &[u8], least: u32, frame: Size<u32>) -> Result<Vec<u8>, GifError> {
    let covered = frame.width.saturating_mul(frame.height);

    match least {
        1..=8 => {},
        _ => return Err(GifError::Corrupt),
    }

    let Ok(all) = index(ENTRIES);
    let Ok(room) = index(covered);
    let Ok(spare) = index(covered.saturating_add(ENTRIES));
    let codes = Codes { bytes: packed, at: 0, held: 0, count: 0 };
    let entries = vec![Written { start: 0, long: 0 }; all];
    let clear = 1u32.wrapping_shl(least);
    let out = Vec::with_capacity(spare);

    let mut unpacking = Unpacking { codes, entries, out, had: 0, covered, clear, next: 0, wide: 0, before: None };
    let Ok(()) = unpacking.cleared();

    let unpacked = iterate(&mut unpacking, |unpacking| {
        Ok(match unpacking.step() {
            Ok(Progress::Going) => Step::Again(unpacking),
            Ok(Progress::Finished) => Step::Halt(Ok(())),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    match unpacked {
        Ok(unpacked) => unpacked?,
        Err(Endless) => return Err(GifError::Corrupt),
    }

    let mut out = unpacking.out;

    match unpacking.had >= covered {
        true => {
            out.truncate(room);

            Ok(out)
        },
        false => Err(GifError::Truncated),
    }
}

impl Unpacking<'_> {
    fn cleared(&mut self) -> Result<(), Never> {
        self.next = self.clear.saturating_add(2);
        self.wide = self.clear.trailing_zeros().saturating_add(1);
        self.before = None;

        Ok(())
    }

    #[inline(always)]
    fn step(&mut self) -> Result<Progress, GifError> {
        match self.had >= self.covered {
            true => return Ok(Progress::Finished),
            false => {},
        }

        let code = match self.codes.read(self.wide) {
            Ok(Code::Read(code)) => code,
            Ok(Code::Ended) => return Ok(Progress::Finished),
        };

        let Ok(meaning) = self.meaning(code);

        let here = match (meaning, self.before) {
            (Meaning::Clear, _) => {
                let Ok(()) = self.cleared();

                return Ok(Progress::Going);
            },
            (Meaning::End, _) => return Ok(Progress::Finished),
            (Meaning::Known(code), before) => {
                let here = self.written(code)?;
                let Ok(()) = self.added(before);

                here
            },
            (Meaning::Next, Some(before)) => {
                let here = self.repeated(before)?;
                let Ok(()) = self.added(Some(before));

                here
            },
            (Meaning::Next, None) | (Meaning::Beyond, _) => return Err(GifError::Corrupt),
        };

        self.before = Some(here);

        Ok(Progress::Going)
    }

    #[inline(always)]
    fn meaning(&self, code: u32) -> Result<Meaning, Never> {
        Ok(match (code.checked_sub(self.clear), code.cmp(&self.next)) {
            (Some(0), _) => Meaning::Clear,
            (Some(1), _) => Meaning::End,
            (None | Some(2..), std::cmp::Ordering::Less) => Meaning::Known(code),
            (None | Some(2..), std::cmp::Ordering::Equal) => Meaning::Next,
            (None | Some(2..), std::cmp::Ordering::Greater) => Meaning::Beyond,
        })
    }

    #[inline(always)]
    fn written(&mut self, code: u32) -> Result<Written, GifError> {
        let start = self.had;

        match (code < self.clear, u8::try_from(code)) {
            (true, Ok(root)) => {
                self.out.push(root);
                self.had = self.had.saturating_add(1);

                return Ok(Written { start, long: 1 });
            },
            (true, Err(_past_a_byte)) => return Err(GifError::Corrupt),
            (false, _) => {},
        }

        let Ok(at) = index(code);

        let entry = match self.entries.get(at) {
            Some(entry) => *entry,
            None => return Err(GifError::Corrupt),
        };

        let Ok(from) = index(entry.start);
        let Ok(past) = index(entry.start.saturating_add(entry.long));

        self.out.extend_from_within(from..past);
        self.had = self.had.saturating_add(entry.long);

        Ok(Written { start, long: entry.long })
    }

    fn repeated(&mut self, before: Written) -> Result<Written, GifError> {
        let start = self.had;
        let Ok(from) = index(before.start);
        let Ok(past) = index(before.start.saturating_add(before.long));

        let first = match self.out.get(from) {
            Some(first) => *first,
            None => return Err(GifError::Corrupt),
        };

        self.out.extend_from_within(from..past);
        self.out.push(first);

        let long = before.long.saturating_add(1);

        self.had = self.had.saturating_add(long);

        Ok(Written { start, long })
    }

    #[inline(always)]
    fn added(&mut self, before: Option<Written>) -> Result<(), Never> {
        let Ok(at) = index(self.next);

        match (before, self.entries.get_mut(at)) {
            (Some(before), Some(slot)) => *slot = Written { start: before.start, long: before.long.saturating_add(1) },
            (None, _) | (_, None) => return Ok(()),
        }

        self.next = self.next.saturating_add(1);

        match (self.next == 1u32.wrapping_shl(self.wide), self.wide < WIDEST) {
            (true, true) => self.wide = self.wide.saturating_add(1),
            (_, _) => {},
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_the_table_has_not_reached_is_damage() {
        let clear_then_seven = [0b0011_1100];

        assert_eq!(unpacked(&clear_then_seven, 2, Size { width: 4, height: 1 }), Err(GifError::Corrupt));
    }

    #[test]
    fn codes_that_end_before_the_frame_does_are_the_frame_cut_short() {
        let clear_then_one_index = [0b0000_0100];

        assert_eq!(unpacked(&clear_then_one_index, 2, Size { width: 10, height: 10 }), Err(GifError::Truncated));
    }

    #[test]
    fn what_the_codes_say_past_the_frame_is_not_read() -> Result<(), GifError> {
        let clear_one_two_three = [0b1000_1100, 0b0000_0110];

        let unpacked = unpacked(&clear_one_two_three, 2, Size { width: 2, height: 1 })?;

        assert_eq!(unpacked, [1, 2]);

        Ok(())
    }
}
