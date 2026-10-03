//! A lossy picture decoded a band at a time, by as many hands as the caller
//! has.
//!
//! The partitions are one walk through their bits each, so the modes and
//! coefficients of a band can only be read once the band above has been. What
//! comes after reading is not like that: a band is drawn from what was read
//! for it and the line above it as it was drawn, the loop filter smooths a band
//! once it is drawn, and a band is coloured once the band under it has been
//! smoothed, since smoothing moves the three lines above its top edge. Each of
//! those needs only the bands before it, so they are done at once on different
//! bands.
//!
//! A round hands back as tasks the reading of one band, the drawing of the
//! band read the round before, the smoothing of the one drawn the round before
//! that, and the colouring of the band two further up, its luma and chroma
//! apart where they do not read each other. The read is the one piece that
//! cannot be split, so a picture spread across the cores takes about as long
//! as its bits take to read. [`crate::decoded`] does the tasks one after
//! another; [`crate::decoded_spread`] hands them to a function of the
//! caller's, which is where how many threads there are is decided -- never in
//! here.
//!
//! A round is as many rows of macroblocks as keeps what its tasks touch in a
//! core's own cache together. Twice that kept the cores busy for longer
//! stretches, but one core then decoded a photograph a tenth slower, and a
//! single row a round was slower spread, since every round waits for its
//! slowest task.
//!
//! The planes hold a run of bands rather than the picture. Only the bands
//! from the one being coloured to the one being drawn are ever read, so when
//! the drawing reaches the end of what is held, those are moved to the front
//! and the rest is drawn over. A photograph's planes had been most of what its
//! decode took, and every page of them was new memory touched once.
//!
//! A caller that takes the rows as they come names the part of the picture it
//! wants. Only that part is coloured, each row into one row handed straight on,
//! and reading stops at the band under its last row, which is still smoothed
//! because its top edge moves the three lines above it. Columns cannot be
//! dropped from the drawing the same way -- a macroblock is predicted from the
//! one left of it and the one above and to its right, so every one is drawn --
//! but the loop filter leaves alone the macroblocks right of the part.

use std::sync::Mutex;

use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::colours::{self, Chroma, Columns, Luma, Rows};
use crate::drawing::{self, Component, Lines};
use crate::loop_filter::{self, Filter, Plane, Smoothed};
use crate::lossy::{self, Band, Opened, Parsing};
use crate::{WebpError, alpha};

const ROUND: u32 = 4;

const BEHIND: u32 = 4;

const HELD: u32 = 20;

const ABOVE_AN_EDGE: u32 = 4;

pub struct Task<'a> {
    work: Mutex<Work<'a>>,
}

pub type Spread<'s> = &'s dyn Fn(&[Task<'_>]) -> Result<(), WebpError>;

pub(crate) trait Reading: Send {
    fn read(&mut self, band: &mut Band, rows: (u32, u32)) -> Result<(), WebpError>;
}

impl Reading for Parsing<'_> {
    fn read(&mut self, band: &mut Band, rows: (u32, u32)) -> Result<(), WebpError> {
        Parsing::read(self, band, rows)
    }
}

enum Work<'a> {
    Read { reading: &'a mut (dyn Reading + 'a), band: &'a mut Band, rows: (u32, u32) },
    Transparency { chunk: &'a [u8], size: Size<u32>, alpha: &'a mut Vec<u8> },
    Predicted { layer: Component<'a>, band: &'a Band, rows: (u32, u32) },
    Smoothed { layer: Smoothed<'a>, band: &'a Band, smoothing: (Filter, u32, u32), rows: (u32, u32) },
    Coloured { into: Rows<'a>, planes: (Luma<'a>, Chroma<'a>, &'a [u8]), rows: (Size<u32>, u32, Columns) },
}

impl Task<'_> {
    pub fn done(&self) -> Result<(), WebpError> {
        let mut work = match self.work.lock() {
            Ok(work) => work,
            Err(_another_job_panicked) => return Err(WebpError::Corrupt),
        };

        match &mut *work {
            Work::Read { reading, band, rows } => reading.read(band, *rows),
            Work::Transparency { chunk, size, alpha } => {
                let decoded = alpha::decoded(chunk, *size)?;

                **alpha = decoded;

                Ok(())
            },
            Work::Predicted { layer, band, rows } => {
                let Ok(()) = drawing::drawn(layer, &band.macroblocks, *rows);

                Ok(())
            },
            Work::Smoothed { layer, band, smoothing, rows } => {
                let (filter, columns, reach) = *smoothing;
                let Ok(()) = loop_filter::filtered(layer, (filter, &band.smoothings, (columns, reach)), *rows);

                Ok(())
            },
            Work::Coloured { into, planes, rows } => {
                let Ok(()) = colours::coloured(into, *planes, *rows);

                Ok(())
            },
        }
    }
}

