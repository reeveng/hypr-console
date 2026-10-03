//! A lossless bitstream undone into the pixels it holds.
//!
//! After a byte saying it is one, fourteen bits each for the width and the
//! height less one, a bit saying whether alpha is used and three bits of
//! version, which is nothing. Then the transforms the encoder applied, each
//! at most once, and the picture they were applied to.
//!
//! A picture is pixels, each a literal colour, a copy of pixels already
//! there, or a colour from a small cache of the ones seen lately. They are
//! coded with groups of five prefix codes: one for green, the length of a
//! copy and an entry in the cache, which share an alphabet, and one each for
//! red, blue, alpha and the distance of a copy. The main picture may be cut
//! into blocks with a group each, which group being said by a smaller
//! picture of its own; the smaller pictures, which also carry what each
//! transform needs, have one group for the whole of them.
//!
//! A distance of up to a hundred and twenty is one of the hundred and twenty
//! pixels nearest above and to the left, so it is a place on the plane, not a
//! count back; the rest are a count back less a hundred and twenty.

use console_core_geometry::Size;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::WebpError;
use console_core_prefix_codes::{Bits, Code, PastTheEnd};

use crate::prefix;
use crate::transforms::{self, Blocks, Palette, Transform};

const SIGNATURE: u8 = 0x2F;

const LITERALS: u32 = 256;

const CODES: u32 = 280;

const DISTANCES: u32 = 40;

const MOST_CACHED: u32 = 11;

const HASH: u32 = 0x1E35_A7BD;

const SIDE: u32 = 0x3FFF;

const PLANE_CODES: u32 = 120;

const GROUP_MOST: u64 = 1 << 15;

const MOST_BYTES: u64 = 1 << 30;

const PLANE: [(i8, u8); 120] = [
    (0, 1), (1, 0), (1, 1), (-1, 1), (0, 2), (2, 0), (1, 2), (-1, 2),
    (2, 1), (-2, 1), (2, 2), (-2, 2), (0, 3), (3, 0), (1, 3), (-1, 3),
    (3, 1), (-3, 1), (2, 3), (-2, 3), (3, 2), (-3, 2), (0, 4), (4, 0),
    (1, 4), (-1, 4), (4, 1), (-4, 1), (3, 3), (-3, 3), (2, 4), (-2, 4),
    (4, 2), (-4, 2), (0, 5), (3, 4), (-3, 4), (4, 3), (-4, 3), (5, 0),
    (1, 5), (-1, 5), (5, 1), (-5, 1), (2, 5), (-2, 5), (5, 2), (-5, 2),
    (4, 4), (-4, 4), (3, 5), (-3, 5), (5, 3), (-5, 3), (0, 6), (6, 0),
    (1, 6), (-1, 6), (6, 1), (-6, 1), (2, 6), (-2, 6), (6, 2), (-6, 2),
    (4, 5), (-4, 5), (5, 4), (-5, 4), (3, 6), (-3, 6), (6, 3), (-6, 3),
    (0, 7), (7, 0), (1, 7), (-1, 7), (5, 5), (-5, 5), (7, 1), (-7, 1),
    (4, 6), (-4, 6), (6, 4), (-6, 4), (2, 7), (-2, 7), (7, 2), (-7, 2),
    (3, 7), (-3, 7), (7, 3), (-7, 3), (5, 6), (-5, 6), (6, 5), (-6, 5),
    (8, 0), (4, 7), (-4, 7), (7, 4), (-7, 4), (8, 1), (8, 2), (6, 6),
    (-6, 6), (8, 3), (5, 7), (-5, 7), (7, 5), (-7, 5), (8, 4), (6, 7),
    (-6, 7), (7, 6), (-7, 6), (8, 5), (7, 7), (-7, 7), (8, 6), (8, 7),
];

struct Group {
    literal: Option<u32>,
    green: Code,
    red: Code,
    blue: Code,
    alpha: Code,
    distance: Code,
}

struct Cache {
    colours: Vec<u32>,
    shift: u32,
}

impl Cache {
    #[inline(always)]
    fn inserted(&mut self, argb: u32) -> Result<(), Never> {
        let Ok(key) = index(argb.wrapping_mul(HASH).wrapping_shr(self.shift));

        match self.colours.get_mut(key) {
            Some(slot) => *slot = argb,
            None => {},
        }

        Ok(())
    }
}

