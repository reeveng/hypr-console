//! A lossy bitstream undone into the pixels it holds.
//!
//! A lossy WebP is one VP8 key frame. Ten bytes say it is one and how large
//! it is, and the rest is partitions read with the boolean decoder: the first
//! holds the frame header and every macroblock's modes, and the others, one
//! for every few rows of macroblocks, hold the coefficients.
//!
//! The frame header says how the picture is cut into up to four segments,
//! each with a quantizer and a smoothing of its own, how strong the loop
//! filter is, how many coefficient partitions there are, how coarsely each
//! kind of coefficient is quantized, which of the chances coefficients are
//! read against it changes, and whether a macroblock can say it sent none.
//!
//! A macroblock is sixteen pixels a side of luma and eight of each chroma
//! plane. It is predicted from the pixels above and to the left of it, its
//! luma whole or as sixteen subblocks, and what is added to the prediction
//! is twenty-four blocks of four by four coefficients, one for each subblock
//! of luma and four for each chroma plane, with the averages of the luma
//! blocks sent apart as a block of their own when the luma was predicted
//! whole. A coefficient is a token read against chances that depend on where
//! it is in its block and on whether the blocks above and to the left had
//! anything in them.
//!
//! The partitions are read a band of macroblock rows at a time, and what is
//! read for a macroblock is handed on as it is: its modes, and its
//! twenty-four blocks already multiplied back up by their steps, for
//! [`crate::drawing`] to predict and add up while [`crate::rounds`] reads the
//! next band. The frame header is read the way libwebp reads it where libwebp
//! and the RFC part: a segment header that turns segments on without sending
//! their values gives them absolute values of nothing, and a segment's filter
//! level has the frame's deltas added before it is held between 0 and 63, not
//! after.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::booleans::Booleans;
use crate::chances::{AVERAGE_STEPS, COEFFICIENTS, DETAIL_STEPS, SUBBLOCK_MODES, UPDATES};
use crate::inverse::{self, Kind};
use crate::loop_filter::{Filter, Smoothing, Strength};
use crate::prediction::{Sub, Whole};
use crate::WebpError;

const KEY_FRAME: [u8; 3] = [0x9D, 0x01, 0x2A];

const SIDE: u16 = 0x3FFF;

