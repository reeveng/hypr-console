//! Slice data decoding: macroblock parsing (CAVLC), motion vector derivation, and reconstruction.

use crate::cavlc;
use crate::deblock;
use crate::error::{Error, Result, ensure, invalid};
use crate::inter::{self, Weight};
use crate::intra::{self, Avail, IntraBlock};
use crate::mbtypes::*;
use crate::params::{DirectInference, IntraPrediction, Pps, PredictionWeighting, Sps};
use crate::picture::*;
use crate::slice::{PredWeightTable, SliceHeader, SliceType, WeightEntry};
use crate::tables::*;
use crate::transform::{self, LevelScale};
use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};
use console_core_pixel_buffers::RowLayoutError;

#[path = "cabac_mb.rs"]
mod cabac_mb;
use console_core_video_decoding_bitstream::{BitReader, FlagBit, Remaining};

#[derive(Clone, Debug, Default)]
pub struct SliceInfo {
  pub disable_deblocking_filter_idc: u32,
  pub alpha_offset: i32,
  pub beta_offset: i32,

  pub ref_ids: [Vec<u32>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowFlush {
  Ready,
  Finish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NeighborAvailability {
  Available,
  Unavailable,
}

#[derive(Clone, Copy)]
enum NeighborSide {
  Left,
  Top,
  TopRight,
  TopLeft,
}

impl From<bool> for NeighborAvailability {
  fn from(available: bool) -> Self {
    match available.then_some(()) {
      Some(()) => Self::Available,
      None => Self::Unavailable,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformEligibility {
  EightByEight,
  FourByFourOnly,
}

impl From<bool> for TransformEligibility {
  fn from(allows_eight: bool) -> Self {
    match allows_eight.then_some(()) {
      Some(()) => Self::EightByEight,
      None => Self::FourByFourOnly,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoefficientRange {
  AcOnly,
  All,
}

impl From<bool> for CoefficientRange {
  fn from(ac_only: bool) -> Self {
    match ac_only.then_some(()) {
      Some(()) => Self::AcOnly,
      None => Self::All,
    }
  }
}

pub struct PicState {
  pub planes: Planes,
  pub mbs: Vec<MbState>,
  pub slices: Vec<SliceInfo>,
  pub mb_w: u32,
  pub mb_h: u32,
  pub poc: i32,
  pub decoded_mbs: u32,
  pub frame: FrameRef,
  row_count: Vec<u16>,
  rows_recon: u32,
  rows_deblocked: u32,
  rows_published: u32,
  pub skip_deblock: bool,
}

impl PicState {
  pub fn new(frame: FrameRef) -> core::result::Result<Self, Never> {
    let Ok(mb_h) = frame.mb_h();
    let mb_w = frame.mb_w;
    let Ok(planes) = Planes::new(Size {
      width: frame.width,
      height: frame.height,
    });
    Ok(Self {
      planes,
      mbs: (0..mb_w.wrapping_mul(mb_h).min(1_048_576))
        .map(|_| MbState::default())
        .collect(),
      slices: Vec::new(),
      mb_w,
      mb_h,
      poc: frame.poc,
      decoded_mbs: 0,
      frame,
      row_count: (0..mb_h.min(1024)).map(|_| 0).collect(),
      rows_recon: 0,
      rows_deblocked: 0,
      rows_published: 0,
      skip_deblock: false,
    })
  }

  pub fn reset(&mut self, frame: FrameRef) -> core::result::Result<(), Never> {
    let Ok(height) = frame.mb_h();

    match ::core::primitive::bool::then_some((self.mb_w, self.mb_h) != (frame.mb_w, height), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(picture) = Self::new(frame);
        *self = picture;
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    for state in &mut self.mbs {
      state.slice_num = None;
    }

    self.slices.clear();
    self.poc = frame.poc;
    self.decoded_mbs = 0;
    self.frame = frame;
    self.row_count.fill(0);
    self.rows_recon = 0;
    self.rows_deblocked = 0;
    self.rows_published = 0;
    self.skip_deblock = false;
    Ok(())
  }

  #[inline]
  pub fn mb_done(&mut self, address: u64) -> core::result::Result<(), Never> {
    let row = match address.checked_div(u64::from(self.mb_w)) {
      Some(row) => row,
      None => return Ok(()),
    };
    let Ok(position) = index(row);
    let count = match self.row_count.get_mut(position) {
      Some(count) => count,
      None => return Ok(()),
    };
    *count = count.wrapping_add(1);
    self.decoded_mbs = self.decoded_mbs.wrapping_add(1);

    match ::core::primitive::bool::then_some(
      u32::from(*count) >= self.mb_w && row == u64::from(self.rows_recon),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.advance_ready_rows();
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  fn advance_ready_rows(&mut self) -> core::result::Result<(), Never> {
    for row in self.rows_recon..self.mb_h {
      let Ok(position) = index(row);

      match self
        .row_count
        .get(position)
        .map(|&count| u32::from(count) >= self.mb_w)
      {
        Some(true) => self.rows_recon = row.wrapping_add(1),
        Some(false) | None => break,
      }
    }

    self.flush_rows(RowFlush::Ready)
  }

  pub fn flush_rows(&mut self, flush: RowFlush) -> core::result::Result<(), Never> {
    let deblock_limit = match flush {
      RowFlush::Finish => self.mb_h,
      RowFlush::Ready => self.rows_recon.saturating_sub(1),
    };

    for row in self.rows_deblocked..deblock_limit {
      match ::core::primitive::bool::then_some(!self.skip_deblock, ()) {
        ::core::option::Option::Some(()) => {
          let start = row.wrapping_mul(self.mb_w);
          let end = start.wrapping_add(self.mb_w);

          for address in start..end {
            let Ok(()) = deblock::deblock_mb(self, address);
          }
        }
        ::core::option::Option::None => {}
      }

      self.rows_deblocked = row.wrapping_add(1);
    }

    let publish_limit = match flush {
      RowFlush::Finish => self.mb_h,
      RowFlush::Ready => self.rows_deblocked.saturating_sub(1),
    };

    for row in self.rows_published..publish_limit {
      let Ok(()) = self.publish_row(row);
      self.rows_published = row.wrapping_add(1);
    }

    Ok(())
  }

  fn publish_row(&self, row: u32) -> core::result::Result<(), Never> {
    let reference_id = |state: &MbState, list: u8, reference: i8| {
      let Ok(id) = self.reference_id(state, list, reference);

      id
    };
    let Ok(samples) = Frame::make_row(&self.planes, u64::from(row), &self.mbs, &reference_id);
    self.frame.publish(row, samples)
  }

  fn block_at(&self, address: u32) -> core::result::Result<Option<&MbState>, Never> {
    let Ok(position) = index(address);
    Ok(self.mbs.get(position))
  }

  fn reference_id(
    &self,
    state: &MbState,
    list: u8,
    reference: i8,
  ) -> core::result::Result<Option<u32>, Never> {
    let slice = match state.slice_num {
      Some(slice) => slice,
      None => return Ok(None),
    };
    let Ok(slice_position) = index(slice);
    let Ok(list_position) = index(list);
    let code = match u8::try_from(reference) {
      Ok(reference_2) => reference_2,
      Err(_negative_reference) => return Ok(None),
    };
    let Ok(reference_position) = index(code);
    Ok(
      self
        .slices
        .get(slice_position)
        .and_then(|slice_2| slice_2.ref_ids.get(list_position))
        .and_then(|references| references.get(reference_position))
        .copied(),
    )
  }
}

enum Weights<'a> {
  Default,
  Explicit(&'a PredWeightTable),
  Implicit(Vec<Vec<(i32, i32)>>),
}

#[derive(Clone, Copy)]
enum WeightPlane {
  Luma,
  Cb,
  Cr,
}

struct TemporalPoc {
  current: i32,
  first: i32,
  second: i32,
}

fn distance_scale(poc: TemporalPoc) -> core::result::Result<Option<i32>, Never> {
  let tb = poc.current.wrapping_sub(poc.first).clamp(-128, 127);
  let td = poc.second.wrapping_sub(poc.first).clamp(-128, 127);
  let numerator = 16_384i32.wrapping_add(td.wrapping_div(2).abs());
  let scale = numerator.checked_div(td).map(|tx| {
    tb.wrapping_mul(tx)
      .wrapping_add(32)
      .wrapping_shr(6)
      .clamp(-1024, 1023)
  });
  Ok(scale)
}

fn implicit_pair(
  first: &RefPic,
  second: &RefPic,
  current: i32,
) -> core::result::Result<(i32, i32), Never> {
  match ::core::primitive::bool::then_some(first.long_term || second.long_term, ()) {
    ::core::option::Option::Some(()) => {
      return Ok((32, 32));
    }
    ::core::option::Option::None => {}
  }

  let Ok(first_poc) = first.poc();
  let Ok(second_poc) = second.poc();
  let Ok(scale) = distance_scale(TemporalPoc {
    current,
    first: first_poc,
    second: second_poc,
  });
  let second_weight = scale.map(|factor| factor.wrapping_shr(2));
  Ok(match second_weight {
    Some(weight @ -64..=128) => (64i32.wrapping_sub(weight), weight),
    Some(_outside_weight_range) => (32, 32),
    None => (32, 32),
  })
}

fn temporal_scales(
  header: &SliceHeader,
  refs: &[Vec<RefPic>; 2],
  current: i32,
) -> core::result::Result<Vec<Option<i32>>, Never> {
  let [first, second] = refs;
  let second_picture = match (
    header.slice_type,
    header.direct_spatial_mv_pred,
    second.first(),
  ) {
    (SliceType::B, false, Some(second_2)) => second_2,
    _ => return Ok(Vec::new()),
  };
  let Ok(second_poc) = second_picture.poc();
  Ok(
    first
      .iter()
      .map(|picture| {
        let Ok(first_poc) = picture.poc();
        let Ok(scale) = distance_scale(TemporalPoc {
          current,
          first: first_poc,
          second: second_poc,
        });

        match (picture.long_term, scale) {
          (false, scale_2) => scale_2,
          (true, _) => None,
        }
      })
      .collect(),
  )
}

impl<'a> Weights<'a> {
  fn new(
    header: &'a SliceHeader,
    pps: &Pps,
    refs: &[Vec<RefPic>; 2],
    poc: i32,
  ) -> core::result::Result<Self, Never> {
    let explicit = (matches!(pps.weighted_pred, PredictionWeighting::Weighted)
      && matches!(header.slice_type, SliceType::P | SliceType::Sp))
      || (pps.weighted_bipred_idc == 1 && header.slice_type == SliceType::B);
    let [first, second] = refs;
    Ok(
      match (
        explicit,
        &header.pred_weight_table,
        header.slice_type,
        pps.weighted_bipred_idc,
      ) {
        (true, Some(table), _, _) => Self::Explicit(table),
        (false, _, SliceType::B, 2) => {
          let weights = first
            .iter()
            .map(|a| {
              second
                .iter()
                .map(|b| {
                  let Ok(pair) = implicit_pair(a, b, poc);
                  pair
                })
                .collect()
            })
            .collect();
          Self::Implicit(weights)
        }
        _ => Self::Default,
      },
    )
  }

  fn for_prediction(
    &self,
    references: [i8; 2],
    plane: WeightPlane,
  ) -> core::result::Result<Weight, Never> {
    let [first, second] = references;
    let weight = match self {
      Self::Default => Weight::Default,
      Self::Implicit(entries) => {
        let Ok(row) = reference_entry(entries, first);
        let pair = match row {
          Some(row_2) => {
            let Ok(pair) = reference_entry(row_2, second);
            pair
          }
          None => None,
        };

        match pair {
          Some(&(w0, w1)) => Weight::Weighted {
            log_wd: 5,
            w0,
            w1,
            o0: 0,
            o1: 0,
          },
          None => Weight::Default,
        }
      }
      Self::Explicit(table) => {
        let denominator = match plane {
          WeightPlane::Luma => table.luma_log2_denom,
          WeightPlane::Cb | WeightPlane::Cr => table.chroma_log2_denom,
        };
        let Ok((w0, o0)) = explicit_weight(&table.l0, first, plane, denominator);
        let Ok((w1, o1)) = explicit_weight(&table.l1, second, plane, denominator);
        let Ok(log_wd) = wrapped(denominator);
        Weight::Weighted {
          log_wd,
          w0,
          w1,
          o0,
          o1,
        }
      }
    };
    Ok(weight)
  }
}

fn reference_entry<T>(entries: &[T], reference: i8) -> core::result::Result<Option<&T>, Never> {
  let code = match u8::try_from(reference) {
    Ok(code) => code,
    Err(_negative_reference) => return Ok(None),
  };
  let Ok(position) = index(code);
  Ok(entries.get(position))
}

fn explicit_weight(
  entries: &[WeightEntry],
  reference: i8,
  plane: WeightPlane,
  denominator: u32,
) -> core::result::Result<(i32, i32), Never> {
  match ::core::primitive::bool::then_some(reference < 0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok((0, 0));
    }
    ::core::option::Option::None => {}
  }

  let Ok(entry) = reference_entry(entries, reference);
  let value = match entry {
    Some(entry_2) => entry_2,
    None => return Ok((1i32.wrapping_shl(denominator), 0)),
  };
  let [u_weight, v_weight] = value.chroma_weight;
  let [u_offset, v_offset] = value.chroma_offset;
  Ok(match plane {
    WeightPlane::Luma => (value.luma_weight, value.luma_offset),
    WeightPlane::Cb => (u_weight, u_offset),
    WeightPlane::Cr => (v_weight, v_offset),
  })
}

#[derive(Clone, Copy)]
enum PredictionList {
  First,
  Second,
}

impl PredictionList {
  fn entry<T>(self, entries: &[T; 2]) -> core::result::Result<&T, Never> {
    let [first, second] = entries;
    Ok(match self {
      Self::First => first,
      Self::Second => second,
    })
  }

  fn entry_mut<T>(self, entries: &mut [T; 2]) -> core::result::Result<&mut T, Never> {
    let [first, second] = entries;
    Ok(match self {
      Self::First => first,
      Self::Second => second,
    })
  }
}

const PREDICTION_LISTS: [(PredictionList, u8); 2] =
  [(PredictionList::First, 1), (PredictionList::Second, 2)];

#[derive(Clone, Copy, Default)]
pub(crate) struct MbCur {
  pub info: MbTypeInfo,
  pub sub: [SubMbInfo; 4],

  pub rem_mode: [i8; 16],
  pub cbp: u8,
  pub ref_idx: [[i8; 4]; 2],

  pub mvd: [[[i16; 2]; 16]; 2],
}

pub(crate) struct Scratch {
  pub coef: [i32; 256],

  pub coef_c: [[i32; 64]; 2],

  pub blk_nz: u16,

  pub c_nz: [u8; 2],
  pub pred: [[u8; 256]; 2],
  pub pred_c: [[[u8; 64]; 2]; 2],
}

impl Scratch {
  fn chroma(
    &mut self,
    plane: ChromaPlane,
  ) -> core::result::Result<(&mut [i32; 64], &mut u8), Never> {
    let [cb, cr] = &mut self.coef_c;
    let [cb_mask, cr_mask] = &mut self.c_nz;
    Ok(match plane {
      ChromaPlane::Cb => (cb, cb_mask),
      ChromaPlane::Cr => (cr, cr_mask),
    })
  }

  pub fn new() -> core::result::Result<Box<Self>, console_core_never::Never> {
    Ok(Box::new(Scratch {
      coef: [0; 256],
      coef_c: [[0; 64]; 2],
      blk_nz: 0,
      c_nz: [0; 2],
      pred: [[0; 256]; 2],
      pred_c: [[[0; 64]; 2]; 2],
    }))
  }
}

#[derive(Clone, Copy)]
struct MotionNeighbor {
  reference: Option<u8>,
  vector: [i16; 2],
}

type MotionNeighbors = [Option<MotionNeighbor>; 3];

struct PartitionSyntax {
  prediction: u8,
  position: u8,
}

#[derive(Clone, Copy)]
struct MotionPair {
  references: [Option<u8>; 2],
  vectors: [[i16; 2]; 2],
}

#[derive(Clone, Copy)]
enum PartitionPreference {
  None,
  Left,
  Top,
  Third,
}

#[derive(Clone, Copy)]
struct ColocatedMotion {
  reference: u8,
  id: Option<u32>,
  vector: [i16; 2],
}

#[derive(Clone, Copy)]
enum SpatialDirect {
  Default,
  NeighborPrediction(MotionPair),
}

#[derive(Clone, Copy)]
enum DirectMode {
  Spatial(SpatialDirect),
  Temporal,
}

#[derive(Clone, Copy)]
enum ColocatedLifetime {
  ShortTerm,
  Other,
}

fn spatial_motion(
  base: SpatialDirect,
  colocated: Option<ColocatedMotion>,
  lifetime: ColocatedLifetime,
) -> core::result::Result<MotionPair, Never> {
  let motion = match base {
    SpatialDirect::Default => {
      return Ok(MotionPair {
        references: [Some(0); 2],
        vectors: [[0, 0]; 2],
      });
    }
    SpatialDirect::NeighborPrediction(motion) => motion,
  };
  let zero = matches!(lifetime, ColocatedLifetime::ShortTerm)
    && matches!(colocated, Some(sample) if sample.reference == 0 && sample.vector.iter().all(|component| (-1..=1).contains(component)));
  let [first, second] = motion.references;
  let [first_mv, second_mv] = motion.vectors;
  let vectors =
    [(first, first_mv), (second, second_mv)].map(|(reference, mv)| match (reference, zero) {
      (None, _) | (Some(0), true) => [0, 0],
      (Some(_), _) => mv,
    });
  Ok(MotionPair {
    references: motion.references,
    vectors,
  })
}

fn median_prediction(
  neighbors: MotionNeighbors,
  reference: u8,
) -> core::result::Result<[i16; 2], Never> {
  let [left, top, right] = match neighbors {
    [Some(left), None, None] => [Some(left); 3],
    others => others,
  };
  let matches_reference = |motion: Option<MotionNeighbor>| {
    motion.filter(|candidate| candidate.reference == Some(reference))
  };

  match [
    matches_reference(left),
    matches_reference(top),
    matches_reference(right),
  ] {
    [Some(candidate), None, None]
    | [None, Some(candidate), None]
    | [None, None, Some(candidate)] => Ok(candidate.vector),
    _ => {
      let [[lx, ly], [tx, ty], [rx, ry]] = [left, top, right].map(|candidate| match candidate {
        Some(motion) => motion.vector,
        None => [0, 0],
      });
      let Ok(x) = median(MedianArguments {
        a: lx,
        b: tx,
        c: rx,
      });
      let Ok(y) = median(MedianArguments {
        a: ly,
        b: ty,
        c: ry,
      });
      Ok([x, y])
    }
  }
}

pub struct SliceDecoder<'a> {
  sh: &'a SliceHeader,
  pps: &'a Pps,
  sps: &'a Sps,
  pic: &'a mut PicState,
  refs: &'a [Vec<RefPic>; 2],
  ls: &'a LevelScale,
  slice_num: u32,
  qp: i32,
  chroma_qp_offset: [i32; 2],

  mb_addr: u32,
  active: MbState,
  mb_x: u32,
  mb_y: u32,

  nb: [Option<u32>; 4],
  mv_done: u16,
  cur: MbCur,
  s: Box<Scratch>,
  weights: Weights<'a>,

  tdirect: Vec<Option<i32>>,
  direct_ref_index: std::collections::BTreeMap<u32, i8>,

  prev_qp_delta_nz: bool,
}

struct PredictionTarget<'a> {
  luma: &'a mut [u8],
  chroma: [&'a mut [u8]; 2],
  luma_stride: u64,
  chroma_stride: u64,
}

impl<'a> PredictionTarget<'a> {
  fn frame(
    planes: &'a mut Planes,
    origin: Point<u64>,
  ) -> core::result::Result<Option<Self>, Never> {
    let luma_stride = u64::from(planes.width);
    let chroma_stride = u64::from(planes.cwidth);
    let Ok(luma_offset) = pixel_offset(luma_stride, origin);
    let Ok(chroma_position) = chroma_origin(origin);
    let Ok(chroma_offset) = pixel_offset(chroma_stride, chroma_position);
    let Ok(luma_index) = index(luma_offset);
    let Ok(chroma_index) = index(chroma_offset);
    Ok(
      match (
        planes.y.get_mut(luma_index..),
        planes.cb.get_mut(chroma_index..),
        planes.cr.get_mut(chroma_index..),
      ) {
        (Some(luma), Some(cb), Some(cr)) => Some(Self {
          luma,
          chroma: [cb, cr],
          luma_stride,
          chroma_stride,
        }),
        _ => None,
      },
    )
  }
}

#[derive(Clone, Copy)]
struct PredictionSource<'a> {
  frame: &'a Frame,
  vector: [i16; 2],
}

fn chroma_origin(luma: Point<u64>) -> core::result::Result<Point<u64>, Never> {
  Ok(Point {
    x: luma.x.wrapping_shr(1),
    y: luma.y.wrapping_shr(1),
  })
}

fn chroma_size(luma: Size<u64>) -> core::result::Result<Size<u64>, Never> {
  Ok(Size {
    width: luma.width.wrapping_shr(1),
    height: luma.height.wrapping_shr(1),
  })
}

fn predict_luma(
  source: PredictionSource<'_>,
  block: Rectangle<u64>,
  destination: &mut [u8],
  stride: u64,
) -> core::result::Result<(), RowLayoutError> {
  let [mx, my] = source.vector;
  let Ok(x) = wrapped::<_, i32>(block.origin.x);
  let Ok(y) = wrapped::<_, i32>(block.origin.y);
  let window = SampleWindow {
    origin: Point {
      x: x
        .wrapping_add(i32::from(mx).wrapping_shr(2))
        .wrapping_sub(2),
      y: y
        .wrapping_add(i32::from(my).wrapping_shr(2))
        .wrapping_sub(2),
    },
    size: Size {
      width: block.size.width.wrapping_add(5),
      height: block.size.height.wrapping_add(5),
    },
    stride: 24,
  };
  let mut samples = [0; 504];
  source.frame.luma_window(window, &mut samples)?;
  let Ok(fraction_x) = wrapped::<_, u32>(mx & 3);
  let Ok(fraction_y) = wrapped::<_, u32>(my & 3);
  let layout = inter::BlockLayout {
    size: block.size,
    source_stride: 24,
    destination_stride: stride,
  };
  let Ok(()) = inter::mc_luma_win(
    &samples,
    destination,
    layout,
    Point {
      x: fraction_x,
      y: fraction_y,
    },
  );
  Ok(())
}

fn predict_chroma(
  source: PredictionSource<'_>,
  component: ChromaPlane,
  block: Rectangle<u64>,
  destination: &mut [u8],
  stride: u64,
) -> core::result::Result<(), RowLayoutError> {
  let [mx, my] = source.vector;
  let Ok(origin) = chroma_origin(block.origin);
  let Ok(x) = wrapped::<_, i32>(origin.x);
  let Ok(y) = wrapped::<_, i32>(origin.y);
  let Ok(size) = chroma_size(block.size);
  let window = SampleWindow {
    origin: Point {
      x: x.wrapping_add(i32::from(mx).wrapping_shr(3)),
      y: y.wrapping_add(i32::from(my).wrapping_shr(3)),
    },
    size: Size {
      width: size.width.wrapping_add(1),
      height: size.height.wrapping_add(1),
    },
    stride: 16,
  };
  let mut samples = [0; 144];
  source
    .frame
    .chroma_window(component, window, &mut samples)?;
  let Ok(fraction_x) = wrapped::<_, u32>(mx & 7);
  let Ok(fraction_y) = wrapped::<_, u32>(my & 7);
  let layout = inter::BlockLayout {
    size,
    source_stride: 16,
    destination_stride: stride,
  };
  let Ok(()) = inter::mc_chroma_win(
    &samples,
    destination,
    layout,
    Point {
      x: fraction_x,
      y: fraction_y,
    },
  );
  Ok(())
}

fn predict_block(
  source: PredictionSource<'_>,
  block: Rectangle<u64>,
  target: PredictionTarget<'_>,
) -> core::result::Result<(), RowLayoutError> {
  predict_luma(source, block, target.luma, target.luma_stride)?;

  for (plane, destination) in [ChromaPlane::Cb, ChromaPlane::Cr]
    .into_iter()
    .zip(target.chroma)
  {
    predict_chroma(source, plane, block, destination, target.chroma_stride)?;
  }

  Ok(())
}

fn blend_prediction(
  target: PredictionTarget<'_>,
  size: Size<u64>,
  weights: [Weight; 3],
  scratch: &Scratch,
  sources: [Option<PredictionSource<'_>>; 2],
) -> core::result::Result<(), Never> {
  let [first, second] = sources;
  let [luma_weight, cb_weight, cr_weight] = weights;
  let [luma_left, luma_right] = &scratch.pred;
  let layout = inter::BlockLayout {
    size,
    source_stride: 16,
    destination_stride: target.luma_stride,
  };
  let luma = [
    first.map(|_| luma_left.as_slice()),
    second.map(|_| luma_right.as_slice()),
  ];
  let Ok(()) = inter::weighted_store(target.luma, luma, layout, luma_weight);
  let Ok(half) = chroma_size(size);
  let chroma_layout = inter::BlockLayout {
    size: half,
    source_stride: 8,
    destination_stride: target.chroma_stride,
  };
  let [[cb_left, cr_left], [cb_right, cr_right]] = &scratch.pred_c;
  let cb_sources = [
    first.map(|_| cb_left.as_slice()),
    second.map(|_| cb_right.as_slice()),
  ];
  let cr_sources = [
    first.map(|_| cr_left.as_slice()),
    second.map(|_| cr_right.as_slice()),
  ];
  let [cb, cr] = target.chroma;
  let Ok(()) = inter::weighted_store(cb, cb_sources, chroma_layout, cb_weight);
  let Ok(()) = inter::weighted_store(cr, cr_sources, chroma_layout, cr_weight);
  Ok(())
}

#[inline(always)]
fn median(
  MedianArguments { a, b, c }: MedianArguments,
) -> core::result::Result<i16, console_core_never::Never> {
  Ok(a.max(b).min(a.min(b).max(c)))
}

struct MedianArguments {
  a: i16,
  b: i16,
  c: i16,
}

fn block8_origin(quadrant: u8) -> core::result::Result<Point<u8>, Never> {
  Ok(Point {
    x: (quadrant & 1).wrapping_mul(2),
    y: quadrant.wrapping_shr(1).wrapping_mul(2),
  })
}

fn subblock_raster(parent: Point<u8>, child: u8) -> core::result::Result<u8, Never> {
  let x = parent.x.wrapping_add(child & 1);
  let y = parent.y.wrapping_add(child.wrapping_shr(1));
  Ok(y.wrapping_mul(4) | x)
}

fn offset_origin(base: Point<u64>, block: Point<u8>) -> core::result::Result<Point<u64>, Never> {
  Ok(Point {
    x: base.x.wrapping_add(u64::from(block.x).wrapping_mul(4)),
    y: base.y.wrapping_add(u64::from(block.y).wrapping_mul(4)),
  })
}

fn pixel_offset(stride: u64, pixel: Point<u64>) -> core::result::Result<u64, Never> {
  Ok(pixel.y.wrapping_mul(stride).wrapping_add(pixel.x))
}

fn write_pcm_plane<const N: usize>(
  source: &[u8],
  destination: &mut [u8],
  origin: Point<u64>,
  stride: u64,
) -> core::result::Result<(), Never> {
  let (rows, _trailing) = source.as_chunks::<N>();
  let Ok(start) = pixel_offset(stride, origin);

  for (row, samples) in (0u64..).zip(rows) {
    let Ok(position) = index(start.wrapping_add(row.wrapping_mul(stride)));
    let target = destination
      .get_mut(position..)
      .and_then(|pixels| pixels.first_chunk_mut::<N>());

    match target {
      Some(row_2) => *row_2 = *samples,
      None => break,
    }
  }

  Ok(())
}

struct CoefficientScan<'a> {
  levels: &'a [i32],
  scan: &'a [u8],
  weights: &'a [i32],
  quantizer: i32,
  range: CoefficientRange,
}

fn dequantize_coefficients(
  destination: &mut [i32],
  block: CoefficientScan<'_>,
  scale: impl Fn(i32, transform::Dequantization) -> core::result::Result<i32, Never>,
) -> core::result::Result<u8, Never> {
  let first = match block.range {
    CoefficientRange::All => 0u8,
    CoefficientRange::AcOnly => 1u8,
  };
  let Ok(skip) = index(first);
  let mut count = 0u8;

  for (&level, &raster) in block.levels.iter().zip(block.scan).skip(skip) {
    match level {
      0 => {
        continue;
      }
      _ => {}
    }

    let Ok(position) = index(raster);

    match (destination.get_mut(position), block.weights.get(position)) {
      (Some(output), Some(&weight)) => {
        let settings = transform::Dequantization {
          weight,
          quantizer: block.quantizer,
        };
        let Ok(value) = scale(level, settings);
        *output = value;
        count = count.wrapping_add(1);
      }
      _ => continue,
    }
  }

  Ok(count)
}

fn quantization_weights<const N: usize>(
  tables: &[[[i32; N]; 6]; 6],
  list: u8,
  quantizer: i32,
) -> core::result::Result<Option<&[i32; N]>, Never> {
  let Ok(remainder) = wrapped::<_, u8>(quantizer.wrapping_rem(6));
  let Ok(position) = index(remainder);
  let Ok(matrix) = index(list);
  Ok(tables.get(matrix).and_then(|values| values.get(position)))
}

impl<'a> SliceDecoder<'a> {
  pub fn new(
    sh: &'a SliceHeader,
    pps: &'a Pps,
    sps: &'a Sps,
    pic: &'a mut PicState,
    refs: &'a [Vec<RefPic>; 2],
    ls: &'a LevelScale,
  ) -> Result<Self> {
    let Ok(slice_num) = wrapped::<_, u32>(pic.slices.len());
    let chroma_qp_offset = [
      pps.chroma_qp_index_offset,
      pps.second_chroma_qp_index_offset,
    ];
    let [first, second] = refs;
    let ref_ids = [first, second].map(|list| {
      list
        .iter()
        .map(|reference| {
          let Ok(id) = reference.id();
          id
        })
        .collect()
    });
    pic.slices.push(SliceInfo {
      disable_deblocking_filter_idc: sh.disable_deblocking_filter_idc,
      alpha_offset: sh.slice_alpha_c0_offset_div2.wrapping_mul(2),
      beta_offset: sh.slice_beta_offset_div2.wrapping_mul(2),
      ref_ids,
    });
    let Ok(weights) = Weights::new(sh, pps, refs, pic.poc);
    let Ok(tdirect) = temporal_scales(sh, refs, pic.poc);

    let mut direct_ref_index = std::collections::BTreeMap::new();
    let [list0, _list1] = refs;

    for (index, reference) in list0.iter().enumerate() {
      let Ok(id) = reference.id();
      let Ok(index_2) = console_core_number_conversion::wrapped::<_, i8>(index);
      direct_ref_index.entry(id).or_insert(index_2);
    }

    Ok(SliceDecoder {
      sh,
      pps,
      sps,
      pic,
      refs,
      ls,
      slice_num,
      qp: ({
        let Ok(explicit_value) = sh.qp(pps);
        explicit_value
      }),
      chroma_qp_offset,
      mb_addr: 0,
      active: MbState::default(),
      mb_x: 0,
      mb_y: 0,
      nb: [None; 4],
      mv_done: 0,
      cur: MbCur::default(),
      s: ({
        let Ok(explicit_value) = Scratch::new();
        explicit_value
      }),
      weights,
      tdirect,
      direct_ref_index,
      prev_qp_delta_nz: false,
    })
  }

  pub fn start_mb(&mut self, address: u32) -> Result<()> {
    let width = self.pic.mb_w;
    let (x, y) = match (address.checked_rem(width), address.checked_div(width)) {
      (Some(x), Some(y)) => (x, y),
      _ => return invalid("macroblock position outside the picture"),
    };
    ensure(y < self.pic.mb_h, || {
      format!("macroblock address {address} out of range")
    })?;
    self.mb_addr = address;
    self.mb_x = x;
    self.mb_y = y;
    let slice = self.slice_num;
    let candidates = [
      (x != 0).then_some(address.wrapping_sub(1)),
      (y != 0).then_some(address.wrapping_sub(width)),
      (y != 0 && x.wrapping_add(1) < width)
        .then_some(address.wrapping_sub(width).wrapping_add(1)),
      (y != 0 && x != 0).then_some(address.wrapping_sub(width).wrapping_sub(1)),
    ];
    self.nb = candidates.map(|candidate| {
      candidate.filter(|&neighbor| {
        let Ok(state) = self.pic.block_at(neighbor);

        matches!(state, Some(mb) if mb.slice_num == Some(slice))
      })
    });
    self.mv_done = 0;
    self.active = MbState {
      slice_num: Some(slice),
      ..MbState::default()
    };
    Ok(())
  }

  fn finish_mb(&mut self, decoded: Result<()>) -> Result<()> {
    let Ok(position) = index(self.mb_addr);
    let output = match self.pic.mbs.get_mut(position) {
      Some(output) => output,
      None => return invalid("macroblock address outside the picture"),
    };
    *output = self.active;
    decoded?;
    let Ok(()) = self.pic.mb_done(u64::from(self.mb_addr));
    Ok(())
  }

  #[inline]
  pub fn mb(&self) -> core::result::Result<&MbState, Never> {
    Ok(&self.active)
  }

  #[inline]
  pub fn mb_mut(&mut self) -> core::result::Result<&mut MbState, Never> {
    Ok(&mut self.active)
  }

  fn macroblock(&self, address: Option<u32>) -> core::result::Result<Option<&MbState>, Never> {
    match address {
      Some(position) => match ::core::primitive::bool::then_some(position == self.mb_addr, ()) {
        ::core::option::Option::Some(()) => Ok(Some(&self.active)),
        ::core::option::Option::None => self.pic.block_at(position),
      },
      None => Ok(None),
    }
  }

  fn intra_nb(
    &self,
    direction: NeighborSide,
  ) -> core::result::Result<NeighborAvailability, Never> {
    let Ok(state) = self.intra_neighbor(direction);
    Ok(NeighborAvailability::from(state.is_some()))
  }

  fn intra_neighbor(
    &self,
    direction: NeighborSide,
  ) -> core::result::Result<Option<&MbState>, Never> {
    let [left, top, top_right, top_left] = self.nb;
    let address = match direction {
      NeighborSide::Left => left,
      NeighborSide::Top => top,
      NeighborSide::TopRight => top_right,
      NeighborSide::TopLeft => top_left,
    };
    let Ok(neighbor) = self.macroblock(address);
    Ok(neighbor.filter(|state| {
      !matches!(
        self.pps.constrained_intra_pred,
        IntraPrediction::IntraNeighbors
      ) || MacroblockPrediction::from(state.kind) == MacroblockPrediction::Intra
    }))
  }

  fn chroma_qp(&self, qp: i32, plane: ChromaPlane) -> core::result::Result<u8, Never> {
    let [cb, cr] = self.chroma_qp_offset;
    let offset = match plane {
      ChromaPlane::Cb => cb,
      ChromaPlane::Cr => cr,
    };
    let qpi = qp.wrapping_add(offset).clamp(0, 51);
    let Ok(quantizer) = wrapped::<_, u8>(qpi);
    let Ok(position) = index(quantizer);
    Ok(match QPC_TABLE.get(position) {
      Some(&value) => value,
      None => quantizer,
    })
  }

  fn nc_luma(&self, raster: u8) -> core::result::Result<i32, Never> {
    self.neighbor_coefficients(raster, 4, |state| Some(&state.nnz))
  }

  fn nc_chroma(&self, component: ChromaPlane, raster: u8) -> core::result::Result<i32, Never> {
    self.neighbor_coefficients(raster, 2, |state| {
      let [cb, cr] = &state.nnz_c;
      Some(match component {
        ChromaPlane::Cb => cb,
        ChromaPlane::Cr => cr,
      })
    })
  }

  fn neighbor_coefficients(
    &self,
    raster: u8,
    width: u16,
    coefficients: impl Fn(&MbState) -> Option<&[u8]>,
  ) -> core::result::Result<i32, Never> {
    let Ok(local_width) = wrapped::<_, u8>(width);
    let [left, top, ..] = self.nb;
    let Ok(left_state) = self.macroblock(left);
    let Ok(top_state) = self.macroblock(top);
    let mask = local_width.wrapping_sub(1);
    let (a, ra) = match raster & mask {
      0 => (left_state, raster.wrapping_add(mask)),
      _ => (Some(&self.active), raster.wrapping_sub(1)),
    };
    let (b, rb) = match ::core::primitive::bool::then_some(raster < local_width, ()) {
      ::core::option::Option::Some(()) => (
        top_state,
        raster.wrapping_add(local_width.wrapping_mul(mask)),
      ),
      ::core::option::Option::None => (Some(&self.active), raster.wrapping_sub(local_width)),
    };
    let Ok(a_index) = index(ra);
    let Ok(b_index) = index(rb);
    let a_count = a
      .and_then(&coefficients)
      .and_then(|values| values.get(a_index));
    let b_count = b
      .and_then(&coefficients)
      .and_then(|values| values.get(b_index));
    Ok(match (a_count, b_count) {
      (Some(&left_count), Some(&top_count)) => i32::from(left_count)
        .wrapping_add(i32::from(top_count))
        .wrapping_add(1)
        .wrapping_shr(1),
      (Some(&count), None) | (None, Some(&count)) => i32::from(count),
      (None, None) => 0,
    })
  }

  fn nb_motion(
    &self,
    list: PredictionList,
    block: Point<i8>,
  ) -> core::result::Result<Option<MotionNeighbor>, Never> {
    let [left, top, top_right, top_left] = self.nb;
    let (address, x, y) = match (block.x, block.y) {
      (..=-1, ..=-1) => (top_left, 3, 3),
      (..=-1, 0..=3) => (left, 3, block.y),
      (4.., ..=-1) => (top_right, 0, 3),
      (0..=3, ..=-1) => (top, block.x, 3),
      (0..=3, 0..=3) => (Some(self.mb_addr), block.x, block.y),
      _ => return Ok(None),
    };
    let Ok(neighbor) = self.macroblock(address);
    let state = match neighbor {
      Some(state) => state,
      None => return Ok(None),
    };
    let Ok(x4) = wrapped::<_, u8>(x);
    let Ok(y4) = wrapped::<_, u8>(y);
    let raster = y4.wrapping_mul(4) | x4;

    match ::core::primitive::bool::then_some(
      address == Some(self.mb_addr) && self.mv_done & 1u16.wrapping_shl(u32::from(raster)) == 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let quadrant = y4.wrapping_shr(1).wrapping_mul(2) | x4.wrapping_shr(1);
    let Ok(position) = index(quadrant);
    let Ok(references) = list.entry(&state.ref_idx);
    let reference = match references.get(position).copied().map(u8::try_from) {
      Some(Ok(reference)) => Some(reference),
      Some(Err(_unused_reference)) => None,
      None => None,
    };
    let vector = match reference {
      Some(_used) => {
        let Ok(vectors) = list.entry(&state.mv);
        let Ok(position_2) = index(raster);

        match vectors.get(position_2) {
          Some(&vector) => vector,
          None => return Ok(None),
        }
      }
      None => [0, 0],
    };
    Ok(Some(MotionNeighbor { reference, vector }))
  }

  fn motion_neighbors(
    &self,
    list: PredictionList,
    block: Rectangle<u8>,
  ) -> core::result::Result<MotionNeighbors, Never> {
    let Ok(x) = wrapped::<_, i8>(block.origin.x);
    let Ok(y) = wrapped::<_, i8>(block.origin.y);
    let Ok(width) = wrapped::<_, i8>(block.size.width);
    let Ok(left) = self.nb_motion(
      list,
      Point {
        x: x.wrapping_sub(1),
        y,
      },
    );
    let top_y = y.wrapping_sub(1);
    let Ok(top) = self.nb_motion(list, Point { x, y: top_y });
    let Ok(top_right) = self.nb_motion(
      list,
      Point {
        x: x.wrapping_add(width),
        y: top_y,
      },
    );
    let third = match top_right {
      Some(_available) => top_right,
      None => {
        let Ok(top_left) = self.nb_motion(
          list,
          Point {
            x: x.wrapping_sub(1),
            y: top_y,
          },
        );
        top_left
      }
    };
    Ok([left, top, third])
  }

  fn motion_vector_prediction(
    &self,
    list: PredictionList,
    block: Rectangle<u8>,
    reference: u8,
    preferred: PartitionPreference,
  ) -> core::result::Result<[i16; 2], Never> {
    let Ok(neighbors) = self.motion_neighbors(list, block);
    let [left, top, third] = neighbors;
    let candidate = match preferred {
      PartitionPreference::Left => left,
      PartitionPreference::Top => top,
      PartitionPreference::Third => third,
      PartitionPreference::None => None,
    };

    match candidate.filter(|neighbor| neighbor.reference == Some(reference)) {
      Some(neighbor) => Ok(neighbor.vector),
      None => median_prediction(neighbors, reference),
    }
  }

  fn set_motion(
    &mut self,
    block: Rectangle<u8>,
    motion: MotionPair,
  ) -> core::result::Result<(), Never> {
    let Rectangle { origin, size } = block;
    let [first, second] = motion.references;
    let [first_mv, second_mv] = motion.vectors;
    let encoded =
      [(first, first_mv), (second, second_mv)].map(|(reference, vector)| match reference {
        Some(code) => {
          let Ok(raw) = wrapped::<_, i8>(code);
          (raw, vector)
        }
        None => (-1, [0, 0]),
      });

    for y in origin.y..origin.y.wrapping_add(size.height) {
      for x in origin.x..origin.x.wrapping_add(size.width) {
        let raster = y.wrapping_mul(4) | x;
        let quadrant = y.wrapping_shr(1).wrapping_mul(2) | x.wrapping_shr(1);
        let Ok(raster_position) = index(raster);
        let Ok(quadrant_position) = index(quadrant);

        'inner_for_3: for ((reference, vector), (references, vectors)) in encoded
          .into_iter()
          .zip(self.active.ref_idx.iter_mut().zip(&mut self.active.mv))
        {
          match (
            references.get_mut(quadrant_position),
            vectors.get_mut(raster_position),
          ) {
            (Some(reference_slot), Some(vector_slot)) => {
              *reference_slot = reference;
              *vector_slot = vector;
            }
            _ => continue 'inner_for_3,
          }
        }

        self.mv_done |= 1u16.wrapping_shl(u32::from(raster));
      }
    }

    Ok(())
  }

  fn colocated_motion(&self, raster: u8) -> core::result::Result<Option<ColocatedMotion>, Never> {
    let [_first, second] = self.refs;
    let colocated = match second.first() {
      Some(picture) => picture,
      None => return Ok(None),
    };
    let Ok(height) = colocated.frame.mb_h();

    match ::core::primitive::bool::then_some(
      colocated.frame.mb_w != self.pic.mb_w || self.mb_y >= height,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(frame_row) = colocated.frame.row(self.mb_y);
    let row = match frame_row {
      Some(row) => row,
      None => return Ok(None),
    };
    let Ok(mb) = index(self.mb_x);

    match ::core::primitive::bool::then_some(!matches!(row.intra.get(mb), Some(&false)), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let block = match self.sps.direct_8x8_inference {
      DirectInference::Block8x8 => {
        let row_bit = match raster & 8 {
          0 => 0,
          _ => 12,
        };
        let column_bit = match raster & 2 {
          0 => 0,
          _ => 3,
        };
        row_bit | column_bit
      }
      DirectInference::Subblock4x4 => raster,
    };
    let block8 = block.wrapping_shr(3).wrapping_mul(2) | (block & 3).wrapping_shr(1);
    let Ok(reference_slot) = index(self.mb_x.wrapping_mul(4).wrapping_add(u32::from(block8)));
    let Ok(vector_slot) = index(self.mb_x.wrapping_mul(16).wrapping_add(u32::from(block)));
    let [first_indexes, second_indexes] = &row.ref_idx;
    let [first_ids, second_ids] = &row.ref_id;
    let [first_vectors, second_vectors] = &row.mv;

    for (indexes, (ids, vectors)) in [first_indexes, second_indexes].into_iter().zip(
      [first_ids, second_ids]
        .into_iter()
        .zip([first_vectors, second_vectors]),
    ) {
      match (
        indexes.get(reference_slot).copied().map(u8::try_from),
        vectors.get(vector_slot),
      ) {
        (Some(Ok(reference)), Some(&vector)) => {
          let id = ids.get(reference_slot).copied().flatten();
          return Ok(Some(ColocatedMotion {
            reference,
            id,
            vector,
          }));
        }
        _ => continue,
      }
    }

    Ok(None)
  }

  fn spatial_direct_base(&self) -> core::result::Result<SpatialDirect, Never> {
    let Ok(block) = Part::P16x16.block(0);
    let Ok(left) = self.motion_neighbors(PredictionList::First, block);
    let Ok(right) = self.motion_neighbors(PredictionList::Second, block);
    let nearest = |neighbors: MotionNeighbors| {
      neighbors
        .into_iter()
        .flatten()
        .filter_map(|neighbor| neighbor.reference)
        .min()
    };
    let references = [nearest(left), nearest(right)];
    let [first, second] = references;

    match references {
      [None, None] => return Ok(SpatialDirect::Default),
      _ => {}
    }

    let vectors =
      [(first, left), (second, right)].map(|(reference, neighbors)| match reference {
        Some(number) => {
          let Ok(vector) = median_prediction(neighbors, number);
          vector
        }
        None => [0, 0],
      });
    Ok(SpatialDirect::NeighborPrediction(MotionPair {
      references,
      vectors,
    }))
  }

  fn temporal_direct_motion(
    &self,
    colocated: Option<ColocatedMotion>,
  ) -> core::result::Result<MotionPair, Never> {
    let (reference, vector) = match colocated {
      Some(motion) => {
        let reference = match motion.id.and_then(|id| self.direct_ref_index.get(&id)) {
          Some(&index) => index,
          None => 0,
        };
        (reference, motion.vector)
      }
      None => (0, [0, 0]),
    };
    let Ok(ref0) = wrapped::<_, u8>(reference);
    let Ok(scale) = reference_entry(&self.tdirect, reference);
    let vectors = match scale {
      Some(&Some(factor)) => {
        let forward = vector.map(|sample| {
          let value = factor
            .wrapping_mul(i32::from(sample))
            .wrapping_add(128)
            .wrapping_shr(8);
          let Ok(component) = wrapped::<_, i16>(value);
          component
        });
        let [fx, fy] = forward;
        let [bx, by] = vector;
        [forward, [fx.wrapping_sub(bx), fy.wrapping_sub(by)]]
      }
      Some(&None) | None => [vector, [0, 0]],
    };
    Ok(MotionPair {
      references: [Some(ref0), Some(0)],
      vectors,
    })
  }

  fn derive_direct(&mut self, mask: u8) -> Result<()> {
    let mode = match ::core::primitive::bool::then_some(self.sh.direct_spatial_mv_pred, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(spatial) = self.spatial_direct_base();
        DirectMode::Spatial(spatial)
      }
      ::core::option::Option::None => DirectMode::Temporal,
    };
    let [_first, second] = self.refs;
    let lifetime = match second.first().map(|picture| picture.long_term) {
      Some(false) => ColocatedLifetime::ShortTerm,
      Some(true) | None => ColocatedLifetime::Other,
    };
    let (subblocks, side) = match self.sps.direct_8x8_inference {
      DirectInference::Block8x8 => (1, 2),
      DirectInference::Subblock4x4 => (4, 1),
    };

    for quadrant in 0u8..4 {
      match ::core::primitive::bool::then_some(
        mask & 1u8.wrapping_shl(u32::from(quadrant)) == 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(parent) = block8_origin(quadrant);

      for child in 0u8..subblocks {
        let origin = Point {
          x: parent.x.wrapping_add(child & 1),
          y: parent.y.wrapping_add(child.wrapping_shr(1)),
        };
        let raster = origin.y.wrapping_mul(4) | origin.x;
        let Ok(colocated) = self.colocated_motion(raster);
        let Ok(motion) = match mode {
          DirectMode::Spatial(spatial) => spatial_motion(spatial, colocated, lifetime),
          DirectMode::Temporal => self.temporal_direct_motion(colocated),
        };
        let partition = Rectangle {
          origin,
          size: Size {
            width: side,
            height: side,
          },
        };
        let Ok(()) = self.set_motion(partition, motion);
      }
    }

    self.active.direct8x8 |= mask;
    Ok(())
  }

  pub fn derive_p_skip(&mut self) -> core::result::Result<(), Never> {
    let Ok(left) = self.nb_motion(PredictionList::First, Point { x: -1, y: 0 });
    let Ok(top) = self.nb_motion(PredictionList::First, Point { x: 0, y: -1 });
    let [left_address, top_address, ..] = self.nb;
    let fixed_zero = left_address.is_none()
      || top_address.is_none()
      || [left, top]
        .into_iter()
        .flatten()
        .any(|neighbor| neighbor.reference == Some(0) && neighbor.vector == [0, 0]);
    let Ok(block) = Part::P16x16.block(0);
    let vector = match ::core::primitive::bool::then_some(fixed_zero, ()) {
      ::core::option::Option::Some(()) => [0, 0],
      ::core::option::Option::None => {
        let Ok(vector) = self.motion_vector_prediction(
          PredictionList::First,
          block,
          0,
          PartitionPreference::None,
        );
        vector
      }
    };
    self.set_motion(
      block,
      MotionPair {
        references: [Some(0), None],
        vectors: [vector, [0, 0]],
      },
    )
  }

  fn inter_references(&self, syntax: PartitionSyntax) -> Result<[Option<u8>; 2]> {
    let PartitionSyntax {
      prediction,
      position,
    } = syntax;
    let mut references = [None; 2];
    let Ok(slot) = index(position);

    for ((list, mask), output) in PREDICTION_LISTS.into_iter().zip(&mut references) {
      match ::core::primitive::bool::then_some(prediction & mask == 0, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(parsed) = list.entry(&self.cur.ref_idx);
      let code = match parsed.get(slot) {
        Some(&code) => code,
        None => return invalid("reference outside the macroblock partition"),
      };
      let Ok(available) = list.entry(self.refs);
      let Ok(picture) = reference_entry(available, code);
      let list_number = mask.wrapping_shr(1);
      ensure(picture.is_some(), || {
        format!(
          "ref_idx {code} out of range (list {list_number}, size {})",
          available.len()
        )
      })?;
      let Ok(reference) = wrapped::<_, u8>(code);
      *output = Some(reference);
    }

    Ok(references)
  }

  fn inter_vectors(
    &self,
    block: Rectangle<u8>,
    references: [Option<u8>; 2],
    position: u8,
    preferred: PartitionPreference,
  ) -> Result<[[i16; 2]; 2]> {
    let mut vectors = [[0, 0]; 2];
    let Ok(offset) = index(position);

    for (((list, _mask), reference), output) in PREDICTION_LISTS
      .into_iter()
      .zip(references)
      .zip(&mut vectors)
    {
      let code = match reference {
        Some(code) => code,
        None => continue,
      };
      let Ok(predicted) = self.motion_vector_prediction(list, block, code, preferred);
      let Ok(deltas) = list.entry(&self.cur.mvd);
      let &[dx, dy] = match deltas.get(offset) {
        Some(delta) => delta,
        None => return invalid("motion vector outside the macroblock partition"),
      };
      let [x, y] = predicted;
      *output = [x.wrapping_add(dx), y.wrapping_add(dy)];
    }

    Ok(vectors)
  }

  fn derive_partitions(&mut self, info: MbTypeInfo) -> Result<()> {
    let Ok(count) = info.part.block_count();

    for (prediction, ordinal) in info.pred.into_iter().zip(0u8..count) {
      let Ok(block) = info.part.block(ordinal);
      let references = self.inter_references(PartitionSyntax {
        prediction,
        position: ordinal,
      })?;
      let preferred = match (info.part, ordinal) {
        (Part::P16x8, 0) => PartitionPreference::Top,
        (Part::P16x8, _) | (Part::P8x16, 0) => PartitionPreference::Left,
        (Part::P8x16, _) => PartitionPreference::Third,
        _ => PartitionPreference::None,
      };
      let vectors = self.inter_vectors(block, references, ordinal.wrapping_mul(4), preferred)?;
      let Ok(()) = self.set_motion(
        block,
        MotionPair {
          references,
          vectors,
        },
      );
    }

    Ok(())
  }

  fn derive_subpartitions(&mut self) -> Result<()> {
    for (subtype, quadrant) in self.cur.sub.into_iter().zip(0u8..4) {
      match ::core::primitive::bool::then_some(subtype.direct, ()) {
        ::core::option::Option::Some(()) => {
          self.derive_direct(1u8.wrapping_shl(u32::from(quadrant)))?;
          continue;
        }
        ::core::option::Option::None => {}
      }

      let references = self.inter_references(PartitionSyntax {
        prediction: subtype.pred,
        position: quadrant,
      })?;
      let Ok(parent) = block8_origin(quadrant);
      let Ok(count) = subtype.shape.block_count();

      for child in 0..count {
        let Ok(relative) = subtype.shape.block(child);
        let origin = Point {
          x: parent.x.wrapping_add(relative.origin.x),
          y: parent.y.wrapping_add(relative.origin.y),
        };
        let block = Rectangle {
          origin,
          size: relative.size,
        };
        let offset = quadrant.wrapping_mul(4).wrapping_add(child);
        let vectors = self.inter_vectors(block, references, offset, PartitionPreference::None)?;
        let Ok(()) = self.set_motion(
          block,
          MotionPair {
            references,
            vectors,
          },
        );
      }
    }

    Ok(())
  }

  pub fn derive_inter_motion(&mut self) -> Result<()> {
    let info = self.cur.info;

    match (info.kind, info.part) {
      (MbKind::BDirect16x16 | MbKind::BSkip, _) => self.derive_direct(15),
      (_, Part::P8x8) => self.derive_subpartitions(),
      _ => self.derive_partitions(info),
    }
  }

  fn motion_sources(
    &self,
    origin: Point<u8>,
  ) -> core::result::Result<([Option<PredictionSource<'a>>; 2], [Weight; 3]), Never> {
    let raster = origin.y.wrapping_mul(4) | origin.x;
    let quadrant = origin.y.wrapping_shr(1).wrapping_mul(2) | origin.x.wrapping_shr(1);
    let Ok(raster_position) = index(raster);
    let Ok(quadrant_position) = index(quadrant);
    let [first_list, second_list] = self.refs;
    let [codes0, codes1] = &self.active.ref_idx;
    let [vectors0, vectors1] = &self.active.mv;
    let references = [codes0, codes1].map(|codes| match codes.get(quadrant_position) {
      Some(&code) => code,
      None => -1,
    });
    let [ref0, ref1] = references;
    let sources = [(ref0, first_list, vectors0), (ref1, second_list, vectors1)].map(
      |(reference, list, vectors)| {
        let Ok(available) = reference_entry(list, reference);
        let picture = available?;
        let vector = ::core::option::Option::<&_>::copied(vectors.get(raster_position))?;
        Some(PredictionSource {
          frame: &picture.frame,
          vector,
        })
      },
    );
    let weights = [WeightPlane::Luma, WeightPlane::Cb, WeightPlane::Cr].map(|plane| {
      let Ok(weight) = self.weights.for_prediction(references, plane);
      weight
    });
    Ok((sources, weights))
  }

  fn mc_rect(&mut self, partition: Rectangle<u8>) -> core::result::Result<(), Never> {
    let Ok(macroblock) = self.macroblock_origin(16);
    let Ok(origin) = offset_origin(macroblock, partition.origin);
    let size = Size {
      width: u64::from(partition.size.width).wrapping_mul(4),
      height: u64::from(partition.size.height).wrapping_mul(4),
    };
    let block = Rectangle { origin, size };
    let Ok((sources, weights)) = self.motion_sources(partition.origin);
    let Ok(output) = PredictionTarget::frame(&mut self.pic.planes, origin);
    let target = match output {
      Some(target) => target,
      None => return Ok(()),
    };

    match (sources, weights) {
      (
        [Some(source), None] | [None, Some(source)],
        [Weight::Default, Weight::Default, Weight::Default],
      ) => match predict_block(source, block, target) {
        Ok(()) => return Ok(()),
        Err(_invalid_sample_window) => return Ok(()),
      },
      _ => {}
    }

    let scratch = &mut *self.s;

    for ((source, luma), [cb, cr]) in sources
      .into_iter()
      .zip(&mut scratch.pred)
      .zip(&mut scratch.pred_c)
    {
      let source_3 = match source {
        Some(source_2) => source_2,
        None => continue,
      };
      let output_2 = PredictionTarget {
        luma,
        chroma: [cb, cr],
        luma_stride: 16,
        chroma_stride: 8,
      };

      match predict_block(source_3, block, output_2) {
        Ok(()) => {}
        Err(_invalid_sample_window) => continue,
      }
    }

    blend_prediction(target, size, weights, scratch, sources)
  }

  fn predict_direct_grid(
    &mut self,
    region: Rectangle<u8>,
    side: u8,
  ) -> core::result::Result<(), Never> {
    let filter = side.wrapping_sub(1);
    let size = Size {
      width: side,
      height: side,
    };
    let Rectangle {
      origin,
      size: bounds,
    } = region;

    for y in (origin.y..origin.y.wrapping_add(bounds.height))
      .filter(|y| y.wrapping_sub(origin.y) & filter == 0)
    {
      for x in (origin.x..origin.x.wrapping_add(bounds.width))
        .filter(|x| x.wrapping_sub(origin.x) & filter == 0)
      {
        let Ok(()) = self.mc_rect(Rectangle {
          origin: Point { x, y },
          size,
        });
      }
    }

    Ok(())
  }

  fn predict_subpartitions(&mut self, side: u8) -> core::result::Result<(), Never> {
    for (sub, quadrant) in self.cur.sub.into_iter().zip(0u8..4) {
      let Ok(parent) = Part::P8x8.block(quadrant);

      match ::core::primitive::bool::then_some(sub.direct, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = self.predict_direct_grid(parent, side);
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(count) = sub.shape.block_count();

      for ordinal in 0..count {
        let Ok(child) = sub.shape.block(ordinal);
        let origin = Point {
          x: parent.origin.x.wrapping_add(child.origin.x),
          y: parent.origin.y.wrapping_add(child.origin.y),
        };
        let Ok(()) = self.mc_rect(Rectangle {
          origin,
          size: child.size,
        });
      }
    }

    Ok(())
  }

  pub fn predict_inter(&mut self) -> core::result::Result<(), Never> {
    let info = self.cur.info;
    let side = match self.sps.direct_8x8_inference {
      DirectInference::Block8x8 => 2,
      DirectInference::Subblock4x4 => 1,
    };

    match info.kind {
      MbKind::PSkip => {
        let Ok(block) = Part::P16x16.block(0);
        self.mc_rect(block)
      }
      MbKind::BSkip | MbKind::BDirect16x16 => {
        let Ok(block) = Part::P16x16.block(0);
        let Ok(layout) = self.active.motion_layout();

        match layout {
          MotionLayout::Uniform => self.mc_rect(block),
          MotionLayout::Partitioned => self.predict_direct_grid(block, side),
        }
      }
      MbKind::None
      | MbKind::I4x4
      | MbKind::I8x8
      | MbKind::I16x16
      | MbKind::IPcm
      | MbKind::Inter => match info.part {
        Part::P8x8 => self.predict_subpartitions(side),
        Part::P16x16 | Part::P16x8 | Part::P8x16 => {
          let Ok(count) = info.part.block_count();

          for partition in 0..count {
            let Ok(block) = info.part.block(partition);
            let Ok(()) = self.mc_rect(block);
          }

          Ok(())
        }
      },
    }
  }

  fn intra_mode_nb(
    &self,
    side: NeighborSide,
    block: Point<u64>,
  ) -> core::result::Result<Option<u8>, Never> {
    let Ok(neighbor) = self.intra_neighbor(side);
    let state = match neighbor {
      Some(state) => state,
      None => return Ok(None),
    };
    let Ok(position) = index(block.y.wrapping_mul(4).wrapping_add(block.x));
    Ok(match state.kind {
      MbKind::I4x4 | MbKind::I8x8 => state.intra_modes.get(position).copied(),
      MbKind::None
      | MbKind::I16x16
      | MbKind::IPcm
      | MbKind::PSkip
      | MbKind::BSkip
      | MbKind::BDirect16x16
      | MbKind::Inter => Some(2),
    })
  }

  pub fn derive_intra_modes(&mut self, size: TransformSize) -> core::result::Result<(), Never> {
    let origins: &[(u8, u8)] = match size {
      TransformSize::EightByEight => &[(0, 0), (2, 0), (0, 2), (2, 2)],
      TransformSize::FourByFour => &BLK4_XY,
    };
    let step: u8 = match size {
      TransformSize::EightByEight => 2,
      TransformSize::FourByFour => 1,
    };
    let codes = self.cur.rem_mode;

    for (&(x, y), code) in origins.iter().zip(codes) {
      let Ok(left) = self.predicted_intra_mode(NeighborSide::Left, Point { x, y });
      let Ok(top) = self.predicted_intra_mode(NeighborSide::Top, Point { x, y });
      let predicted = match (left, top) {
        (Some(a), Some(b)) => a.min(b),
        _ => 2,
      };
      let mode = match u8::try_from(code) {
        Ok(mode) => match ::core::primitive::bool::then_some(mode < predicted, ()) {
          ::core::option::Option::Some(()) => mode,
          ::core::option::Option::None => mode.wrapping_add(1),
        },
        Err(_use_prediction) => predicted,
      };

      for row in y..y.wrapping_add(step) {
        'over_column_3: for column in x..x.wrapping_add(step) {
          let raster = row.wrapping_mul(4) | column;
          let Ok(position) = index(raster);

          match self.active.intra_modes.get_mut(position) {
            Some(slot) => *slot = mode,
            None => continue 'over_column_3,
          }
        }
      }
    }

    Ok(())
  }

  fn predicted_intra_mode(
    &self,
    side: NeighborSide,
    block: Point<u8>,
  ) -> core::result::Result<Option<u8>, Never> {
    let Point { x, y } = block;
    let raster = match (side, x, y) {
      (NeighborSide::Left, 0, _) => {
        return self.intra_mode_nb(
          side,
          Point {
            x: 3,
            y: u64::from(y),
          },
        );
      }
      (NeighborSide::Top, _, 0) => {
        return self.intra_mode_nb(
          side,
          Point {
            x: u64::from(x),
            y: 3,
          },
        );
      }
      (NeighborSide::Left, _, _) => y.wrapping_mul(4).wrapping_add(x.wrapping_sub(1)),
      (NeighborSide::Top, _, _) => y.wrapping_sub(1).wrapping_mul(4).wrapping_add(x),
      (NeighborSide::TopRight | NeighborSide::TopLeft, _, _) => return Ok(None),
    };
    let Ok(position) = index(raster);
    Ok(self.active.intra_modes.get(position).copied())
  }

  fn macroblock_origin(&self, side: u64) -> core::result::Result<Point<u64>, Never> {
    Ok(Point {
      x: u64::from(self.mb_x).wrapping_mul(side),
      y: u64::from(self.mb_y).wrapping_mul(side),
    })
  }

  fn add_luma_block4(&mut self, raster: u8) -> core::result::Result<(), Never> {
    match ::core::primitive::bool::then_some(
      self.s.blk_nz & 1u16.wrapping_shl(u32::from(raster)) == 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let stride = u64::from(self.pic.planes.width);
    let Ok(mb) = self.macroblock_origin(16);
    let Ok(origin) = offset_origin(
      mb,
      Point {
        x: raster & 3,
        y: raster.wrapping_shr(2),
      },
    );
    let Ok(offset) = pixel_offset(stride, origin);
    let Ok(positions) = index(offset);
    let Ok(coefficient) = index(u16::from(raster).wrapping_mul(16));
    let coded = self
      .s
      .coef
      .get_mut(coefficient..)
      .and_then(|values| values.first_chunk_mut::<16>());

    match (coded, self.pic.planes.y.get_mut(positions..)) {
      (Some(block), Some(pixels)) => {
        let Ok(()) = transform::idct4_add(block, pixels, stride);
        block.fill(0);
      }
      _ => {}
    }

    Ok(())
  }

  fn add_luma_block8(&mut self, quadrant: u8) -> core::result::Result<(), Never> {
    match ::core::primitive::bool::then_some(
      self.s.blk_nz & 1u16.wrapping_shl(u32::from(quadrant)) == 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let stride = u64::from(self.pic.planes.width);
    let Ok(mb) = self.macroblock_origin(16);
    let Ok(offset) = block8_origin(quadrant);
    let Ok(origin) = offset_origin(mb, offset);
    let Ok(pixel) = pixel_offset(stride, origin);
    let Ok(position) = index(pixel);
    let Ok(coefficient) = index(u16::from(quadrant).wrapping_mul(64));
    let coded = self
      .s
      .coef
      .get_mut(coefficient..)
      .and_then(|values| values.first_chunk_mut::<64>());

    match (coded, self.pic.planes.y.get_mut(position..)) {
      (Some(block), Some(pixels)) => {
        let Ok(()) = transform::idct8_add(block, pixels, stride);
        block.fill(0);
      }
      _ => {}
    }

    Ok(())
  }

  fn add_chroma(&mut self) -> core::result::Result<(), Never> {
    let stride = u64::from(self.pic.planes.cwidth);
    let Ok(origin) = self.macroblock_origin(8);
    let [cb_coefficients, cr_coefficients] = &mut self.s.coef_c;
    let [cb_active, cr_active] = &mut self.s.c_nz;

    let planes = [
      (&mut self.pic.planes.cb, cb_coefficients, cb_active),
      (&mut self.pic.planes.cr, cr_coefficients, cr_active),
    ];
    for (pixels, coefficients, active) in planes {
      'over_block_2: for block in 0u8..4 {
        match ::core::primitive::bool::then_some(
          *active & 1u8.wrapping_shl(u32::from(block)) == 0,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            continue 'over_block_2;
          }
          ::core::option::Option::None => {}
        }

        let horizontal = block & 1;
        let vertical = block.wrapping_shr(1);
        let Ok(block_origin) = offset_origin(
          origin,
          Point {
            x: horizontal,
            y: vertical,
          },
        );
        let Ok(pixel) = pixel_offset(stride, block_origin);
        let Ok(position) = index(pixel);
        let Ok(coefficient) = index(u16::from(block).wrapping_mul(16));
        let coded = coefficients
          .get_mut(coefficient..)
          .and_then(|values| values.first_chunk_mut::<16>());

        match (coded, pixels.get_mut(position..)) {
          (Some(levels), Some(output)) => {
            let Ok(()) = transform::idct4_add(levels, output, stride);
            levels.fill(0);
          }
          _ => continue 'over_block_2,
        }
      }

      *active = 0;
    }

    Ok(())
  }

  fn macroblock_availability(&self) -> core::result::Result<Avail, Never> {
    Ok(Avail {
      left: matches!(
        self.intra_nb(NeighborSide::Left),
        Ok(NeighborAvailability::Available)
      ),
      top: matches!(
        self.intra_nb(NeighborSide::Top),
        Ok(NeighborAvailability::Available)
      ),
      top_left: matches!(
        self.intra_nb(NeighborSide::TopLeft),
        Ok(NeighborAvailability::Available)
      ),
      top_right: matches!(
        self.intra_nb(NeighborSide::TopRight),
        Ok(NeighborAvailability::Available)
      ),
    })
  }

  fn luma_avail4(&self, block: Point<u8>) -> core::result::Result<Avail, Never> {
    let Ok(external) = self.macroblock_availability();
    let Point { x, y } = block;
    let top_left = match (x, y) {
      (0, 0) => external.top_left,
      (0, _) => external.left,
      (_, 0) => external.top,
      _ => true,
    };
    let top_right = match (x, y) {
      (3, 0) => external.top_right,
      (_, 0) => external.top,
      (3, _) => false,
      _ => {
        let current = y.wrapping_mul(4).wrapping_add(x);
        let previous = current.wrapping_sub(3);
        let Ok(current_position) = index(current);
        let Ok(previous_position) = index(previous);

        match (
          RASTER_TO_BLK4.get(previous_position),
          RASTER_TO_BLK4.get(current_position),
        ) {
          (Some(before), Some(after)) => before < after,
          _ => false,
        }
      }
    };
    Ok(Avail {
      left: x != 0 || external.left,
      top: y != 0 || external.top,
      top_left,
      top_right,
    })
  }

  fn intra_luma_4x4(
    &mut self,
    origin: Point<u64>,
    stride: u64,
  ) -> core::result::Result<(), Never> {
    for (x, y) in BLK4_XY {
      let coordinate = Point { x, y };
      let raster = y.wrapping_mul(4) | x;
      let Ok(available) = self.luma_avail4(coordinate);
      let Ok(position) = index(raster);
      let mode = match self.active.intra_modes.get(position) {
        Some(&mode) => mode,
        None => continue,
      };
      let Ok(start) = offset_origin(origin, coordinate);
      let block = IntraBlock {
        stride,
        origin: start,
        mode,
        available,
      };
      let Ok(()) = intra::pred4x4(&mut self.pic.planes.y, block);
      let Ok(()) = self.add_luma_block4(raster);
    }

    Ok(())
  }

  fn intra_luma_8x8(
    &mut self,
    origin: Point<u64>,
    stride: u64,
  ) -> core::result::Result<(), Never> {
    let Ok(external) = self.macroblock_availability();

    for quadrant in 0u8..4 {
      let Ok(coordinate) = block8_origin(quadrant);
      let raster = coordinate.y.wrapping_mul(4) | coordinate.x;
      let Ok(available) = self.luma_avail4(coordinate);
      let top_right = match quadrant {
        0 => external.top,
        1 => external.top_right,
        2 => true,
        _ => false,
      };
      let Ok(position) = index(raster);
      let mode = match self.active.intra_modes.get(position) {
        Some(&mode) => mode,
        None => continue,
      };
      let Ok(start) = offset_origin(origin, coordinate);
      let block = IntraBlock {
        stride,
        origin: start,
        mode,
        available: Avail {
          top_right,
          ..available
        },
      };
      let Ok(()) = intra::pred8x8(&mut self.pic.planes.y, block);
      let Ok(()) = self.add_luma_block8(quadrant);
    }

    Ok(())
  }

  pub fn reconstruct_intra(&mut self) -> core::result::Result<(), Never> {
    let stride = u64::from(self.pic.planes.width);
    let Ok(origin) = self.macroblock_origin(16);
    let Ok(external) = self.macroblock_availability();
    let available = Avail {
      top_right: false,
      ..external
    };

    match self.active.kind {
      MbKind::I4x4 => {
        let Ok(()) = self.intra_luma_4x4(origin, stride);
      }
      MbKind::I8x8 => {
        let Ok(()) = self.intra_luma_8x8(origin, stride);
      }
      MbKind::I16x16 => {
        let block = IntraBlock {
          stride,
          origin,
          mode: self.cur.info.i16_mode,
          available,
        };
        let Ok(()) = intra::pred16x16(&mut self.pic.planes.y, block);

        for raster in 0..16 {
          let Ok(()) = self.add_luma_block4(raster);
        }
      }
      MbKind::None
      | MbKind::IPcm
      | MbKind::PSkip
      | MbKind::BSkip
      | MbKind::BDirect16x16
      | MbKind::Inter => {}
    }

    let Ok(chroma_origin) = self.macroblock_origin(8);
    let block = IntraBlock {
      stride: u64::from(self.pic.planes.cwidth),
      origin: chroma_origin,
      mode: self.active.intra_chroma_mode,
      available,
    };
    let Ok(()) = intra::pred_chroma(&mut self.pic.planes.cb, block);
    let Ok(()) = intra::pred_chroma(&mut self.pic.planes.cr, block);
    let Ok(()) = self.add_chroma();
    self.s.blk_nz = 0;
    Ok(())
  }

  pub fn reconstruct_inter_residual(&mut self) -> core::result::Result<(), Never> {
    match ::core::primitive::bool::then_some(
      self.active.transform_size == TransformSize::EightByEight,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        for quadrant in 0..4 {
          let Ok(()) = self.add_luma_block8(quadrant);
        }
      }
      ::core::option::Option::None => {
        for raster in 0..16 {
          let Ok(()) = self.add_luma_block4(raster);
        }
      }
    }

    let Ok(()) = self.add_chroma();
    self.s.blk_nz = 0;
    Ok(())
  }

  pub fn write_pcm(&mut self, samples: &[u8]) -> core::result::Result<(), Never> {
    let luma = match samples.first_chunk::<256>() {
      Some(luma) => luma,
      None => return Ok(()),
    };
    let chroma = match samples
      .get(256..)
      .and_then(|rest| rest.first_chunk::<128>())
    {
      Some(chroma) => chroma,
      None => return Ok(()),
    };
    let (cb, cr) = match (chroma.first_chunk::<64>(), chroma.last_chunk::<64>()) {
      (Some(cb), Some(cr)) => (cb, cr),
      _ => return Ok(()),
    };
    let Ok(luma_origin) = self.macroblock_origin(16);
    let Ok(chroma_origin) = self.macroblock_origin(8);
    let Ok(()) = write_pcm_plane::<16>(
      luma,
      &mut self.pic.planes.y,
      luma_origin,
      u64::from(self.pic.planes.width),
    );
    let chroma_stride = u64::from(self.pic.planes.cwidth);
    let Ok(()) = write_pcm_plane::<8>(cb, &mut self.pic.planes.cb, chroma_origin, chroma_stride);
    let Ok(()) = write_pcm_plane::<8>(cr, &mut self.pic.planes.cr, chroma_origin, chroma_stride);
    Ok(())
  }

  pub fn finish_pcm(&mut self) -> core::result::Result<(), Never> {
    let Ok(qpc) = self.chroma_quantizers(0);
    self.active.kind = MbKind::IPcm;
    self.active.qp = 0;
    self.active.qpc = qpc;
    self.active.nnz = [16; 16];
    self.active.nnz_c = [[16; 4]; 2];
    self.active.cbf_dc = 7;
    self.active.cbp = 0x2f;
    self.active.nz_mask = 0xffff;
    self.active.ref_idx = [[-1; 4]; 2];
    Ok(())
  }

  pub fn apply_qp_delta(&mut self, delta: i32) -> Result<()> {
    ensure((-26..=25).contains(&delta), || {
      format!("mb_qp_delta {delta} out of range")
    })?;
    self.qp = self
      .qp
      .wrapping_add(delta)
      .wrapping_add(52)
      .wrapping_rem(52);
    Ok(())
  }

  pub fn store_qp(&mut self) -> core::result::Result<(), Never> {
    let Ok(integer) = wrapped(self.qp);
    let Ok(chroma) = self.chroma_quantizers(self.qp);
    self.active.qp = integer;
    self.active.qpc = chroma;
    Ok(())
  }

  fn chroma_quantizers(&self, qp: i32) -> core::result::Result<[u8; 2], Never> {
    Ok([ChromaPlane::Cb, ChromaPlane::Cr].map(|plane| {
      let Ok(chroma) = self.chroma_qp(qp, plane);
      chroma
    }))
  }

  fn chroma_selection(&self, plane: ChromaPlane) -> core::result::Result<(u8, i32), Never> {
    let list = match (MacroblockPrediction::from(self.active.kind), plane) {
      (MacroblockPrediction::Intra, ChromaPlane::Cb) => 1,
      (MacroblockPrediction::Intra, ChromaPlane::Cr) => 2,
      (MacroblockPrediction::Inter, ChromaPlane::Cb) => 4,
      (MacroblockPrediction::Inter, ChromaPlane::Cr) => 5,
    };
    let Ok(quantizer) = self.chroma_qp(self.qp, plane);
    Ok((list, i32::from(quantizer)))
  }

  pub fn put_luma4(
    &mut self,
    raster: u8,
    levels: &[i32; 16],
    range: CoefficientRange,
  ) -> core::result::Result<(), Never> {
    let list = match MacroblockPrediction::from(self.active.kind) {
      MacroblockPrediction::Intra => 0,
      MacroblockPrediction::Inter => 3,
    };
    let quantizer = self.qp;
    let Ok(weights) = quantization_weights(&self.ls.ls4, list, quantizer);
    let Ok(position) = index(u16::from(raster).wrapping_mul(16));
    let block = self
      .s
      .coef
      .get_mut(position..)
      .and_then(|values| values.first_chunk_mut::<16>());

    match (block, weights) {
      (Some(coefficients), Some(scale)) => {
        let Ok(count) = dequantize_coefficients(
          coefficients,
          CoefficientScan {
            levels,
            scan: &ZIGZAG4,
            weights: scale,
            quantizer,
            range,
          },
          transform::scale4,
        );

        match count {
          0 => {}
          _ => {
            let bit = 1u16.wrapping_shl(u32::from(raster));
            self.s.blk_nz |= bit;
            self.active.nz_mask |= bit;
          }
        }
      }
      _ => {}
    }

    Ok(())
  }

  pub fn put_luma8(
    &mut self,
    quadrant: u8,
    levels: &[i32; 64],
  ) -> core::result::Result<(), Never> {
    let list = match MacroblockPrediction::from(self.active.kind) {
      MacroblockPrediction::Intra => 0,
      MacroblockPrediction::Inter => 1,
    };
    let quantizer = self.qp;
    let Ok(weights) = quantization_weights(&self.ls.ls8, list, quantizer);
    let Ok(position) = index(u16::from(quadrant).wrapping_mul(64));
    let block = self
      .s
      .coef
      .get_mut(position..)
      .and_then(|values| values.first_chunk_mut::<64>());

    match (block, weights) {
      (Some(coefficients), Some(scale)) => {
        let Ok(count) = dequantize_coefficients(
          coefficients,
          CoefficientScan {
            levels,
            scan: &ZIGZAG8,
            weights: scale,
            quantizer,
            range: CoefficientRange::All,
          },
          transform::scale8,
        );

        match count {
          0 => {}
          _ => {
            self.s.blk_nz |= 1u16.wrapping_shl(u32::from(quadrant));
            let Ok(origin) = block8_origin(quadrant);
            let raster = origin.y.wrapping_mul(4) | origin.x;
            self.active.nz_mask |= 0x33u16.wrapping_shl(u32::from(raster));
          }
        }
      }
      _ => {}
    }

    Ok(())
  }

  pub fn put_luma_dc(&mut self, levels: &[i32; 16]) -> core::result::Result<(), Never> {
    let mut expanded = [0; 16];

    for (&raster, &coefficient) in ZIGZAG4.iter().zip(levels) {
      let Ok(position) = index(raster);

      match expanded.get_mut(position) {
        Some(output) => *output = coefficient,
        None => continue,
      }
    }

    let quantizer = self.qp;
    let Ok(weight) = quantization_weights(&self.ls.ls4, 0, quantizer);
    let weight_2 = match weight {
      Some(&[dc, ..]) => dc,
      None => return Ok(()),
    };
    let Ok(()) = transform::luma_dc_dequant(
      &mut expanded,
      transform::Dequantization {
        weight: weight_2,
        quantizer,
      },
    );
    let mut slots = self.s.coef.iter_mut().step_by(16);

    for ((bit, coefficient), output) in (0u32..16).zip(expanded).zip(&mut slots) {
      match coefficient {
        0 => {}
        _ => {
          *output = coefficient;
          self.s.blk_nz |= 1u16.wrapping_shl(bit);
        }
      }
    }

    Ok(())
  }

  pub fn put_chroma_dc(
    &mut self,
    plane: ChromaPlane,
    levels: &[i32; 4],
  ) -> core::result::Result<(), Never> {
    let Ok((list, quantizer)) = self.chroma_selection(plane);
    let Ok(weights) = quantization_weights(&self.ls.ls4, list, quantizer);
    let weight = match weights {
      Some(&[dc, ..]) => dc,
      None => return Ok(()),
    };
    let mut expanded = *levels;
    let Ok(()) = transform::chroma_dc_dequant_420(
      &mut expanded,
      transform::Dequantization { weight, quantizer },
    );
    let Ok((coefficients, active)) = self.s.chroma(plane);

    for ((bit, coefficient), output) in (0u32..4)
      .zip(expanded)
      .zip(coefficients.iter_mut().step_by(16))
    {
      match coefficient {
        0 => {}
        _ => {
          *output = coefficient;
          *active |= 1u8.wrapping_shl(bit);
        }
      }
    }

    Ok(())
  }

  pub fn put_chroma_ac(
    &mut self,
    plane: ChromaPlane,
    raster: u8,
    levels: &[i32; 16],
  ) -> core::result::Result<(), Never> {
    let Ok((list, quantizer)) = self.chroma_selection(plane);
    let Ok(weights) = quantization_weights(&self.ls.ls4, list, quantizer);
    let Ok((coefficients, active)) = self.s.chroma(plane);
    let Ok(position) = index(u16::from(raster).wrapping_mul(16));
    let block = coefficients
      .get_mut(position..)
      .and_then(|values| values.first_chunk_mut::<16>());

    match (block, weights) {
      (Some(coefficients_2), Some(scale)) => {
        let Ok(count) = dequantize_coefficients(
          coefficients_2,
          CoefficientScan {
            levels,
            scan: &ZIGZAG4,
            weights: scale,
            quantizer,
            range: CoefficientRange::AcOnly,
          },
          transform::scale4,
        );

        match count {
          0 => {}
          _ => {
            *active |= 1u8.wrapping_shl(u32::from(raster));
          }
        }
      }
      _ => {}
    }

    Ok(())
  }

  pub fn decode_skip(&mut self) -> Result<()> {
    self.cur = MbCur::default();
    self.cur.info.kind = match self.sh.slice_type {
      SliceType::B => MbKind::BSkip,
      SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => MbKind::PSkip,
    };
    self.active.kind = self.cur.info.kind;
    let Ok(()) = self.store_qp();

    match self.sh.slice_type {
      SliceType::B => self.derive_direct(15)?,
      SliceType::P | SliceType::Sp | SliceType::I | SliceType::Si => {
        let [first, _second] = self.refs;
        ensure(!first.is_empty(), || "P_Skip with empty reference list")?;
        let Ok(()) = self.derive_p_skip();
      }
    }

    let Ok(()) = self.predict_inter();
    self.prev_qp_delta_nz = false;
    Ok(())
  }

  fn read_type_cavlc(&self, reader: &mut BitReader) -> Result<MbTypeInfo> {
    let code = reader.read_ue()?;
    let Ok(mapped) = match self.sh.slice_type {
      SliceType::I | SliceType::Si => intra_mb_type(code),
      SliceType::P | SliceType::Sp => match code {
        0..=4 => p_mb_type(code),
        _ => intra_mb_type(code.wrapping_sub(5)),
      },
      SliceType::B => match code {
        0..=22 => b_mb_type(code),
        _ => intra_mb_type(code.wrapping_sub(23)),
      },
    };

    match mapped {
      Some(info) => Ok(info),
      None => invalid(format!("mb_type {code} out of range")),
    }
  }

  fn decode_pcm_cavlc(&mut self, reader: &mut BitReader) -> Result<()> {
    let Ok(()) = reader.byte_align();
    let Ok(offset) = reader.byte_pos();
    let Ok(position) = index(offset);
    let Ok(data) = reader.data();
    let samples = match data
      .get(position..)
      .and_then(|bytes| bytes.first_chunk::<384>())
    {
      Some(samples) => samples,
      None => return invalid("truncated I_PCM macroblock"),
    };
    reader.skip(384u64.wrapping_mul(8))?;
    let Ok(()) = self.finish_pcm();
    let Ok(()) = self.write_pcm(samples);
    Ok(())
  }

  fn decode_intra_modes_cavlc(
    &mut self,
    reader: &mut BitReader,
    size: TransformSize,
  ) -> Result<()> {
    let limit = match size {
      TransformSize::EightByEight => 4u8,
      TransformSize::FourByFour => 16u8,
    };
    let Ok(count) = index(limit);

    for mode in self.cur.rem_mode.iter_mut().take(count) {
      let checked_value = reader.read_flag()?;
      *mode = match checked_value {
        FlagBit::One => -1,
        FlagBit::Zero => {
          let bits = reader.read_bits(3)?;
          let Ok(value) = wrapped(bits);
          value
        }
      };
    }

    let Ok(()) = self.derive_intra_modes(size);
    Ok(())
  }

  fn read_ref_cavlc(reader: &mut BitReader, count: u32) -> Result<i8> {
    let code = match count {
      0 | 1 => 0,
      2 => {
        let bit = reader.read_flag()?;
        u32::from(matches!(bit, FlagBit::Zero))
      }
      _ => reader.read_ue()?,
    };
    let Ok(reference) = wrapped(code);
    Ok(reference)
  }

  fn store_ref_cavlc(
    &mut self,
    reader: &mut BitReader,
    list: PredictionList,
    block: u8,
    count: u32,
  ) -> Result<()> {
    let Ok(references) = list.entry_mut(&mut self.cur.ref_idx);
    let Ok(position) = index(block);
    let slot = references
      .get_mut(position)
      .ok_or_else(|| Error::Invalid("reference outside the macroblock partition".to_owned()))?;
    let checked_value = Self::read_ref_cavlc(reader, count)?;
    *slot = checked_value;
    Ok(())
  }

  fn store_mvd_cavlc(
    &mut self,
    reader: &mut BitReader,
    list: PredictionList,
    block: u8,
  ) -> Result<()> {
    let horizontal = reader.read_se()?;
    let vertical = reader.read_se()?;
    let Ok(motion_vectors) = list.entry_mut(&mut self.cur.mvd);
    let Ok(position) = index(block);
    let slot = motion_vectors.get_mut(position).ok_or_else(|| {
      Error::Invalid("motion vector outside the macroblock partition".to_owned())
    })?;
    *slot = [horizontal, vertical].map(|component| {
      let Ok(value) = wrapped(component);
      value
    });
    Ok(())
  }

  fn read_sub_type_cavlc(&self, reader: &mut BitReader) -> Result<SubMbInfo> {
    let code = reader.read_ue()?;
    let Ok(mapped) = match self.sh.slice_type {
      SliceType::B => b_sub_mb_type(code),
      SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => p_sub_mb_type(code),
    };

    match mapped {
      Some(info) => Ok(info),
      None => invalid(format!("sub_mb_type {code} out of range")),
    }
  }

  fn decode_sub_refs_cavlc(
    &mut self,
    reader: &mut BitReader,
    info: MbTypeInfo,
    subtypes: [SubMbInfo; 4],
  ) -> Result<()> {
    for ((list, mask), num_refs) in PREDICTION_LISTS.into_iter().zip(self.sh.num_ref_idx_active) {
      let count = match ::core::primitive::bool::then_some(info.ref0, ()) {
        ::core::option::Option::Some(()) => 1,
        ::core::option::Option::None => num_refs,
      };

      'inner_for_2: for (subtype, block) in subtypes.into_iter().zip(0u8..4) {
        match ::core::primitive::bool::then_some(subtype.direct || subtype.pred & mask == 0, ()) {
          ::core::option::Option::Some(()) => {
            continue 'inner_for_2;
          }
          ::core::option::Option::None => {}
        }

        self.store_ref_cavlc(reader, list, block, count)?;
      }
    }

    Ok(())
  }

  fn decode_subparts_cavlc(&mut self, reader: &mut BitReader, info: MbTypeInfo) -> Result<()> {
    let checked_value = self.read_sub_type_cavlc(reader)?;
    let subtypes = [
      checked_value,
      { self.read_sub_type_cavlc(reader)? },
      { self.read_sub_type_cavlc(reader)? },
      { self.read_sub_type_cavlc(reader)? },
    ];
    self.cur.sub = subtypes;
    self.decode_sub_refs_cavlc(reader, info, subtypes)?;

    for (list, mask) in PREDICTION_LISTS {
      'inner_for_2: for (subtype, block) in subtypes.into_iter().zip(0u8..4) {
        match ::core::primitive::bool::then_some(subtype.direct || subtype.pred & mask == 0, ()) {
          ::core::option::Option::Some(()) => {
            continue 'inner_for_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(length) = subtype.shape.block_count();

        for child in 0..length {
          self.store_mvd_cavlc(reader, list, block.wrapping_mul(4).wrapping_add(child))?;
        }
      }
    }

    Ok(())
  }

  fn decode_partitions_cavlc(&mut self, reader: &mut BitReader, info: MbTypeInfo) -> Result<()> {
    let Ok(length) = info.part.block_count();

    for ((list, mask), count) in PREDICTION_LISTS.into_iter().zip(self.sh.num_ref_idx_active) {
      for (prediction, block) in info.pred.into_iter().zip(0..length) {
        match ::core::primitive::bool::then_some(prediction & mask != 0, ()) {
          ::core::option::Option::Some(()) => {
            self.store_ref_cavlc(reader, list, block, count)?;
          }
          ::core::option::Option::None => {}
        }
      }
    }

    for (list, mask) in PREDICTION_LISTS {
      for (prediction, block) in info.pred.into_iter().zip(0..length) {
        match ::core::primitive::bool::then_some(prediction & mask != 0, ()) {
          ::core::option::Option::Some(()) => {
            self.store_mvd_cavlc(reader, list, block.wrapping_mul(4))?;
          }
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(())
  }

  fn macroblock_prediction_cavlc(
    &mut self,
    reader: &mut BitReader,
    info: MbTypeInfo,
  ) -> Result<()> {
    match info.kind {
      MbKind::I4x4 | MbKind::I8x8 | MbKind::I16x16 => {
        match info.kind {
          MbKind::I4x4 => self.decode_intra_modes_cavlc(reader, TransformSize::FourByFour)?,
          MbKind::I8x8 => self.decode_intra_modes_cavlc(reader, TransformSize::EightByEight)?,
          MbKind::None
          | MbKind::I16x16
          | MbKind::IPcm
          | MbKind::PSkip
          | MbKind::BSkip
          | MbKind::BDirect16x16
          | MbKind::Inter => {}
        }

        let chroma_mode = reader.read_ue()?;
        ensure(chroma_mode <= 3, || "intra_chroma_pred_mode out of range")?;
        let Ok(mode) = wrapped(chroma_mode);
        self.active.intra_chroma_mode = mode;
      }
      MbKind::BDirect16x16 => {}
      MbKind::None | MbKind::IPcm | MbKind::PSkip | MbKind::BSkip | MbKind::Inter => match info
        .part
      {
        Part::P8x8 => self.decode_subparts_cavlc(reader, info)?,
        Part::P16x16 | Part::P16x8 | Part::P8x16 => self.decode_partitions_cavlc(reader, info)?,
      },
    }

    Ok(())
  }

  fn coded_pattern_cavlc(&self, reader: &mut BitReader, info: MbTypeInfo) -> Result<u8> {
    match ::core::primitive::bool::then_some(info.kind == MbKind::I16x16, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(info.i16_cbp);
      }
      ::core::option::Option::None => {}
    }

    let code = reader.read_ue()?;
    let Ok(position) = index(code);
    let &(intra, inter) = match CBP_ME.get(position) {
      Some(pair) => pair,
      None => return invalid("coded_block_pattern out of range"),
    };
    Ok(match MacroblockPrediction::from(info.kind) {
      MacroblockPrediction::Intra => intra,
      MacroblockPrediction::Inter => inter,
    })
  }

  fn finish_cavlc_mb(&mut self, reader: &mut BitReader, info: MbTypeInfo, cbp: u8) -> Result<()> {
    self.cur.cbp = cbp;
    self.active.cbp = cbp;
    let prediction = MacroblockPrediction::from(info.kind);

    match prediction {
      MacroblockPrediction::Intra => {}
      MacroblockPrediction::Inter => {
        let Ok(eligibility) = self.transform_eligibility();

        match ::core::primitive::bool::then_some(
          cbp & 15 != 0
            && self.pps.transform_8x8_mode
            && eligibility == TransformEligibility::EightByEight,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            let size = reader.read_flag()?;
            self.active.transform_size = TransformSize::from(matches!(size, FlagBit::One));
          }
          ::core::option::Option::None => {}
        }

        self.derive_inter_motion()?;
      }
    }

    match ::core::primitive::bool::then_some(cbp != 0 || info.kind == MbKind::I16x16, ()) {
      ::core::option::Option::Some(()) => {
        let delta = reader.read_se()?;
        self.apply_qp_delta(delta)?;
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.store_qp();
    self.residual_cavlc(reader, cbp)?;

    match prediction {
      MacroblockPrediction::Intra => {
        let Ok(()) = self.reconstruct_intra();
      }
      MacroblockPrediction::Inter => {
        let Ok(()) = self.predict_inter();
        let Ok(()) = self.reconstruct_inter_residual();
      }
    }

    Ok(())
  }

  pub fn decode_mb_cavlc(&mut self, reader: &mut BitReader) -> Result<()> {
    let mut info = self.read_type_cavlc(reader)?;
    self.cur = MbCur {
      info,
      ..Default::default()
    };

    match ::core::primitive::bool::then_some(info.kind == MbKind::IPcm, ()) {
      ::core::option::Option::Some(()) => {
        return self.decode_pcm_cavlc(reader);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      info.kind == MbKind::I4x4 && self.pps.transform_8x8_mode,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let transform_size = reader.read_flag()?;

        match transform_size {
          FlagBit::One => {
            info.kind = MbKind::I8x8;
            self.cur.info.kind = MbKind::I8x8;
            self.active.transform_size = TransformSize::EightByEight;
          }
          FlagBit::Zero => {}
        }
      }
      ::core::option::Option::None => {}
    }

    self.active.kind = info.kind;
    self.macroblock_prediction_cavlc(reader, info)?;
    let cbp = self.coded_pattern_cavlc(reader, info)?;
    self.finish_cavlc_mb(reader, info, cbp)
  }

  pub fn transform_eligibility(&self) -> core::result::Result<TransformEligibility, Never> {
    let info = self.cur.info;
    let spatial = matches!(self.sps.direct_8x8_inference, DirectInference::Block8x8);
    Ok(TransformEligibility::from(match (info.kind, info.part) {
      (MbKind::BDirect16x16, _) => spatial,
      (MbKind::Inter, Part::P8x8) => {
        self
          .cur
          .sub
          .iter()
          .all(|s| match ::core::primitive::bool::then_some(s.direct, ()) {
            ::core::option::Option::Some(()) => spatial,
            ::core::option::Option::None => s.shape == SubPart::S8x8,
          })
      }
      _ => true,
    }))
  }

  fn residual_luma_subblock_cavlc(
    &mut self,
    reader: &mut BitReader,
    raster: u8,
    range: CoefficientRange,
    levels: &mut [i32; 16],
  ) -> Result<()> {
    let Ok(context) = self.nc_luma(raster);
    let coefficients: &mut [i32] = match range {
      CoefficientRange::All => levels,
      CoefficientRange::AcOnly => {
        let [_, tail @ ..] = levels;
        tail
      }
    };
    let count = cavlc::residual_block(reader, coefficients, context)?;
    let Ok(position) = index(raster);
    let output = self.active.nnz.get_mut(position).ok_or_else(|| {
      Error::Invalid("coefficient position outside the luma macroblock".to_owned())
    })?;
    *output = count;
    Ok(())
  }

  fn residual_luma_8x8_cavlc(
    &mut self,
    reader: &mut BitReader,
    base: Point<u8>,
    quadrant: u8,
  ) -> Result<()> {
    let mut levels = [0; 64];

    for child in 0u8..4 {
      let Ok(block) = subblock_raster(base, child);
      let mut coefficients = [0; 16];
      self.residual_luma_subblock_cavlc(
        reader,
        block,
        CoefficientRange::All,
        &mut coefficients,
      )?;
      let Ok(offset) = index(child);

      for (coefficient, destination) in coefficients
        .into_iter()
        .zip(levels.iter_mut().skip(offset).step_by(4))
      {
        *destination = coefficient;
      }
    }

    let Ok(()) = self.put_luma8(quadrant, &levels);
    Ok(())
  }

  fn residual_luma_4x4_cavlc(
    &mut self,
    reader: &mut BitReader,
    base: Point<u8>,
    range: CoefficientRange,
  ) -> Result<()> {
    for child in 0u8..4 {
      let Ok(block) = subblock_raster(base, child);
      let mut levels = [0; 16];
      self.residual_luma_subblock_cavlc(reader, block, range, &mut levels)?;
      let Ok(()) = self.put_luma4(block, &levels, range);
    }

    Ok(())
  }

  fn residual_chroma_cavlc(&mut self, reader: &mut BitReader, chroma: u8) -> Result<()> {
    match ::core::primitive::bool::then_some(chroma & 3 != 0, ()) {
      ::core::option::Option::Some(()) => {
        for plane in [ChromaPlane::Cb, ChromaPlane::Cr] {
          let mut levels = [0; 4];
          cavlc::residual_block(reader, &mut levels, -1)?;
          let Ok(()) = self.put_chroma_dc(plane, &levels);
        }
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(chroma & 2 != 0, ()) {
      ::core::option::Option::Some(()) => {
        for plane in [ChromaPlane::Cb, ChromaPlane::Cr] {
          self.residual_chroma_ac_cavlc(reader, plane)?;
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  fn residual_chroma_ac_cavlc(
    &mut self,
    reader: &mut BitReader,
    plane: ChromaPlane,
  ) -> Result<()> {
    for block in 0u8..4 {
      let Ok(context) = self.nc_chroma(plane, block);
      let mut levels = [0; 16];
      let [_, ac @ ..] = &mut levels;
      let count = cavlc::residual_block(reader, ac, context)?;
      let [cb, cr] = &mut self.active.nnz_c;
      let pattern = match plane {
        ChromaPlane::Cb => cb,
        ChromaPlane::Cr => cr,
      };
      let Ok(position) = index(block);
      let output = pattern.get_mut(position).ok_or_else(|| {
        Error::Invalid("coefficient position outside the chroma macroblock".to_owned())
      })?;
      *output = count;
      let Ok(()) = self.put_chroma_ac(plane, block, &levels);
    }

    Ok(())
  }

  fn residual_cavlc(&mut self, reader: &mut BitReader, cbp: u8) -> Result<()> {
    let range = CoefficientRange::from(self.active.kind == MbKind::I16x16);

    match range {
      CoefficientRange::AcOnly => {
        let Ok(context) = self.nc_luma(0);
        let mut levels = [0; 16];
        cavlc::residual_block(reader, &mut levels, context)?;
        let Ok(()) = self.put_luma_dc(&levels);
      }
      CoefficientRange::All => {}
    }

    for block in 0u8..4 {
      match ::core::primitive::bool::then_some(cbp & 1u8.wrapping_shl(u32::from(block)) == 0, ())
      {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(origin) = block8_origin(block);

      match ::core::primitive::bool::then_some(
        self.active.transform_size == TransformSize::EightByEight,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          self.residual_luma_8x8_cavlc(reader, origin, block)?
        }
        ::core::option::Option::None => self.residual_luma_4x4_cavlc(reader, origin, range)?,
      }
    }

    self.residual_chroma_cavlc(reader, cbp.wrapping_shr(4))
  }

  fn read_skip_run(
    &mut self,
    reader: &mut BitReader,
    remaining: core::ops::Range<u64>,
  ) -> Result<u64> {
    let run = reader.read_ue()?;
    let end = remaining.start.wrapping_add(u64::from(run));
    ensure(end <= remaining.end, || "mb_skip_run out of range")?;

    for skipped in remaining.start..end {
      let Ok(block) = wrapped::<_, u32>(skipped);
      self.start_mb(block)?;
      let decoded = self.decode_skip();
      self.finish_mb(decoded)?;
    }

    Ok(end)
  }

  pub fn decode_cavlc(&mut self, reader: &mut BitReader) -> Result<()> {
    let total = u64::from(self.pic.mb_w).wrapping_mul(u64::from(self.pic.mb_h));
    let first = u64::from(self.sh.first_mb_in_slice);
    let mut address = first;

    for _ in first..total {
      match self.sh.slice_type {
        SliceType::I | SliceType::Si => {}
        SliceType::P | SliceType::Sp | SliceType::B => {
          let next = self.read_skip_run(reader, address..total)?;

          match ::core::primitive::bool::then_some(
            next != address && matches!(reader.more_rbsp_data(), Ok(Remaining::TrailingBits)),
            (),
          ) {
            ::core::option::Option::Some(()) => {
              return Ok(());
            }
            ::core::option::Option::None => {}
          }

          address = next;
          ensure(address < total, || {
            format!("macroblock address {address} out of range")
          })?;
        }
      }

      let Ok(block) = wrapped::<_, u32>(address);
      self.start_mb(block)?;
      let decoded = self.decode_mb_cavlc(reader);
      self.finish_mb(decoded)?;
      address = address.wrapping_add(1);

      let Ok(remaining) = reader.more_rbsp_data();

      match remaining {
        Remaining::TrailingBits => return Ok(()),
        Remaining::Payload => {}
      }

      ensure(address < total, || {
        format!("macroblock address {address} out of range")
      })?;
    }

    invalid(format!("macroblock address {address} out of range"))
  }
}
