//! The frame: how large the picture is, what it is made of, and how large each
//! part of it is drawn.
//!
//! A JPEG's components need not be the same size. A photograph nearly always
//! keeps its two color components at half the width and half the height of
//! its brightness, and says so as sampling factors: two by two for the one,
//! one by one for the others. The blocks of every component are walked in
//! units, minimum coded units, each holding as many blocks of each component
//! as its factors say.
//!
//! Reading at a fraction of the size, a block of brightness is drawn at one,
//! two or four pixels a side rather than eight. A block of color covers twice
//! as much of the picture, so it is drawn at twice that, which at a quarter or
//! an eighth of the size means the color arrives at the brightness's own
//! resolution and is never stretched at all. Only where a block would have to
//! be drawn wider than eight is it drawn at eight and stretched the rest of
//! the way.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::{JpegError, Unsupported};

const BLOCK: u32 = 8;

const MOST_FACTOR: u32 = 4;

const MOST_BYTES: u64 = 1 << 30;

const COEFFICIENT_BYTES: u64 = 128;

const RGBA: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Process {
    Sequential,
    Progressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Component {
    pub(crate) id: u8,
    pub(crate) sampling: Size<u32>,
    pub(crate) quantization: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    pub(crate) process: Process,
    pub(crate) size: Size<u32>,
    pub(crate) components: Vec<Component>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) sampling: Size<u32>,
    pub(crate) grid: Size<u32>,
    pub(crate) filled: Size<u32>,
    pub(crate) drawn: Size<u32>,
    pub(crate) stretched: Size<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Geometry {
    pub(crate) units: Size<u32>,
    pub(crate) layouts: Vec<Layout>,
    pub(crate) output: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Axis {
    sampling: u32,
    most: u32,
    units: u32,
    stored: u32,
    eighths: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Measured {
    grid: u32,
    filled: u32,
    drawn: u32,
    stretched: u32,
}

pub(crate) fn frame(process: Process, payload: &[u8]) -> Result<Frame, JpegError> {
    let (header, listed) = match payload.split_first_chunk::<6>() {
        Some(split) => split,
        None => return Err(JpegError::Truncated),
    };

    let [precision, tall_high, tall_low, wide_high, wide_low, count] = *header;
    let height = u32::from(u16::from_be_bytes([tall_high, tall_low]));
    let width = u32::from(u16::from_be_bytes([wide_high, wide_low]));

    match (precision, width, height, count) {
        (8, 1.., 1.., 1 | 3) => {},
        (8, 1.., 1.., 4) => return Err(JpegError::Unsupported(Unsupported::Cmyk)),
        (8, 1.., 1.., other) => return Err(JpegError::Unsupported(Unsupported::Components(other))),
        (8, 1.., 0, _) => return Err(JpegError::Unsupported(Unsupported::HeightAfterwards)),
        (8, 0, _, _) => return Err(JpegError::Corrupt),
        (_, _, _, _) => return Err(JpegError::Unsupported(Unsupported::TwelveBits)),
    }

    let components = components(listed, count)?;

    Ok(Frame { process, size: Size { width, height }, components })
}

fn components(listed: &[u8], count: u8) -> Result<Vec<Component>, JpegError> {
    let mut components = Vec::new();
    let Ok(count) = index(count);

    for [id, factors, quantization] in listed.as_chunks::<3>().0.iter().take(count) {
        let (id, factors, quantization) = (*id, *factors, *quantization);

        let sampling = Size { width: u32::from(factors.wrapping_shr(4)), height: u32::from(factors & 0x0F) };

        match (sampling.width, sampling.height, quantization) {
            (1..=MOST_FACTOR, 1..=MOST_FACTOR, 0..=3) => {},
            (_, _, _) => return Err(JpegError::Corrupt),
        }

        components.push(Component { id, sampling, quantization });
    }

    match components.len() == count {
        true => Ok(components),
        false => Err(JpegError::Truncated),
    }
}

pub(crate) fn scaled(stored: Size<u32>, eighths: u32) -> Result<Size<u32>, Never> {
    stored.map(|side| side.saturating_mul(eighths).div_ceil(BLOCK))
}

pub(crate) fn eighths(stored: Size<u32>, covering: Size<u32>) -> Result<u32, Never> {
    let covers = [1u32, 2, 4].into_iter().find(|eighths| {
        let Ok(scaled) = scaled(stored, *eighths);

        scaled.width >= covering.width && scaled.height >= covering.height
    });

    Ok(match covers {
        Some(eighths) => eighths,
        None => BLOCK,
    })
}

pub(crate) fn geometry(frame: &Frame, eighths: u32) -> Result<Geometry, JpegError> {
    let alone = Size { width: 1, height: 1 };

    let samplings: Vec<Size<u32>> = match frame.components.as_slice() {
        [_] => vec![alone],
        _ => frame.components.iter().map(|component| component.sampling).collect(),
    };

    let most = samplings.iter().fold(alone, |most, sampling| Size {
        width: most.width.max(sampling.width),
        height: most.height.max(sampling.height),
    });

    let units = Size {
        width: frame.size.width.div_ceil(most.width.saturating_mul(BLOCK)),
        height: frame.size.height.div_ceil(most.height.saturating_mul(BLOCK)),
    };

    let mut layouts = Vec::new();

    for sampling in samplings {
        let across = axis(Axis { sampling: sampling.width, most: most.width, units: units.width, stored: frame.size.width, eighths })?;
        let down = axis(Axis { sampling: sampling.height, most: most.height, units: units.height, stored: frame.size.height, eighths })?;

        layouts.push(Layout {
            sampling,
            grid: Size { width: across.grid, height: down.grid },
            filled: Size { width: across.filled, height: down.filled },
            drawn: Size { width: across.drawn, height: down.drawn },
            stretched: Size { width: across.stretched, height: down.stretched },
        });
    }

    let Ok(output) = scaled(frame.size, eighths);

    Ok(Geometry { units, layouts, output })
}

fn axis(axis: Axis) -> Result<Measured, JpegError> {
    let ratio = match axis.most.checked_div(axis.sampling) {
        Some(ratio) => ratio,
        None => return Err(JpegError::Corrupt),
    };

    match (ratio, ratio.saturating_mul(axis.sampling) == axis.most) {
        (1 | 2 | 4, true) => {},
        (_, _) => return Err(JpegError::Unsupported(Unsupported::Sampling)),
    }

    let span = axis.eighths.saturating_mul(ratio);
    let drawn = span.min(BLOCK);

    let stretched = match span.checked_div(drawn) {
        Some(stretch) => stretch.trailing_zeros(),
        None => return Err(JpegError::Corrupt),
    };

    Ok(Measured {
        grid: axis.units.saturating_mul(axis.sampling),
        filled: axis.stored.saturating_mul(axis.sampling).div_ceil(axis.most).div_ceil(BLOCK),
        drawn,
        stretched,
    })
}

pub(crate) fn plane(layout: Layout) -> Result<Size<u32>, Never> {
    Ok(Size {
        width: layout.grid.width.saturating_mul(layout.drawn.width),
        height: layout.grid.height.saturating_mul(layout.drawn.height),
    })
}

fn area(size: Size<u32>) -> Result<u64, Never> {
    Ok(u64::from(size.width).saturating_mul(u64::from(size.height)))
}

pub(crate) fn affordable(geometry: &Geometry, process: Process) -> Result<(), JpegError> {
    let Ok(output) = area(geometry.output);
    let mut needed = output.saturating_mul(RGBA);

    for layout in &geometry.layouts {
        let Ok(plane) = plane(*layout);
        let Ok(drawn) = area(plane);
        let Ok(blocks) = area(layout.grid);

        let gathered = match process {
            Process::Sequential => 0,
            Process::Progressive => blocks.saturating_mul(COEFFICIENT_BYTES),
        };

        needed = needed.saturating_add(drawn).saturating_add(gathered);
    }

    match needed <= MOST_BYTES {
        true => Ok(()),
        false => Err(JpegError::TooLarge),
    }
}
