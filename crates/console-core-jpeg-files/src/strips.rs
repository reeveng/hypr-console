//! Blocks drawn into their planes a strip at a time, by as many hands as the
//! caller has.
//!
//! Reading a scan is one walk through its bits: where a block ends is only
//! known by reading it, so nothing after it can start before it is read. Drawing
//! what was read is not like that. A block's pixels depend on that block alone,
//! so a row of blocks can be drawn while the next row is drawn somewhere else,
//! into rows of the plane that no other strip touches.
//!
//! So the scan is read a round of rows at a time, keeping of each block only
//! the frequencies its drawn size needs, and each round is handed back as tasks:
//! the reading of the next round, first; the rows of each component and the
//! strip of its plane they are drawn into; and the color of the rows the round
//! before finished. The read is the one piece that cannot be split, so the rest
//! is done while it goes, and a photograph spread across the cores takes about
//! as long as its bits take to read. [`crate::decoded`] does the tasks one after
//! another; [`crate::decoded_spread`] hands them to a function of the caller's,
//! which is where how many threads there are is decided -- never in here.
//!
//! A block drawn an eighth of its size is its average alone, one pixel, so a
//! row of them is set straight from the averages rather than a block at a time
//! through the inverse DCT.

use std::sync::Mutex;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::JpegError;
use crate::assembly::{self, Coloring, Window, Turning};
use crate::frame::{self, Layout};
use crate::idct::{self, Block, Target};
use crate::kept::{DETAILED, Prepared};
use crate::scans::Plane;

pub struct Task<'a> {
    strip: Mutex<&'a mut [u8]>,
    work: Work<'a>,
}

