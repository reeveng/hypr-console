//! The seams between blocks smoothed over once a band of them is drawn.
//!
//! A picture coded in blocks shows them, so VP8 smooths across every edge of
//! every macroblock and, in a macroblock that had anything sent for it or was
//! predicted in subblocks, across the edges between its four by four blocks
//! too. Each edge is looked at a line of eight pixels at a time, four on each
//! side: the line is left alone when the step across the edge is larger than
//! the smoothing's limit, which is a real edge in the picture, or when either
//! side is busier than its interior limit. The normal filter then moves up to
//! three pixels each side at a macroblock edge and two inside one, or only
//! the two nearest when either side changes fast; the simple filter moves the
//! two nearest, and only in luma.
//!
//! Macroblocks are smoothed in the order they were decoded, each its left
//! edge, the edges inside it running down, its top edge, and the edges inside
//! it running across, because every one reads pixels the ones before it
//! moved. The arithmetic is libwebp's, clamps and all.
//!
//! A band comes with the four lines above its top edge, which the edge reads
//! and moves three of, and luma is smoothed apart from chroma, since neither
//! reads the other.
//!
//! A macroblock's edges are smoothed together rather than one at a time. The
//! lines across its edges running down are copied out once, four pixels to a
//! word, and each goes through every one of those edges in a single pass,
//! which the compiler does for all of the lines at once since none reads
//! another; the edges running across are the same, done down its columns. An
//! edge at a time copied every line in and out once per edge and spent more on
//! the copying than on the arithmetic. The filters and the shapes of a strip
//! are types rather than closures, because a closure handed down was compiled
//! as a call per line and nothing ran side by side.
//!
//! When only part of the picture is wanted, the macroblocks right of it are
//! left alone: a macroblock's edges move its own pixels and the three columns
//! left of it, and nothing in the part reads further right. Every macroblock
//! left of the part is smoothed, since each edge reads what the one before it
//! moved.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Filter {
    Off,
    Simple,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Strength {
    pub(crate) limit: i32,
    pub(crate) interior: i32,
    pub(crate) variance: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Smoothing {
    pub(crate) strength: Strength,
    pub(crate) inner: u32,
}

pub(crate) struct Plane<'a> {
    pub(crate) pixels: &'a mut [u8],
    pub(crate) stride: u32,
}

