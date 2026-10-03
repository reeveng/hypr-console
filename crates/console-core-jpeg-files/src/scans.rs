//! A scan: the entropy-coded data after a start-of-scan marker, read a block at
//! a time.
//!
//! A baseline JPEG is one scan, or one per component, and each block in it is
//! whole when it is read: its average, then its other frequencies in zigzag
//! order until an end-of-block says the rest are nothing. Every frequency is
//! read, because the next block starts where this one ends, but only the ones
//! the drawn size needs are kept, and a round of rows of them is handed to
//! [`crate::strips`] to be drawn while the next round is read.
//!
//! A progressive JPEG sends every block several times: the averages first,
//! then bands of frequencies, then the low bits of what was already sent. No
//! block is whole until the last scan, so the frequencies are gathered for
//! every block and drawn once the file ends, a round at a time like a scan.
//! That costs the memory of the whole picture in frequencies, which is why
//! [`crate::frame`] counts it before any of it is taken.
//!
//! A plane is held a round of rows at a time when one scan carries every
//! component, which is how a camera writes one. A file that sends each
//! component in a scan of its own holds every plane whole, because the first
//! component is finished before the last one starts.

use std::num::NonZeroU32;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::assembly::{self, Colors, Paint, Painting};
use crate::bits::Bits;
use crate::exif::Orientation;
use crate::frame::{self, Frame, Layout, Process};
use crate::huffman::{Huffman, Step};
use crate::kept::{self, Prepared, UNKEPT};
use crate::segments::Tables;
use crate::strips::{self, Beside, Drawing, Read, Reading, Row, Spread};
use crate::{JpegError, Picture};

const LAST: u32 = 63;

const UNGREYED: u8 = 128;

const ZERO_RUN: u32 = 15;

const ROUND: u32 = 32;

pub(crate) struct Image {
    pub(crate) frame: Frame,
    pub(crate) geometry: frame::Geometry,
    pub(crate) planes: Vec<Plane>,
    pub(crate) store: Store,
    pub(crate) painting: Painting,
}

pub(crate) struct Plane {
    pub(crate) samples: Vec<u8>,
    pub(crate) top: u32,
    rows: u32,
    holding: Holding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holding {
    Whole,
    Rolling,
}

pub(crate) enum Store {
    Straight,
    Gathering(Vec<Vec<[i16; 64]>>),
}

pub(crate) struct Source<'s> {
    pub(crate) bytes: &'s [u8],
    pub(crate) at: u32,
    pub(crate) tables: &'s Tables,
    pub(crate) restart: Option<NonZeroU32>,
    pub(crate) adobe: Option<u8>,
    pub(crate) spread: Spread<'s>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Member {
    component: u32,
    dc: u8,
    ac: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Band {
    start: u32,
    end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Spectral {
    band: Band,
    low: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Sequential,
    DcFirst,
    DcRefine,
    AcFirst,
    AcRefine,
}

pub(crate) struct Scan {
    members: Vec<Member>,
    spectral: Spectral,
    pass: Pass,
}

struct Walk<'d> {
    bits: Bits<'d>,
    predictions: [i32; 4],
    empty_bands: u32,
}

struct Meanwhile<'b> {
    reading: Beside<'b>,
    colors: Option<Colors>,
    band: &'b mut Vec<u8>,
}

struct NextRound<'n, 'r, 'd, 't> {
    walking: &'n mut Walking<'r, 'd, 't>,
    rows: &'n mut Vec<Row>,
}

impl Reading for NextRound<'_, '_, '_, '_> {
    fn read(&mut self) -> Result<(), JpegError> {
        walked(self.walking, self.rows)
    }
}