pub(crate) fn one_after_another(tasks: &[Task<'_>]) -> Result<(), WebpError> {
    tasks.iter().try_for_each(Task::done)
}

struct Planes {
    luma: Vec<u8>,
    blue: Vec<u8>,
    red: Vec<u8>,
}

struct Decoding<'b> {
    parsing: Parsing<'b>,
    planes: Planes,
    above: Planes,
    bands: [Band; 3],
    rgba: Vec<u8>,
    alpha: Vec<u8>,
    held_from: u32,
}

pub trait Receiver: Send {
    fn received(&mut self, rgba: &[u8]) -> Result<(), Never>;
}

pub(crate) enum Destination<'r> {
    Picture,
    Bands(&'r mut dyn Receiver, Rectangle<u32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layout {
    size: Size<u32>,
    columns: u32,
    rows: u32,
    bands: u32,
    filter: Filter,
    kept: Rectangle<u32>,
}

struct Carved<'a> {
    coloured: &'a [u8],
    smoothed: &'a mut [u8],
    drawn: &'a mut [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cuts {
    coloured: (u32, u32),
    smoothed: (u32, u32),
    drawn: (u32, u32),
}

pub(crate) fn decoded(bitstream: &[u8], transparency: Option<&[u8]>, drawing: (Spread<'_>, Destination<'_>)) -> Result<(Size<u32>, Vec<u8>), WebpError> {
    let (spread, mut destination) = drawing;
    let Opened { size, filter, parsing } = lossy::opened(bitstream)?;
    let columns = size.width.div_ceil(16);
    let rows = size.height.div_ceil(16);
    let whole = Rectangle { origin: Point { x: 0, y: 0 }, size };

    let kept = match &destination {
        Destination::Picture => whole,
        Destination::Bands(_, kept) => *kept,
    };

    let last_kept = kept.origin.y.saturating_add(kept.size.height).min(size.height).saturating_sub(1);
    let below_the_kept = last_kept.wrapping_div(ROUND.saturating_mul(16)).saturating_add(2);
    let layout = Layout { size, columns, rows, bands: rows.div_ceil(ROUND).min(below_the_kept), filter, kept };
    let macroblocks = u64::from(columns).saturating_mul(u64::from(layout.bands.min(HELD).saturating_mul(ROUND)));
    let Ok(luma_area) = index(macroblocks.saturating_mul(256));
    let Ok(chroma_area) = index(macroblocks.saturating_mul(64));
    let Ok(luma_line) = index(columns.saturating_mul(16));
    let Ok(chroma_line) = index(columns.saturating_mul(8));
    let line = u64::from(kept.size.width).saturating_mul(4);

    let coloured_rows = match destination {
        Destination::Picture => size.height,
        Destination::Bands(_, _) => 1,
    };

    let Ok(rgba_area) = index(u64::from(coloured_rows).saturating_mul(line));

    let mut decoding = Decoding {
        parsing,
        planes: Planes { luma: vec![0; luma_area], blue: vec![0; chroma_area], red: vec![0; chroma_area] },
        above: Planes { luma: vec![0; luma_line], blue: vec![0; chroma_line], red: vec![0; chroma_line] },
        bands: [Band::default(), Band::default(), Band::default()],
        rgba: vec![0; rgba_area],
        alpha: Vec::new(),
        held_from: 0,
    };

    for round in 0..layout.bands.saturating_add(BEHIND) {
        let Ok(()) = moved_up(&mut decoding, (layout, round));
        let Ok(tasks) = tasks(&mut decoding, (layout, round), (transparency, &mut destination));

        spread(&tasks)?;
    }

    Ok((size, decoding.rgba))
}

