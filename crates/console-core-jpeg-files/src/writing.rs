//! A picture written as a baseline JPEG, sixteen rows at a time.
//!
//! What is written is what libjpeg writes when it is given a quality and
//! nothing else: JFIF, brightness and two differences of color, the color at
//! half the size each way, the Annex K tables scaled to the quality, and one
//! scan. That is the JPEG every reader there is reads, which is the point of
//! writing one.
//!
//! The picture is walked in strips of sixteen rows, the height of the square
//! one round of blocks covers: four of brightness and one of each color. A
//! strip is turned into its three planes, the color averaged down to half,
//! and its blocks transformed and written, so what is held at once is a strip
//! rather than the picture three times over in floats. A picture whose sides
//! are not whole squares is filled out by repeating its last row and column,
//! which is what libjpeg does and what keeps a hard edge from ringing into
//! the picture.
//!
//! Alpha is not kept, because a JPEG has nowhere to keep it; a picture with
//! anything see-through in it wants a PNG. Nor is an orientation written,
//! since the pixels handed over are already the right way up.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::entropy::{self, Table, Tables, Writer};
use crate::fdct::{self, Matrix};
use crate::kept::NATURAL;
use crate::{JpegError, Picture, Quality};

const SQUARE: u32 = 16;

const BLOCK: u32 = 8;

const LARGEST_SIDE: u32 = 65_535;

const RGBA: u64 = 4;

const MARKER: u8 = 0xFF;

const START_OF_IMAGE: u8 = 0xD8;

const END_OF_IMAGE: u8 = 0xD9;

const JFIF: u8 = 0xE0;

const QUANTIZATION: u8 = 0xDB;

const BASELINE: u8 = 0xC0;

const HUFFMAN: u8 = 0xC4;

const START_OF_SCAN: u8 = 0xDA;

const JFIF_PAYLOAD: [u8; 14] = [b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0];

const SCAN_PAYLOAD: [u8; 10] = [3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0];

struct Coding {
    basis: Matrix,
    luminance: [u8; 64],
    chrominance: [u8; 64],
    luminance_tables: [Table; 2],
    chrominance_tables: [Table; 2],
}