pub(crate) enum Smoothed<'a> {
    Luma(Plane<'a>),
    Chroma(Plane<'a>, Plane<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Place {
    column: u32,
    row: u32,
    line: u32,
}

type Group = [i16; 4];

pub(crate) fn filtered(layer: &mut Smoothed<'_>, smoothing: (Filter, &[Smoothing], (u32, u32)), rows: (u32, u32)) -> Result<(), Never> {
    let (filter, smoothings, columns) = smoothing;

    match (filter, &*layer) {
        (Filter::Off, _) | (Filter::Simple, Smoothed::Chroma(_, _)) => Ok(()),
        (Filter::Simple, Smoothed::Luma(_)) => every::<Simple, Simple>(layer, (smoothings, columns), rows),
        (Filter::Normal, _) => every::<Outer, Inner>(layer, (smoothings, columns), rows),
    }
}

fn every<E: Seam, W: Seam>(layer: &mut Smoothed<'_>, smoothing: (&[Smoothing], (u32, u32)), rows: (u32, u32)) -> Result<(), Never> {
    let (smoothings, (columns, reach)) = smoothing;
    let (first, above) = rows;

    let side = match layer {
        Smoothed::Luma(_) => 16u32,
        Smoothed::Chroma(_, _) => 8,
    };

    let Ok(wide) = index(columns);
    let Ok(reach) = index(reach);

    for (row, line) in (first..).zip(smoothings.chunks(wide.max(1))) {
        let top = row.wrapping_sub(first).wrapping_mul(side).wrapping_add(above);

        for (column, smoothing) in (0u32..).zip(line).take(reach) {
            let Ok(()) = macroblock::<E, W>(layer, *smoothing, Place { column, row, line: top });
        }
    }

    Ok(())
}

fn macroblock<E: Seam, W: Seam>(layer: &mut Smoothed<'_>, smoothing: Smoothing, at: Place) -> Result<(), Never> {
    let Smoothing { strength, inner } = smoothing;
    let Place { column, row, line } = at;

    match strength.limit {
        0 => return Ok(()),
        _ => {},
    }

    let Ok(outside) = limits(strength, 4);
    let Ok(inside) = limits(strength, 0);
    let limits = (outside, inside);

    match layer {
        Smoothed::Luma(luma) => {
            let left = column.wrapping_mul(16);

            let Ok(()) = match (column, inner) {
                (0, 0) => Ok(()),
                (0, _) => vertical::<4, LumaInside, E, W>(luma, (left, line), limits),
                (_, 0) => vertical::<2, EdgeOnly, E, W>(luma, (left.wrapping_sub(4), line), limits),
                (_, _) => vertical::<5, LumaEvery, E, W>(luma, (left.wrapping_sub(4), line), limits),
            };

            match (row, inner) {
                (0, 0) => Ok(()),
                (0, _) => horizontal::<4, LumaInside, E, W>(luma, (left, line), limits),
                (_, 0) => horizontal::<2, EdgeOnly, E, W>(luma, (left, line.wrapping_sub(4)), limits),
                (_, _) => horizontal::<5, LumaEvery, E, W>(luma, (left, line.wrapping_sub(4)), limits),
            }
        },
        Smoothed::Chroma(blue, red) => {
            let left = column.wrapping_mul(8);

            let Ok(()) = match (column, inner) {
                (0, 0) => Ok(()),
                (0, _) => vertical_both::<2, InsideOnly, E, W>((blue, red), (left, line), limits),
                (_, 0) => vertical_both::<2, EdgeOnly, E, W>((blue, red), (left.wrapping_sub(4), line), limits),
                (_, _) => vertical_both::<3, ChromaEvery, E, W>((blue, red), (left.wrapping_sub(4), line), limits),
            };

            match (row, inner) {
                (0, 0) => Ok(()),
                (0, _) => horizontal_both::<2, InsideOnly, E, W>((blue, red), (left, line), limits),
                (_, 0) => horizontal_both::<2, EdgeOnly, E, W>((blue, red), (left, line.wrapping_sub(4)), limits),
                (_, _) => horizontal_both::<3, ChromaEvery, E, W>((blue, red), (left, line.wrapping_sub(4)), limits),
            }
        },
    }
}

trait Seam {
    fn smoothed(pixels: [i16; 8], limits: Limits) -> Result<[i16; 8], Never>;
}

enum Simple {}

enum Inner {}

enum Outer {}

impl Seam for Simple {
    #[inline(always)]
    fn smoothed(pixels: [i16; 8], limits: Limits) -> Result<[i16; 8], Never> {
        simple(pixels, limits)
    }
}

impl Seam for Inner {
    #[inline(always)]
    fn smoothed(pixels: [i16; 8], limits: Limits) -> Result<[i16; 8], Never> {
        inner(pixels, limits)
    }
}

impl Seam for Outer {
    #[inline(always)]
    fn smoothed(pixels: [i16; 8], limits: Limits) -> Result<[i16; 8], Never> {
        outer(pixels, limits)
    }
}

trait Seams<const G: usize> {
    fn smoothed<E: Seam, W: Seam>(groups: [Group; G], limits: (Limits, Limits)) -> Result<[Group; G], Never>;
}

enum EdgeOnly {}

enum InsideOnly {}

enum LumaInside {}

enum LumaEvery {}

enum ChromaEvery {}

impl Seams<2> for EdgeOnly {
    #[inline(always)]
    fn smoothed<E: Seam, W: Seam>(groups: [Group; 2], limits: (Limits, Limits)) -> Result<[Group; 2], Never> {
        let [a, b] = groups;
        let (outside, _) = limits;
        let Ok((a, b)) = seam::<E>((a, b), outside);

        Ok([a, b])
    }
}

impl Seams<2> for InsideOnly {
    #[inline(always)]
    fn smoothed<E: Seam, W: Seam>(groups: [Group; 2], limits: (Limits, Limits)) -> Result<[Group; 2], Never> {
        let [a, b] = groups;
        let (_, inside) = limits;
        let Ok((a, b)) = seam::<W>((a, b), inside);

        Ok([a, b])
    }
}

impl Seams<4> for LumaInside {
    #[inline(always)]
    fn smoothed<E: Seam, W: Seam>(groups: [Group; 4], limits: (Limits, Limits)) -> Result<[Group; 4], Never> {
        let [a, b, c, d] = groups;
        let (_, inside) = limits;
        let Ok((a, b)) = seam::<W>((a, b), inside);
        let Ok((b, c)) = seam::<W>((b, c), inside);
        let Ok((c, d)) = seam::<W>((c, d), inside);

        Ok([a, b, c, d])
    }
}

impl Seams<5> for LumaEvery {
    #[inline(always)]
    fn smoothed<E: Seam, W: Seam>(groups: [Group; 5], limits: (Limits, Limits)) -> Result<[Group; 5], Never> {
        let [a, b, c, d, e] = groups;
        let (outside, inside) = limits;
        let Ok((a, b)) = seam::<E>((a, b), outside);
        let Ok((b, c)) = seam::<W>((b, c), inside);
        let Ok((c, d)) = seam::<W>((c, d), inside);
        let Ok((d, e)) = seam::<W>((d, e), inside);

        Ok([a, b, c, d, e])
    }
}

impl Seams<3> for ChromaEvery {
    #[inline(always)]
    fn smoothed<E: Seam, W: Seam>(groups: [Group; 3], limits: (Limits, Limits)) -> Result<[Group; 3], Never> {
        let [a, b, c] = groups;
        let (outside, inside) = limits;
        let Ok((a, b)) = seam::<E>((a, b), outside);
        let Ok((b, c)) = seam::<W>((b, c), inside);

        Ok([a, b, c])
    }
}

fn vertical<const G: usize, C: Seams<G>, E: Seam, W: Seam>(plane: &mut Plane<'_>, at: (u32, u32), limits: (Limits, Limits)) -> Result<(), Never> {
    let (left, top) = at;
    let Ok(stride) = index(plane.stride);
    let Ok(top) = index(top);

    let mut words = [[0u32; 16]; G];
    let Ok(()) = gathered(&mut words, (plane.pixels.chunks_exact(stride.max(1)).skip(top).take(16), left));
    let Ok(()) = by_rows::<G, C, E, W>(&mut words, limits);

    scattered(&words, (plane.pixels.chunks_exact_mut(stride.max(1)).skip(top).take(16), left))
}

fn vertical_both<'p, const G: usize, C: Seams<G>, E: Seam, W: Seam>(planes: (&mut Plane<'p>, &mut Plane<'p>), at: (u32, u32), limits: (Limits, Limits)) -> Result<(), Never> {
    let (blue, red) = planes;
    let (left, top) = at;
    let Ok(stride) = index(blue.stride);
    let Ok(top) = index(top);

    let mut words = [[0u32; 16]; G];
    let rows = blue.pixels.chunks_exact(stride.max(1)).skip(top).take(8).chain(red.pixels.chunks_exact(stride.max(1)).skip(top).take(8));
    let Ok(()) = gathered(&mut words, (rows, left));
    let Ok(()) = by_rows::<G, C, E, W>(&mut words, limits);
    let rows = blue.pixels.chunks_exact_mut(stride.max(1)).skip(top).take(8).chain(red.pixels.chunks_exact_mut(stride.max(1)).skip(top).take(8));

    scattered(&words, (rows, left))
}