struct Walking<'r, 'd, 't> {
    walk: Walk<'d>,
    counted: u32,
    reached: u32,
    restart: Option<NonZeroU32>,
    members: &'r [Ready<'t>],
    drawings: &'r [Drawing],
    units: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Codes<'h> {
    dc: &'h Huffman,
    ac: &'h Huffman,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tools<'t> {
    Sequential { codes: Codes<'t>, prepared: Box<Prepared> },
    DcFirst { dc: &'t Huffman },
    DcRefine,
    AcFirst { ac: &'t Huffman },
    AcRefine { ac: &'t Huffman },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Ready<'t> {
    member: Member,
    layout: Layout,
    tools: Tools<'t>,
}

struct Scanning<'r, 't> {
    members: &'r [Ready<'t>],
    spectral: Spectral,
    colors: Option<Colors>,
}

pub(crate) fn image(frame: Frame, covering: Size<u32>, orientation: Orientation) -> Result<Image, JpegError> {
    let Ok(stored) = orientation.upright(covering);
    let Ok(eighths) = frame::eighths(frame.size, stored);
    let geometry = frame::geometry(&frame, eighths)?;

    frame::affordable(&geometry, frame.process)?;

    let mut gathered = Vec::new();

    for layout in &geometry.layouts {
        let Ok(blocks) = index(u64::from(layout.grid.width).saturating_mul(u64::from(layout.grid.height)));

        match frame.process {
            Process::Sequential => {},
            Process::Progressive => gathered.push(vec![[0i16; 64]; blocks]),
        }
    }

    let store = match frame.process {
        Process::Sequential => Store::Straight,
        Process::Progressive => Store::Gathering(gathered),
    };

    let Ok(painting) = assembly::painting(&geometry, orientation);

    Ok(Image { frame, geometry, planes: Vec::new(), store, painting })
}

fn planes(image: &mut Image, holding: Holding) -> Result<(), Never> {
    match image.planes.is_empty() {
        true => {},
        false => return Ok(()),
    }

    for layout in &image.geometry.layouts {
        let Ok(plane) = frame::plane(*layout);
        let Ok(strip) = strip_rows(*layout);

        let rows = match holding {
            Holding::Whole => plane.height,
            Holding::Rolling => ROUND.saturating_add(1).saturating_mul(strip).min(plane.height),
        };

        let Ok(area) = index(u64::from(plane.width).saturating_mul(u64::from(rows)));

        image.planes.push(Plane { samples: vec![UNGREYED; area], top: 0, rows, holding });
    }

    Ok(())
}

fn strip_rows(layout: Layout) -> Result<u32, Never> {
    Ok(layout.sampling.height.saturating_mul(layout.drawn.height))
}

fn drawn_rows(image: &Image, units: u32) -> Result<Vec<u32>, Never> {
    Ok(image
        .geometry
        .layouts
        .iter()
        .map(|layout| {
            let Ok(strip) = strip_rows(*layout);
            let Ok(plane) = frame::plane(*layout);

            units.saturating_mul(strip).min(plane.height)
        })
        .collect())
}

fn slid(image: &mut Image, ready: &[u32]) -> Result<(), JpegError> {
    let Image { geometry, planes, painting, .. } = image;
    let done = painting.colored >= geometry.output.height;

    for ((plane, layout), ready) in planes.iter_mut().zip(&geometry.layouts).zip(ready) {
        let Ok(size) = frame::plane(*layout);
        let Ok(strip) = strip_rows(*layout);
        let Ok((first, _)) = assembly::needed(*layout, painting.colored);

        let keep = match (plane.holding, done) {
            (Holding::Rolling, false) => first.min(*ready).max(plane.top),
            (Holding::Rolling, true) | (Holding::Whole, _) => continue,
        };

        let Ok(from) = index(u64::from(keep.saturating_sub(plane.top)).saturating_mul(u64::from(size.width)));
        let Ok(to) = index(u64::from(ready.saturating_sub(plane.top)).saturating_mul(u64::from(size.width)));
        let rows = ready.saturating_sub(keep).saturating_add(ROUND.saturating_mul(strip)).min(size.height);
        let Ok(room) = index(u64::from(rows).saturating_mul(u64::from(size.width)));

        match plane.samples.get(from..to).is_some() {
            true => plane.samples.copy_within(from..to, 0),
            false => return Err(JpegError::Corrupt),
        }

        plane.top = keep;

        match plane.rows < rows {
            true => {
                plane.samples.resize(room, UNGREYED);
                plane.rows = rows;
            },
            false => {},
        }
    }

    Ok(())
}

pub(crate) fn scan(payload: &[u8], frame: &Frame) -> Result<Scan, JpegError> {
    let (count, rest) = match payload.split_first() {
        Some((count, rest)) => (*count, rest),
        None => return Err(JpegError::Truncated),
    };

    let Ok(listed) = index(u32::from(count).saturating_mul(2));

    let (pairs, tail) = match (count, rest.split_at_checked(listed)) {
        (1..=4, Some(split)) => split,
        (1..=4, None) => return Err(JpegError::Truncated),
        (_, _) => return Err(JpegError::Corrupt),
    };

    let (start, end, approximation) = match tail {
        [start, end, approximation, ..] => (u32::from(*start), u32::from(*end), *approximation),
        _ => return Err(JpegError::Truncated),
    };

    let members = members(pairs, frame)?;
    let spectral = Spectral { band: Band { start, end }, low: u32::from(approximation & 0x0F) };
    let pass = pass(frame.process, spectral, u32::from(approximation.wrapping_shr(4)), &members)?;

    Ok(Scan { members, spectral, pass })
}

fn members(pairs: &[u8], frame: &Frame) -> Result<Vec<Member>, JpegError> {
    let mut by_id: [Option<u32>; 256] = [None; 256];

    for (at, component) in (0u32..).zip(&frame.components) {
        let Ok(id) = index(component.id);

        match by_id.get_mut(id) {
            Some(slot) => *slot = Some(at),
            None => return Err(JpegError::Corrupt),
        }
    }

    let mut members = Vec::new();

    for [selector, tables] in pairs.as_chunks::<2>().0 {
        let Ok(id) = index(*selector);
        let tables = *tables;

        match by_id.get(id) {
            Some(Some(component)) => members.push(Member { component: *component, dc: tables.wrapping_shr(4), ac: tables & 0x0F }),
            Some(None) | None => return Err(JpegError::Corrupt),
        }
    }

    Ok(members)
}

fn pass(process: Process, spectral: Spectral, high: u32, members: &[Member]) -> Result<Pass, JpegError> {
    let Spectral { band, low } = spectral;

    match (process, low <= 13) {
        (Process::Sequential, _) => Ok(Pass::Sequential),
        (Process::Progressive, false) => Err(JpegError::Corrupt),
        (Process::Progressive, true) => match (band.start, band.end, high) {
            (0, 0, 0) => Ok(Pass::DcFirst),
            (0, 0, _) => Ok(Pass::DcRefine),
            (0, _, _) => Err(JpegError::Corrupt),
            (_, _, _) => match (members, band.start <= band.end && band.end <= LAST, high) {
                ([_], true, 0) => Ok(Pass::AcFirst),
                ([_], true, _) => Ok(Pass::AcRefine),
                (_, _, _) => Err(JpegError::Corrupt),
            },
        },
    }
}

fn table(held: &[Option<Huffman>; 4], at: u8) -> Result<&Huffman, JpegError> {
    let Ok(at) = index(at);

    match held.get(at) {
        Some(Some(table)) => Ok(table),
        Some(None) | None => Err(JpegError::Corrupt),
    }
}

fn quantization(tables: &Tables, at: u8) -> Result<&[u16; 64], JpegError> {
    let Ok(at) = index(at);

    match tables.quantization.get(at) {
        Some(Some(steps)) => Ok(steps),
        Some(None) | None => Err(JpegError::Corrupt),
    }
}

fn ready<'t>(source: &Source<'t>, image: &Image, pass: Pass, member: Member) -> Result<Ready<'t>, JpegError> {
    let Ok(slot) = index(member.component);

    let (component, layout) = match (image.frame.components.get(slot), image.geometry.layouts.get(slot)) {
        (Some(component), Some(layout)) => (component, *layout),
        (None, _) | (_, None) => return Err(JpegError::Corrupt),
    };

    let tools = match pass {
        Pass::Sequential => {
            let dc = table(&source.tables.dc, member.dc)?;
            let ac = table(&source.tables.ac, member.ac)?;
            let quantization = quantization(source.tables, component.quantization)?;
            let Ok(prepared) = kept::prepared(quantization, layout.drawn);

            Tools::Sequential { codes: Codes { dc, ac }, prepared: Box::new(prepared) }
        },
        Pass::DcFirst => {
            let dc = table(&source.tables.dc, member.dc)?;

            Tools::DcFirst { dc }
        },
        Pass::DcRefine => Tools::DcRefine,
        Pass::AcFirst => {
            let ac = table(&source.tables.ac, member.ac)?;

            Tools::AcFirst { ac }
        },
        Pass::AcRefine => {
            let ac = table(&source.tables.ac, member.ac)?;

            Tools::AcRefine { ac }
        },
    };

    Ok(Ready { member, layout, tools })
}

pub(crate) fn decode(source: &Source<'_>, image: &mut Image, scan: &Scan) -> Result<u32, JpegError> {
    let mut members = Vec::new();

    for member in &scan.members {
        let one = ready(source, image, scan.pass, *member)?;

        members.push(one);
    }

    let units = match members.as_slice() {
        [only] => only.layout.filled,
        _ => image.geometry.units,
    };

    let colors = match scan.pass {
        Pass::Sequential => held(image, &members, source.adobe)?,
        Pass::DcFirst | Pass::DcRefine | Pass::AcFirst | Pass::AcRefine => None,
    };

    let Ok(bits) = Bits::new(source.bytes, source.at);
    let walk = Walk { bits, predictions: [0; 4], empty_bands: 0 };
    let scanning = Scanning { members: &members, spectral: scan.spectral, colors };

    let walk = match scan.pass {
        Pass::Sequential => straight(source, image, &scanning, (walk, units))?,
        Pass::DcFirst | Pass::DcRefine | Pass::AcFirst | Pass::AcRefine => spread_out(source, image, &scanning, (walk, units))?,
    };

    let Ok(at) = walk.bits.at();

    Ok(at)
}

fn held(image: &mut Image, members: &[Ready<'_>], adobe: Option<u8>) -> Result<Option<Colors>, JpegError> {
    let every = members.len() == image.frame.components.len();

    let holding = match every {
        true => Holding::Rolling,
        false => Holding::Whole,
    };

    let Ok(()) = planes(image, holding);

    match (image.planes.first().map(|plane| plane.holding), every) {
        (Some(Holding::Rolling), true) => {
            let colors = assembly::colors(&image.frame, adobe)?;

            Ok(Some(colors))
        },
        (Some(Holding::Rolling), false) => Err(JpegError::Corrupt),
        (Some(Holding::Whole) | None, _) => Ok(None),
    }
}

fn painted_round(image: &mut Image, colors: Option<Colors>, units: u32, spread: Spread<'_>) -> Result<(), JpegError> {
    let colors = match colors {
        Some(colors) => colors,
        None => return Ok(()),
    };

    let Ok(ready) = drawn_rows(image, units);

    assembly::painted(image, &ready, colors, spread)?;

    slid(image, &ready)
}

fn spread_out<'d>(
    source: &Source<'_>,
    image: &mut Image,
    scanning: &Scanning<'_, '_>,
    start: (Walk<'d>, Size<u32>),
) -> Result<Walk<'d>, JpegError> {
    let (mut walk, units) = start;
    let mut counted = 0u32;

    for down in 0..units.height {
        for across in 0..units.width {
            let Ok(()) = restarted(&mut walk, source.restart, counted);

            unit(&mut walk, scanning, image, Point { x: across, y: down })?;

            counted = counted.saturating_add(1);
        }
    }

    Ok(walk)
}

fn straight<'d>(
    source: &Source<'_>,
    image: &mut Image,
    scanning: &Scanning<'_, '_>,
    start: (Walk<'d>, Size<u32>),
) -> Result<Walk<'d>, JpegError> {
    let (walk, units) = start;
    let Ok(drawings) = drawings(scanning.members);
    let Ok(round) = index(ROUND);
    let mut walking = Walking { walk, counted: 0, reached: 0, restart: source.restart, members: scanning.members, drawings: &drawings, units };
    let mut rows: Vec<Row> = Vec::new();
    let mut next: Vec<Row> = Vec::new();

    let mut band = Vec::new();

    walked(&mut walking, &mut rows)?;

    for _round in (0..units.height).step_by(round) {
        let drawn = match rows.last() {
            Some(row) => row.y.saturating_add(1),
            None => return Err(JpegError::Corrupt),
        };

        let meanwhile = Meanwhile { reading: &mut NextRound { walking: &mut walking, rows: &mut next }, colors: scanning.colors, band: &mut band };

        drawn_beside(image, (&rows, &drawings), meanwhile, source.spread)?;

        let Ok(ready) = drawn_rows(image, drawn);

        slid(image, &ready)?;

        std::mem::swap(&mut rows, &mut next);
    }

    painted_round(image, scanning.colors, units.height, source.spread)?;

    Ok(walking.walk)
}

fn drawn_beside(image: &mut Image, round: (&[Row], &[Drawing]), meanwhile: Meanwhile<'_>, spread: Spread<'_>) -> Result<(), JpegError> {
    let (rows, drawings) = round;

    let first = match rows.first() {
        Some(row) => row.y,
        None => return Ok(()),
    };

    let Ok(before) = drawn_rows(image, first);
    let Ok(cuts) = cuts(image, drawings, first);
    let Image { geometry, planes, painting, .. } = image;
    let Ok(through) = assembly::through(geometry, &before, painting.colored);
    let mut held = Vec::new();
    let mut below = Vec::new();

    for ((plane, layout), cut) in planes.iter_mut().zip(&geometry.layouts).zip(cuts) {
        let (above, rest) = strips::parted(plane, *layout, cut)?;

        held.push(above);
        below.push(Some(rest));
    }

    let Ok(reading) = strips::beside(meanwhile.reading);
    let mut tasks = vec![reading];

    strips::drawing((rows, drawings), &mut below, &mut tasks)?;

    match meanwhile.colors {
        Some(colors) => assembly::coloring(Paint { held: &held, geometry, colors, through }, (&mut *painting, &mut *meanwhile.band), &mut tasks)?,
        None => {},
    }

    spread(&tasks)?;

    drop(tasks);

    match meanwhile.colors {
        Some(_) => assembly::set_down(painting, (meanwhile.band, geometry.output), through, spread),
        None => Ok(()),
    }
}

fn cuts(image: &Image, drawings: &[Drawing], first: u32) -> Result<Vec<Option<u32>>, Never> {
    let mut cuts: Vec<Option<u32>> = image.planes.iter().map(|_| None).collect();

    for drawing in drawings {
        let Ok(slot) = index(drawing.component);

        match cuts.get_mut(slot) {
            Some(cut) => *cut = Some(first.saturating_mul(drawing.each.height).saturating_mul(drawing.layout.drawn.height)),
            None => {},
        }
    }

    Ok(cuts)
}

fn walked(walking: &mut Walking<'_, '_, '_>, rows: &mut Vec<Row>) -> Result<(), JpegError> {
    let from = walking.reached;
    let until = from.saturating_add(ROUND).min(walking.units.height);
    let mut spare = std::mem::take(rows);

    for down in from..until {
        let mut row = match spare.pop() {
            Some(row) => row,
            None => Row { y: down, read: walking.drawings.iter().map(|_| Read::default()).collect() },
        };

        row.y = down;

        for (read, drawing) in row.read.iter_mut().zip(walking.drawings) {
            let Ok(blocks) = index(walking.units.width.saturating_mul(drawing.each.width).saturating_mul(drawing.each.height));
            let Ok(count) = index(drawing.prepared.kept.count);

            read.values.clear();
            read.marks.clear();
            read.values.resize(blocks.saturating_mul(count), 0);
        }

        for _across in 0..walking.units.width {
            let Ok(()) = restarted(&mut walking.walk, walking.restart, walking.counted);

            straight_unit(&mut walking.walk, walking.members, &mut row)?;

            walking.counted = walking.counted.saturating_add(1);
        }

        rows.push(row);
    }

    walking.reached = until;

    Ok(())
}

#[inline(always)]
fn straight_unit(walk: &mut Walk<'_>, members: &[Ready<'_>], row: &mut Row) -> Result<(), JpegError> {
    for (ready, read) in members.iter().zip(row.read.iter_mut()) {
        let Ok(each) = each(members, ready);

        let (codes, prepared) = match &ready.tools {
            Tools::Sequential { codes, prepared } => (*codes, prepared),
            Tools::DcFirst { .. } | Tools::DcRefine | Tools::AcFirst { .. } | Tools::AcRefine { .. } => return Err(JpegError::Corrupt),
        };

        for _block in 0..each.width.saturating_mul(each.height) {
            sequential(walk, (codes, prepared), ready.member.component, read)?;
        }
    }

    Ok(())
}

