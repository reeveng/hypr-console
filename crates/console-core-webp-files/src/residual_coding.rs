//! A block's levels walked as the tokens that send them.
//!
//! The mirror of `lossy`'s reading: a level is a run down a tree, nought or
//! not, one or more, two to four or more, and so on out to the six categories
//! whose extra bits are sent with fixed chances, then its sign at even chances.
//! Every bit the tree asks for is weighed against the chance the table holds
//! for its place -- the kind of block, the band the position falls in, what
//! the token before it was, and which branch of the tree -- and a block ends
//! early with a token that says nothing more is coming.
//!
//! The walk is written once and spent three ways, which is what a bit is for
//! here: costed while a macroblock's modes are chosen, counted when every
//! macroblock is chosen so that the table sent can be the one the picture
//! wants, and written. Three walks would be three chances to disagree about a
//! tree that has to come out bit for bit the same each time.

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::boolean_encoder::{BooleanEncoder, Costs};
use crate::lossy::{BANDS, LARGEST};

pub(crate) type Chances = [[[[u8; 11]; 3]; 8]; 4];

pub(crate) type Counts = [[[[[u32; 2]; 11]; 3]; 8]; 4];

const EVEN: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Place {
    kind: u32,
    band: u32,
    context: u32,
    branch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Weighed {
    Table(Place),
    Fixed(u8),
}

pub(crate) trait Spending {
    fn spent(&mut self, bit: (u32, Weighed), chances: &Chances) -> Result<(), Never>;
}

pub(crate) struct Costing<'a> {
    pub(crate) costs: &'a Costs,
    pub(crate) total: u32,
}

pub(crate) struct Counting<'a> {
    pub(crate) counts: &'a mut Counts,
}

pub(crate) struct Writing<'a> {
    pub(crate) encoder: &'a mut BooleanEncoder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Coded<'a> {
    pub(crate) kind: u32,
    pub(crate) first: u32,
    pub(crate) context: u32,
    pub(crate) levels: &'a [i16; 16],
}

pub(crate) fn chance_at(chances: &Chances, weighed: Weighed) -> Result<u8, Never> {
    let place = match weighed {
        Weighed::Table(place) => place,
        Weighed::Fixed(chance) => return Ok(chance),
    };

    let Ok(kind) = index(place.kind);
    let Ok(band) = index(place.band);
    let Ok(context) = index(place.context);
    let Ok(branch) = index(place.branch);

    Ok(match chances.get(kind).and_then(|bands| bands.get(band)).and_then(|contexts| contexts.get(context)).and_then(|branches| branches.get(branch)) {
        Some(chance) => *chance,
        None => EVEN,
    })
}

impl Spending for Costing<'_> {
    fn spent(&mut self, bit: (u32, Weighed), chances: &Chances) -> Result<(), Never> {
        let (value, weighed) = bit;
        let Ok(chance) = chance_at(chances, weighed);
        let Ok(cost) = self.costs.of((value, chance));

        self.total = self.total.saturating_add(cost);

        Ok(())
    }
}

impl Spending for Counting<'_> {
    fn spent(&mut self, bit: (u32, Weighed), _chances: &Chances) -> Result<(), Never> {
        let (value, weighed) = bit;

        let place = match weighed {
            Weighed::Table(place) => place,
            Weighed::Fixed(_) => return Ok(()),
        };

        let Ok(kind) = index(place.kind);
        let Ok(band) = index(place.band);
        let Ok(context) = index(place.context);
        let Ok(branch) = index(place.branch);
        let Ok(which) = index(value.min(1));
        let found = self.counts.get_mut(kind).and_then(|bands| bands.get_mut(band)).and_then(|contexts| contexts.get_mut(context)).and_then(|branches| branches.get_mut(branch)).and_then(|both| both.get_mut(which));

        match found {
            Some(count) => *count = count.saturating_add(1),
            None => {},
        }

        Ok(())
    }
}

impl Spending for Writing<'_> {
    fn spent(&mut self, bit: (u32, Weighed), chances: &Chances) -> Result<(), Never> {
        let (value, weighed) = bit;
        let Ok(chance) = chance_at(chances, weighed);

        self.encoder.bit((value, chance))
    }
}

