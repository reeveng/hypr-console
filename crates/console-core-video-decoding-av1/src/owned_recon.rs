//! Reconstruction against an EXCLUSIVELY-OWNED, column-compact tile band.
//!
//! # What this is
//!
//! PR #474 gave each tile a private full-plane [`Rav1dPictureDataComponent`]
//! but kept the existing borrow primitives on it, so the tracker still ran on
//! every reconstruction access — just uncontended. PR #481 priced removing it
//! outright with an `tracker: None` probe: **1,970,944 registrations per frame
//! across 21 sites**, worth 0.8996 / 0.8955 / 0.9071 of #474's wall at 8bpc
//! t=2/4/8.
//!
//! This module earns those numbers SOUNDLY. The private band is a plain
//! `Vec<Chunk>` owned by [`Rav1dTaskContext`], the worker's own struct, and it
//! is handed to the reconstruction kernels as a `&mut [u8]`. Uniqueness is
//! therefore proved by **borrowck**, statically, with no run-time record, no
//! lock, and no `unsafe` — `#![forbid(unsafe_code)]` is untouched.
//!
//! # The two changes are one change
//!
//! #474's buffers took the picture's byte length AND stride so that every
//! frame-coordinate offset reconstruction computes indexes the same pixel — a
//! three-line seam, at the cost of `tile_columns × plane` residency
//! (+96.3 MB at 8bpc, +191.0 MB at 10bpc, measured). A kernel signature that
//! takes `(&mut [u8], base, stride)` already carries the origin and stride a
//! compact buffer needs, so the coordinate translation is free once the
//! signature change is being made anyway. Doing them in sequence pays the
//! translation cost twice.
//!
//! So the band here is **column-compact and one superblock row tall**, with its
//! OWN stride, one per WORKER rather than one per tile:
//! `n_workers × max_tile_width × sb_height × planes`.
//!
//! # Why one superblock row is enough
//!
//! Reconstruction never reads a picture row above the current superblock row:
//! `src/recon.rs:2243` and `:3138` gate on `t.b.y & f.sb_step - 1 == 0` and, at
//! a superblock-row boundary, source the top edge from `f.ipred_edge` instead
//! of the plane ([`crate::src::ipred_prepare::rav1d_prepare_intra_edges`] takes
//! it as `prefilter_toplevel_sb_edge`). Left, top-right and bottom-left reads
//! are all clamped to the TILE (`ts.tiling.col_start/col_end/row_end`), which
//! is what AV1 tile independence guarantees.
//!
//! [`ReconBand::at`] translates plane coordinates into band coordinates by
//! subtracting the band origin. A coordinate outside the band returns a decode
//! error before it can address the buffer.
//!
//! # Scope, stated before the wins
//!
//! Armed only for frames whose reconstruction is entirely INTRA (`KEY` /
//! `INTRA_ONLY`), because every writer of a recon plane has to move together —
//! intra prediction, palette, CfL and the inverse transform all write the same
//! pixels, so converting a subset leaves the tracker on the path. Inter frames,
//! `allow_intrabc`, frame threading (`n_fc > 1`), the `c-ffi` allocator,
//! negative strides and single-tile frames all decline to the shared picture
//! and behave exactly as today.

use core::cmp::Ordering;

use console_core_api_compatibility::{NativeOffset, NativeSize};
use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
#[cfg(any(target_arch = "aarch64", target_arch = "wasm32"))]
use console_core_number_conversion::wrapped_offset;
use console_core_number_conversion::{checked_index, fixed_index, fixed_offset, index, indices};

use crate::include::common::bitdepth::BitDepth;
use crate::include::dav1d::picture::{PicOffset, Rav1dPictureDataComponent};
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::strided::Strided;
use std::ops::Deref;
use std::ops::DerefMut;

#[repr(C, align(64))]
#[derive(Clone, Copy, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable)]
pub(crate) struct Chunk([u8; 64]);

const CHUNK_BYTES: u64 = 64;
const _: () = assert!(core::mem::size_of::<Chunk>() == 64);

#[derive(Clone, Copy)]
enum BandState {
  Armed(u8),
  Idle,
}

#[derive(Clone, Copy)]
struct PlaneGeometry {
  rectangle: Rectangle<u64>,
  superblock_width: u64,
  sample_bytes: u8,
}

impl PlaneGeometry {
  fn subsampled(self, shifts: Point<u32>) -> Result<Self, console_core_never::Never> {
    let Rectangle { origin, size } = self.rectangle;
    Ok(Self {
      rectangle: Rectangle {
        origin: Point {
          x: origin.x.wrapping_shr(shifts.x),
          y: origin.y.wrapping_shr(shifts.y),
        },
        size: Size {
          width: size.width.wrapping_shr(shifts.x),
          height: size.height.wrapping_shr(shifts.y),
        },
      },
      superblock_width: self.superblock_width.wrapping_shr(shifts.x),
      sample_bytes: self.sample_bytes,
    })
  }
}

struct BandGeometry {
  luma: PlaneGeometry,
  chroma: Option<PlaneGeometry>,
}

#[derive(Default)]
struct BandPlane {
  storage: Vec<Chunk>,
  rectangle: Rectangle<u64>,
  stride: u64,
  row_bytes: u64,
  live_rows: u64,
}