fn gathered<'p, const G: usize>(words: &mut [[u32; 16]; G], from: (impl Iterator<Item = &'p [u8]>, u32)) -> Result<(), Never> {
    let (rows, left) = from;
    let Ok(left) = index(left);

    for (row, pixels) in (0usize..16).zip(rows) {
        let levels = match pixels.get(left..).and_then(|pixels| pixels.as_chunks::<4>().0.first_chunk::<G>()) {
            Some(levels) => levels,
            None => continue,
        };

        for (group, levels) in words.iter_mut().zip(levels) {
            match group.get_mut(row) {
                Some(word) => *word = u32::from_le_bytes(*levels),
                None => {},
            }
        }
    }

    Ok(())
}

fn scattered<'p, const G: usize>(words: &[[u32; 16]; G], into: (impl Iterator<Item = &'p mut [u8]>, u32)) -> Result<(), Never> {
    let (rows, left) = into;
    let Ok(left) = index(left);

    for (row, pixels) in (0usize..16).zip(rows) {
        let levels = match pixels.get_mut(left..).and_then(|pixels| pixels.as_chunks_mut::<4>().0.first_chunk_mut::<G>()) {
            Some(levels) => levels,
            None => continue,
        };

        for (group, levels) in words.iter().zip(levels) {
            match group.get(row) {
                Some(word) => *levels = word.to_le_bytes(),
                None => {},
            }
        }
    }

    Ok(())
}

