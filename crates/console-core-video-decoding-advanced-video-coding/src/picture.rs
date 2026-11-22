//! Picture buffers and per-macroblock state.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_pixel_buffers::{RowLayout, RowLayoutError, copy_clamped_window};
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub struct Planes {
  pub y: Vec<u8>,
  pub cb: Vec<u8>,
  pub cr: Vec<u8>,
  pub width: u32,
  pub height: u32,
  pub cwidth: u32,
}

impl Planes {
  pub fn new(size: Size<u32>) -> Result<Self, Never> {
    let width = size.width;
    let height = size.height;
    let cwidth = width.wrapping_div(2);
    let cheight = height.wrapping_div(2);
    let Ok(luma) = index(u64::from(width).saturating_mul(u64::from(height)));
    let Ok(chroma) = index(u64::from(cwidth).saturating_mul(u64::from(cheight)));
    Ok(Planes {
      y: vec![0; luma.min(268_435_456)],
      cb: vec![128; chroma.min(67_108_864)],
      cr: vec![128; chroma.min(67_108_864)],
      width,
      height,
      cwidth,
    })
  }

  pub fn gray(size: Size<u32>) -> Result<Self, Never> {
    let Ok(mut planes) = Self::new(size);
    planes.y.fill(128);
    Ok(planes)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MbKind {
  #[default]
  None,
  I4x4,
  I8x8,
  I16x16,
  IPcm,
  PSkip,
  BSkip,
  BDirect16x16,

  Inter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroblockPrediction {
  Intra,
  Inter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication {
  Published,
  Pending,
}

impl From<MbKind> for MacroblockPrediction {
  #[inline]
  fn from(kind: MbKind) -> Self {
    match kind {
      MbKind::I4x4 | MbKind::I8x8 | MbKind::I16x16 | MbKind::IPcm => MacroblockPrediction::Intra,
      MbKind::None | MbKind::PSkip | MbKind::BSkip | MbKind::BDirect16x16 | MbKind::Inter => {
        MacroblockPrediction::Inter
      }
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransformSize {
  EightByEight,
  #[default]
  FourByFour,
}

impl From<bool> for TransformSize {
  fn from(eight: bool) -> Self {
    match eight.then_some(()) {
      Some(()) => Self::EightByEight,
      None => Self::FourByFour,
    }
  }
}

#[derive(Clone, Copy)]
pub struct MbState {
  pub slice_num: Option<u32>,
  pub kind: MbKind,
  pub transform_size: TransformSize,

  pub cbp: u8,
  pub qp: u8,

  pub qpc: [u8; 2],
  pub intra_chroma_mode: u8,

  pub intra_modes: [u8; 16],

  pub nnz: [u8; 16],

  pub nnz_c: [[u8; 4]; 2],

  pub cbf_dc: u8,

  pub nz_mask: u16,
  pub ref_idx: [[i8; 4]; 2],

  pub mv: [[[i16; 2]; 16]; 2],

  pub mvd: [[[u8; 2]; 16]; 2],

  pub direct8x8: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotionLayout {
  Uniform,
  Partitioned,
}

impl MbState {
  pub(crate) fn motion_layout(&self) -> Result<MotionLayout, Never> {
    let uniform = self
      .ref_idx
      .iter()
      .zip(&self.mv)
      .all(|(references, vectors)| {
        let [first_ref, other_refs @ ..] = references;
        let [first_vector, other_vectors @ ..] = vectors;
        other_refs.iter().all(|code| code == first_ref)
          && other_vectors.iter().all(|vector| vector == first_vector)
      });
    Ok(match ::core::primitive::bool::then_some(uniform, ()) {
      ::core::option::Option::Some(()) => MotionLayout::Uniform,
      ::core::option::Option::None => MotionLayout::Partitioned,
    })
  }
}

impl Default for MbState {
  fn default() -> Self {
    MbState {
      slice_num: None,
      kind: MbKind::None,
      transform_size: TransformSize::FourByFour,
      cbp: 0,
      qp: 0,
      qpc: [0; 2],
      intra_chroma_mode: 0,
      intra_modes: [2; 16],
      nnz: [0; 16],
      nnz_c: [[0; 4]; 2],
      cbf_dc: 0,
      nz_mask: 0,
      ref_idx: [[-1; 4]; 2],
      mv: [[[0; 2]; 16]; 2],
      mvd: [[[0; 2]; 16]; 2],
      direct8x8: 0,
    }
  }
}

pub struct FrameRow {
  pub y: Box<[u8]>,

  pub cb: Box<[u8]>,
  pub cr: Box<[u8]>,

  pub mv: [Box<[[i16; 2]]>; 2],

  pub ref_idx: [Box<[i8]>; 2],

  pub ref_id: [Box<[Option<u32>]>; 2],
  pub intra: Box<[bool]>,
}

pub struct Frame {
  pub id: u32,
  pub poc: i32,
  pub width: u32,
  pub height: u32,
  pub cwidth: u32,
  pub cheight: u32,
  pub mb_w: u32,
  rows: Box<[OnceLock<FrameRow>]>,
}

impl Frame {
  pub fn new(
    id: u32,
    poc: i32,
    Size {
      width: mb_w,
      height: mb_h,
    }: Size<u32>,
  ) -> Result<Self, Never> {
    Ok(Frame {
      id,
      poc,
      width: mb_w.wrapping_mul(16),
      height: mb_h.wrapping_mul(16),
      cwidth: mb_w.wrapping_mul(8),
      cheight: mb_h.wrapping_mul(8),
      mb_w,
      rows: (0..mb_h).map(|_| OnceLock::new()).collect(),
    })
  }

  pub fn mb_h(&self) -> Result<u32, Never> {
    Ok(self.height.wrapping_div(16))
  }

  pub fn publish(&self, row_index: u32, row: FrameRow) -> Result<(), Never> {
    let Ok(position) = index(row_index);

    match self.rows.get(position) {
      Some(slot) => match slot.set(row) {
        Ok(()) => {}
        Err(duplicate) => drop(duplicate),
      },
      None => {}
    }

    Ok(())
  }

  pub fn publication(&self, row_index: u32) -> Result<Publication, Never> {
    let Ok(position) = index(row_index);
    Ok(match self.rows.get(position).and_then(OnceLock::get) {
      Some(_) => Publication::Published,
      None => Publication::Pending,
    })
  }

  #[inline]
  pub fn row(&self, row_index: u32) -> Result<Option<&FrameRow>, Never> {
    let Ok(position) = index(row_index);
    let slot = match self.rows.get(position) {
      Some(slot) => slot,
      None => return Ok(None),
    };

    #[cfg(feature = "threads")]
    let row = Some(slot.wait());
    #[cfg(not(feature = "threads"))]
    let row = slot.get();
    Ok(row)
  }

  pub fn wait_complete(&self) -> Result<(), Never> {
    let Ok(count) = self.mb_h();

    for row in 0..count {
      let Ok(_published_row) = self.row(row);
    }

    Ok(())
  }

  pub fn make_row(
    planes: &Planes,
    row: u64,
    mbs: &[MbState],
    ref_ids: &dyn Fn(&MbState, u8, i8) -> Option<u32>,
  ) -> Result<FrameRow, Never> {
    let width = u64::from(planes.width);
    let chroma_width = u64::from(planes.cwidth);
    let Ok(y) = source_band(
      &planes.y,
      RowBand {
        row,
        length: width.wrapping_mul(16),
      },
    );
    let Ok(cb) = source_band(
      &planes.cb,
      RowBand {
        row,
        length: chroma_width.wrapping_mul(8),
      },
    );
    let Ok(cr) = source_band(
      &planes.cr,
      RowBand {
        row,
        length: chroma_width.wrapping_mul(8),
      },
    );
    let Ok(row_mbs) = source_band(
      mbs,
      RowBand {
        row,
        length: width.wrapping_div(16),
      },
    );
    let mut intra = Vec::with_capacity(row_mbs.len());
    let mut motion = core::array::from_fn(|_| Vec::with_capacity(row_mbs.len().wrapping_mul(16)));
    let mut indices = core::array::from_fn(|_| Vec::with_capacity(row_mbs.len().wrapping_mul(4)));
    let mut references =
      core::array::from_fn(|_| Vec::with_capacity(row_mbs.len().wrapping_mul(4)));

    for state in row_mbs {
      match state.slice_num {
        None => {
          intra.push(true);

          for ((mv, reference_index), reference_id) in
            motion.iter_mut().zip(&mut indices).zip(&mut references)
          {
            mv.extend([[0_i16; 2]; 16]);
            reference_index.extend([-1_i8; 4]);
            reference_id.extend([None; 4]);
          }
        }
        Some(_slice_number) => {
          intra.push(matches!(
            MacroblockPrediction::from(state.kind),
            MacroblockPrediction::Intra
          ));

          for (list, (((mv, reference_index), reference_id), (vectors, codes))) in (0_u8..2).zip(
            motion
              .iter_mut()
              .zip(&mut indices)
              .zip(&mut references)
              .zip(state.mv.into_iter().zip(state.ref_idx)),
          ) {
            mv.extend(vectors);
            reference_index.extend(codes);

            reference_id.extend(codes.map(|code| match code {
              0.. => ref_ids(state, list, code),
              ..=-1 => None,
            }));
          }
        }
      }
    }

    Ok(FrameRow {
      y: Box::from(y),
      cb: Box::from(cb),
      cr: Box::from(cr),
      mv: motion.map(Vec::into_boxed_slice),
      ref_idx: indices.map(Vec::into_boxed_slice),
      ref_id: references.map(Vec::into_boxed_slice),
      intra: intra.into_boxed_slice(),
    })
  }

  pub fn from_planes(
    id: u32,
    poc: i32,
    planes: &Planes,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let (mb_w, mb_h) = (
      planes.width.wrapping_div(16),
      planes.height.wrapping_div(16),
    );
    let Ok(f) = Frame::new(
      id,
      poc,
      Size {
        width: mb_w,
        height: mb_h,
      },
    );
    let Ok(count) = index(mb_w.saturating_mul(mb_h));
    let blank = vec![MbState::default(); count.min(1_048_576)];

    for r in 0..mb_h {
      let Ok(row) = Frame::make_row(planes, u64::from(r), &blank, &|_, _, _| None);
      let Ok(()) = f.publish(r, row);
    }

    Ok(f)
  }

  #[inline]
  pub fn luma_window(&self, window: SampleWindow, out: &mut [u8]) -> Result<(), RowLayoutError> {
    self.copy_window(Plane::Luma, window, out)
  }

  pub fn chroma_window(
    &self,
    component: ChromaPlane,
    window: SampleWindow,
    out: &mut [u8],
  ) -> Result<(), RowLayoutError> {
    let plane = match component {
      ChromaPlane::Cb => Plane::Cb,
      ChromaPlane::Cr => Plane::Cr,
    };
    self.copy_window(plane, window, out)
  }

  fn copy_window(
    &self,
    plane: Plane,
    window: SampleWindow,
    out: &mut [u8],
  ) -> Result<(), RowLayoutError> {
    let dimensions = match plane {
      Plane::Luma => Size {
        width: self.width,
        height: self.height,
      },
      Plane::Cb | Plane::Cr => Size {
        width: self.cwidth,
        height: self.cheight,
      },
    };
    let Ok(width) = index(dimensions.width);
    let Ok(height) = index(dimensions.height);
    let layout = RowLayout::new(window.size, 0, window.stride)?;
    copy_clamped_window(
      Size { width, height },
      Point {
        x: i64::from(window.origin.x),
        y: i64::from(window.origin.y),
      },
      (&layout, out),
      |source_y, columns| {
        let Ok(y) = fixed_index(source_y);
        let (band, line) = match plane {
          Plane::Luma => (y.wrapping_div(16), y.wrapping_rem(16)),
          Plane::Cb | Plane::Cr => (y.wrapping_div(8), y.wrapping_rem(8)),
        };
        let Ok(row_index) = wrapped::<_, u32>(band);
        let Ok(row) = self.row(row_index);
        let strip = row?;
        let samples = match plane {
          Plane::Luma => &strip.y,
          Plane::Cb => &strip.cb,
          Plane::Cr => &strip.cr,
        };
        let Ok(column_start) = fixed_index(columns.start);
        let Ok(column_end) = fixed_index(columns.end);
        let offset = line.checked_mul(u64::from(dimensions.width))?;
        let start = offset.checked_add(column_start)?;
        let end = offset.checked_add(column_end)?;
        let Ok(native_start) = index(start);
        let Ok(native_end) = index(end);
        samples.get(native_start..native_end)
      },
      |source, destination| {
        for (target, &sample) in destination.iter_mut().zip(source) {
          *target = sample;
        }
      },
    )
  }

  pub fn copy_cropped(&self, [x, y, width, height]: [u64; 4]) -> Result<CroppedPlanes, Never> {
    let Ok(allocate) = plane_allocator();
    let Ok(y_capacity) = index(width.wrapping_mul(height));
    let mut luma = allocate(y_capacity);
    let stride = u64::from(self.width);

    for sample_row in y..y.wrapping_add(height) {
      let Ok(row_index) = wrapped::<_, u32>(sample_row.wrapping_div(16));
      let Ok(found) = self.row(row_index);
      let row = match found {
        Some(row) => row,
        None => break,
      };
      let offset = (sample_row & 15).wrapping_mul(stride).wrapping_add(x);
      let Ok(bytes) = source_range(&row.y, offset..offset.wrapping_add(width));
      luma.extend_from_slice(bytes);
    }

    let chroma_x = x.wrapping_div(2);
    let chroma_y = y.wrapping_div(2);
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let Ok(chroma_capacity) = index(chroma_width.wrapping_mul(chroma_height));
    let mut cb = allocate(chroma_capacity);
    let mut cr = allocate(chroma_capacity);
    let chroma_stride = u64::from(self.cwidth);

    for sample_row in chroma_y..chroma_y.wrapping_add(chroma_height) {
      let Ok(row_index) = wrapped::<_, u32>(sample_row.wrapping_div(8));
      let Ok(found) = self.row(row_index);
      let row = match found {
        Some(row) => row,
        None => break,
      };
      let offset = (sample_row & 7)
        .wrapping_mul(chroma_stride)
        .wrapping_add(chroma_x);
      let range = offset..offset.wrapping_add(chroma_width);
      let Ok(u) = source_range(&row.cb, range.clone());
      let Ok(v) = source_range(&row.cr, range);
      cb.extend_from_slice(u);
      cr.extend_from_slice(v);
    }

    Ok(CroppedPlanes { y: luma, cb, cr })
  }
}

pub(crate) struct CroppedPlanes {
  pub(crate) y: Vec<u8>,
  pub(crate) cb: Vec<u8>,
  pub(crate) cr: Vec<u8>,
}

pub(crate) struct SampleWindow {
  pub(crate) size: Size<u64>,
  pub(crate) origin: Point<i32>,
  pub(crate) stride: i64,
}

#[derive(Clone, Copy)]
pub(crate) enum ChromaPlane {
  Cb,
  Cr,
}

enum Plane {
  Luma,
  Cb,
  Cr,
}

struct RowBand {
  row: u64,
  length: u64,
}

fn source_band<T>(source: &[T], band: RowBand) -> Result<&[T], Never> {
  let start = band.row.wrapping_mul(band.length);
  source_range(source, start..start.wrapping_add(band.length))
}

fn source_range<T>(source: &[T], range: core::ops::Range<u64>) -> Result<&[T], Never> {
  let Ok(start) = index(range.start);
  let Ok(end) = index(range.end);
  Ok(match source.get(start..end) {
    Some(bytes) => bytes,
    None => &[],
  })
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "the compatibility allocation hook is process-wide by its public contract and is fixed at first registration"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the compatibility hook accepts one process-wide allocator function and the registered callback cannot subsequently be changed"
  )
)]
static PLANE_ALLOCATOR: std::sync::OnceLock<PlaneAllocator> = std::sync::OnceLock::new();

pub fn set_plane_allocator(
  alloc: fn(::console_core_api_compatibility::NativeSize) -> Vec<u8>,
) -> core::result::Result<(), console_core_never::Never> {
  let _registered = PLANE_ALLOCATOR.get_or_init(|| alloc);

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "The existing public plane allocation callback accepts a native Vec capacity"
  )
)]
type PlaneAllocator = fn(console_core_api_compatibility::NativeSize) -> Vec<u8>;

fn plane_allocator() -> Result<PlaneAllocator, Never> {
  Ok(match PLANE_ALLOCATOR.get() {
    Some(allocator) => *allocator,
    None => Vec::with_capacity,
  })
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Decode workers publish independent immutable rows while reference lists and the display queue retain the same frame"
  )
)]
pub type FrameRef = Arc<Frame>;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Frame storage is readable concurrently by reference workers and the display queue while its owner publishes rows"
  )
)]
pub(crate) fn share_frame(frame: Frame) -> Result<FrameRef, Never> {
  Ok(Arc::new(frame))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Readers retain a published frame through DPB eviction and completion of queued workers"
  )
)]
pub(crate) fn retain_frame(frame: &FrameRef) -> Result<FrameRef, Never> {
  Ok(Arc::clone(frame))
}

#[derive(Clone)]
pub struct RefPic {
  pub frame: FrameRef,
  pub long_term: bool,
}

impl RefPic {
  pub fn poc(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(self.frame.poc)
  }

  pub fn id(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.frame.id)
  }
}