impl BandPlane {
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "checked stride and extent arithmetic computes the allocation layout before reserving an exact number of aligned chunks"
    )
  )]
  fn prepare(&mut self, geometry: PlaneGeometry) -> Rav1dResult {
    let Size { width, height } = geometry.rectangle.size;
    let aligned_width = match geometry.superblock_width {
      0 => width,
      alignment => width
        .checked_next_multiple_of(alignment)
        .ok_or(Rav1dError::ERANGE)?,
    };
    let sample_size = u64::from(geometry.sample_bytes);
    let padded_bytes = aligned_width
      .checked_mul(sample_size)
      .ok_or(Rav1dError::ERANGE)?;
    let stride = padded_bytes
      .checked_next_multiple_of(CHUNK_BYTES)
      .ok_or(Rav1dError::ERANGE)?;
    let row_bytes = width.checked_mul(sample_size).ok_or(Rav1dError::ERANGE)?;
    let chunks_per_row = stride.wrapping_div(CHUNK_BYTES);
    let target_chunks = height
      .checked_mul(chunks_per_row)
      .ok_or(Rav1dError::ERANGE)?;
    let Ok(available_chunks) = fixed_index(self.storage.len());

    match target_chunks.checked_sub(available_chunks) {
      Some(extra @ 1..) => {
        let Ok(target) = index(target_chunks);
        let Ok(round_trip) = fixed_index(target);

        match round_trip.cmp(&target_chunks) {
          Ordering::Equal => {}
          Ordering::Less | Ordering::Greater => return Err(Rav1dError::ENOMEM),
        }

        let Ok(additional) = index(extra);
        self
          .storage
          .try_reserve(additional)
          .map_err(|_allocation| Rav1dError::ENOMEM)?;
        self.storage.resize(target, Chunk([0; 64]));
      }
      Some(0) | None => {}
    }

    self.rectangle = geometry.rectangle;
    self.stride = stride;
    self.row_bytes = row_bytes;
    self.live_rows = height;
    Ok(())
  }

  fn row_bytes(&self, row: u64) -> Rav1dResult<&[u8]> {
    let start = row.checked_mul(self.stride).ok_or(Rav1dError::ERANGE)?;
    let end = start
      .checked_add(self.row_bytes)
      .ok_or(Rav1dError::ERANGE)?;
    let Ok(range) = indices(start..end);
    let bytes = zerocopy::IntoBytes::as_bytes(self.storage.as_slice());
    bytes.get(range).ok_or(Rav1dError::EINVAL)
  }

  fn pixel_offset<BD: BitDepth>(&self, position: Point<u64>) -> Rav1dResult<u64> {
    let Rectangle { origin, size } = self.rectangle;
    let row = position.y.checked_sub(origin.y).ok_or(Rav1dError::EINVAL)?;
    let column = position.x.checked_sub(origin.x).ok_or(Rav1dError::EINVAL)?;
    let Ok(sample_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
    let row_pixels = self
      .stride
      .checked_div(sample_size)
      .ok_or(Rav1dError::EINVAL)?;

    match (row.cmp(&size.height), column.cmp(&row_pixels)) {
      (Ordering::Less, Ordering::Less) => {}
      _ => return Err(Rav1dError::EINVAL),
    }

    row
      .checked_mul(row_pixels)
      .and_then(|start| start.checked_add(column))
      .ok_or(Rav1dError::ERANGE)
  }
}

pub(crate) struct ReconBand {
  planes: [BandPlane; 3],
  state: BandState,
}

impl Default for ReconBand {
  fn default() -> Self {
    Self {
      planes: Default::default(),
      state: BandState::Idle,
    }
  }
}

impl ReconBand {
  fn disarm(&mut self) -> Result<(), console_core_never::Never> {
    self.state = BandState::Idle;
    Ok(())
  }

  fn arm(&mut self, geometry: BandGeometry) -> Rav1dResult {
    self.state = BandState::Idle;
    let [luma, u, v] = &mut self.planes;
    luma.prepare(geometry.luma)?;

    let planes = match geometry.chroma {
      Some(shape) => {
        u.prepare(shape)?;
        v.prepare(shape)?;
        3
      }
      None => 1,
    };

    self.state = BandState::Armed(planes);
    Ok(())
  }

  fn at<BD: BitDepth>(&mut self, plane: u8, position: Point<u64>) -> Rav1dResult<Band<'_>> {
    let Ok(plane_index) = index(plane);
    let storage = self.planes.get_mut(plane_index).ok_or(Rav1dError::EINVAL)?;
    let offset = storage.pixel_offset::<BD>(position)?;
    let bytes = zerocopy::IntoBytes::as_mut_bytes(storage.storage.as_mut_slice());
    Ok(Band {
      bytes,
      offset,
      stride: storage.stride,
    })
  }
}

pub(crate) struct Band<'a> {
  bytes: &'a mut [u8],
  offset: u64,
  stride: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct BandRef<'a> {
  bytes: &'a [u8],
  offset: u64,
  stride: u64,
}