fn horizontal<const G: usize, C: Seams<G>, E: Seam, W: Seam>(plane: &mut Plane<'_>, at: (u32, u32), limits: (Limits, Limits)) -> Result<(), Never> {
    let (left, top) = at;
    let Ok(stride) = index(plane.stride);
    let Ok(top) = index(top);
    let Ok(left) = index(left);

    let mut lanes = [[[0u8; 16]; 4]; G];

    for (lane, pixels) in lanes.as_flattened_mut().iter_mut().zip(plane.pixels.chunks_exact(stride.max(1)).skip(top)) {
        match pixels.get(left..).and_then(<[u8]>::first_chunk::<16>) {
            Some(levels) => *lane = *levels,
            None => {},
        }
    }

    let Ok(()) = by_columns::<G, C, E, W>(&mut lanes, limits);

    for (lane, pixels) in lanes.as_flattened().iter().zip(plane.pixels.chunks_exact_mut(stride.max(1)).skip(top)) {
        match pixels.get_mut(left..).and_then(<[u8]>::first_chunk_mut::<16>) {
            Some(levels) => *levels = *lane,
            None => {},
        }
    }

    Ok(())
}

fn horizontal_both<'p, const G: usize, C: Seams<G>, E: Seam, W: Seam>(planes: (&mut Plane<'p>, &mut Plane<'p>), at: (u32, u32), limits: (Limits, Limits)) -> Result<(), Never> {
    let (blue, red) = planes;
    let (left, top) = at;
    let Ok(stride) = index(blue.stride);
    let Ok(top) = index(top);
    let Ok(left) = index(left);

    let mut lanes = [[[0u8; 16]; 4]; G];
    let rows = blue.pixels.chunks_exact(stride.max(1)).skip(top).zip(red.pixels.chunks_exact(stride.max(1)).skip(top));

    for (lane, (blue_pixels, red_pixels)) in lanes.as_flattened_mut().iter_mut().zip(rows) {
        let (blue_lanes, red_lanes) = lane.split_at_mut(8);

        for (lanes, pixels) in [(blue_lanes, blue_pixels), (red_lanes, red_pixels)] {
            match (lanes.first_chunk_mut::<8>(), pixels.get(left..).and_then(<[u8]>::first_chunk::<8>)) {
                (Some(lanes), Some(levels)) => *lanes = *levels,
                (_, _) => {},
            }
        }
    }

    let Ok(()) = by_columns::<G, C, E, W>(&mut lanes, limits);
    let rows = blue.pixels.chunks_exact_mut(stride.max(1)).skip(top).zip(red.pixels.chunks_exact_mut(stride.max(1)).skip(top));

    for (lane, (blue_pixels, red_pixels)) in lanes.as_flattened().iter().zip(rows) {
        let (blue_lanes, red_lanes) = lane.split_at(8);

        for (lanes, pixels) in [(blue_lanes, blue_pixels), (red_lanes, red_pixels)] {
            match (lanes.first_chunk::<8>(), pixels.get_mut(left..).and_then(<[u8]>::first_chunk_mut::<8>)) {
                (Some(lanes), Some(levels)) => *levels = *lanes,
                (_, _) => {},
            }
        }
    }

    Ok(())
}

#[inline(always)]
fn by_rows<const G: usize, C: Seams<G>, E: Seam, W: Seam>(words: &mut [[u32; 16]; G], limits: (Limits, Limits)) -> Result<(), Never> {
    for row in 0..16usize {
        let mut wide = [[0i16; 4]; G];

        for (into, group) in wide.iter_mut().zip(words.iter()) {
            match group.get(row) {
                Some(word) => {
                    let [a, b, c, d] = word.to_le_bytes();

                    *into = [i16::from(a), i16::from(b), i16::from(c), i16::from(d)];
                },
                None => {},
            }
        }

        let Ok(smoothed) = C::smoothed::<E, W>(wide, limits);

        for (group, [a, b, c, d]) in words.iter_mut().zip(smoothed) {
            match group.get_mut(row) {
                Some(word) => {
                    let (Ok(a), Ok(b), Ok(c), Ok(d)) = (narrowed(a), narrowed(b), narrowed(c), narrowed(d));

                    *word = u32::from_le_bytes([a, b, c, d]);
                },
                None => {},
            }
        }
    }

    Ok(())
}