fn each(members: &[Ready<'_>], ready: &Ready<'_>) -> Result<Size<u32>, Never> {
    Ok(match members {
        [_] => Size { width: 1, height: 1 },
        _ => ready.layout.sampling,
    })
}

fn drawings(members: &[Ready<'_>]) -> Result<Vec<Drawing>, Never> {
    Ok(members
        .iter()
        .filter_map(|ready| match &ready.tools {
            Tools::Sequential { prepared, .. } => {
                let Ok(each) = each(members, ready);

                Some(Drawing { component: ready.member.component, layout: ready.layout, each, prepared: **prepared })
            },
            Tools::DcFirst { .. } | Tools::DcRefine | Tools::AcFirst { .. } | Tools::AcRefine { .. } => None,
        })
        .collect())
}

#[inline(always)]
fn restarted(walk: &mut Walk<'_>, interval: Option<NonZeroU32>, counted: u32) -> Result<(), Never> {
    let due = match interval {
        Some(interval) => counted.checked_rem(interval.get()),
        None => None,
    };

    match (counted, due) {
        (1.., Some(0)) => {
            let Ok(()) = walk.bits.restart();

            walk.predictions = [0; 4];
            walk.empty_bands = 0;
        },
        (_, Some(_) | None) => {},
    }

    Ok(())
}

fn unit(walk: &mut Walk<'_>, scanning: &Scanning<'_, '_>, image: &mut Image, at: Point<u32>) -> Result<(), JpegError> {
    for ready in scanning.members {
        let Ok(each) = each(scanning.members, ready);

        for down in 0..each.height {
            for across in 0..each.width {
                let block = Point {
                    x: at.x.saturating_mul(each.width).saturating_add(across),
                    y: at.y.saturating_mul(each.height).saturating_add(down),
                };

                one_block(walk, scanning, image, (ready, block))?;
            }
        }
    }

    Ok(())
}

fn one_block(
    walk: &mut Walk<'_>,
    scanning: &Scanning<'_, '_>,
    image: &mut Image,
    placed: (&Ready<'_>, Point<u32>),
) -> Result<(), JpegError> {
    let (ready, block) = placed;
    let component = ready.member.component;
    let low = scanning.spectral.low;

    match &ready.tools {
        Tools::Sequential { .. } => Err(JpegError::Corrupt),
        Tools::DcFirst { dc } => {
            let coefficients = gathered(image, ready, block)?;

            dc_first(walk, dc, coefficients, (component, low))
        },
        Tools::DcRefine => {
            let coefficients = gathered(image, ready, block)?;
            let Ok(()) = dc_refine(walk, coefficients, low);

            Ok(())
        },
        Tools::AcFirst { ac } => {
            let coefficients = gathered(image, ready, block)?;

            ac_first(walk, ac, coefficients, scanning.spectral)
        },
        Tools::AcRefine { ac } => {
            let coefficients = gathered(image, ready, block)?;

            ac_refine(walk, ac, coefficients, scanning.spectral)
        },
    }
}

fn gathered<'i>(image: &'i mut Image, ready: &Ready<'_>, block: Point<u32>) -> Result<&'i mut [i16; 64], JpegError> {
    let Ok(slot) = index(ready.member.component);
    let at = block.y.saturating_mul(ready.layout.grid.width).saturating_add(block.x);
    let Ok(at) = index(at);

    let blocks = match &mut image.store {
        Store::Gathering(gathered) => gathered.get_mut(slot),
        Store::Straight => None,
    };

    match blocks.and_then(|blocks| blocks.get_mut(at)) {
        Some(coefficients) => Ok(coefficients),
        None => Err(JpegError::Corrupt),
    }
}

fn sequential(walk: &mut Walk<'_>, tools: (Codes<'_>, &Prepared), component: u32, read: &mut Read) -> Result<(), JpegError> {
    let (codes, prepared) = tools;
    let kept = &prepared.kept;
    let Ok(slot) = index(component);
    let Ok(count) = index(kept.count);
    let Ok(done) = fitted::<_, u32>(read.marks.len());
    let Ok(from) = index(done.saturating_mul(kept.count));

    let values = match read.values.get_mut(from..from.saturating_add(count)) {
        Some(values) => values,
        None => return Err(JpegError::Corrupt),
    };

    let mut bits = walk.bits;
    let category = codes.dc.decode(&mut bits)?;
    let Ok(difference) = bits.extended(u32::from(category));

    let average = match walk.predictions.get_mut(slot) {
        Some(prediction) => {
            *prediction = prediction.wrapping_add(difference);

            *prediction
        },
        None => return Err(JpegError::Corrupt),
    };

    match values.first_mut() {
        Some(first) => {
            let Ok(average) = fitted::<i32, i16>(average);

            *first = average;
        },
        None => return Err(JpegError::Corrupt),
    }

    let mut marks = 1u16;
    let mut zigzag = 1u32;

    for _symbol in 1..=LAST {
        match zigzag > kept.reach {
            true => break,
            false => {},
        }

        let Step { advance, value } = codes.ac.ac(&mut bits)?;
        let landed = zigzag.saturating_add(advance).saturating_sub(1);

        zigzag = zigzag.saturating_add(advance);

        match value {
            0 => {},
            _ => {
                let Ok(at) = index(landed);

                match (kept.slots.get(at), kept.marks.get(at)) {
                    (Some(&UNKEPT), _) => {},
                    (Some(slot), Some(mark)) => {
                        let Ok(place) = index(*slot);
                        let Ok(value) = fitted::<i32, i16>(value);

                        match values.get_mut(place) {
                            Some(kept) => *kept = value,
                            None => return Err(JpegError::Corrupt),
                        }

                        marks |= *mark;
                    },
                    (_, _) => return Err(JpegError::Corrupt),
                }
            },
        }
    }

    read.marks.push(marks);

    walk.bits = bits;

    passed(walk, codes.ac, zigzag)
}

fn passed(walk: &mut Walk<'_>, ac: &Huffman, from: u32) -> Result<(), JpegError> {
    let mut bits = walk.bits;
    let mut zigzag = from;

    for _symbol in 0..=LAST {
        match zigzag > LAST {
            true => break,
            false => {},
        }

        let Step { advance, value } = ac.ac(&mut bits)?;

        zigzag = zigzag.saturating_add(advance);

        match (value, zigzag.saturating_sub(1) > LAST) {
            (0, _) | (_, false) => {},
            (_, true) => return Err(JpegError::Corrupt),
        }
    }

    walk.bits = bits;

    Ok(())
}

fn dc_first(walk: &mut Walk<'_>, dc: &Huffman, coefficients: &mut [i16; 64], at: (u32, u32)) -> Result<(), JpegError> {
    let (component, low) = at;
    let Ok(slot) = index(component);
    let category = dc.decode(&mut walk.bits)?;
    let Ok(difference) = walk.bits.extended(u32::from(category));

    let average = match walk.predictions.get_mut(slot) {
        Some(prediction) => {
            *prediction = prediction.wrapping_add(difference);

            *prediction
        },
        None => return Err(JpegError::Corrupt),
    };

    let Ok(average) = fitted::<i32, i16>(average.wrapping_shl(low));

    match coefficients.first_mut() {
        Some(first) => *first = average,
        None => return Err(JpegError::Corrupt),
    }

    Ok(())
}

fn dc_refine(walk: &mut Walk<'_>, coefficients: &mut [i16; 64], low: u32) -> Result<(), Never> {
    let Ok(bit) = walk.bits.take(1);

    match (bit, coefficients.first_mut()) {
        (1.., Some(first)) => *first |= 1i16.wrapping_shl(low),
        (_, Some(_) | None) => {},
    }

    Ok(())
}

fn ac_first(walk: &mut Walk<'_>, ac: &Huffman, coefficients: &mut [i16; 64], spectral: Spectral) -> Result<(), JpegError> {
    match walk.empty_bands {
        0 => {},
        _ => {
            walk.empty_bands = walk.empty_bands.saturating_sub(1);

            return Ok(());
        },
    }

    let mut zigzag = spectral.band.start;

    for _symbol in 0..=LAST {
        match zigzag > spectral.band.end {
            true => break,
            false => {},
        }

        let symbol = ac.decode(&mut walk.bits)?;
        let run = u32::from(symbol.wrapping_shr(4));
        let size = u32::from(symbol & 0x0F);

        match (run, size) {
            (ZERO_RUN, 0) => zigzag = zigzag.saturating_add(16),
            (_, 0) => {
                let Ok(extra) = walk.bits.take(run);

                walk.empty_bands = 1u32.wrapping_shl(run).saturating_add(extra).saturating_sub(1);

                break;
            },
            (_, _) => {
                zigzag = zigzag.saturating_add(run);

                let Ok(value) = walk.bits.extended(size);
                let Ok(value) = fitted::<i32, i16>(value.wrapping_shl(spectral.low));
                let Ok(at) = index(zigzag);

                match coefficients.get_mut(at) {
                    Some(coefficient) => *coefficient = value,
                    None => return Err(JpegError::Corrupt),
                }

                zigzag = zigzag.saturating_add(1);
            },
        }
    }

    Ok(())
}

fn ac_refine(walk: &mut Walk<'_>, ac: &Huffman, coefficients: &mut [i16; 64], spectral: Spectral) -> Result<(), JpegError> {
    let reached = match walk.empty_bands {
        0 => refined(walk, ac, coefficients, spectral)?,
        _ => spectral.band.start,
    };

    match walk.empty_bands {
        0 => {},
        _ => {
            let Ok(()) = corrected_after(walk, coefficients, (spectral, reached));

            walk.empty_bands = walk.empty_bands.saturating_sub(1);
        },
    }

    Ok(())
}

fn refined(walk: &mut Walk<'_>, ac: &Huffman, coefficients: &mut [i16; 64], spectral: Spectral) -> Result<u32, JpegError> {
    let positive = 1i32.wrapping_shl(spectral.low);
    let mut zigzag = spectral.band.start;

    for _symbol in 0..=LAST {
        match zigzag > spectral.band.end {
            true => break,
            false => {},
        }

        let symbol = ac.decode(&mut walk.bits)?;
        let run = u32::from(symbol.wrapping_shr(4));

        let fresh = match (run, symbol & 0x0F) {
            (ZERO_RUN, 0) => None,
            (_, 0) => {
                let Ok(extra) = walk.bits.take(run);

                walk.empty_bands = 1u32.wrapping_shl(run).saturating_add(extra);

                break;
            },
            (_, _) => {
                let Ok(sign) = walk.bits.take(1);

                Some(match sign {
                    0 => positive.wrapping_neg(),
                    _ => positive,
                })
            },
        };

        let Ok(reached) = advanced(walk, coefficients, (spectral, zigzag, run));

        zigzag = reached;

        match fresh {
            Some(value) => {
                let Ok(at) = index(zigzag);
                let Ok(value) = fitted::<i32, i16>(value);

                match coefficients.get_mut(at) {
                    Some(coefficient) => *coefficient = value,
                    None => return Err(JpegError::Corrupt),
                }
            },
            None => {},
        }

        zigzag = zigzag.saturating_add(1);
    }

    Ok(zigzag)
}

fn advanced(walk: &mut Walk<'_>, coefficients: &mut [i16; 64], from: (Spectral, u32, u32)) -> Result<u32, Never> {
    let (spectral, mut zigzag, mut zeros) = from;
    let positive = 1i32.wrapping_shl(spectral.low);

    for _coefficient in 0..=LAST {
        match zigzag > spectral.band.end {
            true => break,
            false => {},
        }

        let Ok(at) = index(zigzag);

        match coefficients.get_mut(at) {
            Some(0) => match zeros {
                0 => break,
                _ => zeros = zeros.saturating_sub(1),
            },
            Some(coefficient) => {
                let Ok(()) = corrected(&mut walk.bits, coefficient, positive);
            },
            None => break,
        }

        zigzag = zigzag.saturating_add(1);
    }

    Ok(zigzag)
}

fn corrected(bits: &mut Bits<'_>, coefficient: &mut i16, positive: i32) -> Result<(), Never> {
    let Ok(bit) = bits.take(1);
    let current = i32::from(*coefficient);

    match (bit, current & positive, current >= 0) {
        (0, _, _) | (_, 1.., _) | (_, i32::MIN..=-1, _) => {},
        (_, 0, true) => {
            let Ok(moved) = fitted::<i32, i16>(current.wrapping_add(positive));

            *coefficient = moved;
        },
        (_, 0, false) => {
            let Ok(moved) = fitted::<i32, i16>(current.wrapping_sub(positive));

            *coefficient = moved;
        },
    }

    Ok(())
}

fn corrected_after(walk: &mut Walk<'_>, coefficients: &mut [i16; 64], from: (Spectral, u32)) -> Result<(), Never> {
    let (spectral, reached) = from;
    let positive = 1i32.wrapping_shl(spectral.low);
    let Ok(start) = index(reached);
    let Ok(past) = index(spectral.band.end.saturating_add(1));

    match coefficients.get_mut(start..past) {
        Some(rest) => {
            for coefficient in rest {
                match *coefficient {
                    0 => {},
                    _ => {
                        let Ok(()) = corrected(&mut walk.bits, coefficient, positive);
                    },
                }
            }
        },
        None => {},
    }

    Ok(())
}

pub(crate) fn finished(image: Image, tables: &Tables, colors: Colors, spread: Spread<'_>) -> Result<Picture, JpegError> {
    let Image { frame, geometry, planes: held, store, painting } = image;
    let mut image = Image { frame, geometry, planes: held, store: Store::Straight, painting };

    match store {
        Store::Straight => {},
        Store::Gathering(gathered) => drawn_gathered(&mut image, &gathered, tables, colors, spread)?,
    }

    let Ok(()) = planes(&mut image, Holding::Whole);

    let ready: Vec<u32> = image
        .planes
        .iter()
        .zip(&image.geometry.layouts)
        .map(|(plane, layout)| {
            let Ok(size) = frame::plane(*layout);

            plane.top.saturating_add(plane.rows).min(size.height)
        })
        .collect();

    assembly::painted(&mut image, &ready, colors, spread)?;

    let Ok(upright) = image.painting.orientation.upright(image.geometry.output);

    Ok(Picture { size: upright, rgba: image.painting.rgba })
}

fn drawn_gathered(image: &mut Image, gathered: &[Vec<[i16; 64]>], tables: &Tables, colors: Colors, spread: Spread<'_>) -> Result<(), JpegError> {
    let mut drawings = Vec::new();

    for ((at, component), layout) in (0u32..).zip(&image.frame.components).zip(&image.geometry.layouts) {
        let quantization = quantization(tables, component.quantization)?;
        let Ok(prepared) = kept::prepared(quantization, layout.drawn);
        let each = Size { width: 1, height: 1 };

        drawings.push(Drawing { component: at, layout: *layout, each, prepared });
    }

    let Ok(()) = planes(image, Holding::Rolling);
    let units = image.geometry.units.height;
    let Ok(round) = index(ROUND);

    for first in (0..units).step_by(round) {
        let last = first.saturating_add(ROUND).min(units);
        let mut tasks = Vec::new();

        for ((drawing, plane), blocks) in drawings.iter().zip(image.planes.iter_mut()).zip(gathered) {
            strips::gathered(plane, blocks, drawing, (first, last), &mut tasks)?;
        }

        spread(&tasks)?;

        drop(tasks);

        painted_round(image, Some(colors), last, spread)?;
    }

    Ok(())
}