pub(crate) enum ReconDst<'a> {
  Pic(PicOffset<'a>),
  Own(Band<'a>),
}

#[derive(Clone, Copy)]
pub(crate) enum ReconSrc<'a> {
  Pic(PicOffset<'a>),
  Own(BandRef<'a>),
}

pub(crate) enum PxMut<'a, BD: BitDepth> {
  Pic(
    crate::src::disjoint_mut::DisjointMutGuard<
      'a,
      crate::include::dav1d::picture::Rav1dPictureDataComponentInner,
      [BD::Pixel],
    >,
  ),
  Own(&'a mut [BD::Pixel]),
}

impl<BD: BitDepth> Deref for PxMut<'_, BD> {
  type Target = [BD::Pixel];

  #[inline(always)]
  fn deref(&self) -> &[BD::Pixel] {
    match self {
      Self::Pic(g) => g,
      Self::Own(s) => s,
    }
  }
}

impl<BD: BitDepth> DerefMut for PxMut<'_, BD> {
  #[inline(always)]
  fn deref_mut(&mut self) -> &mut [BD::Pixel] {
    match self {
      Self::Pic(g) => g,
      Self::Own(s) => s,
    }
  }
}

pub(crate) enum Px<'a, BD: BitDepth> {
  Pic(
    crate::src::disjoint_mut::DisjointImmutGuard<
      'a,
      crate::include::dav1d::picture::Rav1dPictureDataComponentInner,
      [BD::Pixel],
    >,
  ),
  Own(&'a [BD::Pixel]),
}

impl<BD: BitDepth> Deref for Px<'_, BD> {
  type Target = [BD::Pixel];

  #[inline(always)]
  fn deref(&self) -> &[BD::Pixel] {
    match self {
      Self::Pic(g) => g,
      Self::Own(s) => s,
    }
  }
}

#[cfg(any(target_arch = "aarch64", target_arch = "wasm32"))]
pub(crate) enum DstBlock<'a, BD: BitDepth> {
  Pic(crate::include::dav1d::picture::BlockMut<'a, BD>),
  Own {
    bytes: &'a mut [u8],
    base: u64,
    stride: u64,
  },
}

#[cfg(any(target_arch = "aarch64", target_arch = "wasm32"))]
impl<BD: BitDepth> DstBlock<'_, BD> {
  #[inline(always)]
  pub(crate) fn as_mut_bytes(
    &mut self,
  ) -> core::result::Result<&mut [u8], console_core_never::Never> {
    Ok({
      match self {
        Self::Pic(b) => b.as_mut_bytes(),
        Self::Own { bytes, .. } => bytes,
      }
    })
  }

  #[inline(always)]
  pub(crate) fn base(&self) -> core::result::Result<usize, console_core_never::Never> {
    Ok({
      match self {
        Self::Pic(b) => b.base(),
        Self::Own { base, .. } => match index(*base) {
          Ok(position) => position,
        },
      }
    })
  }

  #[inline(always)]
  pub(crate) fn byte_stride(&self) -> core::result::Result<isize, console_core_never::Never> {
    Ok({
      match self {
        Self::Pic(b) => b.byte_stride(),
        Self::Own { stride, .. } => match wrapped_offset(*stride) {
          Ok(offset) => offset,
        },
      }
    })
  }
}

struct PixelSpan {
  offset: u64,
  length: u64,
}

impl PixelSpan {
  #[inline]
  fn bytes<BD: BitDepth>(self) -> Rav1dResult<core::ops::Range<u64>> {
    let Ok(sample_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
    let first = self
      .offset
      .checked_mul(sample_size)
      .ok_or(Rav1dError::ERANGE)?;
    let last = self
      .offset
      .checked_add(self.length)
      .and_then(|end| end.checked_mul(sample_size))
      .ok_or(Rav1dError::ERANGE)?;
    Ok(first..last)
  }
}

#[inline]
fn px<BD: BitDepth>(bytes: &[u8], span: PixelSpan) -> Rav1dResult<&[BD::Pixel]> {
  let core::ops::Range { start, end } = span.bytes::<BD>()?;
  let first = checked_index(start).map_err(|_range| Rav1dError::ERANGE)?;
  let last = checked_index(end).map_err(|_range| Rav1dError::ERANGE)?;
  let row = bytes.get(first..last).ok_or(Rav1dError::EINVAL)?;
  zerocopy::FromBytes::ref_from_bytes(row).map_err(|_alignment| Rav1dError::EINVAL)
}

#[inline]
fn px_mut<BD: BitDepth>(bytes: &mut [u8], span: PixelSpan) -> Rav1dResult<&mut [BD::Pixel]> {
  let core::ops::Range { start, end } = span.bytes::<BD>()?;
  let first = checked_index(start).map_err(|_range| Rav1dError::ERANGE)?;
  let last = checked_index(end).map_err(|_range| Rav1dError::ERANGE)?;
  let row = bytes.get_mut(first..last).ok_or(Rav1dError::EINVAL)?;
  zerocopy::FromBytes::mut_from_bytes(row).map_err(|_alignment| Rav1dError::EINVAL)
}

impl Strided for ReconDst<'_> {
  fn fixed_stride(&self) -> Result<i64, Never> {
    match self {
      Self::Pic(picture) => picture.fixed_stride(),
      Self::Own(band) => Ok(band.stride.cast_signed()),
    }
  }
}

