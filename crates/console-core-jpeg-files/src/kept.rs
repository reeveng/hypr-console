//! The frequencies a block keeps at the size it is drawn, and where each goes.
//!
//! A block drawn n pixels a side needs only its n lowest frequencies each way,
//! so of the sixty-four it is read with, one is kept at an eighth of the size
//! and sixteen at half. Each kept frequency has a slot of its own, in the order
//! it sits in the block, and a block is read straight into its slots: nothing
//! is held that will not be drawn, and nothing has to be found again when it
//! is. Past the last kept frequency the rest of a block is only read through,
//! to find where the next one starts.
//!
//! The slots of a block are its kept frequencies a row at a time, as many to a
//! row as the block is drawn wide, which is the order the inverse DCT reads
//! them in. A slot holds the sixteen bits the file gave it. Its quantization
//! step is multiplied in when it is drawn, by slot, so the step is looked up
//! once per table rather than once per frequency, and the product is kept in
//! sixteen bits, as libjpeg-turbo keeps it.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub(crate) const NATURAL: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14,
    21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60,
    61, 54, 47, 55, 62, 63,
];

pub(crate) const UNKEPT: u8 = u8::MAX;

pub(crate) const DETAILED: u16 = 1 << 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Retained {
    pub(crate) count: u32,
    pub(crate) reach: u32,
    pub(crate) slots: [u8; 64],
    pub(crate) marks: [u16; 64],
    pub(crate) zigzags: [u8; 64],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Prepared {
    pub(crate) kept: Retained,
    pub(crate) steps: [i16; 64],
}

pub(crate) fn kept(drawn: Size<u32>) -> Result<Retained, Never> {
    let mut kept = Retained { count: 0, reach: 0, slots: [UNKEPT; 64], marks: [0; 64], zigzags: [0; 64] };

    for (zigzag, natural) in (0u32..).zip(NATURAL) {
        let across = u32::from(natural & 7);
        let down = u32::from(natural.wrapping_shr(3));

        match (across < drawn.width, down < drawn.height) {
            (true, true) => {
                let Ok(()) = placed(&mut kept, (zigzag, natural), down.saturating_mul(drawn.width).saturating_add(across));
            },
            (_, _) => {},
        }
    }

    Ok(kept)
}

fn placed(kept: &mut Retained, frequency: (u32, u8), slot: u32) -> Result<(), Never> {
    let (zigzag, natural) = frequency;
    let Ok(at) = index(zigzag);
    let Ok(place) = index(slot);
    let Ok(short_slot) = fitted::<u32, u8>(slot);
    let Ok(short_zigzag) = fitted::<u32, u8>(zigzag);

    let detail = match zigzag {
        0 => 0,
        _ => DETAILED,
    };

    match (kept.slots.get_mut(at), kept.marks.get_mut(at)) {
        (Some(slot), Some(mark)) => {
            *slot = short_slot;
            *mark = 1u16.wrapping_shl(u32::from(natural & 7)) | detail;
        },
        (_, _) => {},
    }

    match kept.zigzags.get_mut(place) {
        Some(frequency) => *frequency = short_zigzag,
        None => {},
    }

    kept.count = kept.count.saturating_add(1);
    kept.reach = zigzag;

    Ok(())
}

pub(crate) fn prepared(quantization: &[u16; 64], drawn: Size<u32>) -> Result<Prepared, Never> {
    let Ok(kept) = kept(drawn);
    let Ok(count) = index(kept.count);
    let mut steps = [0i16; 64];

    for (step, zigzag) in steps.iter_mut().zip(&kept.zigzags).take(count) {
        let Ok(at) = index(*zigzag);

        *step = match quantization.get(at) {
            Some(step) => i16::from_le_bytes(step.to_le_bytes()),
            None => 0,
        };
    }

    Ok(Prepared { kept, steps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_eighth_keeps_the_average_alone() {
        let Ok(kept) = kept(Size { width: 1, height: 1 });

        assert_eq!((kept.count, kept.reach), (1, 0));
        assert_eq!(kept.slots.iter().filter(|slot| **slot != UNKEPT).count(), 1);
    }

    #[test]
    fn half_keeps_the_four_lowest_each_way_in_the_order_they_sit() {
        let Ok(kept) = kept(Size { width: 4, height: 4 });
        let zigzags: Vec<u8> = kept.zigzags.iter().take(16).copied().collect();

        assert_eq!(kept.count, 16);
        assert_eq!(zigzags, [0, 1, 5, 6, 2, 4, 7, 13, 3, 8, 12, 17, 9, 11, 18, 24]);
        assert_eq!(kept.reach, 24);
    }

    #[test]
    fn a_frequency_is_marked_with_its_column_and_whether_it_is_more_than_the_average() {
        let Ok(kept) = kept(Size { width: 8, height: 8 });

        assert_eq!(kept.marks.first(), Some(&1));
        assert_eq!(kept.marks.get(1), Some(&(2 | DETAILED)));
        assert_eq!(kept.marks.get(2), Some(&(1 | DETAILED)));
    }
}