fn band_of(position: u32) -> Result<u32, Never> {
    let Ok(at) = index(position);

    Ok(match BANDS.get(at) {
        Some(band) => u32::from(*band),
        None => 7,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Standing {
    position: u32,
    context: u32,
}

fn at(block: &Coded<'_>, standing: Standing) -> Result<impl Fn(u32) -> Weighed, Never> {
    let Standing { position, context } = standing;
    let Ok(band) = band_of(position);
    let kind = block.kind;

    Ok(move |branch| Weighed::Table(Place { kind, band, context, branch }))
}

pub(crate) fn walked(spending: &mut dyn Spending, block: Coded<'_>, chances: &Chances) -> Result<u32, Never> {
    let last = (0u32..).zip(block.levels).skip_while(|(position, _)| *position < block.first).filter(|(_, level)| **level != 0).map(|(position, _)| position).last();
    let Ok(starting) = at(&block, Standing { position: block.first, context: block.context });

    let last = match last {
        Some(last) => last,
        None => {
            let Ok(()) = spending.spent((0, starting(0)), chances);

            return Ok(0);
        },
    };

    let Ok(()) = spending.spent((1, starting(0)), chances);
    let mut context = block.context;

    for (position, level) in (0u32..).zip(block.levels).skip_while(|(position, _)| *position < block.first) {
        let Ok(here) = at(&block, Standing { position, context });
        let size = level.unsigned_abs();

        let Ok(()) = spending.spent((u32::from(size != 0), here(1)), chances);

        match size {
            0 => {
                context = 0;

                continue;
            },
            1 => {
                let Ok(()) = spending.spent((0, here(2)), chances);

                context = 1;
            },
            _ => {
                let Ok(()) = spending.spent((1, here(2)), chances);
                let Ok(()) = larger(spending, (u32::from(size), &here), chances);

                context = 2;
            },
        }

        let Ok(()) = spending.spent((u32::from(*level < 0), Weighed::Fixed(EVEN)), chances);
        let next = position.wrapping_add(1);

        match next {
            16 => return Ok(1),
            _ => {},
        }

        let Ok(following) = at(&block, Standing { position: next, context });
        let Ok(()) = spending.spent((u32::from(next <= last), following(0)), chances);

        match next <= last {
            true => {},
            false => return Ok(1),
        }
    }

    Ok(1)
}

fn larger(spending: &mut dyn Spending, level: (u32, &dyn Fn(u32) -> Weighed), chances: &Chances) -> Result<(), Never> {
    let (size, here) = level;

    let Ok(()) = spending.spent((u32::from(size > 4), here(3)), chances);

    match size {
        2..=4 => {
            let Ok(()) = spending.spent((u32::from(size != 2), here(4)), chances);

            return match size {
                2 => Ok(()),
                _ => spending.spent((u32::from(size == 4), here(5)), chances),
            };
        },
        _ => {},
    }

    let Ok(()) = spending.spent((u32::from(size > 10), here(6)), chances);

    match size {
        5..=6 => {
            let Ok(()) = spending.spent((0, here(7)), chances);

            return spending.spent((u32::from(size == 6), Weighed::Fixed(159)), chances);
        },
        7..=10 => {
            let Ok(()) = spending.spent((1, here(7)), chances);
            let Ok(()) = spending.spent((u32::from(size >= 9), Weighed::Fixed(165)), chances);

            return spending.spent((u32::from(size & 1 == 0), Weighed::Fixed(145)), chances);
        },
        _ => {},
    }

    let category = match size {
        ..=18 => 0u32,
        19..=34 => 1,
        35..=66 => 2,
        _ => 3,
    };

    let high = category.wrapping_shr(1);
    let Ok(()) = spending.spent((high, here(8)), chances);

    let Ok(()) = match high {
        0 => spending.spent((category & 1, here(9)), chances),
        _ => spending.spent((category & 1, here(10)), chances),
    };

    let extra = size.wrapping_sub(3).wrapping_sub(8u32.wrapping_shl(category));
    let Ok(at) = index(category);
    let extra_chances: &[u8] = match LARGEST.get(at) {
        Some(chances) => chances,
        None => &[],
    };
    let Ok(bits) = u32::try_from(extra_chances.len()).map_or(Ok::<u32, Never>(0), Ok);

    for (chance, bit) in extra_chances.iter().zip((0..bits).rev()) {
        let Ok(()) = spending.spent((extra.wrapping_shr(bit) & 1, Weighed::Fixed(*chance)), chances);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_of_noughts_is_one_token_and_a_full_one_ends_at_its_last_level() -> Result<(), Never> {
        let costs = Costs::new()?;
        let chances = crate::chances::COEFFICIENTS;
        let mut empty = Costing { costs: &costs, total: 0 };
        let mut full = Costing { costs: &costs, total: 0 };
        let mut count = Counts::default();
        let levels = [3, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let Ok(nothing) = walked(&mut empty, Coded { kind: 3, first: 0, context: 0, levels: &[0; 16] }, &chances);
        let Ok(something) = walked(&mut full, Coded { kind: 3, first: 0, context: 0, levels: &levels }, &chances);
        let Ok(_counted) = walked(&mut Counting { counts: &mut count }, Coded { kind: 3, first: 0, context: 0, levels: &levels }, &chances);

        assert_eq!((nothing, something), (0, 1));
        assert!(empty.total < full.total);
        assert_eq!(count.get(3).and_then(|kind| kind.first()).and_then(|band| band.first()).and_then(|context| context.first()), Some(&[0, 1]));

        Ok(())
    }
}