impl Strided for ReconSrc<'_> {
  fn fixed_stride(&self) -> Result<i64, Never> {
    match self {
      Self::Pic(picture) => picture.fixed_stride(),
      Self::Own(band) => Ok(band.stride.cast_signed()),
    }
  }
}

impl<'a> ReconDst<'a> {
  pub(crate) fn check_block_bounds<BD: BitDepth>(&self, size: Size<u64>) -> Rav1dResult {
    let rows_after_first = size.height.checked_sub(1).ok_or(Rav1dError::EINVAL)?;
    let Ok(stride) = self.fixed_pixel_stride::<BD>();
    let span = rows_after_first
      .checked_mul(stride.unsigned_abs())
      .ok_or(Rav1dError::ERANGE)?;
    let (offset, available) = match self {
      Self::Pic(picture) => {
        let Ok(start) = fixed_index(picture.offset);
        let Ok(pixel_count) = fixed_index(picture.data.pixel_len::<BD>());
        (start, pixel_count)
      }
      Self::Own(band) => {
        let Ok(byte_count) = fixed_index(band.bytes.len());
        let Ok(sample_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
        let pixel_count = byte_count
          .checked_div(sample_size)
          .ok_or(Rav1dError::EINVAL)?;
        (band.offset, pixel_count)
      }
    };
    let highest_start = match stride.cmp(&0) {
      Ordering::Less => {
        let _lowest_start = offset.checked_sub(span).ok_or(Rav1dError::EINVAL)?;
        offset
      }
      Ordering::Equal | Ordering::Greater => {
        offset.checked_add(span).ok_or(Rav1dError::ERANGE)?
      }
    };
    let end = highest_start
      .checked_add(size.width)
      .ok_or(Rav1dError::ERANGE)?;

    match end.cmp(&available) {
      Ordering::Less | Ordering::Equal => Ok(()),
      Ordering::Greater => Err(Rav1dError::EINVAL),
    }
  }

  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the legacy picture offset uses the signed displacement of the backing Rust slice"
    )
  )]
  pub(crate) fn at(
    &mut self,
    delta: NativeOffset,
  ) -> core::result::Result<ReconDst<'_>, console_core_never::Never> {
    Ok({
      match self {
        Self::Pic(p) => ReconDst::Pic(*p + delta),
        Self::Own(b) => ReconDst::Own(Band {
          offset: b.offset.wrapping_add_signed(match fixed_offset(delta) {
            Ok(amount) => amount,
          }),
          bytes: b.bytes,
          stride: b.stride,
        }),
      }
    })
  }

  #[inline(always)]
  pub(crate) fn as_src(&self) -> core::result::Result<ReconSrc<'_>, console_core_never::Never> {
    Ok({
      match self {
        Self::Pic(p) => ReconSrc::Pic(*p),
        Self::Own(b) => ReconSrc::Own(BandRef {
          bytes: b.bytes,
          offset: b.offset,
          stride: b.stride,
        }),
      }
    })
  }

  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the borrowed picture guard takes an already converted native slice length"
    )
  )]
  pub(crate) fn slice_mut<BD: BitDepth>(
    &mut self,
    length: NativeSize,
  ) -> Rav1dResult<PxMut<'_, BD>> {
    match self {
      Self::Pic(p) => Ok(PxMut::Pic(p.slice_mut::<BD>(length))),
      Self::Own(band) => {
        let Ok(samples) = fixed_index(length);
        let span = PixelSpan {
          offset: band.offset,
          length: samples,
        };
        px_mut::<BD>(band.bytes, span).map(PxMut::Own)
      }
    }
  }

  #[cfg(test)]
  #[inline(always)]
  fn set<BD: BitDepth>(&mut self, value: BD::Pixel) -> Rav1dResult {
    let mut pixels = self.slice_mut::<BD>(1)?;
    let pixel = pixels.first_mut().ok_or(Rav1dError::EINVAL)?;
    *pixel = value;
    Ok(())
  }

  #[inline(always)]
  pub(crate) fn for_rows_mut<BD: BitDepth, F: FnMut(u64, &mut [BD::Pixel])>(
    &mut self,
    size: Size<u64>,
    mut visit: F,
  ) -> Rav1dResult {
    let Size { width, height } = size;

    match (width, height) {
      (0, _) | (_, 0) => return Ok(()),
      _ => {}
    }

    self.check_block_bounds::<BD>(size)?;

    match self {
      Self::Pic(picture) => {
        let Ok(()) = picture.for_rows_mut_with_context::<BD, _, Never>(
          size,
          &mut visit,
          |row, pixels, next| {
            next(row, pixels);
            Ok(())
          },
        );
      }
      Self::Own(band) => {
        let pitch = BD::pxstride(band.stride);

        for row in 0..height {
          let offset = band.offset.wrapping_add(row.wrapping_mul(pitch));
          let span = PixelSpan {
            offset,
            length: width,
          };
          let pixels = px_mut::<BD>(band.bytes, span)?;
          visit(row, pixels);
        }
      }
    }

    Ok(())
  }

  #[inline(always)]
  pub(crate) fn with_block_mut<BD: BitDepth, C, R>(
    &mut self,
    extent: Size<u64>,
    context: C,
    visit: impl FnOnce(&mut [u8], u64, i64, C) -> R,
  ) -> Rav1dResult<R> {
    self.check_block_bounds::<BD>(extent)?;

    match self {
      Self::Pic(picture) => {
        let columns = checked_index(extent.width).map_err(|_too_wide| Rav1dError::ERANGE)?;
        let rows = checked_index(extent.height).map_err(|_too_tall| Rav1dError::ERANGE)?;
        let result = crate::include::dav1d::picture::with_pixel_guard_mut::<BD, R>(
          picture,
          columns,
          rows,
          |bytes, native_position, native_stride| {
            let Ok(offset) = fixed_index(native_position);
            let Ok(stride) = fixed_offset(native_stride);
            visit(bytes, offset, stride, context)
          },
        );
        Ok(result)
      }
      Self::Own(band) => {
        let Ok(sample_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
        let offset = band
          .offset
          .checked_mul(sample_size)
          .ok_or(Rav1dError::ERANGE)?;
        let stride = i64::try_from(band.stride).map_err(|_stride| Rav1dError::ERANGE)?;
        Ok(visit(band.bytes, offset, stride, context))
      }
    }
  }

  #[cfg(any(target_arch = "aarch64", target_arch = "wasm32"))]
  #[inline(always)]
  pub(crate) fn block_mut<BD: BitDepth>(
    &mut self,
    w: usize,
    h: usize,
  ) -> core::result::Result<DstBlock<'_, BD>, console_core_never::Never> {
    Ok({
      match self {
        Self::Pic(p) => DstBlock::Pic(p.block_mut::<BD>(w, h)),
        Self::Own(b) => {
          let _ = (w, h);

          let Ok(sample_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
          let Ok(offset) = index(b.offset.wrapping_mul(sample_size));
          DstBlock::Own {
            bytes: &mut b.bytes[offset..],
            base: 0,
            stride: b.stride,
          }
        }
      }
    })
  }
}