struct Chosen {
    shift: u32,
    columns: u32,
    groups: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Progress {
    Going,
    Finished,
}

struct Decoding<'a, 'b> {
    bits: &'b mut Bits<'a>,
    pixels: Vec<u32>,
    at: u32,
    total: u32,
    width: u32,
    x: u32,
    y: u32,
    mask: u32,
    cache: Cache,
    groups: &'b [Group],
    chosen: Option<&'b Chosen>,
    group: &'b Group,
}

pub(crate) fn header(bitstream: &[u8]) -> Result<Size<u32>, WebpError> {
    let [signature, b0, b1, b2, b3] = match bitstream.first_chunk::<5>() {
        Some(header) => *header,
        None => return Err(WebpError::Truncated),
    };

    let fields = u32::from_le_bytes([b0, b1, b2, b3]);
    let width = (fields & SIDE).saturating_add(1);
    let height = (fields.wrapping_shr(14) & SIDE).saturating_add(1);

    match (signature, fields.wrapping_shr(29)) {
        (SIGNATURE, 0) => Ok(Size { width, height }),
        (_, _) => Err(WebpError::Corrupt),
    }
}

pub(crate) fn decoded(bitstream: &[u8]) -> Result<(Size<u32>, Vec<u32>), WebpError> {
    let size = header(bitstream)?;

    let rest = match bitstream.get(5..) {
        Some(rest) => rest,
        None => return Err(WebpError::Truncated),
    };

    let pixels = headerless(rest, size)?;

    Ok((size, pixels))
}

pub(crate) fn headerless(bitstream: &[u8], size: Size<u32>) -> Result<Vec<u32>, WebpError> {
    let mut bits = Bits::new(bitstream)?;

    match (coded(&mut bits, size), bits.within()) {
        (Ok(pixels), Ok(())) => Ok(pixels),
        (Err(fault), Ok(())) => Err(fault),
        (_, Err(PastTheEnd)) => Err(WebpError::Truncated),
    }
}

fn coded(bits: &mut Bits<'_>, size: Size<u32>) -> Result<Vec<u32>, WebpError> {
    let (transforms, width) = transforms(bits, size)?;
    let mut pixels = spatially_coded(bits, Size { width, height: size.height })?;

    for transform in transforms.iter().rev() {
        let undone = transform.undone(pixels)?;

        pixels = undone;
    }

    Ok(pixels)
}

fn transforms(bits: &mut Bits<'_>, size: Size<u32>) -> Result<(Vec<Transform>, u32), WebpError> {
    let mut transforms = Vec::new();
    let mut width = size.width;
    let mut seen = 0u32;

    for _each_kind_once_and_the_end in 0..5 {
        let more = bits.take(1)?;

        match more {
            1 => {},
            _ => return Ok((transforms, width)),
        }

        let kind = bits.take(2)?;
        let flag = 1u32.wrapping_shl(kind);

        match seen & flag {
            0 => seen |= flag,
            _ => return Err(WebpError::Corrupt),
        }

        let here = Size { width, height: size.height };

        let transform = match kind {
            0 => {
                let blocks = blocks(bits, here)?;

                Transform::Predictor(blocks)
            },
            1 => {
                let blocks = blocks(bits, here)?;

                Transform::Colour(blocks)
            },
            2 => Transform::SubtractGreen,
            _ => {
                let palette = palette(bits, here)?;

                width = here.width.div_ceil(1u32.wrapping_shl(palette.bits));

                Transform::Indexing(palette)
            },
        };

        transforms.push(transform);
    }

    Err(WebpError::Corrupt)
}

fn blocks(bits: &mut Bits<'_>, size: Size<u32>) -> Result<Blocks, WebpError> {
    let shift = bits.take(3)?;
    let shift = shift.saturating_add(2);
    let side = 1u32.wrapping_shl(shift);
    let columns = size.width.div_ceil(side);
    let data = entropy_coded(bits, Size { width: columns, height: size.height.div_ceil(side) })?;

    Ok(Blocks { shift, columns, size, data })
}

fn palette(bits: &mut Bits<'_>, size: Size<u32>) -> Result<Palette, WebpError> {
    let count = bits.take(8)?;
    let count = count.saturating_add(1);
    let mut colours = entropy_coded(bits, Size { width: count, height: 1 })?;

    let mut before = 0u32;

    for colour in &mut colours {
        let Ok(added) = transforms::added((*colour, before));

        *colour = added;
        before = added;
    }

    colours.resize(256, 0);

    let packing = match count {
        0..=2 => 3,
        3..=4 => 2,
        5..=16 => 1,
        17.. => 0,
    };

    Ok(Palette { bits: packing, size, colours })
}

fn cache_bits(bits: &mut Bits<'_>) -> Result<u32, WebpError> {
    let cached = bits.take(1)?;

    match cached {
        1 => {},
        _ => return Ok(0),
    }

    let cached = bits.take(4)?;

    match cached {
        1..=MOST_CACHED => Ok(cached),
        _ => Err(WebpError::Corrupt),
    }
}

fn spatially_coded(bits: &mut Bits<'_>, size: Size<u32>) -> Result<Vec<u32>, WebpError> {
    let cached = cache_bits(bits)?;

    let entropy = bits.take(1)?;

    let chosen = match entropy {
        1 => {
            let chosen = chosen(bits, size)?;

            Some(chosen)
        },
        _ => None,
    };

    let most = match &chosen {
        Some(chosen) => chosen.groups.iter().copied().max(),
        None => None,
    };

    let count = match most {
        Some(most) => most.saturating_add(1),
        None => 1,
    };

    let area = u64::from(size.width).saturating_mul(u64::from(size.height)).saturating_mul(4);

    match u64::from(count).saturating_mul(GROUP_MOST).saturating_add(area) <= MOST_BYTES {
        true => {},
        false => return Err(WebpError::TooLarge),
    }

    let mut groups = Vec::new();

    for _group in 0..count {
        let group = group(bits, cached)?;

        groups.push(group);
    }

    pixels(bits, size, cached, (&groups, chosen.as_ref()))
}

fn chosen(bits: &mut Bits<'_>, size: Size<u32>) -> Result<Chosen, WebpError> {
    let shift = bits.take(3)?;
    let shift = shift.saturating_add(2);
    let side = 1u32.wrapping_shl(shift);
    let columns = size.width.div_ceil(side);
    let mut groups = entropy_coded(bits, Size { width: columns, height: size.height.div_ceil(side) })?;

    for group in &mut groups {
        *group = group.wrapping_shr(8) & 0xFFFF;
    }

    Ok(Chosen { shift, columns, groups })
}

fn entropy_coded(bits: &mut Bits<'_>, size: Size<u32>) -> Result<Vec<u32>, WebpError> {
    let cached = cache_bits(bits)?;
    let group = group(bits, cached)?;

    pixels(bits, size, cached, (&[group], None))
}

fn group(bits: &mut Bits<'_>, cached: u32) -> Result<Group, WebpError> {
    let cache = match cached {
        0 => 0,
        _ => 1u32.wrapping_shl(cached),
    };

    let green = prefix::read(bits, CODES.saturating_add(cache))?;
    let red = prefix::read(bits, LITERALS)?;
    let blue = prefix::read(bits, LITERALS)?;
    let alpha = prefix::read(bits, LITERALS)?;
    let distance = prefix::read(bits, DISTANCES)?;
    let Ok(literal) = literal((&red, &blue, &alpha));

    Ok(Group { literal, green, red, blue, alpha, distance })
}

fn literal(codes: (&Code, &Code, &Code)) -> Result<Option<u32>, Never> {
    let (red, blue, alpha) = codes;
    let Ok(red) = red.only();
    let Ok(blue) = blue.only();
    let Ok(alpha) = alpha.only();

    Ok(match (red, blue, alpha) {
        (Some(red), Some(blue), Some(alpha)) => Some(u32::from(alpha).wrapping_shl(24) | u32::from(red).wrapping_shl(16) | u32::from(blue)),
        (_, _, _) => None,
    })
}

fn pixels(bits: &mut Bits<'_>, size: Size<u32>, cached: u32, coded: (&[Group], Option<&Chosen>)) -> Result<Vec<u32>, WebpError> {
    let (groups, chosen) = coded;

    let first = match groups.first() {
        Some(first) => first,
        None => return Err(WebpError::Corrupt),
    };

    let total = size.width.saturating_mul(size.height);
    let Ok(room) = index(total);
    let Ok(entries) = index(1u32.wrapping_shl(cached));

    let colours = match cached {
        0 => Vec::new(),
        1.. => vec![0; entries],
    };

    let mask = match chosen {
        Some(chosen) => 1u32.wrapping_shl(chosen.shift).wrapping_sub(1),
        None => u32::MAX,
    };

    let mut decoding = Decoding {
        bits,
        pixels: Vec::with_capacity(room),
        at: 0,
        total,
        width: size.width,
        x: 0,
        y: 0,
        mask,
        cache: Cache { colours, shift: 32u32.saturating_sub(cached) },
        groups,
        chosen,
        group: first,
    };

    let group = decoding.grouped()?;

    decoding.group = group;

    let decoded = iterate(&mut decoding, |decoding| {
        Ok(match decoding.step() {
            Ok(Progress::Going) => Step::Again(decoding),
            Ok(Progress::Finished) => Step::Halt(Ok(())),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    match decoded {
        Ok(decoded) => decoded?,
        Err(Endless) => return Err(WebpError::Corrupt),
    }

    decoding.bits.within()?;

    Ok(decoding.pixels)
}

impl<'b> Decoding<'_, 'b> {
    #[inline(always)]
    fn step(&mut self) -> Result<Progress, WebpError> {
        match self.at < self.total {
            true => {},
            false => return Ok(Progress::Finished),
        }

        let group = self.group;
        let Ok(()) = self.bits.fill();
        let green = group.green.decoded(self.bits)?;
        let green = u32::from(green);

        match (green, group.literal) {
            (0..LITERALS, Some(literal)) => self.pushed(literal | green.wrapping_shl(8))?,
            (0..LITERALS, None) => {
                let red = group.red.decoded(self.bits)?;
                let red = u32::from(red);
                let blue = group.blue.decoded(self.bits)?;
                let blue = u32::from(blue);
                let Ok(()) = self.bits.fill();
                let alpha = group.alpha.decoded(self.bits)?;
                let alpha = u32::from(alpha);
                let argb = alpha.wrapping_shl(24) | red.wrapping_shl(16) | green.wrapping_shl(8) | blue;

                self.pushed(argb)?;
            },
            (LITERALS..CODES, _) => {
                let Ok(long) = extra(self.bits, green.saturating_sub(LITERALS));
                let Ok(()) = self.bits.fill();
                let code = group.distance.decoded(self.bits)?;
                let Ok(code) = extra(self.bits, u32::from(code));
                let Ok(back) = distance((code, self.width));

                self.copied((back, long))?;
            },
            (CODES.., _) => {
                let Ok(key) = index(green.saturating_sub(CODES));

                let argb = match self.cache.colours.get(key) {
                    Some(argb) => *argb,
                    None => return Err(WebpError::Corrupt),
                };

                self.pushed(argb)?;
            },
        }

        Ok(Progress::Going)
    }

    #[inline(always)]
    fn pushed(&mut self, argb: u32) -> Result<(), WebpError> {
        self.pixels.push(argb);

        let Ok(()) = self.cache.inserted(argb);

        self.at = self.at.saturating_add(1);
        self.x = self.x.saturating_add(1);

        match self.x == self.width {
            true => {
                self.x = 0;
                self.y = self.y.saturating_add(1);
                self.bits.within()?;
            },
            false => {},
        }

        match (self.x & self.mask, self.at < self.total) {
            (0, true) => {
                let group = self.grouped()?;

                self.group = group;
            },
            (_, _) => {},
        }

        Ok(())
    }

    fn copied(&mut self, reference: (u32, u32)) -> Result<(), WebpError> {
        let (back, long) = reference;

        let from = match self.at.checked_sub(back) {
            Some(from) => from,
            None => return Err(WebpError::Corrupt),
        };

        let past = match self.at.checked_add(long) {
            Some(past) => past,
            None => return Err(WebpError::Corrupt),
        };

        match past <= self.total {
            true => {},
            false => return Err(WebpError::Corrupt),
        }

        let Ok(start) = index(from);
        let Ok(end) = index(from.saturating_add(long));
        let Ok(here) = index(self.at);

        match (back < long, back) {
            (false, _) => self.pixels.extend_from_within(start..end),
            (true, 1) => {
                let last = match self.pixels.last() {
                    Some(last) => *last,
                    None => return Err(WebpError::Corrupt),
                };

                let Ok(room) = index(past);

                self.pixels.resize(room, last);
            },
            (true, _) => {
                for step in start..end {
                    let argb = match self.pixels.get(step) {
                        Some(argb) => *argb,
                        None => return Err(WebpError::Corrupt),
                    };

                    self.pixels.push(argb);
                }
            },
        }

        match (self.cache.colours.is_empty(), self.pixels.get(here..)) {
            (false, Some(copied)) => {
                for argb in copied {
                    let Ok(()) = self.cache.inserted(*argb);
                }
            },
            (true, _) | (_, None) => {},
        }

        let along = self.x.saturating_add(long);

        let (rows, x) = match (along.checked_div(self.width), along.checked_rem(self.width)) {
            (Some(rows), Some(x)) => (rows, x),
            (None, _) | (_, None) => return Err(WebpError::Corrupt),
        };

        self.at = past;
        self.y = self.y.saturating_add(rows);
        self.x = x;

        match self.at < self.total {
            true => {
                let group = self.grouped()?;

                self.group = group;
            },
            false => {},
        }

        Ok(())
    }

    #[inline(always)]
    fn grouped(&self) -> Result<&'b Group, WebpError> {
        let named = match self.chosen {
            Some(chosen) => {
                let row = self.y.wrapping_shr(chosen.shift).saturating_mul(chosen.columns);
                let Ok(at) = index(row.saturating_add(self.x.wrapping_shr(chosen.shift)));

                match chosen.groups.get(at) {
                    Some(named) => *named,
                    None => return Err(WebpError::Corrupt),
                }
            },
            None => 0,
        };

        let Ok(named) = index(named);

        match self.groups.get(named) {
            Some(group) => Ok(group),
            None => Err(WebpError::Corrupt),
        }
    }
}

#[inline(always)]
fn extra(bits: &mut Bits<'_>, code: u32) -> Result<u32, Never> {
    match code {
        0..4 => Ok(code.saturating_add(1)),
        4.. => {
            let more = code.saturating_sub(2).wrapping_shr(1);
            let offset = (code & 1).saturating_add(2).wrapping_shl(more);

            let Ok(more) = bits.held(more);

            Ok(offset.saturating_add(more).saturating_add(1))
        },
    }
}

fn distance(coded: (u32, u32)) -> Result<u32, Never> {
    let (code, width) = coded;

    match code.checked_sub(PLANE_CODES.saturating_add(1)) {
        Some(beyond) => Ok(beyond.saturating_add(1)),
        None => {
            let Ok(at) = index(code.saturating_sub(1));

            let (sideways, rows) = match PLANE.get(at) {
                Some(place) => *place,
                None => (1, 0),
            };

            let back = i64::from(rows).saturating_mul(i64::from(width)).saturating_add(i64::from(sideways));
            let Ok(back) = fitted::<i64, u32>(back.max(1));

            Ok(back)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_places_on_the_plane_are_above_and_to_the_left() {
        assert_eq!(distance((1, 100)), Ok(100));
        assert_eq!(distance((2, 100)), Ok(1));
        assert_eq!(distance((4, 100)), Ok(99));
        assert_eq!(distance((4, 1)), Ok(1));
        assert_eq!(distance((121, 100)), Ok(1));
        assert_eq!(distance((130, 100)), Ok(10));
    }

    #[test]
    fn the_header_says_the_size_and_that_it_is_lossless() {
        let fourteen_by_three = [SIGNATURE, 13, 0x80, 0, 0];

        assert_eq!(header(&fourteen_by_three), Ok(Size { width: 14, height: 3 }));
        assert_eq!(header(&[0x2E, 0, 0, 0, 0]), Err(WebpError::Corrupt));
        assert_eq!(header(&[SIGNATURE, 0, 0, 0, 0x20]), Err(WebpError::Corrupt));
    }
}