#[inline(always)]
fn by_columns<const G: usize, C: Seams<G>, E: Seam, W: Seam>(lanes: &mut [[[u8; 16]; 4]; G], limits: (Limits, Limits)) -> Result<(), Never> {
    for column in 0..16usize {
        let mut wide = [[0i16; 4]; G];

        for (into, group) in wide.iter_mut().zip(lanes.iter()) {
            for (value, lane) in into.iter_mut().zip(group.iter()) {
                match lane.get(column) {
                    Some(level) => *value = i16::from(*level),
                    None => {},
                }
            }
        }

        let Ok(smoothed) = C::smoothed::<E, W>(wide, limits);

        for (group, values) in lanes.iter_mut().zip(smoothed) {
            for (lane, value) in group.iter_mut().zip(values) {
                match lane.get_mut(column) {
                    Some(level) => {
                        let Ok(narrowed) = narrowed(value);

                        *level = narrowed;
                    },
                    None => {},
                }
            }
        }
    }

    Ok(())
}

#[inline(always)]
fn narrowed(value: i16) -> Result<u8, Never> {
    let [level, ..] = value.to_le_bytes();

    Ok(level)
}

#[inline(always)]
fn seam<F: Seam>(pair: (Group, Group), limits: Limits) -> Result<(Group, Group), Never> {
    let ([p3, p2, p1, p0], [q0, q1, q2, q3]) = pair;
    let Ok([p3, p2, p1, p0, q0, q1, q2, q3]) = F::smoothed([p3, p2, p1, p0, q0, q1, q2, q3], limits);

    Ok(([p3, p2, p1, p0], [q0, q1, q2, q3]))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Limits {
    threshold: i16,
    interior: i16,
    variance: i16,
}

#[inline(always)]
fn chosen(choice: (i16, i16), mask: i16) -> Result<i16, Never> {
    let (when, otherwise) = choice;

    Ok((when & mask) | (otherwise & !mask))
}

#[inline(always)]
fn signed_clipped(value: i16) -> Result<i16, Never> {
    Ok(value.clamp(-128, 127))
}

#[inline(always)]
fn step_clipped(value: i16) -> Result<i16, Never> {
    Ok(value.clamp(-16, 15))
}

#[inline(always)]
fn moved(value: i16, by: Lift) -> Result<i16, Never> {
    let Ok(moved) = level(value.wrapping_add(by.amount));

    chosen((moved, value), by.mask)
}

#[inline(always)]
fn level(value: i16) -> Result<i16, Never> {
    Ok(value.clamp(0, 255))
}

#[inline(always)]
fn limits(strength: Strength, widened: i32) -> Result<Limits, Never> {
    let Ok(threshold) = fitted::<i32, i16>(strength.limit.wrapping_add(widened).wrapping_mul(2).wrapping_add(1));
    let Ok(interior) = fitted::<i32, i16>(strength.interior);
    let Ok(variance) = fitted::<i32, i16>(strength.variance);

    Ok(Limits { threshold, interior, variance })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lift {
    amount: i16,
    mask: i16,
}

#[inline(always)]
fn stepped(near: (i16, i16, i16, i16), limits: Limits) -> Result<i16, Never> {
    let (p1, p0, q0, q1) = near;
    let step = p0.wrapping_sub(q0).wrapping_abs().wrapping_mul(4).wrapping_add(p1.wrapping_sub(q1).wrapping_abs());

    Ok(i16::from(step <= limits.threshold).wrapping_neg())
}

#[inline(always)]
fn masks(pixels: [i16; 8], limits: Limits) -> Result<(i16, i16), Never> {
    let [p3, p2, p1, p0, q0, q1, q2, q3] = pixels;
    let near = p1.wrapping_sub(p0).wrapping_abs().max(q1.wrapping_sub(q0).wrapping_abs());
    let busiest = p3.wrapping_sub(p2).wrapping_abs().max(p2.wrapping_sub(p1).wrapping_abs()).max(q3.wrapping_sub(q2).wrapping_abs()).max(q2.wrapping_sub(q1).wrapping_abs()).max(near);
    let Ok(stepped) = stepped((p1, p0, q0, q1), limits);
    let mask = stepped & i16::from(busiest <= limits.interior).wrapping_neg();

    Ok((mask, i16::from(near > limits.variance).wrapping_neg()))
}

#[inline(always)]
fn adjusted(pixels: (i16, i16, i16, i16), taps: i16) -> Result<(i16, i16, i16), Never> {
    let (p1, p0, q0, q1) = pixels;
    let Ok(outside) = signed_clipped(p1.wrapping_sub(q1));
    let Ok(adjustment) = signed_clipped(q0.wrapping_sub(p0).wrapping_mul(3).wrapping_add(outside & taps));
    let Ok(lowered) = step_clipped(adjustment.wrapping_add(4).wrapping_shr(3));
    let Ok(raised) = step_clipped(adjustment.wrapping_add(3).wrapping_shr(3));

    Ok((adjustment, lowered, raised))
}

#[inline(always)]
fn simple(pixels: [i16; 8], limits: Limits) -> Result<[i16; 8], Never> {
    let [p3, p2, p1, p0, q0, q1, q2, q3] = pixels;
    let Ok(mask) = stepped((p1, p0, q0, q1), limits);
    let Ok((_, lowered, raised)) = adjusted((p1, p0, q0, q1), -1);
    let Ok(new_p0) = moved(p0, Lift { amount: raised, mask });
    let Ok(new_q0) = moved(q0, Lift { amount: lowered.wrapping_neg(), mask });

    Ok([p3, p2, p1, new_p0, new_q0, q1, q2, q3])
}

#[inline(always)]
fn inner(pixels: [i16; 8], limits: Limits) -> Result<[i16; 8], Never> {
    let [p3, p2, p1, p0, q0, q1, q2, q3] = pixels;
    let Ok((mask, changing)) = masks(pixels, limits);
    let Ok((_, lowered, raised)) = adjusted((p1, p0, q0, q1), changing);
    let half = lowered.wrapping_add(1).wrapping_shr(1) & !changing;
    let Ok(new_p1) = moved(p1, Lift { amount: half, mask });
    let Ok(new_p0) = moved(p0, Lift { amount: raised, mask });
    let Ok(new_q0) = moved(q0, Lift { amount: lowered.wrapping_neg(), mask });
    let Ok(new_q1) = moved(q1, Lift { amount: half.wrapping_neg(), mask });

    Ok([p3, p2, new_p1, new_p0, new_q0, new_q1, q2, q3])
}

#[inline(always)]
fn outer(pixels: [i16; 8], limits: Limits) -> Result<[i16; 8], Never> {
    let [p3, p2, p1, p0, q0, q1, q2, q3] = pixels;
    let Ok((mask, changing)) = masks(pixels, limits);
    let Ok((adjustment, lowered, raised)) = adjusted((p1, p0, q0, q1), -1);
    let nearest = adjustment.wrapping_mul(27).wrapping_add(63).wrapping_shr(7);
    let middle = adjustment.wrapping_mul(18).wrapping_add(63).wrapping_shr(7);
    let farthest = adjustment.wrapping_mul(9).wrapping_add(63).wrapping_shr(7);
    let two = mask & changing;
    let six = mask & !changing;
    let Ok(p0_six) = moved(p0, Lift { amount: nearest, mask: six });
    let Ok(q0_six) = moved(q0, Lift { amount: nearest.wrapping_neg(), mask: six });
    let Ok(new_p0) = moved(p0_six, Lift { amount: raised, mask: two });
    let Ok(new_q0) = moved(q0_six, Lift { amount: lowered.wrapping_neg(), mask: two });
    let Ok(new_p2) = moved(p2, Lift { amount: farthest, mask: six });
    let Ok(new_p1) = moved(p1, Lift { amount: middle, mask: six });
    let Ok(new_q1) = moved(q1, Lift { amount: middle.wrapping_neg(), mask: six });
    let Ok(new_q2) = moved(q2, Lift { amount: farthest.wrapping_neg(), mask: six });

    Ok([p3, new_p2, new_p1, new_p0, new_q0, new_q1, new_q2, q3])
}