impl<'a> ReconSrc<'a> {
  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the legacy picture offset uses the signed displacement of the backing Rust slice"
    )
  )]
  pub(crate) fn at(
    &self,
    delta: NativeOffset,
  ) -> core::result::Result<ReconSrc<'a>, console_core_never::Never> {
    Ok({
      match self {
        Self::Pic(p) => ReconSrc::Pic(*p + delta),
        Self::Own(b) => ReconSrc::Own(BandRef {
          bytes: b.bytes,
          offset: b.offset.wrapping_add_signed(match fixed_offset(delta) {
            Ok(amount) => amount,
          }),
          stride: b.stride,
        }),
      }
    })
  }

  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the borrowed picture guard takes an already converted native slice length"
    )
  )]
  pub(crate) fn slice<BD: BitDepth>(&self, length: NativeSize) -> Rav1dResult<Px<'a, BD>> {
    match self {
      Self::Pic(p) => Ok(Px::Pic(p.slice::<BD>(length))),
      Self::Own(band) => {
        let Ok(samples) = fixed_index(length);
        let span = PixelSpan {
          offset: band.offset,
          length: samples,
        };
        px::<BD>(band.bytes, span).map(Px::Own)
      }
    }
  }

  #[inline(always)]
  pub(crate) fn get<BD: BitDepth>(&self) -> Rav1dResult<BD::Pixel> {
    let pixels = self.slice::<BD>(1)?;
    pixels.first().copied().ok_or(Rav1dError::EINVAL)
  }

  pub(crate) fn with_block_context<BD: BitDepth, C, R>(
    &self,
    extent: Size<u64>,
    context: C,
    f: impl FnOnce(C, &[u8], u64, i64) -> R,
  ) -> Result<R, console_core_never::Never> {
    match self {
      Self::Pic(picture) => {
        let Ok(columns) = index(extent.width);
        let Ok(rows) = index(extent.height);
        let output = crate::include::dav1d::picture::with_pixel_guard_immut::<BD, _>(
          picture,
          columns,
          rows,
          move |pixels, position, pitch| {
            let Ok(offset) = fixed_index(position);
            let Ok(stride) = fixed_offset(pitch);
            f(context, pixels, offset, stride)
          },
        );
        Ok(output)
      }
      Self::Own(band) => {
        let Ok(sample_size) = fixed_index(core::mem::size_of::<BD::Pixel>());
        let offset = band.offset.wrapping_mul(sample_size);
        let Ok(stride) = console_core_number_conversion::wrapped::<_, i64>(band.stride);
        Ok(f(context, band.bytes, offset, stride))
      }
    }
  }
}