struct Planes {
    luma: Vec<f32>,
    blue: Vec<f32>,
    red: Vec<f32>,
    wide: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Before {
    luma: i16,
    blue: i16,
    red: i16,
}

pub(crate) fn encoded(picture: &Picture, quality: Quality) -> Result<Vec<u8>, JpegError> {
    let size = checked(picture)?;
    let Ok(coding) = coding(quality);
    let Ok(quantization) = quantization(&coding);
    let Ok(frame) = frame(size);
    let Ok(huffman) = huffman();
    let mut bytes = vec![MARKER, START_OF_IMAGE];

    for (marker, payload) in [
        (JFIF, JFIF_PAYLOAD.as_slice()),
        (QUANTIZATION, quantization.as_slice()),
        (BASELINE, frame.as_slice()),
        (HUFFMAN, huffman.as_slice()),
        (START_OF_SCAN, SCAN_PAYLOAD.as_slice()),
    ] {
        segment(&mut bytes, (marker, payload))?;
    }

    let Ok(scan) = scan(picture, &coding);

    bytes.extend_from_slice(&scan);
    bytes.extend_from_slice(&[MARKER, END_OF_IMAGE]);

    Ok(bytes)
}

fn checked(picture: &Picture) -> Result<Size<u32>, JpegError> {
    let Picture { size, rgba } = picture;
    let area = u64::from(size.width).saturating_mul(u64::from(size.height));
    let said = size.width <= LARGEST_SIDE && size.height <= LARGEST_SIDE;

    match (u64::try_from(rgba.len()) == Ok(area.saturating_mul(RGBA)), area > 0, said) {
        (true, true, true) => Ok(*size),
        (false, _, _) | (true, false, _) => Err(JpegError::Mismatched),
        (true, true, false) => Err(JpegError::TooLarge),
    }
}

fn coding(quality: Quality) -> Result<Coding, Never> {
    let Ok(basis) = fdct::basis();
    let Ok(luminance) = fdct::steps(&fdct::LUMINANCE, quality);
    let Ok(chrominance) = fdct::steps(&fdct::CHROMINANCE, quality);
    let Ok(dc_luminance) = entropy::table(&entropy::DC_LUMINANCE_COUNTS, &entropy::DC_SYMBOLS);
    let Ok(ac_luminance) = entropy::table(&entropy::AC_LUMINANCE_COUNTS, &entropy::AC_LUMINANCE_SYMBOLS);
    let Ok(dc_chrominance) = entropy::table(&entropy::DC_CHROMINANCE_COUNTS, &entropy::DC_SYMBOLS);
    let Ok(ac_chrominance) = entropy::table(&entropy::AC_CHROMINANCE_COUNTS, &entropy::AC_CHROMINANCE_SYMBOLS);

    Ok(Coding {
        basis,
        luminance,
        chrominance,
        luminance_tables: [dc_luminance, ac_luminance],
        chrominance_tables: [dc_chrominance, ac_chrominance],
    })
}

fn segment(bytes: &mut Vec<u8>, segment: (u8, &[u8])) -> Result<(), JpegError> {
    let (marker, payload) = segment;

    let long = match u16::try_from(payload.len().saturating_add(2)) {
        Ok(long) => long,
        Err(_longer_than_a_segment) => return Err(JpegError::TooLarge),
    };

    bytes.extend_from_slice(&[MARKER, marker]);
    bytes.extend_from_slice(&long.to_be_bytes());
    bytes.extend_from_slice(payload);

    Ok(())
}

fn quantization(coding: &Coding) -> Result<Vec<u8>, Never> {
    let mut payload = Vec::with_capacity(130);

    for (named, steps) in [(0u8, &coding.luminance), (1, &coding.chrominance)] {
        payload.push(named);

        for natural in NATURAL {
            let Ok(at) = index(natural);

            match steps.get(at) {
                Some(step) => payload.push(*step),
                None => {},
            }
        }
    }

    Ok(payload)
}

fn frame(size: Size<u32>) -> Result<Vec<u8>, Never> {
    let [_, _, high_tall, low_tall] = size.height.to_be_bytes();
    let [_, _, high_wide, low_wide] = size.width.to_be_bytes();

    Ok(vec![8, high_tall, low_tall, high_wide, low_wide, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1])
}

fn huffman() -> Result<Vec<u8>, Never> {
    let tables: [(u8, &[u8; 16], &[u8]); 4] = [
        (0x00, &entropy::DC_LUMINANCE_COUNTS, &entropy::DC_SYMBOLS),
        (0x10, &entropy::AC_LUMINANCE_COUNTS, &entropy::AC_LUMINANCE_SYMBOLS),
        (0x01, &entropy::DC_CHROMINANCE_COUNTS, &entropy::DC_SYMBOLS),
        (0x11, &entropy::AC_CHROMINANCE_COUNTS, &entropy::AC_CHROMINANCE_SYMBOLS),
    ];

    Ok(tables
        .into_iter()
        .flat_map(|(named, counts, symbols)| std::iter::once(named).chain(counts.iter().copied()).chain(symbols.iter().copied()))
        .collect())
}

fn scan(picture: &Picture, coding: &Coding) -> Result<Vec<u8>, Never> {
    let across = picture.size.width.div_ceil(SQUARE);
    let down = picture.size.height.div_ceil(SQUARE);
    let Ok(mut writer) = Writer::new();
    let mut before = Before { luma: 0, blue: 0, red: 0 };

    for strip in 0..down {
        let Ok(planes) = planes(picture, (strip, across));

        for square in 0..across {
            let Ok(after) = square_put(&mut writer, (&planes, square), coding, before);

            before = after;
        }
    }

    writer.finished()
}

fn planes(picture: &Picture, strip: (u32, u32)) -> Result<Planes, Never> {
    let (down, across) = strip;
    let size = picture.size;
    let wide = across.saturating_mul(SQUARE);
    let Ok(width) = index(size.width);
    let Ok(room) = index(wide.saturating_mul(SQUARE));
    let Ok(stretched) = index(wide);
    let (pixels, _) = picture.rgba.as_chunks::<4>();
    let first = down.saturating_mul(SQUARE);
    let mut luma = Vec::with_capacity(room);
    let mut blue = Vec::with_capacity(room);
    let mut red = Vec::with_capacity(room);

    for row in first..first.saturating_add(SQUARE) {
        let Ok(from) = index(row.min(size.height.saturating_sub(1)));

        let row = match pixels.chunks(width).nth(from) {
            Some(row) => row,
            None => continue,
        };

        let last = row.last().copied();

        for pixel in row.iter().copied().chain(std::iter::repeat_n(last, stretched).flatten()).take(stretched) {
            let Ok([y, cb, cr]) = ycbcr(pixel);

            luma.push(y);
            blue.push(cb);
            red.push(cr);
        }
    }

    let Ok(blue) = halved(&blue, wide);
    let Ok(red) = halved(&red, wide);

    Ok(Planes { luma, blue, red, wide })
}

#[inline(always)]
fn ycbcr(pixel: [u8; 4]) -> Result<[f32; 3], Never> {
    let [red, green, blue, _] = pixel.map(f32::from);

    Ok([
        0.299 * red + 0.587 * green + 0.114 * blue,
        -0.168_736 * red - 0.331_264 * green + 0.5 * blue + 128.0,
        0.5 * red - 0.418_688 * green - 0.081_312 * blue + 128.0,
    ])
}

fn halved(plane: &[f32], wide: u32) -> Result<Vec<f32>, Never> {
    let Ok(pair) = index(wide.saturating_mul(2));
    let Ok(row) = index(wide);
    let mut halved = Vec::with_capacity(plane.len().div_ceil(4));

    for rows in plane.chunks_exact(pair) {
        let (top, bottom) = rows.split_at(row.min(rows.len()));
        let (top, _) = top.as_chunks::<2>();
        let (bottom, _) = bottom.as_chunks::<2>();

        for ([one, two], [three, four]) in top.iter().zip(bottom) {
            halved.push((one + two + three + four) * 0.25);
        }
    }

    Ok(halved)
}

fn block(plane: &[f32], wide: u32, corner: Point<u32>) -> Result<Matrix, Never> {
    let mut block = [[0f32; 8]; 8];
    let Ok(stride) = index(wide);
    let Ok(from) = index(corner.x);
    let Ok(skipped) = index(corner.y);

    for (row, samples) in block.iter_mut().zip(plane.chunks_exact(stride).skip(skipped)) {
        match samples.get(from..).and_then(<[f32]>::first_chunk::<8>) {
            Some(eight) => {
                for (slot, sample) in row.iter_mut().zip(eight) {
                    let Ok(shifted) = fdct::shifted(*sample);

                    *slot = shifted;
                }
            },
            None => {},
        }
    }

    Ok(block)
}

fn square_put(writer: &mut Writer, at: (&Planes, u32), coding: &Coding, before: Before) -> Result<Before, Never> {
    let (planes, square) = at;
    let left = square.saturating_mul(SQUARE);
    let [dc_luminance, ac_luminance] = &coding.luminance_tables;
    let [dc_chrominance, ac_chrominance] = &coding.chrominance_tables;
    let luminance = Tables { dc: dc_luminance, ac: ac_luminance };
    let chrominance = Tables { dc: dc_chrominance, ac: ac_chrominance };
    let mut luma = before.luma;

    for (x, y) in [(left, 0), (left.saturating_add(BLOCK), 0), (left, BLOCK), (left.saturating_add(BLOCK), BLOCK)] {
        let Ok(samples) = block(&planes.luma, planes.wide, Point { x, y });
        let Ok(quantized) = fdct::quantized(&samples, &coding.basis, &coding.luminance);
        let Ok(dc) = entropy::block(writer, &quantized, luma, luminance);

        luma = dc;
    }

    let half = planes.wide.div_euclid(2);
    let corner = Point { x: square.saturating_mul(BLOCK), y: 0 };
    let mut colors = [before.blue, before.red];

    for (plane, previous) in [&planes.blue, &planes.red].into_iter().zip(colors.iter_mut()) {
        let Ok(samples) = block(plane, half, corner);
        let Ok(quantized) = fdct::quantized(&samples, &coding.basis, &coding.chrominance);
        let Ok(dc) = entropy::block(writer, &quantized, *previous, chrominance);

        *previous = dc;
    }

    let [blue, red] = colors;

    Ok(Before { luma, blue, red })
}
