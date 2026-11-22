//! Once-published CTB rows hold filtered samples and the compressed temporal motion field.

use std::sync::{Arc, OnceLock};

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, indices};
use console_core_pixel_buffers::{RowLayout, RowLayoutError, copy_clamped_window};

use crate::params::Sps;
use crate::places::{read, window};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColMv {
  pub mv: [[i16; 2]; 2],
  pub ref_poc: [i32; 2],
  pub flags: u8,
}

pub struct FrameRow {
  pub y: Box<[u16]>,
  pub cb: Box<[u16]>,
  pub cr: Box<[u16]>,
  pub col: Box<[ColMv]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Component {
  Luma,
  Blue,
  Red,
}

impl From<Component> for u64 {
  fn from(component: Component) -> Self {
    match component {
      Component::Luma => 0,
      Component::Blue => 1,
      Component::Red => 2,
    }
  }
}

impl FrameRow {
  fn samples(&self, component: Component) -> Result<&[u16], Never> {
    Ok(match component {
      Component::Luma => &self.y,
      Component::Blue => &self.cb,
      Component::Red => &self.cr,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication {
  Published,
  Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completeness {
  Complete,
  Pending,
}

pub struct Identity {
  pub id: u32,
  pub poc: i32,
}

#[derive(Clone, Copy)]
pub struct Format {
  pub size: Size<u64>,
  pub log2_ctb: u32,
  pub bit_depth: u32,
  pub bit_depth_c: u32,
}

impl From<&Sps> for Format {
  fn from(sps: &Sps) -> Self {
    Self {
      size: Size {
        width: u64::from(sps.width),
        height: u64::from(sps.height),
      },
      log2_ctb: sps.log2_ctb,
      bit_depth: sps.bit_depth_luma,
      bit_depth_c: sps.bit_depth_chroma,
    }
  }
}

pub struct Frame {
  pub id: u32,
  pub poc: i32,
  pub width: u64,
  pub height: u64,
  cwidth: u64,
  cheight: u64,
  log2_ctb: u32,
  pub bit_depth: u32,
  pub bit_depth_c: u32,
  rows: Box<[OnceLock<FrameRow>]>,
}

#[derive(Clone, Copy)]
pub struct Crop {
  pub origin: Point<u64>,
  pub size: Size<u64>,
}

pub struct SampleWindow {
  pub plane: Component,
  pub origin: Point<i64>,
  pub size: Size<u64>,
}

pub struct WindowOutput<'a> {
  pub samples: &'a mut [i16],
  pub stride: u64,
}

impl Frame {
  pub fn new(Identity { id, poc }: Identity, format: Format) -> Result<Self, Never> {
    let Size { width, height } = format.size;
    let rows = height.div_ceil(1u64.wrapping_shl(format.log2_ctb));

    Ok(Self {
      id,
      poc,
      width,
      height,
      cwidth: width.wrapping_div(2),
      cheight: height.wrapping_div(2),
      log2_ctb: format.log2_ctb,
      bit_depth: format.bit_depth,
      bit_depth_c: format.bit_depth_c,
      rows: (0..rows).map(|_| OnceLock::new()).collect(),
    })
  }

  pub fn num_rows(&self) -> Result<u64, Never> {
    fixed_index(self.rows.len())
  }

  pub fn publish(&self, r: u64, row: FrameRow) -> Result<(), Never> {
    let Ok(at) = index(r);

    match self.rows.get(at) {
      Some(slot) => match slot.set(row) {
        Ok(()) => {}
        Err(already_published) => drop(already_published),
      },
      None => drop(row),
    }

    Ok(())
  }

  pub fn publication(&self, r: u64) -> Result<Publication, Never> {
    let Ok(at) = index(r);
    Ok(match self.rows.get(at).and_then(OnceLock::get) {
      Some(_) => Publication::Published,
      None => Publication::Pending,
    })
  }

  pub fn completeness(&self) -> Result<Completeness, Never> {
    Ok(match self.rows.last() {
      None => Completeness::Complete,
      Some(row) => match row.get() {
        Some(_) => Completeness::Complete,
        None => Completeness::Pending,
      },
    })
  }

  #[inline]
  fn row(&self, r: u64) -> Result<Option<&FrameRow>, Never> {
    let Ok(at) = index(r);

    match self.rows.get(at) {
      Some(row) => published_row(row),
      None => Ok(None),
    }
  }

  pub fn wait_complete(&self) -> Result<(), Never> {
    for row in &self.rows {
      let Ok(_published) = published_row(row);
    }

    Ok(())
  }

  #[inline]
  pub fn col_mv(&self, spot: Point<u64>) -> Result<ColMv, Never> {
    let Ok(row) = self.row(spot.y.wrapping_shr(self.log2_ctb));
    let row_3 = match row {
      Some(row_2) => row_2,
      None => return Ok(ColMv::default()),
    };
    let within = spot.y & 1u64.wrapping_shl(self.log2_ctb).wrapping_sub(1);
    let index = within
      .wrapping_shr(4)
      .wrapping_mul(self.width.div_ceil(16))
      .wrapping_add(spot.x.wrapping_shr(4));
    read(&row_3.col, index)
  }

  fn component_extent(&self, component: Component) -> Result<Size<u64>, Never> {
    Ok(match component {
      Component::Luma => Size {
        width: self.width,
        height: self.height,
      },
      Component::Blue | Component::Red => Size {
        width: self.cwidth,
        height: self.cheight,
      },
    })
  }

  fn band_shift(&self, component: Component) -> Result<u32, Never> {
    Ok(match component {
      Component::Luma => self.log2_ctb,
      Component::Blue | Component::Red => self.log2_ctb.wrapping_sub(1),
    })
  }

  pub fn copy_window(
    &self,
    requested: SampleWindow,
    output: WindowOutput<'_>,
  ) -> Result<(), RowLayoutError> {
    let Ok(coded) = self.component_extent(requested.plane);
    let Ok(shift) = self.band_shift(requested.plane);
    let Ok(width) = index(coded.width);
    let Ok(height) = index(coded.height);
    let stride =
      i64::try_from(output.stride).map_err(|_not_a_signed_stride| RowLayoutError::Stride)?;
    let layout = RowLayout::new(requested.size, 0, stride)?;

    copy_clamped_window(
      Size { width, height },
      requested.origin,
      (&layout, output.samples),
      |y, columns| {
        let Ok(line) = fixed_index(y);
        let Ok(first) = fixed_index(columns.start);
        let Ok(last) = fixed_index(columns.end);
        let Ok(row) = self.row(line.wrapping_shr(shift));

        match row {
          Some(row_2) => {
            let Ok(samples) = row_2.samples(requested.plane);
            let within = line & 1u64.wrapping_shl(shift).wrapping_sub(1);
            let base = within.wrapping_mul(coded.width);
            let span = base.wrapping_add(first)..base.wrapping_add(last);
            let Ok(native) = indices(span);
            samples.get(native)
          }
          None => None,
        }
      },
      |source, destination| {
        for (&sample, target) in source.iter().zip(destination) {
          *target = sample.cast_signed();
        }
      },
    )
  }

  pub fn copy_cropped(&self, crop: Crop) -> Result<[Vec<u16>; 3], Never> {
    let chroma = Crop {
      origin: Point {
        x: crop.origin.x.wrapping_div(2),
        y: crop.origin.y.wrapping_div(2),
      },
      size: Size {
        width: crop.size.width.div_ceil(2),
        height: crop.size.height.div_ceil(2),
      },
    };
    let Ok(y) = self.copy_plane(Component::Luma, crop);
    let Ok(cb) = self.copy_plane(Component::Blue, chroma);
    let Ok(cr) = self.copy_plane(Component::Red, chroma);
    Ok([y, cb, cr])
  }

  fn copy_plane(&self, plane: Component, crop: Crop) -> Result<Vec<u16>, Never> {
    let Ok(shift) = self.band_shift(plane);
    let Ok(coded) = self.component_extent(plane);
    let area = crop.size.width.wrapping_mul(crop.size.height);
    let Ok(estimated) = index(area);

    let mut samples = Vec::with_capacity(estimated.min(285204544));
    let after = crop.origin.y.wrapping_add(crop.size.height);
    let mask = 1u64.wrapping_shl(shift).wrapping_sub(1);

    for line in crop.origin.y..after {
      let Ok(row) = self.row(line.wrapping_shr(shift));

      match row {
        Some(row_2) => {
          let Ok(source) = row_2.samples(plane);
          let start = (line & mask)
            .wrapping_mul(coded.width)
            .wrapping_add(crop.origin.x);
          let Ok(run) = window(source, start..start.wrapping_add(crop.size.width));
          samples.extend_from_slice(run);
        }
        None => {}
      }
    }

    Ok(samples)
  }

  pub fn neutral_row(&self, r: u64) -> Result<FrameRow, Never> {
    let ctb = 1u64.wrapping_shl(self.log2_ctb);
    let lines = self.height.saturating_sub(r.wrapping_mul(ctb)).min(ctb);
    let Ok(y) = index(self.width.wrapping_mul(lines));
    let Ok(chroma) = index(self.cwidth.wrapping_mul(lines.div_ceil(2)));
    let Ok(motion) = index(self.width.div_ceil(16).wrapping_mul(lines.div_ceil(16)));
    let mid_y = 1u16.wrapping_shl(self.bit_depth.wrapping_sub(1));
    let mid_c = 1u16.wrapping_shl(self.bit_depth_c.wrapping_sub(1));

    Ok(FrameRow {
      y: vec![mid_y; y.min(1080832)].into_boxed_slice(),
      cb: vec![mid_c; chroma.min(270208)].into_boxed_slice(),
      cr: vec![mid_c; chroma.min(270208)].into_boxed_slice(),
      col: vec![ColMv::default(); motion.min(4224)].into_boxed_slice(),
    })
  }

  pub fn gray(identity: Identity, like: &Frame) -> Result<Frame, Never> {
    let format = Format {
      size: Size {
        width: like.width,
        height: like.height,
      },
      log2_ctb: like.log2_ctb,
      bit_depth: like.bit_depth,
      bit_depth_c: like.bit_depth_c,
    };
    let Ok(frame) = Self::new(identity, format);
    let Ok(rows) = frame.num_rows();

    for r in 0..rows {
      let Ok(neutral) = frame.neutral_row(r);
      let Ok(()) = frame.publish(r, neutral);
    }

    Ok(frame)
  }
}

#[cfg(feature = "threads")]
fn published_row(row: &OnceLock<FrameRow>) -> Result<Option<&FrameRow>, Never> {
  Ok(Some(row.wait()))
}

#[cfg(not(feature = "threads"))]
fn published_row(row: &OnceLock<FrameRow>) -> Result<Option<&FrameRow>, Never> {
  Ok(row.get())
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the DPB and workers read the same once-published row buffers; old references must remain alive until queued consumers finish"
  )
)]
pub type FrameRef = Arc<Frame>;

#[derive(Clone)]
pub struct RefPic {
  pub frame: FrameRef,
  pub poc: i32,
  pub long_term: bool,
}