pub(crate) enum ReconPlanes<'a> {
  Pic(&'a [crate::include::dav1d::picture::Rav1dPictureDataComponent; 3]),
  Own(&'a mut ReconBand),
}

impl<'a> ReconPlanes<'a> {
  #[inline(always)]
  pub(crate) fn bind(
    pic: &'a [crate::include::dav1d::picture::Rav1dPictureDataComponent; 3],
    band: &'a mut ReconBand,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      match band.state {
        BandState::Armed(_) => Self::Own(band),
        BandState::Idle => Self::Pic(pic),
      }
    })
  }

  #[inline(always)]
  pub(crate) fn dst<BD: BitDepth>(
    &mut self,
    plane: u8,
    position: Point<i32>,
  ) -> Rav1dResult<ReconDst<'_>> {
    let pixels = unsigned_position(position)?;

    match self {
      Self::Pic(picture) => {
        let Ok(plane_index) = index(plane);
        let component = picture.get(plane_index).ok_or(Rav1dError::EINVAL)?;
        let location = picture_position::<BD>(component, pixels)?;
        Ok(ReconDst::Pic(location))
      }
      Self::Own(band) => {
        let location = band.at::<BD>(plane, pixels)?;
        Ok(ReconDst::Own(location))
      }
    }
  }

  #[inline(always)]
  pub(crate) fn src<BD: BitDepth>(
    &self,
    plane: u8,
    position: Point<i32>,
  ) -> Rav1dResult<ReconSrc<'_>> {
    let pixels = unsigned_position(position)?;
    let Ok(plane_index) = index(plane);

    match self {
      Self::Pic(picture) => {
        let component = picture.get(plane_index).ok_or(Rav1dError::EINVAL)?;
        let location = picture_position::<BD>(component, pixels)?;
        Ok(ReconSrc::Pic(location))
      }
      Self::Own(band) => {
        let selected = band.planes.get(plane_index).ok_or(Rav1dError::EINVAL)?;
        let offset = selected.pixel_offset::<BD>(pixels)?;
        Ok(ReconSrc::Own(BandRef {
          bytes: zerocopy::IntoBytes::as_bytes(selected.storage.as_slice()),
          offset,
          stride: selected.stride,
        }))
      }
    }
  }
}

fn unsigned_position(position: Point<i32>) -> Rav1dResult<Point<u64>> {
  let x = u64::try_from(position.x).map_err(|_outside_picture| Rav1dError::EINVAL)?;
  let y = u64::try_from(position.y).map_err(|_outside_picture| Rav1dError::EINVAL)?;
  Ok(Point { x, y })
}

fn picture_position<BD: BitDepth>(
  picture: &Rav1dPictureDataComponent,
  position: Point<u64>,
) -> Rav1dResult<PicOffset<'_>> {
  let column = i64::try_from(position.x).map_err(|_outside_picture| Rav1dError::EINVAL)?;
  let row = i64::try_from(position.y).map_err(|_outside_picture| Rav1dError::EINVAL)?;
  let Ok(byte_stride) = fixed_offset(picture.stride());
  let stride = BD::pxstride(byte_stride);
  let displacement = row
    .checked_mul(stride)
    .and_then(|offset| offset.checked_add(column))
    .ok_or(Rav1dError::EINVAL)?;
  let Ok(origin) = fixed_index(picture.with_offset::<BD>().offset);
  let address = origin
    .checked_add_signed(displacement)
    .ok_or(Rav1dError::EINVAL)?;
  let Ok(length) = fixed_index(picture.pixel_len::<BD>());

  match address.cmp(&length) {
    Ordering::Less => {
      let Ok(offset) = index(address);
      Ok(PicOffset {
        data: picture,
        offset,
      })
    }
    Ordering::Equal | Ordering::Greater => Err(Rav1dError::EINVAL),
  }
}

use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dFrameData;
use crate::src::internal::{Rav1dTaskContext, Rav1dTileStateTiling};

#[derive(Debug, PartialEq, Eq)]
enum ReconstructionPolicy {
  AllowOwned,
  UsePicture,
}

const RECONSTRUCTION_POLICY: ReconstructionPolicy = {
  #[cfg(not(feature = "c-ffi"))]
  let policy = ReconstructionPolicy::AllowOwned;
  #[cfg(feature = "c-ffi")]
  let policy = ReconstructionPolicy::UsePicture;
  policy
};

#[cfg(test)]
#[test]
fn owned_reconstruction_is_enabled_for_rust_builds() {
  #[cfg(not(feature = "c-ffi"))]
  assert_eq!(RECONSTRUCTION_POLICY, ReconstructionPolicy::AllowOwned);
  #[cfg(feature = "c-ffi")]
  assert_eq!(RECONSTRUCTION_POLICY, ReconstructionPolicy::UsePicture);
}

#[derive(Clone, Copy, Default)]
pub(crate) enum ReconstructionStorage {
  #[default]
  Picture,
  Band,
}

pub(crate) fn frame_setup(
  c: &Rav1dContext,
  f: &mut Rav1dFrameData,
) -> Result<(), console_core_never::Never> {
  f.reconstruction_storage = ReconstructionStorage::Picture;

  match RECONSTRUCTION_POLICY {
    ReconstructionPolicy::UsePicture => return Ok(()),
    ReconstructionPolicy::AllowOwned => {}
  }

  let (header, picture) = match (f.frame_hdr.as_ref(), f.cur.data.as_ref()) {
    (Some(header), Some(picture)) => (header, picture),
    (None, _) | (_, None) => return Ok(()),
  };

  match (header.frame_type.is_key_or_intra() && !header.allow_intrabc && c.fc.len() <= 1)
    .then_some(())
  {
    Some(()) => {}
    None => return Ok(()),
  }

  let [luma, chroma_u, chroma_v] = &picture.data;
  let plane_is_available =
    |plane: &Rav1dPictureDataComponent| plane.stride() > 0 && plane.byte_len() != 0;
  let available = match f.cur.p.layout {
    Rav1dPixelLayout::I400 => plane_is_available(luma),
    Rav1dPixelLayout::I420 | Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => {
      plane_is_available(luma) && plane_is_available(chroma_u) && plane_is_available(chroma_v)
    }
  };

  f.reconstruction_storage = match ::core::primitive::bool::then_some(available, ()) {
    ::core::option::Option::Some(()) => ReconstructionStorage::Band,
    ::core::option::Option::None => ReconstructionStorage::Picture,
  };
  Ok(())
}

