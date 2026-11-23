//! Decoded frame storage and the per-8x8 mode info grid.
//!
//! A frame is kept as bands, one superblock row of every plane apiece, each with eight rows of
//! padding above it for the loop filter of the band below to reach into. A band is published
//! once nothing can change it any more, and a frame decoded behind this one waits for the bands
//! it reads rather than for the whole frame.

use std::sync::{Arc, Mutex, OnceLock};

use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};
use console_core_size::Size;

use crate::places::{length, span};
use crate::tables::{INTRA_FRAME, NONE};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mv {
  pub row: i16,
  pub col: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WideMv {
  pub row: i32,
  pub col: i32,
}

impl Mv {
  pub const ZERO: Mv = Mv { row: 0, col: 0 };

  pub fn narrowed(wide: WideMv) -> Result<Mv, Never> {
    let Ok(row) = wrapped::<i32, i16>(wide.row);
    let Ok(col) = wrapped::<i32, i16>(wide.col);

    Ok(Mv { row, col })
  }

  pub fn wide(self) -> Result<WideMv, Never> {
    Ok(WideMv {
      row: i32::from(self.row),
      col: i32::from(self.col),
    })
  }
}

#[derive(Clone, Copy, Debug)]
pub struct MiInfo {
  pub sb_size: u8,
  pub skip: bool,
  pub tx_size: u8,
  pub y_mode: u8,
  pub sub_modes: [u8; 4],
  pub seg_id: u8,
  pub ref_frame: [i8; 2],
  pub interp_filter: u8,
  pub mv: [[Mv; 4]; 2],
}

impl MiInfo {
  pub const BLANK: MiInfo = MiInfo {
    sb_size: 0,
    skip: false,
    tx_size: 0,
    y_mode: 0,
    sub_modes: [0; 4],
    seg_id: 0,
    ref_frame: [INTRA_FRAME, NONE],
    interp_filter: 0,
    mv: [[Mv::ZERO; 4]; 2],
  };
}

impl Default for MiInfo {
  fn default() -> Self {
    MiInfo::BLANK
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MiPlace {
  pub row: u64,
  pub col: u64,
}

#[derive(Clone, Debug, Default)]
pub struct MiGrid {
  pub cols: u64,
  pub rows: u64,
  pub mi: Vec<MiInfo>,
}

impl MiGrid {
  #[inline]
  pub fn at(&self, place: MiPlace) -> Result<MiInfo, Never> {
    let Ok(at) = index(place.row.wrapping_mul(self.cols).wrapping_add(place.col));

    Ok(match self.mi.get(at) {
      Some(found) => *found,
      None => MiInfo::BLANK,
    })
  }
}

pub const BAND_PAD: u64 = 8;

pub struct Band {
  pub planes: [Vec<u16>; 3],
}

impl Band {
  pub fn plane(&self, plane: u64) -> Result<&[u16], Never> {
    let Ok(at) = index(plane);

    Ok(match self.planes.get(at) {
      Some(samples) => samples,
      None => &[],
    })
  }

  pub fn plane_mut(&mut self, plane: u64) -> Result<&mut [u16], Never> {
    let Ok(at) = index(plane);

    Ok(match self.planes.get_mut(at) {
      Some(samples) => samples,
      None => &mut [],
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subsampling {
  pub x: u32,
  pub y: u32,
}

impl Subsampling {
  pub const NONE: Subsampling = Subsampling { x: 0, y: 0 };

  pub fn of_plane(self, plane: u64) -> Result<Subsampling, Never> {
    Ok(match plane {
      0 => Subsampling::NONE,
      _chroma => self,
    })
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameKind {
  #[default]
  Key,
  IntraOnly,
  Inter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleRange {
  Limited,
  Full,
}

#[derive(Clone, Copy, Debug)]
pub struct FrameInfo {
  pub size: Size<u32>,
  pub subsampling: Subsampling,
  pub bit_depth: u8,
  pub color_space: u8,
  pub color_range: SampleRange,
  pub render: Size<u32>,
  pub kind: FrameKind,
}

pub type Place = console_core_geometry::Point<u64>;

pub struct Samples<'a> {
  pub data: &'a mut [u16],
  pub stride: u64,
  pub x_offset: u64,
}

impl Samples<'_> {
  #[inline]
  pub fn offset(&self, place: Place) -> Result<u64, Never> {
    Ok(
      place
        .y
        .wrapping_mul(self.stride)
        .wrapping_add(place.x)
        .wrapping_sub(self.x_offset),
    )
  }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Layout {
  pub stride: u64,
  pub band_rows: u64,
  pub band_log2: u32,
  pub visible: Size<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
  pub plane: u64,
  pub y: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication {
  Published,
  Waiting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completeness {
  Complete,
  Incomplete,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "frame bands may be freed by any ordered post job, while the decoder and tile jobs still need the same optional buffer reuse pools"
  )
)]
pub(crate) type SharedPools = Arc<Pools>;

pub struct Frame {
  pub info: FrameInfo,
  pub layouts: [Layout; 3],
  bands: Box<[OnceLock<Band>]>,
  pools: SharedPools,
}

impl Frame {
  pub fn new(info: FrameInfo, pools: SharedPools) -> Result<Frame, Never> {
    let Ok(layouts) = plane_layouts(info.size, info.subsampling);
    let Ok(rows) = index(u64::from(info.size.height).div_ceil(64));

    Ok(Frame {
      info,
      layouts,
      bands: (0..rows).map(|_| OnceLock::new()).collect(),
      pools,
    })
  }

  pub fn layout(&self, plane: u64) -> Result<Layout, Never> {
    let Ok(at) = index(plane);

    Ok(match self.layouts.get(at) {
      Some(layout) => *layout,
      None => Layout::default(),
    })
  }

  pub fn sb_rows(&self) -> Result<u64, Never> {
    length(&self.bands)
  }

  pub fn new_band(&self) -> Result<Band, Never> {
    let planes = self.layouts.map(|layout| {
      let Ok(samples) = self.pools.bands.take_any(
        layout
          .band_rows
          .wrapping_add(BAND_PAD)
          .wrapping_mul(layout.stride),
        0,
      );

      samples
    });

    Ok(Band { planes })
  }

  pub fn publish(&self, r: u64, band: Band) -> Result<(), Never> {
    let Ok(at) = index(r);

    let refused = match self.bands.get(at) {
      Some(slot) => slot.set(band),
      None => Err(band),
    };

    match refused {
      Ok(()) => {}
      Err(band_2) => {
        for samples in band_2.planes {
          let Ok(()) = self.pools.bands.put(samples);
        }
      }
    }

    Ok(())
  }

  pub fn publication(&self, r: u64) -> Result<Publication, Never> {
    let Ok(at) = index(r);

    Ok(match self.bands.get(at).and_then(OnceLock::get) {
      Some(_band) => Publication::Published,
      None => Publication::Waiting,
    })
  }

  pub fn completeness(&self) -> Result<Completeness, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self.bands.iter().all(|band| band.get().is_some()),
        (),
      ) {
        ::core::option::Option::Some(()) => Completeness::Complete,
        ::core::option::Option::None => Completeness::Incomplete,
      },
    )
  }

  #[inline]
  pub fn band(&self, r: u64) -> Result<Option<&Band>, Never> {
    let Ok(at) = index(r);

    match self.bands.get(at) {
      Some(slot) => waited(slot),
      None => Ok(None),
    }
  }

  #[inline]
  pub fn row(&self, line: Line) -> Result<&[u16], Never> {
    let Ok(layout) = self.layout(line.plane);
    let within = line.y & 1_u64.wrapping_shl(layout.band_log2).wrapping_sub(1);
    let start = within.wrapping_add(BAND_PAD).wrapping_mul(layout.stride);
    let Ok(band) = self.band(line.y.wrapping_shr(layout.band_log2));

    match band {
      Some(band_2) => {
        let Ok(samples) = band_2.plane(line.plane);

        span(samples, start..start.wrapping_add(layout.stride))
      }
      None => Ok(&[]),
    }
  }

  #[inline]
  pub fn span(
    &self,
    plane: u64,
    rows: std::ops::Range<u64>,
  ) -> Result<Option<(&[u16], u64)>, Never> {
    let Ok(layout) = self.layout(plane);
    let first = rows.start.wrapping_shr(layout.band_log2);
    let last = rows.end.wrapping_sub(1).wrapping_shr(layout.band_log2);
    let within = rows.start & 1_u64.wrapping_shl(layout.band_log2).wrapping_sub(1);

    match ::core::primitive::bool::then_some(first == last, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(band) = self.band(first);

        match band {
          Some(band_2) => {
            let Ok(samples) = band_2.plane(plane);

            Ok(Some((
              samples,
              within.wrapping_add(BAND_PAD).wrapping_mul(layout.stride),
            )))
          }
          None => Ok(None),
        }
      }
      ::core::option::Option::None => Ok(None),
    }
  }

  #[cfg(test)]
  pub fn from_planes(info: FrameInfo, planes: [&[u16]; 3]) -> Result<Frame, Never> {
    let Ok(frame) = Frame::new(info, SharedPools::default());
    let Ok(rows) = frame.sb_rows();

    for r in 0..rows {
      let Ok(mut band) = frame.new_band();

      for ((source, layout), target) in
        planes.iter().zip(frame.layouts).zip(band.planes.iter_mut())
      {
        for k in 0..layout.band_rows {
          let from = r
            .wrapping_mul(layout.band_rows)
            .wrapping_add(k)
            .wrapping_mul(layout.stride);
          let to = k.wrapping_add(BAND_PAD).wrapping_mul(layout.stride);
          let Ok(written) = span(source, from..from.wrapping_add(layout.stride));
          let Ok(into) = crate::places::span_mut(target, to..to.wrapping_add(layout.stride));

          for (destination, &source_2) in into.iter_mut().zip(written) {
            *destination = source_2;
          }
        }
      }

      let Ok(()) = frame.publish(r, band);
    }

    Ok(frame)
  }
}

#[cfg(feature = "threads")]
fn waited(slot: &OnceLock<Band>) -> Result<Option<&Band>, Never> {
  Ok(Some(slot.wait()))
}

#[cfg(not(feature = "threads"))]
fn waited(slot: &OnceLock<Band>) -> Result<Option<&Band>, Never> {
  Ok(slot.get())
}

impl Drop for Frame {
  fn drop(&mut self) {
    for band in std::mem::take(&mut self.bands)
      .into_vec()
      .into_iter()
      .filter_map(OnceLock::into_inner)
    {
      for samples in band.planes {
        let Ok(()) = self.pools.bands.put(samples);
      }
    }
  }
}

fn plane_layouts(size: Size<u32>, subsampling: Subsampling) -> Result<[Layout; 3], Never> {
  let width = u64::from(size.width);
  let height = u64::from(size.height);
  let stride = width.div_ceil(64).wrapping_mul(64);
  let luma = Layout {
    stride,
    band_rows: 64,
    band_log2: 6,
    visible: Size { width, height },
  };

  let chroma = Layout {
    stride: stride.wrapping_shr(subsampling.x),
    band_rows: 64_u64.wrapping_shr(subsampling.y),
    band_log2: 6_u32.wrapping_sub(subsampling.y),
    visible: Size {
      width: width
        .wrapping_add(u64::from(subsampling.x))
        .wrapping_shr(subsampling.x),
      height: height
        .wrapping_add(u64::from(subsampling.y))
        .wrapping_shr(subsampling.y),
    },
  };

  Ok([luma, chroma, chroma])
}

pub(crate) fn filled<T: Clone, const LIMIT: usize>(len: u64, fill: T) -> Result<Vec<T>, Never> {
  let Ok(wanted) = index(len);
  Ok(vec![fill; wanted.min(LIMIT)])
}

pub struct Pool<T, const LIMIT: usize> {
  bufs: Mutex<Vec<Vec<T>>>,
  max: u64,
}

#[derive(Clone, Copy)]
enum Reuse {
  Samples,
  Storage,
}

impl<T, const LIMIT: usize> Default for Pool<T, LIMIT> {
  fn default() -> Self {
    Pool {
      bufs: Mutex::default(),
      max: 32,
    }
  }
}

impl<T, const LIMIT: usize> Pool<T, LIMIT> {
  fn kept(&self) -> Result<Option<std::sync::MutexGuard<'_, Vec<Vec<T>>>>, Never> {
    Ok(match self.bufs.try_lock() {
      Ok(kept) => Some(kept),
      Err(std::sync::TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
      Err(std::sync::TryLockError::WouldBlock) => None,
    })
  }

  fn recycle(&self, len: u64, kind: Reuse) -> Result<Option<Vec<T>>, Never> {
    let Ok(wanted) = index(len);
    let Ok(kept) = self.kept();
    let mut kept_3 = match kept {
      Some(kept_2) => kept_2,
      None => return Ok(None),
    };
    Ok(
      match kept_3.iter().position(|buffer| match kind {
        Reuse::Samples => buffer.len() >= wanted.min(LIMIT),
        Reuse::Storage => buffer.capacity() >= wanted.min(LIMIT),
      }) {
        Some(at) => Some(kept_3.swap_remove(at)),
        None => None,
      },
    )
  }

  pub fn put(&self, buffer: Vec<T>) -> Result<(), Never> {
    match buffer.capacity() {
      0 => return Ok(()),
      _ => {}
    }

    let Ok(kept) = self.kept();

    match kept {
      Some(mut kept_2) => {
        let Ok(count) = length(&kept_2);

        match (count < self.max).then_some(()) {
          Some(()) => kept_2.push(buffer),
          None => {}
        }
      }
      None => {}
    }

    Ok(())
  }
}

impl<T: Clone, const LIMIT: usize> Pool<T, LIMIT> {
  pub fn take_any(&self, len: u64, fill: T) -> Result<Vec<T>, Never> {
    let Ok(found) = self.recycle(len, Reuse::Samples);
    Ok(match found {
      Some(mut buffer) => {
        let Ok(wanted) = index(len);
        buffer.truncate(wanted.min(LIMIT));
        buffer
      }
      None => {
        let Ok(fresh) = filled::<T, LIMIT>(len, fill);
        fresh
      }
    })
  }

  pub fn take(&self, len: u64, fill: T) -> Result<Vec<T>, Never> {
    let Ok(found) = self.recycle(len, Reuse::Storage);
    Ok(match found {
      Some(mut buffer) => {
        let Ok(wanted) = index(len);
        buffer.clear();
        buffer.resize(wanted.min(LIMIT), fill);
        buffer
      }
      None => {
        let Ok(fresh) = filled::<T, LIMIT>(len, fill);
        fresh
      }
    })
  }
}

pub struct Pools {
  pub samples: Pool<u16, 268435456>,
  pub bands: Pool<u16, 1179648>,
  pub mi: Pool<MiInfo, 4194304>,
}

impl Default for Pools {
  fn default() -> Self {
    Pools {
      samples: Pool::default(),
      bands: Pool {
        bufs: Mutex::default(),
        max: 1024,
      },
      mi: Pool::default(),
    }
  }
}