fn tasks<'a>(decoding: &'a mut Decoding<'_>, at: (Layout, u32), given: (Option<&'a [u8]>, &'a mut Destination<'_>)) -> Result<Vec<Task<'a>>, Never> {
    let (layout, round) = at;
    let (transparency, destination) = given;
    let Decoding { parsing, planes, above, bands, rgba, alpha, held_from } = decoding;
    let held = held_from.saturating_mul(ROUND);
    let Ok(reading) = within(Some(round), layout);
    let Ok(drawing) = within(round.checked_sub(1), layout);
    let Ok(smoothing) = within(round.checked_sub(2), layout);
    let Ok(colouring) = within(round.checked_sub(BEHIND), layout);

    let Ok(luma_cuts) = cuts((drawing, smoothing, colouring), (layout, 16));
    let Ok(chroma_cuts) = cuts((drawing, smoothing, colouring), (layout, 8));
    let Ok(Carved { coloured: luma_coloured, smoothed: luma_smoothed, drawn: luma_drawn }) = carved(&mut planes.luma, (layout.columns.saturating_mul(16), luma_cuts, held.saturating_mul(16)));
    let Ok(Carved { coloured: blue_coloured, smoothed: blue_smoothed, drawn: blue_drawn }) = carved(&mut planes.blue, (layout.columns.saturating_mul(8), chroma_cuts, held.saturating_mul(8)));
    let Ok(Carved { coloured: red_coloured, smoothed: red_smoothed, drawn: red_drawn }) = carved(&mut planes.red, (layout.columns.saturating_mul(8), chroma_cuts, held.saturating_mul(8)));

    let [first, second, third] = bands;

    let (read_into, drawn_from, smoothed_from) = match round.wrapping_rem(3) {
        0 => (first, &*third, &*second),
        1 => (second, &*first, &*third),
        _ => (third, &*second, &*first),
    };

    let (alpha_into, alpha_from): (Option<&mut Vec<u8>>, &[u8]) = match round {
        0 => (Some(alpha), &[]),
        _ => (None, alpha.as_slice()),
    };

    let mut work = Vec::new();

    match reading {
        Some(band) => {
            let first_row = band.saturating_mul(ROUND);
            let count = layout.rows.saturating_sub(first_row).min(ROUND);

            work.push(Work::Read { reading: parsing, band: read_into, rows: (first_row, count) });
        },
        None => {},
    }

    match (transparency, alpha_into) {
        (Some(chunk), Some(alpha)) => work.push(Work::Transparency { chunk, size: layout.size, alpha }),
        (_, _) => {},
    }

    match drawing {
        Some(band) => {
            let rows = (band.saturating_mul(ROUND), layout.columns);

            work.push(Work::Predicted { layer: Component::Luma(Lines { pixels: luma_drawn, above: &mut above.luma }), band: drawn_from, rows });
            work.push(Work::Predicted {
                layer: Component::Chroma(Lines { pixels: blue_drawn, above: &mut above.blue }, Lines { pixels: red_drawn, above: &mut above.red }),
                band: drawn_from,
                rows,
            });
        },
        None => {},
    }

    match (smoothing, layout.filter) {
        (None, _) | (_, Filter::Off) => {},
        (Some(band), filter) => {
            let first_row = band.saturating_mul(ROUND);

            let rows = match band {
                0 => (0, 0),
                _ => (first_row, ABOVE_AN_EDGE),
            };

            let right = layout.kept.origin.x.saturating_add(layout.kept.size.width);
            let luma_reach = right.saturating_add(3).div_ceil(16).min(layout.columns);
            let chroma_reach = right.div_ceil(2).saturating_add(4).div_ceil(8).min(layout.columns);
            let luma = Smoothed::Luma(Plane { pixels: luma_smoothed, stride: layout.columns.saturating_mul(16) });

            work.push(Work::Smoothed { layer: luma, band: smoothed_from, smoothing: (filter, layout.columns, luma_reach), rows });

            match filter {
                Filter::Normal => {
                    let stride = layout.columns.saturating_mul(8);
                    let chroma = Smoothed::Chroma(Plane { pixels: blue_smoothed, stride }, Plane { pixels: red_smoothed, stride });

                    work.push(Work::Smoothed { layer: chroma, band: smoothed_from, smoothing: (filter, layout.columns, chroma_reach), rows });
                },
                Filter::Off | Filter::Simple => {},
            }
        },
    }

    match colouring {
        Some(band) => {
            let Ok((first_row, _)) = kept_rows(band, layout);
            let Ok(line) = index(layout.kept.size.width.saturating_mul(4));
            let Ok(wide) = index(layout.size.width);
            let Ok(round_rows) = index(ROUND.saturating_mul(16));
            let Ok(band_at) = index(band);
            let Ok(top) = index(first_row);

            let into = match destination {
                Destination::Picture => match rgba.chunks_mut(line.saturating_mul(round_rows).max(1)).nth(band_at) {
                    Some(rgba) => Rows::Picture(rgba),
                    None => Rows::Picture(&mut []),
                },
                Destination::Bands(receiver, _) => Rows::Streamed(rgba, &mut **receiver),
            };

            let alpha = match alpha_from.get(top.saturating_mul(wide)..) {
                Some(alpha) => alpha,
                None => &[],
            };

            let (origin, _) = chroma_cuts.coloured;
            let columns = Columns { left: layout.kept.origin.x, width: layout.kept.size.width };

            let luma = Luma { pixels: luma_coloured, stride: layout.columns.saturating_mul(16) };
            let chroma = Chroma { blue: blue_coloured, red: red_coloured, stride: layout.columns.saturating_mul(8), origin };

            work.push(Work::Coloured { into, planes: (luma, chroma, alpha), rows: (layout.size, first_row, columns) });
        },
        None => {},
    }

    Ok(work.into_iter().map(|work| Task { work: Mutex::new(work) }).collect())
}