fn band_geometry(
  f: &Rav1dFrameData,
  tile: &Rav1dTileStateTiling,
  row: i32,
) -> Rav1dResult<BandGeometry> {
  let column_count = tile
    .col_end
    .checked_sub(tile.col_start)
    .ok_or(Rav1dError::ERANGE)?;
  let stride_blocks = u64::try_from(f.sb_step).map_err(|_bad_step| Rav1dError::EINVAL)?;
  let block_columns = u64::try_from(column_count).map_err(|_bad_width| Rav1dError::EINVAL)?;
  let start_column = u64::try_from(tile.col_start).map_err(|_bad_column| Rav1dError::EINVAL)?;
  let start_row = u64::try_from(row).map_err(|_bad_row| Rav1dError::EINVAL)?;
  let superblock_size = stride_blocks.wrapping_mul(4);

  let luma = PlaneGeometry {
    rectangle: Rectangle {
      origin: Point {
        x: start_column.wrapping_mul(4),
        y: start_row.wrapping_mul(4),
      },
      size: Size {
        width: block_columns.wrapping_mul(4),
        height: superblock_size,
      },
    },
    superblock_width: superblock_size,
    sample_bytes: match f.cur.p.bpc.cmp(&8) {
      Ordering::Greater => 2,
      Ordering::Equal | Ordering::Less => 1,
    },
  };

  let chroma = match f.cur.p.layout {
    Rav1dPixelLayout::I400 => None,
    Rav1dPixelLayout::I420 => {
      let Ok(chroma) = luma.subsampled(Point { x: 1, y: 1 });
      Some(chroma)
    }
    Rav1dPixelLayout::I422 => {
      let Ok(chroma) = luma.subsampled(Point { x: 1, y: 0 });
      Some(chroma)
    }
    Rav1dPixelLayout::I444 => Some(luma),
  };

  Ok(BandGeometry { luma, chroma })
}

pub(crate) fn arm_sbrow(f: &Rav1dFrameData, t: &mut Rav1dTaskContext) -> Rav1dResult {
  let Ok(()) = t.recon_band.disarm();

  match f.reconstruction_storage {
    ReconstructionStorage::Band => {}
    ReconstructionStorage::Picture => return Ok(()),
  }

  let tile = f.tile(t.tile_index)?;
  let tiling = &tile.tiling;

  match (tiling.col_end > tiling.col_start && t.b.y >= 0).then_some(()) {
    Some(()) => {}
    None => return Ok(()),
  }

  let row_end = t.b.y.checked_add(f.sb_step).ok_or(Rav1dError::ERANGE)?;
  let visible_end = row_end.min(tiling.row_end);
  let row_count = visible_end.checked_sub(t.b.y).ok_or(Rav1dError::ERANGE)?;
  let live = match row_count {
    1.. => {
      let checked_value = u64::try_from(row_count).map_err(|_bad_count| Rav1dError::EINVAL)?;
      checked_value
    }
    .wrapping_mul(4),
    _ => return Ok(()),
  };

  let geometry = band_geometry(f, tiling, t.b.y)?;

  match t.recon_band.arm(geometry) {
    Ok(()) => {}
    Err(Rav1dError::ENOMEM) => return Ok(()),
    Err(error) => return Err(error),
  }

  let chroma_shift = match f.cur.p.layout {
    Rav1dPixelLayout::I420 => 1,
    Rav1dPixelLayout::I400 | Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => 0,
  };
  let chroma_live = live.wrapping_shr(chroma_shift);
  let [luma, u, v] = &mut t.recon_band.planes;
  luma.live_rows = live.min(luma.rectangle.size.height);
  u.live_rows = chroma_live.min(u.rectangle.size.height);
  v.live_rows = chroma_live.min(v.rectangle.size.height);
  Ok(())
}

