#![forbid(unsafe_code)]

use crate::include::common::bitdepth::BitDepth;
use crate::src::owned_recon::ReconDst;
use console_core_geometry::Size;
use console_core_number_conversion::{checked_index, checked_offset, fixed_offset};
use console_core_pixel_buffers::RowLayout;
use core::fmt::Display;
use std::io::{self, Write as _};

fn pixel_rows<BD: BitDepth>(
  out: &mut impl io::Write,
  buf: &[BD::Pixel],
  size: Size<u64>,
  stride: u64,
  what: &str,
) -> io::Result<()> {
  write!(out, "{what}")?;

  match size.height {
    0 => return Ok(()),
    1.. => {}
  }

  let row_step = match size.height {
    1 => size.width,
    _ => stride,
  };
  let signed_step = i64::try_from(row_step).map_err(|_too_wide| io::ErrorKind::InvalidInput)?;
  let layout =
    RowLayout::new(size, 0, signed_step).map_err(|_layout| io::ErrorKind::InvalidInput)?;
  let rows = layout
    .rows(buf)
    .map_err(|_bounds| io::ErrorKind::UnexpectedEof)?;

  for row in rows {
    for &pixel in row {
      write!(out, " {}", BD::display(pixel))?;
    }

    writeln!(out)?;
  }

  Ok(())
}

pub fn hex_dump<BD: BitDepth>(
  buf: &[BD::Pixel],
  size: Size<u64>,
  stride: u64,
  what: &str,
) -> io::Result<()> {
  pixel_rows::<BD>(&mut io::stdout().lock(), buf, size, stride, what)
}

pub fn hex_dump_pic<BD: BitDepth>(
  buf: &ReconDst<'_>,
  size: Size<u64>,
  what: &str,
) -> io::Result<()> {
  let width =
    checked_index(size.width).map_err(|_outside_indices| io::ErrorKind::InvalidInput)?;
  let Ok(source) = buf.as_src();
  let Ok(stride) = fixed_offset(match buf.pixel_stride::<BD>() {
    Ok(pixel_stride) => pixel_stride,
  });
  let mut out = io::stdout().lock();
  write!(out, "{what}")?;

  for y in 0..size.height {
    let row_number = i64::try_from(y).map_err(|_too_many_rows| io::ErrorKind::InvalidInput)?;
    let position = row_number
      .checked_mul(stride)
      .ok_or(io::ErrorKind::InvalidInput)?;
    let offset =
      checked_offset(position).map_err(|_outside_offsets| io::ErrorKind::InvalidInput)?;
    let Ok(row) = source.at(offset);
    let Ok(pixels) = row.slice::<BD>(width);

    for &pixel in &*pixels {
      write!(out, " {}", BD::display(pixel))?;
    }

    writeln!(out)?;
  }

  Ok(())
}

pub fn coef_dump<Coef: Display>(
  buf: &[Coef],
  size: Size<u64>,
  field_width: u32,
  what: &str,
) -> io::Result<()> {
  let mut out = io::stdout().lock();
  writeln!(out, "{what}")?;

  match size.height {
    0 => return Ok(()),
    1.. => {}
  }

  let decimal_places =
    checked_index(field_width).map_err(|_outside_indices| io::ErrorKind::InvalidInput)?;
  let stride = i64::try_from(size.width).map_err(|_too_wide| io::ErrorKind::InvalidInput)?;
  let layout = RowLayout::new(size, 0, stride).map_err(|_layout| io::ErrorKind::InvalidInput)?;
  let rows = layout
    .rows(buf)
    .map_err(|_bounds| io::ErrorKind::UnexpectedEof)?;

  for row in rows {
    for coefficient in row {
      write!(out, " {coefficient:0decimal_places$}")?;
    }

    writeln!(out)?;
  }

  Ok(())
}
