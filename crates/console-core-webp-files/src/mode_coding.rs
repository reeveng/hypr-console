//! How a macroblock says the way it was predicted.
//!
//! The mirror of `lossy`'s mode reading, as trees of bits at fixed chances:
//! whole or split, then which of the four whole modes, or each subblock's one
//! of ten at chances that depend on the modes above it and to its left, then
//! the chroma mode. Like the tokens these are walked rather than written, so
//! that what a mode costs while it is being chosen is what writing it spends.

use console_core_never::Never;

use crate::prediction::{Sub, Whole};
use crate::residual_coding::{Chances, Spending, Weighed};

const SPLIT: u8 = 145;

pub(crate) fn whole_walked(spending: &mut dyn Spending, mode: Whole, chances: &Chances) -> Result<(), Never> {
    let Ok(()) = spending.spent((1, Weighed::Fixed(SPLIT)), chances);

    match mode {
        Whole::Average | Whole::Vertical => {
            let Ok(()) = spending.spent((0, Weighed::Fixed(156)), chances);

            spending.spent((u32::from(mode == Whole::Vertical), Weighed::Fixed(163)), chances)
        },
        Whole::Horizontal | Whole::TrueMotion => {
            let Ok(()) = spending.spent((1, Weighed::Fixed(156)), chances);

            spending.spent((u32::from(mode == Whole::TrueMotion), Weighed::Fixed(128)), chances)
        },
    }
}

pub(crate) fn split_walked(spending: &mut dyn Spending, chances: &Chances) -> Result<(), Never> {
    spending.spent((0, Weighed::Fixed(SPLIT)), chances)
}

pub(crate) fn chroma_walked(spending: &mut dyn Spending, mode: Whole, chances: &Chances) -> Result<(), Never> {
    let Ok(()) = spending.spent((u32::from(mode != Whole::Average), Weighed::Fixed(142)), chances);

    match mode {
        Whole::Average => Ok(()),
        Whole::Vertical => spending.spent((0, Weighed::Fixed(114)), chances),
        Whole::Horizontal | Whole::TrueMotion => {
            let Ok(()) = spending.spent((1, Weighed::Fixed(114)), chances);

            spending.spent((u32::from(mode == Whole::TrueMotion), Weighed::Fixed(183)), chances)
        },
    }
}

pub(crate) fn sub_walked(spending: &mut dyn Spending, mode: (Sub, &[u8; 9]), chances: &Chances) -> Result<(), Never> {
    let (mode, [average, true_motion, vertical, split, horizontal, down_right, down_left, vertical_left, horizontal_down]) = mode;
    let on = |bit: bool, chance: &u8| (u32::from(bit), Weighed::Fixed(*chance));

    let Ok(()) = spending.spent(on(mode != Sub::Average, average), chances);

    match mode {
        Sub::Average => return Ok(()),
        Sub::TrueMotion | Sub::Vertical | Sub::Horizontal | Sub::DownLeft | Sub::DownRight | Sub::VerticalRight | Sub::VerticalLeft | Sub::HorizontalDown | Sub::HorizontalUp => {},
    }

    let Ok(()) = spending.spent(on(mode != Sub::TrueMotion, true_motion), chances);

    match mode {
        Sub::TrueMotion => return Ok(()),
        Sub::Average | Sub::Vertical | Sub::Horizontal | Sub::DownLeft | Sub::DownRight | Sub::VerticalRight | Sub::VerticalLeft | Sub::HorizontalDown | Sub::HorizontalUp => {},
    }

    let Ok(()) = spending.spent(on(mode != Sub::Vertical, vertical), chances);

    match mode {
        Sub::Vertical | Sub::Average | Sub::TrueMotion => Ok(()),
        Sub::Horizontal | Sub::DownRight | Sub::VerticalRight => {
            let Ok(()) = spending.spent(on(false, split), chances);
            let Ok(()) = spending.spent(on(mode != Sub::Horizontal, horizontal), chances);

            match mode {
                Sub::Horizontal => Ok(()),
                Sub::Average | Sub::TrueMotion | Sub::Vertical | Sub::DownLeft | Sub::DownRight | Sub::VerticalRight | Sub::VerticalLeft | Sub::HorizontalDown | Sub::HorizontalUp => {
                    spending.spent(on(mode == Sub::VerticalRight, down_right), chances)
                },
            }
        },
        Sub::DownLeft | Sub::VerticalLeft | Sub::HorizontalDown | Sub::HorizontalUp => {
            let Ok(()) = spending.spent(on(true, split), chances);
            let Ok(()) = spending.spent(on(mode != Sub::DownLeft, down_left), chances);

            match mode {
                Sub::DownLeft => return Ok(()),
                Sub::Average | Sub::TrueMotion | Sub::Vertical | Sub::Horizontal | Sub::DownRight | Sub::VerticalRight | Sub::VerticalLeft | Sub::HorizontalDown | Sub::HorizontalUp => {},
            }

            let Ok(()) = spending.spent(on(mode != Sub::VerticalLeft, vertical_left), chances);

            match mode {
                Sub::VerticalLeft => Ok(()),
                Sub::Average | Sub::TrueMotion | Sub::Vertical | Sub::Horizontal | Sub::DownLeft | Sub::DownRight | Sub::VerticalRight | Sub::HorizontalDown | Sub::HorizontalUp => {
                    spending.spent(on(mode == Sub::HorizontalUp, horizontal_down), chances)
                },
            }
        },
    }
}