const ZIGZAG: [u8; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

const BANDS: [u8; 16] = [0, 1, 2, 3, 6, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7];

const LARGEST: [&[u8]; 4] = [&[173, 148, 140], &[176, 155, 140, 135], &[180, 157, 141, 134, 130], &[254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129]];

type Positions = [[[u8; 11]; 3]; 16];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Segments {
    map: Option<[u8; 3]>,
    enabled: u32,
    absolute: u32,
    quantizers: [i32; 4],
    strengths: [i32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Filtering {
    filter: Filter,
    level: i32,
    sharpness: i32,
    reference: i32,
    mode: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Steps {
    luma: [i32; 2],
    second: [i32; 2],
    chroma: [i32; 2],
}

struct Header {
    segments: Segments,
    filtering: Filtering,
    partitions: u32,
    steps: [Steps; 4],
    positions: Box<[Positions; 4]>,
    skip: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prediction {
    Whole(Whole),
    Split([Sub; 16]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Modes {
    pub(crate) segment: u32,
    pub(crate) skip: u32,
    pub(crate) luma: Prediction,
    pub(crate) chroma: Whole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Sent {
    luma: u32,
    blue: u32,
    red: u32,
    second: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Context {
    modes: [u8; 4],
    sent: Sent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum After {
    Value,
    Nought,
}

struct Frame<'a> {
    size: Size<u32>,
    first: &'a [u8],
    rest: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Macroblock {
    pub(crate) modes: Modes,
    pub(crate) kinds: [Kind; 24],
    pub(crate) blocks: [[i16; 16]; 24],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Band {
    pub(crate) macroblocks: Vec<Macroblock>,
    pub(crate) smoothings: Vec<Smoothing>,
}

pub(crate) struct Parsing<'a> {
    first: Booleans<'a>,
    partitions: Vec<Booleans<'a>>,
    header: Header,
    strengths: [[Strength; 2]; 4],
    above: Vec<Context>,
}

pub(crate) struct Opened<'a> {
    pub(crate) size: Size<u32>,
    pub(crate) filter: Filter,
    pub(crate) parsing: Parsing<'a>,
}

const UNREAD: Macroblock = Macroblock {
    modes: Modes { segment: 0, skip: 0, luma: Prediction::Whole(Whole::Average), chroma: Whole::Average },
    kinds: [Kind::Empty; 24],
    blocks: [[0; 16]; 24],
};

pub(crate) fn opened(bitstream: &[u8]) -> Result<Opened<'_>, WebpError> {
    let frame = framed(bitstream)?;
    let Ok(mut first) = Booleans::new(frame.first);
    let Ok(header) = header(&mut first);

    first.within()?;

    let partitions = partitions(frame.rest, header.partitions)?;
    let Ok(wide) = index(frame.size.width.div_ceil(16));
    let Ok(strengths) = strengths(&header);
    let filter = header.filtering.filter;

    Ok(Opened { size: frame.size, filter, parsing: Parsing { first, partitions, header, strengths, above: vec![Context::default(); wide] } })
}

impl Parsing<'_> {
    pub(crate) fn read(&mut self, band: &mut Band, rows: (u32, u32)) -> Result<(), WebpError> {
        let (first_row, count) = rows;
        let Ok(columns) = fitted::<_, u32>(self.above.len());
        let Ok(area) = index(columns.saturating_mul(count));
        let Ok(wide) = index(columns);
        let mask = self.header.partitions.saturating_sub(1);

        band.macroblocks.resize(area, UNREAD);
        band.smoothings.clear();

        for (row, line) in (first_row..).zip(band.macroblocks.chunks_exact_mut(wide.max(1))) {
            let Ok(at) = index(row & mask);

            let tokens = match self.partitions.get_mut(at) {
                Some(tokens) => tokens,
                None => return Err(WebpError::Corrupt),
            };

            let mut left = Context::default();

            for (macroblock, above) in line.iter_mut().zip(self.above.iter_mut()) {
                let Ok(chosen) = modes(&mut self.first, &self.header, (&mut above.modes, &mut left.modes));
                let Ok(segment) = index(chosen.segment);

                let steps = match self.header.steps.get(segment) {
                    Some(steps) => *steps,
                    None => Steps::default(),
                };

                let split = match chosen.luma {
                    Prediction::Whole(_) => 0,
                    Prediction::Split(_) => 1,
                };

                let Ok(kinds) = match chosen.skip {
                    0 => residuals(tokens, (&self.header.positions, &steps, split), (&mut above.sent, &mut left.sent), &mut macroblock.blocks),
                    _ => skipped((&mut above.sent, &mut left.sent), split),
                };

                let anything = kinds.iter().any(|kind| *kind != Kind::Empty);

                let strength = match self.strengths.get(segment) {
                    Some([whole, subblocks]) => match split {
                        0 => *whole,
                        _ => *subblocks,
                    },
                    None => Strength::default(),
                };

                macroblock.modes = chosen;
                macroblock.kinds = kinds;
                band.smoothings.push(Smoothing { strength, inner: split | u32::from(anything) });
            }

            self.first.within()?;
            tokens.within()?;
        }

        Ok(())
    }
}

pub(crate) fn measured(bitstream: &[u8]) -> Result<Size<u32>, WebpError> {
    let [tag, _, _, k0, k1, k2, w0, w1, h0, h1] = match bitstream.first_chunk::<10>() {
        Some(said) => *said,
        None => return Err(WebpError::Truncated),
    };

    let width = u32::from(u16::from_le_bytes([w0, w1]) & SIDE);
    let height = u32::from(u16::from_le_bytes([h0, h1]) & SIDE);

    match (tag & 1, tag.wrapping_shr(1) & 7, tag.wrapping_shr(4) & 1, [k0, k1, k2] == KEY_FRAME, width, height) {
        (0, 0..=3, 1, true, 1.., 1..) => Ok(Size { width, height }),
        (_, _, _, _, _, _) => Err(WebpError::Corrupt),
    }
}

fn framed(bitstream: &[u8]) -> Result<Frame<'_>, WebpError> {
    let size = measured(bitstream)?;

    let (said, rest) = match bitstream.split_first_chunk::<10>() {
        Some(([t0, t1, t2, ..], rest)) => (u32::from_le_bytes([*t0, *t1, *t2, 0]), rest),
        None => return Err(WebpError::Truncated),
    };

    let Ok(long) = index(said.wrapping_shr(5));

    match rest.split_at_checked(long) {
        Some((first, rest)) => Ok(Frame { size, first, rest }),
        None => Err(WebpError::Truncated),
    }
}

fn partitions(rest: &[u8], count: u32) -> Result<Vec<Booleans<'_>>, WebpError> {
    let Ok(sized) = index(count.saturating_sub(1).saturating_mul(3));

    let (sizes, mut data) = match rest.split_at_checked(sized) {
        Some(split) => split,
        None => return Err(WebpError::Truncated),
    };

    let mut partitions = Vec::new();

    for [s0, s1, s2] in sizes.as_chunks::<3>().0 {
        let Ok(long) = index(u32::from_le_bytes([*s0, *s1, *s2, 0]));
        let (partition, after) = data.split_at(long.min(data.len()));
        let Ok(booleans) = Booleans::new(partition);

        partitions.push(booleans);
        data = after;
    }

    match data {
        [] => Err(WebpError::Truncated),
        [_, ..] => {
            let Ok(booleans) = Booleans::new(data);

            partitions.push(booleans);

            Ok(partitions)
        },
    }
}

fn header(booleans: &mut Booleans<'_>) -> Result<Header, Never> {
    let Ok(_colour_space) = booleans.even();
    let Ok(_clamping) = booleans.even();
    let Ok(segments) = segments(booleans);
    let Ok(filtering) = filtering(booleans);
    let Ok(partition_bits) = booleans.number(2);
    let Ok(steps) = steps(booleans, &segments);
    let Ok(_refresh) = booleans.even();
    let Ok(positions) = positions(booleans);
    let Ok(skipping) = booleans.even();

    let skip = match skipping {
        0 => None,
        _ => {
            let Ok(chance) = booleans.number(8);
            let [chance, ..] = chance.to_le_bytes();

            Some(chance)
        },
    };

    Ok(Header { segments, filtering, partitions: 1u32.wrapping_shl(partition_bits), steps, positions, skip })
}

fn segments(booleans: &mut Booleans<'_>) -> Result<Segments, Never> {
    let mut segments = Segments { map: None, enabled: 0, absolute: 1, quantizers: [0; 4], strengths: [0; 4] };

    let Ok(enabled) = booleans.even();

    match enabled {
        0 => return Ok(segments),
        _ => segments.enabled = 1,
    }

    let Ok(mapped) = booleans.even();
    let Ok(valued) = booleans.even();

    match valued {
        0 => {},
        _ => {
            let Ok(absolute) = booleans.even();

            segments.absolute = absolute;

            for quantizer in &mut segments.quantizers {
                let Ok(value) = booleans.flagged(7);

                *quantizer = value;
            }

            for strength in &mut segments.strengths {
                let Ok(value) = booleans.flagged(6);

                *strength = value;
            }
        },
    }

    match mapped {
        0 => {},
        _ => {
            let mut chances = [u8::MAX; 3];

            for chance in &mut chances {
                let Ok(sent) = booleans.even();

                match sent {
                    0 => {},
                    _ => {
                        let Ok(value) = booleans.number(8);
                        let [value, ..] = value.to_le_bytes();

                        *chance = value;
                    },
                }
            }

            segments.map = Some(chances);
        },
    }

    Ok(segments)
}

fn filtering(booleans: &mut Booleans<'_>) -> Result<Filtering, Never> {
    let Ok(simple) = booleans.even();
    let Ok(level) = booleans.number(6);
    let Ok(sharpness) = booleans.number(3);
    let Ok(adjusted) = booleans.even();

    let mut references = [0i32; 4];
    let mut modes = [0i32; 4];

    match adjusted {
        0 => {},
        _ => {
            let Ok(updated) = booleans.even();

            match updated {
                0 => {},
                _ => {
                    for delta in references.iter_mut().chain(modes.iter_mut()) {
                        let Ok(sent) = booleans.even();

                        match sent {
                            0 => {},
                            _ => {
                                let Ok(value) = booleans.signed(6);

                                *delta = value;
                            },
                        }
                    }
                },
            }
        },
    }

    let filter = match (level, simple) {
        (0, _) => Filter::Off,
        (_, 0) => Filter::Normal,
        (_, _) => Filter::Simple,
    };

    let [reference, ..] = references;
    let [mode, ..] = modes;
    let Ok(level) = fitted::<u32, i32>(level);
    let Ok(sharpness) = fitted::<u32, i32>(sharpness);

    Ok(Filtering { filter, level, sharpness, reference, mode })
}

fn steps(booleans: &mut Booleans<'_>, segments: &Segments) -> Result<[Steps; 4], Never> {
    let Ok(base) = booleans.number(7);
    let Ok(luma_average) = booleans.flagged(4);
    let Ok(second_average) = booleans.flagged(4);
    let Ok(second_detail) = booleans.flagged(4);
    let Ok(chroma_average) = booleans.flagged(4);
    let Ok(chroma_detail) = booleans.flagged(4);
    let Ok(base) = fitted::<u32, i32>(base);

    Ok(segments.quantizers.map(|quantizer| {
        let quantizer = match (segments.enabled, segments.absolute) {
            (0, _) => base,
            (_, 0) => quantizer.wrapping_add(base),
            (_, _) => quantizer,
        };

        let Ok(luma) = stepped((&AVERAGE_STEPS, quantizer.wrapping_add(luma_average), 127));
        let Ok(luma_detail) = stepped((&DETAIL_STEPS, quantizer, 127));
        let Ok(second) = stepped((&AVERAGE_STEPS, quantizer.wrapping_add(second_average), 127));
        let Ok(second_rest) = stepped((&DETAIL_STEPS, quantizer.wrapping_add(second_detail), 127));
        let Ok(chroma) = stepped((&AVERAGE_STEPS, quantizer.wrapping_add(chroma_average), 117));
        let Ok(chroma_rest) = stepped((&DETAIL_STEPS, quantizer.wrapping_add(chroma_detail), 127));

        Steps {
            luma: [luma, luma_detail],
            second: [second.wrapping_mul(2), second_rest.wrapping_mul(101_581).wrapping_shr(16).max(8)],
            chroma: [chroma, chroma_rest],
        }
    }))
}

fn stepped(looked: (&[u16; 128], i32, i32)) -> Result<i32, Never> {
    let (table, at, most) = looked;
    let Ok(at) = index(at.clamp(0, most));

    Ok(match table.get(at) {
        Some(step) => i32::from(*step),
        None => 0,
    })
}

fn positions(booleans: &mut Booleans<'_>) -> Result<Box<[Positions; 4]>, Never> {
    let mut bands = COEFFICIENTS;

    for (kind, updates) in bands.iter_mut().zip(&UPDATES) {
        for (band, updates) in kind.iter_mut().zip(updates) {
            for (context, updates) in band.iter_mut().zip(updates) {
                for (chance, update) in context.iter_mut().zip(updates) {
                    let Ok(changed) = booleans.bit(*update);

                    match changed {
                        0 => {},
                        _ => {
                            let Ok(value) = booleans.number(8);
                            let [value, ..] = value.to_le_bytes();

                            *chance = value;
                        },
                    }
                }
            }
        }
    }

    Ok(Box::new(bands.map(|kind| {
        BANDS.map(|band| {
            let Ok(at) = index(u32::from(band));

            match kind.get(at) {
                Some(chances) => *chances,
                None => [[0; 11]; 3],
            }
        })
    })))
}

#[inline(never)]
fn modes(booleans: &mut Booleans<'_>, header: &Header, contexts: (&mut [u8; 4], &mut [u8; 4])) -> Result<Modes, Never> {
    let (above, left) = contexts;

    let Ok(segment) = match header.segments.map {
        Some([high, low, higher]) => {
            let Ok(upper) = booleans.bit(high);

            match upper {
                0 => booleans.bit(low),
                _ => {
                    let Ok(lower) = booleans.bit(higher);

                    Ok(lower.wrapping_add(2))
                },
            }
        },
        None => Ok(0),
    };

    let Ok(skip) = match header.skip {
        Some(chance) => booleans.bit(chance),
        None => Ok(0),
    };

    let Ok(whole) = booleans.bit(145);

    let luma = match whole {
        0 => {
            let mut subblocks = [Sub::Average; 16];

            for (row, left) in subblocks.as_chunks_mut::<4>().0.iter_mut().zip(left.iter_mut()) {
                for (subblock, above) in row.iter_mut().zip(above.iter_mut()) {
                    let Ok(from_above) = index(u32::from(*above));
                    let Ok(from_left) = index(u32::from(*left));

                    let chances = match SUBBLOCK_MODES.get(from_above).and_then(|by_left| by_left.get(from_left)) {
                        Some(chances) => chances,
                        None => &[128; 9],
                    };

                    let Ok(mode) = subblock_mode(booleans, chances);
                    let Ok(number) = numbered(mode);

                    *subblock = mode;
                    *above = number;
                    *left = number;
                }
            }

            Prediction::Split(subblocks)
        },
        _ => {
            let Ok(far) = booleans.bit(156);

            let Ok(mode) = match far {
                0 => {
                    let Ok(vertical) = booleans.bit(163);

                    Ok::<Whole, Never>(match vertical {
                        0 => Whole::Average,
                        _ => Whole::Vertical,
                    })
                },
                _ => {
                    let Ok(true_motion) = booleans.even();

                    Ok(match true_motion {
                        0 => Whole::Horizontal,
                        _ => Whole::TrueMotion,
                    })
                },
            };

            let Ok(number) = whole_numbered(mode);

            *above = [number; 4];
            *left = [number; 4];

            Prediction::Whole(mode)
        },
    };

    let Ok(chroma) = chroma_mode(booleans);

    Ok(Modes { segment, skip, luma, chroma })
}

fn chroma_mode(booleans: &mut Booleans<'_>) -> Result<Whole, Never> {
    let Ok(average) = booleans.bit(142);

    match average {
        0 => return Ok(Whole::Average),
        _ => {},
    }

    let Ok(vertical) = booleans.bit(114);

    match vertical {
        0 => return Ok(Whole::Vertical),
        _ => {},
    }

    let Ok(true_motion) = booleans.bit(183);

    Ok(match true_motion {
        0 => Whole::Horizontal,
        _ => Whole::TrueMotion,
    })
}

fn subblock_mode(booleans: &mut Booleans<'_>, chances: &[u8; 9]) -> Result<Sub, Never> {
    let [average, true_motion, vertical, split, horizontal, down_right, down_left, vertical_left, horizontal_down] = *chances;

    let Ok(bit) = booleans.bit(average);

    match bit {
        0 => return Ok(Sub::Average),
        _ => {},
    }

    let Ok(bit) = booleans.bit(true_motion);

    match bit {
        0 => return Ok(Sub::TrueMotion),
        _ => {},
    }

    let Ok(bit) = booleans.bit(vertical);

    match bit {
        0 => return Ok(Sub::Vertical),
        _ => {},
    }

    let Ok(bit) = booleans.bit(split);

    match bit {
        0 => {
            let Ok(bit) = booleans.bit(horizontal);

            match bit {
                0 => return Ok(Sub::Horizontal),
                _ => {},
            }

            let Ok(bit) = booleans.bit(down_right);

            Ok(match bit {
                0 => Sub::DownRight,
                _ => Sub::VerticalRight,
            })
        },
        _ => {
            let Ok(bit) = booleans.bit(down_left);

            match bit {
                0 => return Ok(Sub::DownLeft),
                _ => {},
            }

            let Ok(bit) = booleans.bit(vertical_left);

            match bit {
                0 => return Ok(Sub::VerticalLeft),
                _ => {},
            }

            let Ok(bit) = booleans.bit(horizontal_down);

            Ok(match bit {
                0 => Sub::HorizontalDown,
                _ => Sub::HorizontalUp,
            })
        },
    }
}

fn numbered(mode: Sub) -> Result<u8, Never> {
    Ok(match mode {
        Sub::Average => 0,
        Sub::TrueMotion => 1,
        Sub::Vertical => 2,
        Sub::Horizontal => 3,
        Sub::DownLeft => 4,
        Sub::DownRight => 5,
        Sub::VerticalRight => 6,
        Sub::VerticalLeft => 7,
        Sub::HorizontalDown => 8,
        Sub::HorizontalUp => 9,
    })
}

fn whole_numbered(mode: Whole) -> Result<u8, Never> {
    Ok(match mode {
        Whole::Average => 0,
        Whole::TrueMotion => 1,
        Whole::Vertical => 2,
        Whole::Horizontal => 3,
    })
}

#[inline(always)]
fn bit_of(read: (u32, u32)) -> Result<u32, Never> {
    let (bits, at) = read;

    Ok(bits.wrapping_shr(at) & 1)
}

#[inline(always)]
fn with_bit(bits: u32, set: (u32, u32)) -> Result<u32, Never> {
    let (at, bit) = set;

    Ok((bits & !1u32.wrapping_shl(at)) | bit.wrapping_shl(at))
}

#[inline(always)]
fn truncated(value: i32) -> Result<i16, Never> {
    let [low, high, ..] = value.to_le_bytes();

    Ok(i16::from_le_bytes([low, high]))
}

#[inline(always)]
fn classified(count: u32, block: &[i16; 16]) -> Result<Kind, Never> {
    let [average, ..] = *block;

    Ok(match (count > 1, average) {
        (true, _) => Kind::Full,
        (false, 0) => Kind::Empty,
        (false, _) => Kind::Average,
    })
}

fn skipped(sent: (&mut Sent, &mut Sent), split: u32) -> Result<[Kind; 24], Never> {
    let (above, left) = sent;

    *above = Sent { second: match split {
        0 => 0,
        _ => above.second,
    }, ..Sent::default() };
    *left = Sent { second: match split {
        0 => 0,
        _ => left.second,
    }, ..Sent::default() };

    Ok([Kind::Empty; 24])
}

#[inline(never)]
fn residuals(tokens: &mut Booleans<'_>, coded: (&[Positions; 4], &Steps, u32), sent: (&mut Sent, &mut Sent), blocks: &mut [[i16; 16]; 24]) -> Result<[Kind; 24], Never> {
    let (positions, steps, split) = coded;
    let (above, left) = sent;
    let [details, seconds, chromas, lumas] = positions;
    let mut kinds = [Kind::Empty; 24];

    *blocks = [[0; 16]; 24];

    let (first, luma_positions) = match split {
        0 => {
            let mut second = [0i16; 16];
            let context = above.second.wrapping_add(left.second);
            let Ok(count) = coefficients(tokens, (seconds, context, 0), steps.second, &mut second);
            let had = u32::from(count > 0);

            above.second = had;
            left.second = had;

            let Ok(averages) = match count > 1 {
                true => inverse::walsh(&second),
                false => {
                    let [average, ..] = second;

                    Ok([i32::from(average).wrapping_add(3).wrapping_shr(3); 16])
                },
            };

            for (block, average) in blocks.iter_mut().zip(averages) {
                match block.first_mut() {
                    Some(first) => {
                        let Ok(average) = truncated(average);

                        *first = average;
                    },
                    None => {},
                }
            }

            (1, details)
        },
        _ => (0, lumas),
    };

    let (luma_blocks, chroma_blocks) = blocks.split_at_mut(16);
    let (luma_kinds, chroma_kinds) = kinds.split_at_mut(16);

    for ((row_blocks, row_kinds), y) in luma_blocks.as_chunks_mut::<4>().0.iter_mut().zip(luma_kinds.as_chunks_mut::<4>().0.iter_mut()).zip(0u32..) {
        for ((block, kind), x) in row_blocks.iter_mut().zip(row_kinds.iter_mut()).zip(0u32..) {
            let Ok(from_above) = bit_of((above.luma, x));
            let Ok(from_left) = bit_of((left.luma, y));
            let Ok(count) = coefficients(tokens, (luma_positions, from_above.wrapping_add(from_left), first), steps.luma, block);
            let had = u32::from(count > first);
            let Ok(now_above) = with_bit(above.luma, (x, had));
            let Ok(now_left) = with_bit(left.luma, (y, had));
            let Ok(sorted) = classified(count, block);

            above.luma = now_above;
            left.luma = now_left;
            *kind = sorted;
        }
    }

    let (blue_blocks, red_blocks) = chroma_blocks.split_at_mut(4);
    let (blue_kinds, red_kinds) = chroma_kinds.split_at_mut(4);

    let planes = [(blue_blocks, blue_kinds, (&mut above.blue, &mut left.blue)), (red_blocks, red_kinds, (&mut above.red, &mut left.red))];

    for (plane_blocks, plane_kinds, (above_bits, left_bits)) in planes {
        for ((row_blocks, row_kinds), y) in plane_blocks.as_chunks_mut::<2>().0.iter_mut().zip(plane_kinds.as_chunks_mut::<2>().0.iter_mut()).zip(0u32..) {
            for ((block, kind), x) in row_blocks.iter_mut().zip(row_kinds.iter_mut()).zip(0u32..) {
                let Ok(from_above) = bit_of((*above_bits, x));
                let Ok(from_left) = bit_of((*left_bits, y));
                let Ok(count) = coefficients(tokens, (chromas, from_above.wrapping_add(from_left), 0), steps.chroma, block);
                let had = u32::from(count > 0);
                let Ok(now_above) = with_bit(*above_bits, (x, had));
                let Ok(now_left) = with_bit(*left_bits, (y, had));
                let Ok(sorted) = classified(count, block);

                *above_bits = now_above;
                *left_bits = now_left;
                *kind = sorted;
            }
        }
    }

    Ok(kinds)
}

#[inline(always)]
fn coefficients(tokens: &mut Booleans<'_>, reading: (&Positions, u32, u32), steps: [i32; 2], block: &mut [i16; 16]) -> Result<u32, Never> {
    let (positions, context, first) = reading;
    let [average_step, detail_step] = steps;
    let Ok(skipped) = index(first);
    let mut context = context;
    let mut after = After::Value;

    for ((contexts, zigzag), at) in positions.iter().zip(ZIGZAG).zip(0u32..).skip(skipped) {
        let Ok(looked) = index(context);

        let [ended, nought, one, large @ ..] = match contexts.get(looked) {
            Some(chances) => *chances,
            None => return Ok(at),
        };

        match after {
            After::Value => {
                let Ok(more) = tokens.bit(ended);

                match more {
                    0 => return Ok(at),
                    _ => {},
                }
            },
            After::Nought => {},
        }

        let Ok(something) = tokens.bit(nought);

        match something {
            0 => {
                context = 0;
                after = After::Nought;

                continue;
            },
            _ => {},
        }

        let Ok(bigger) = tokens.bit(one);

        let value = match bigger {
            0 => {
                context = 1;

                1
            },
            _ => {
                context = 2;

                let Ok(value) = larger(tokens, large);

                value
            },
        };

        let Ok(negative) = tokens.even();

        let value = match negative {
            0 => value,
            _ => value.wrapping_neg(),
        };

        let step = match at {
            0 => average_step,
            _ => detail_step,
        };

        let Ok(place) = index(u32::from(zigzag));

        match block.get_mut(place) {
            Some(coefficient) => {
                let Ok(scaled) = truncated(value.wrapping_mul(step));

                *coefficient = scaled;
            },
            None => {},
        }

        after = After::Value;
    }

    Ok(16)
}

#[inline(never)]
fn larger(tokens: &mut Booleans<'_>, chances: [u8; 8]) -> Result<i32, Never> {
    let [two_or_more, three_or_four, four, five_or_more, ten_or_fewer, categories, third, fourth] = chances;

    let Ok(beyond) = tokens.bit(two_or_more);

    match beyond {
        0 => {
            let Ok(three) = tokens.bit(three_or_four);

            return match three {
                0 => Ok(2),
                _ => {
                    let Ok(four) = tokens.bit(four);

                    Ok(3i32.wrapping_add(i32::from(four != 0)))
                },
            };
        },
        _ => {},
    }

    let Ok(far) = tokens.bit(five_or_more);

    match far {
        0 => {
            let Ok(second) = tokens.bit(ten_or_fewer);

            return match second {
                0 => {
                    let Ok(extra) = tokens.bit(159);

                    Ok(5i32.wrapping_add(i32::from(extra != 0)))
                },
                _ => {
                    let Ok(high) = tokens.bit(165);
                    let Ok(low) = tokens.bit(145);

                    Ok(7i32.wrapping_add(i32::from(high != 0).wrapping_mul(2)).wrapping_add(i32::from(low != 0)))
                },
            };
        },
        _ => {},
    }

    let Ok(high) = tokens.bit(categories);

    let Ok(low) = match high {
        0 => tokens.bit(third),
        _ => tokens.bit(fourth),
    };

    let category = high.wrapping_mul(2).wrapping_add(low);
    let Ok(at) = index(category);
    let mut extra = 0i32;

    for chance in LARGEST.get(at).copied().into_iter().flatten() {
        let Ok(bit) = tokens.bit(*chance);

        extra = extra.wrapping_add(extra).wrapping_add(i32::from(bit != 0));
    }

    Ok(extra.wrapping_add(3).wrapping_add(8i32.wrapping_shl(category)))
}

fn strengths(header: &Header) -> Result<[[Strength; 2]; 4], Never> {
    let segments = &header.segments;
    let filtering = &header.filtering;

    match filtering.filter {
        Filter::Off => return Ok([[Strength::default(); 2]; 4]),
        Filter::Simple | Filter::Normal => {},
    }

    Ok(segments.strengths.map(|strength| {
        let base = match (segments.enabled, segments.absolute) {
            (0, _) => filtering.level,
            (_, 0) => strength.wrapping_add(filtering.level),
            (_, _) => strength,
        };

        [0i32, 1].map(|split| {
            let level = base.wrapping_add(filtering.reference).wrapping_add(match split {
                0 => 0,
                _ => filtering.mode,
            });

            let level = level.clamp(0, 63);

            match level {
                0 => Strength::default(),
                _ => {
                    let interior = match filtering.sharpness {
                        0 => level,
                        1..=4 => level.wrapping_shr(1).min(9i32.wrapping_sub(filtering.sharpness)),
                        _ => level.wrapping_shr(2).min(9i32.wrapping_sub(filtering.sharpness)),
                    };

                    let interior = interior.max(1);

                    let variance = match level {
                        40.. => 2,
                        15..=39 => 1,
                        _ => 0,
                    };

                    Strength { limit: level.wrapping_mul(2).wrapping_add(interior), interior, variance }
                },
            }
        })
    }))
}