pub type Spread<'s> = &'s dyn Fn(&[Task<'_>]) -> Result<(), JpegError>;

pub(crate) trait Reading: Send {
    fn read(&mut self) -> Result<(), JpegError>;
}

pub(crate) type Beside<'b> = &'b mut (dyn Reading + 'b);

pub(crate) enum Work<'a> {
    Read { rows: &'a [Row], member: u32, drawing: &'a Drawing },
    Gathered { coefficients: &'a [[i16; 64]], drawing: &'a Drawing },
    Colored(Coloring<'a>),
    Turned(Turning<'a>),
    Beside(Mutex<Beside<'a>>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Read {
    pub(crate) values: Vec<i16>,
    pub(crate) marks: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) y: u32,
    pub(crate) read: Vec<Read>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Drawing {
    pub(crate) component: u32,
    pub(crate) layout: Layout,
    pub(crate) each: Size<u32>,
    pub(crate) prepared: Prepared,
}

pub(crate) fn task<'a>(strip: &'a mut [u8], work: Work<'a>) -> Result<Task<'a>, Never> {
    Ok(Task { strip: Mutex::new(strip), work })
}

pub(crate) fn one_after_another(tasks: &[Task<'_>]) -> Result<(), JpegError> {
    tasks.iter().try_for_each(Task::done)
}

impl Task<'_> {
    pub fn done(&self) -> Result<(), JpegError> {
        let mut strip = match self.strip.lock() {
            Ok(strip) => strip,
            Err(_another_job_panicked) => return Err(JpegError::Corrupt),
        };

        let strip: &mut [u8] = &mut strip;

        match &self.work {
            Work::Read { rows, member, drawing } => read_drawn(strip, (rows, *member), drawing),
            Work::Gathered { coefficients, drawing } => gathered_drawn(strip, coefficients, drawing),
            Work::Colored(coloring) => assembly::colored(strip, coloring),
            Work::Turned(turning) => assembly::turned_into(strip, turning),
            Work::Beside(beside) => match beside.lock() {
                Ok(mut beside) => beside.read(),
                Err(_another_job_panicked) => Err(JpegError::Corrupt),
            },
        }
    }
}

fn read_drawn(strip: &mut [u8], read: (&[Row], u32), drawing: &Drawing) -> Result<(), JpegError> {
    let (rows, member) = read;
    let Ok(member) = index(member);
    let length = strip_length(drawing.layout, drawing.each.height)?;
    let Ok(length) = index(length);

    for (row, strip) in rows.iter().zip(strip.chunks_mut(length)) {
        match row.read.get(member) {
            Some(read) => row_drawn(strip, read, drawing)?,
            None => return Err(JpegError::Corrupt),
        }
    }

    Ok(())
}

fn row_drawn(strip: &mut [u8], read: &Read, drawing: &Drawing) -> Result<(), JpegError> {
    match (drawing.prepared.kept.count, drawing.layout.drawn) {
        (1, Size { width: 1, height: 1 }) => return averages_drawn(strip, read, drawing),
        (_, _) => {},
    }

    let Ok(count) = index(drawing.prepared.kept.count);
    let Ok(mut scratch) = idct::empty();

    let mut blocks = match count {
        0 => return Err(JpegError::Corrupt),
        _ => read.values.chunks(count).zip(&read.marks),
    };

    'units: for unit in 0..drawing.layout.grid.width {
        for down in 0..drawing.each.height {
            for across in 0..drawing.each.width {
                let (values, marks) = match blocks.next() {
                    Some(block) => block,
                    None => break 'units,
                };

                let at = Point { x: unit.saturating_mul(drawing.each.width).saturating_add(across), y: down };

                drawn((values, *marks), drawing, &mut scratch, (strip, at))?;
            }
        }
    }

    Ok(())
}

fn averages_drawn(strip: &mut [u8], read: &Read, drawing: &Drawing) -> Result<(), JpegError> {
    let Ok(size) = frame::plane(drawing.layout);
    let Ok(stride) = index(size.width);
    let Ok(wide) = index(drawing.each.width);
    let Ok(tall) = index(drawing.each.height);

    let step = match drawing.prepared.steps.first() {
        Some(step) => *step,
        None => return Err(JpegError::Corrupt),
    };

    let mut lines: Vec<&mut [u8]> = strip.chunks_mut(stride).take(tall).collect();

    for (unit, averages) in read.values.chunks(wide.saturating_mul(tall)).enumerate() {
        for (line, averages) in lines.iter_mut().zip(averages.chunks(wide)) {
            let pixels = match line.get_mut(unit.saturating_mul(wide)..) {
                Some(pixels) => pixels,
                None => return Err(JpegError::Corrupt),
            };

            for (pixel, average) in pixels.iter_mut().zip(averages) {
                let Ok(level) = idct::level(i32::from(average.wrapping_mul(step)));

                *pixel = level;
            }
        }
    }

    Ok(())
}

fn gathered_drawn(strip: &mut [u8], coefficients: &[[i16; 64]], drawing: &Drawing) -> Result<(), JpegError> {
    let kept = &drawing.prepared.kept;
    let Ok(count) = index(kept.count);
    let Ok(mut scratch) = idct::empty();
    let mut values = [0i16; 64];

    for (across, coefficients) in (0u32..).zip(coefficients) {
        let mut marks = 0u16;

        for (value, zigzag) in values.iter_mut().zip(&kept.zigzags).take(count) {
            let Ok(at) = index(*zigzag);

            *value = match coefficients.get(at) {
                Some(value) => *value,
                None => return Err(JpegError::Corrupt),
            };

            match (*value, kept.marks.get(at)) {
                (0, _) | (_, None) => {},
                (_, Some(mark)) => marks |= *mark,
            }
        }

        let block = match values.get(..count) {
            Some(block) => block,
            None => return Err(JpegError::Corrupt),
        };

        drawn((block, marks), drawing, &mut scratch, (strip, Point { x: across, y: 0 }))?;
    }

    Ok(())
}

fn drawn(block: (&[i16], u16), drawing: &Drawing, scratch: &mut Block, into: (&mut [u8], Point<u32>)) -> Result<(), JpegError> {
    let (values, marks) = block;
    let (strip, at) = into;
    let layout = drawing.layout;
    let Ok(size) = frame::plane(layout);

    let target = Target {
        plane: strip,
        stride: size.width,
        at: Point { x: at.x.saturating_mul(layout.drawn.width), y: at.y.saturating_mul(layout.drawn.height) },
        drawn: layout.drawn,
    };

    match marks & DETAILED {
        0 => {
            let average = match (values.first(), drawing.prepared.steps.first()) {
                (Some(average), Some(step)) => i32::from(average.wrapping_mul(*step)),
                (_, _) => return Err(JpegError::Corrupt),
            };

            let Ok(level) = idct::level(average);

            idct::filled(level, target)
        },
        _ => {
            for ((slot, value), step) in scratch.values.iter_mut().zip(values).zip(&drawing.prepared.steps) {
                *slot = value.wrapping_mul(*step);
            }

            let [columns, _] = marks.to_le_bytes();

            scratch.columns = columns;

            idct::transformed(scratch, target)
        },
    }
}

fn held_from(plane: &mut Plane, layout: Layout, blocks: u32) -> Result<&mut [u8], JpegError> {
    let Ok(size) = frame::plane(layout);
    let row = blocks.saturating_mul(layout.drawn.height);

    let held = match row.checked_sub(plane.top) {
        Some(held) => held,
        None => return Err(JpegError::Corrupt),
    };

    let Ok(at) = index(u64::from(held).saturating_mul(u64::from(size.width)));

    match plane.samples.get_mut(at..) {
        Some(held) => Ok(held),
        None => Err(JpegError::Corrupt),
    }
}

fn strip_length(layout: Layout, tall: u32) -> Result<u64, JpegError> {
    let Ok(size) = frame::plane(layout);

    match u64::from(size.width).saturating_mul(u64::from(tall)).saturating_mul(u64::from(layout.drawn.height)) {
        0 => Err(JpegError::Corrupt),
        length => Ok(length),
    }
}

pub(crate) fn beside(reading: Beside<'_>) -> Result<Task<'_>, Never> {
    task(&mut [], Work::Beside(Mutex::new(reading)))
}

pub(crate) fn parted(plane: &mut Plane, layout: Layout, cut: Option<u32>) -> Result<(Window<'_>, &mut [u8]), JpegError> {
    let top = plane.top;
    let Ok(size) = frame::plane(layout);

    let row = match cut {
        Some(row) => row,
        None => return Ok((Window { samples: &plane.samples, top }, &mut [])),
    };

    let held = match row.checked_sub(top) {
        Some(held) => held,
        None => return Err(JpegError::Corrupt),
    };

    let Ok(at) = index(u64::from(held).saturating_mul(u64::from(size.width)));

    match plane.samples.split_at_mut_checked(at) {
        Some((above, below)) => Ok((Window { samples: above, top }, below)),
        None => Err(JpegError::Corrupt),
    }
}

pub(crate) fn drawing<'a>(round: (&'a [Row], &'a [Drawing]), below: &mut [Option<&'a mut [u8]>], tasks: &mut Vec<Task<'a>>) -> Result<(), JpegError> {
    let (rows, drawings) = round;

    for (member, drawing) in (0u32..).zip(drawings) {
        let Ok(slot) = index(drawing.component);
        let length = strip_length(drawing.layout, drawing.each.height)?;
        let Ok(length) = index(length);

        let held = match below.get_mut(slot).and_then(Option::take) {
            Some(held) => held,
            None => return Err(JpegError::Corrupt),
        };

        let strip = match held.chunks_mut(length.saturating_mul(rows.len())).next() {
            Some(strip) => strip,
            None => return Err(JpegError::Corrupt),
        };

        let Ok(one) = task(strip, Work::Read { rows, member, drawing });

        tasks.push(one);
    }

    Ok(())
}

pub(crate) fn gathered<'a>(
    plane: &'a mut Plane,
    blocks: &'a [[i16; 64]],
    drawing: &'a Drawing,
    units: (u32, u32),
    tasks: &mut Vec<Task<'a>>,
) -> Result<(), JpegError> {
    let (first, last) = units;
    let length = strip_length(drawing.layout, 1)?;
    let Ok(length) = index(length);
    let Ok(wide) = index(drawing.layout.grid.width);
    let tall = drawing.layout.sampling.height;
    let Ok(from) = index(first.saturating_mul(tall));
    let Ok(rows) = index(last.saturating_sub(first).saturating_mul(tall));

    match wide {
        0 => return Err(JpegError::Corrupt),
        _ => {},
    }

    let held = held_from(plane, drawing.layout, first.saturating_mul(tall))?;

    for (coefficients, strip) in blocks.chunks(wide).skip(from).take(rows).zip(held.chunks_mut(length)) {
        let Ok(one) = task(strip, Work::Gathered { coefficients, drawing });

        tasks.push(one);
    }

    Ok(())
}