fn moved_up(decoding: &mut Decoding<'_>, at: (Layout, u32)) -> Result<(), Never> {
    let (layout, round) = at;

    match round.saturating_sub(decoding.held_from) > HELD {
        true => {},
        false => return Ok(()),
    }

    let keep_from = round.saturating_sub(BEHIND.saturating_add(1));
    let dropped = keep_from.saturating_sub(decoding.held_from).saturating_mul(ROUND);
    let planes = &mut decoding.planes;

    for (plane, side) in [(planes.luma.as_mut_slice(), 16u32), (planes.blue.as_mut_slice(), 8), (planes.red.as_mut_slice(), 8)] {
        let Ok(from) = index(u64::from(dropped.saturating_mul(side)).saturating_mul(u64::from(layout.columns.saturating_mul(side))));

        plane.copy_within(from.min(plane.len()).., 0);
    }

    decoding.held_from = keep_from;

    Ok(())
}

fn kept_rows(band: u32, layout: Layout) -> Result<(u32, u32), Never> {
    let tall = ROUND.saturating_mul(16);
    let top = layout.kept.origin.y;
    let bottom = top.saturating_add(layout.kept.size.height).min(layout.size.height);
    let from = band.saturating_mul(tall).max(top);
    let to = band.saturating_add(1).saturating_mul(tall).min(bottom);

    Ok((from, to.max(from)))
}

fn within(band: Option<u32>, layout: Layout) -> Result<Option<u32>, Never> {
    Ok(match band {
        Some(band) => match band < layout.bands {
            true => Some(band),
            false => None,
        },
        None => None,
    })
}

fn cuts(bands: (Option<u32>, Option<u32>, Option<u32>), plane: (Layout, u32)) -> Result<Cuts, Never> {
    let (drawing, smoothing, colouring) = bands;
    let (layout, side) = plane;
    let tall = ROUND.saturating_mul(side);
    let lines = layout.rows.saturating_mul(side);

    let drawn = match drawing {
        Some(band) => (band.saturating_mul(tall), band.saturating_add(1).saturating_mul(tall).min(lines)),
        None => (lines, lines),
    };

    let smoothed = match smoothing {
        Some(band) => (band.saturating_mul(tall).saturating_sub(ABOVE_AN_EDGE), band.saturating_add(1).saturating_mul(tall).min(lines)),
        None => (drawn.0, drawn.0),
    };

    let coloured = match (colouring, side) {
        (Some(band), 16) => {
            let Ok(kept) = kept_rows(band, layout);

            kept
        },
        (Some(band), _) => {
            let Ok((from, to)) = kept_rows(band, layout);

            match to > from {
                true => (from.wrapping_shr(1).saturating_sub(1), to.saturating_sub(1).wrapping_shr(1).saturating_add(2).min(lines)),
                false => (from.wrapping_shr(1), from.wrapping_shr(1)),
            }
        },
        (None, _) => (smoothed.0, smoothed.0),
    };

    Ok(Cuts { coloured, smoothed, drawn })
}

fn carved(plane: &mut [u8], cutting: (u32, Cuts, u32)) -> Result<Carved<'_>, Never> {
    let (stride, Cuts { coloured, smoothed, drawn }, held) = cutting;
    let line = u64::from(stride);
    let at = |line_at: u32| u64::from(line_at.saturating_sub(held)).saturating_mul(line);
    let Ok(coloured_from) = index(at(coloured.0));
    let Ok(coloured_to) = index(at(coloured.1));
    let Ok(smoothed_from) = index(at(smoothed.0));
    let Ok(smoothed_to) = index(at(smoothed.1));
    let Ok(drawn_from) = index(at(drawn.0));
    let Ok(drawn_to) = index(at(drawn.1));

    let (below, drawing) = plane.split_at_mut(drawn_from.min(plane.len()));

    let drawn = match drawing.get_mut(..drawn_to.saturating_sub(drawn_from)) {
        Some(drawn) => drawn,
        None => &mut [],
    };

    let (above, smoothing) = below.split_at_mut(smoothed_from.min(below.len()));

    let smoothed = match smoothing.get_mut(..smoothed_to.saturating_sub(smoothed_from)) {
        Some(smoothed) => smoothed,
        None => &mut [],
    };

    let coloured = match above.get(coloured_from..coloured_to) {
        Some(coloured) => coloured,
        None => &[],
    };

    Ok(Carved { coloured, smoothed, drawn })
}