pub(crate) fn stitch_sbrow<BD: BitDepth>(
  f: &Rav1dFrameData,
  t: &mut Rav1dTaskContext,
) -> Rav1dResult {
  let planes = match core::mem::replace(&mut t.recon_band.state, BandState::Idle) {
    BandState::Idle => return Ok(()),
    BandState::Armed(count) => count,
  };
  let picture = f.cur.data.as_ref().ok_or(Rav1dError::EINVAL)?;
  let Ok(plane_count) = index(planes);

  for (band, destination) in t
    .recon_band
    .planes
    .iter()
    .zip(&picture.data)
    .take(plane_count)
  {
    let Rectangle { origin, size } = band.rectangle;
    let Ok(stride) = destination.fixed_pixel_stride::<BD>();
    let pitch = u64::try_from(stride).map_err(|_negative_pitch| Rav1dError::EINVAL)?;
    let Ok(pixel_capacity) = fixed_index(destination.pixel_len::<BD>());
    let Ok(columns) = index(size.width);

    for row in 0..band.live_rows {
      let source = band.row_bytes(row)?;
      let picture_row = origin.y.checked_add(row).ok_or(Rav1dError::ERANGE)?;
      let start = picture_row
        .checked_mul(pitch)
        .and_then(|offset| offset.checked_add(origin.x))
        .ok_or(Rav1dError::ERANGE)?;
      let end = start.checked_add(size.width).ok_or(Rav1dError::ERANGE)?;

      match end.cmp(&pixel_capacity) {
        Ordering::Less | Ordering::Equal => {}
        Ordering::Greater => return Err(Rav1dError::EINVAL),
      }

      let Ok(offset) = index(start);
      let mut destination_row = destination.slice_mut::<BD, _>((offset.., ..columns));
      let destination_bytes = zerocopy::IntoBytes::as_mut_bytes(&mut *destination_row);

      match destination_bytes.len().cmp(&source.len()) {
        Ordering::Equal => {
          for (target, &byte) in destination_bytes.iter_mut().zip(source) {
            *target = byte;
          }
        }
        Ordering::Less | Ordering::Greater => return Err(Rav1dError::EINVAL),
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::include::common::bitdepth::BitDepth8;

  fn band_of(size: Size<u64>) -> Rav1dResult<ReconBand> {
    let mut band = ReconBand::default();
    band.arm(BandGeometry {
      luma: PlaneGeometry {
        rectangle: Rectangle {
          origin: Point { x: 32, y: 16 },
          size,
        },
        superblock_width: 0,
        sample_bytes: 1,
      },
      chroma: None,
    })?;
    Ok(band)
  }

  fn dst_at(band: &mut ReconBand, position: Point<u64>) -> Rav1dResult<ReconDst<'_>> {
    band.at::<BitDepth8>(0, position).map(ReconDst::Own)
  }

  #[test]
  fn translation_is_plane_coordinates_minus_origin() -> Rav1dResult {
    let mut band = band_of(Size {
      width: 96,
      height: 8,
    })?;
    let mut first = dst_at(&mut band, Point { x: 32, y: 16 })?;
    first.set::<BitDepth8>(7)?;
    let mut other = dst_at(&mut band, Point { x: 34, y: 17 })?;
    other.set::<BitDepth8>(9)?;

    let [luma, ..] = &band.planes;
    let first_row = luma.row_bytes(0)?;
    let next_row = luma.row_bytes(1)?;
    assert_eq!(first_row.first(), Some(&7));
    assert_eq!(next_row.get(2), Some(&9));
    Ok(())
  }

  #[test]
  fn a_row_above_the_band_returns_invalid_data() -> Rav1dResult {
    let mut band = band_of(Size {
      width: 96,
      height: 8,
    })?;
    assert!(matches!(
      dst_at(&mut band, Point { x: 32, y: 15 }),
      Err(Rav1dError::EINVAL)
    ));
    Ok(())
  }

  #[test]
  fn a_row_below_the_band_returns_invalid_data() -> Rav1dResult {
    let mut band = band_of(Size {
      width: 96,
      height: 8,
    })?;
    assert!(matches!(
      dst_at(&mut band, Point { x: 32, y: 24 }),
      Err(Rav1dError::EINVAL)
    ));
    Ok(())
  }

  #[test]
  fn a_span_past_the_band_returns_invalid_data_before_writing() -> Rav1dResult {
    let mut band = band_of(Size {
      width: 64,
      height: 4,
    })?;
    let mut view = dst_at(&mut band, Point { x: 32, y: 19 })?;
    assert!(matches!(
      view.slice_mut::<BitDepth8>(65),
      Err(Rav1dError::EINVAL)
    ));
    let Ok(source) = view.as_src();
    assert!(matches!(
      source.slice::<BitDepth8>(65),
      Err(Rav1dError::EINVAL)
    ));
    let result = view.for_rows_mut::<BitDepth8, _>(
      Size {
        width: 64,
        height: 2,
      },
      |_, row| row.fill(1),
    );
    assert_eq!(result, Err(Rav1dError::EINVAL));
    let [luma, ..] = &band.planes;
    let row = luma.row_bytes(3)?;
    assert!(row.iter().all(|&sample| sample == 0));

    Ok(())
  }

  #[test]
  fn for_rows_mut_walks_the_bands_own_stride_not_the_pictures() -> Rav1dResult {
    let mut band = band_of(Size {
      width: 96,
      height: 4,
    })?;
    let [luma, ..] = &band.planes;
    assert_eq!(luma.stride, 128);

    let mut view = dst_at(&mut band, Point { x: 32, y: 16 })?;
    view.for_rows_mut::<BitDepth8, _>(
      Size {
        width: 96,
        height: 4,
      },
      |row, pixels| {
        let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(row);
        pixels.fill(byte.wrapping_add(1));
      },
    )?;

    let [updated, ..] = &band.planes;

    for row in 0..4u8 {
      let pixels = updated.row_bytes(u64::from(row))?;
      assert!(
        pixels.iter().all(|&value| value == row.wrapping_add(1)),
        "row {row}"
      );
    }

    Ok(())
  }
}
