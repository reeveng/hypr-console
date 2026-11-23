//! Top-level decoder: superframes, frame header handling, tile scheduling (tile columns in
//! parallel), probability adaptation, reference frame management, and the per-frame "post" stage
//! (band assembly, loop filter, publication, output picture) that runs behind the next frame
//! (frame threading).

use crate::error::{Error, Invalid, MissingReference, Result};
use crate::frame::{
  BAND_PAD, Band, Frame, FrameInfo, FrameKind, MiGrid, MiInfo, MiPlace, Pools, SampleRange,
  SharedPools, filled,
};
use crate::header::{
  Adaptation, ContextRefresh, FeatureValues, FrameHeader, HeaderState, MapUpdate, Parsed,
  Resilience, SegmentMapReset, Segmentation, SegmentationStatus, Visibility, parse_compressed,
  parse_uncompressed, split_superframe,
};
use crate::loopfilter::{LfFrame, PlaneView, filter_superblock};
use crate::places::{read, span, span_mut};
use crate::probs::{Counts, FrameContext, adapt_coef_probs, adapt_noncoef_probs};
use crate::tables::*;
use crate::tile::{FrameShared, ProbabilityCounts, RefUse, TileBand, TileDecoder};
use crate::{ColorInfo, DecodeQuality, Picture, Plane};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_size::Size;
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecodeStats {
  pub frames: u64,
  pub shown: u64,
  pub hidden: u64,
  pub key_frames: u64,
  pub intra_only: u64,
  pub inter_frames: u64,
  pub show_existing: u64,
  pub superframes: u64,
  pub error_resilient: u64,
  pub no_backward_adaptation: u64,
  pub size_changes: u64,
  pub scaled_ref_blocks: u64,
  pub compound_blocks: u64,
  pub intra_blocks: u64,
  pub inter_blocks: u64,
  pub lossless_frames: u64,
  pub max_tile_cols: u32,
  pub max_tile_rows: u32,
  pub segmentation_frames: u64,
  pub tx_select_frames: u64,
  pub switchable_interp_frames: u64,
  pub high_precision_mv_frames: u64,
  pub compound_frames: u64,
  pub bit_depths: [u64; 3],
  pub profiles: [u64; 4],

  pub draft_frames: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drain {
  Ready,
  All,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "ordered post jobs publish each reference band once; decoder slots and pending displays need to retain those immutable bands past the job that produced them"
  )
)]
type FrameRef = Arc<Frame>;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the post job reads this frame's completed mode grid for loop filtering while the decoder can read the same grid for the next frame's motion prediction"
  )
)]
type GridRef = Arc<MiGrid>;

enum Pending {
  Ready(Picture),

  #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
  Task(std::sync::mpsc::Receiver<Picture>),

  Existing(FrameRef, i64, DecodeQuality),
}

pub struct Decoder {
  st: HeaderState,
  slots: [Option<FrameRef>; 8],
  prev_mi: Option<GridRef>,
  prev_seg_ids: Vec<u8>,
  last_size: Option<Size<u32>>,
  last_visibility: crate::header::Visibility,
  stats: DecodeStats,
  threads: u64,
  bufs: SharedPools,
  draft: DecodeQuality,
  pending: VecDeque<Pending>,
  #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
  jobs: VecDeque<std::thread::JoinHandle<std::result::Result<(), Never>>>,
}

impl Default for Decoder {
  fn default() -> Self {
    let Ok(decoder) = Self::new();

    decoder
  }
}

#[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
fn default_threads() -> std::result::Result<u64, Never> {
  match std::thread::available_parallelism() {
    Ok(available) => fixed_index(available.get().min(16)),
    Err(_executor_could_not_measure_parallelism) => Ok(1),
  }
}

#[cfg(not(all(feature = "threads", not(target_arch = "wasm32"))))]
fn default_threads() -> std::result::Result<u64, Never> {
  Ok(1)
}

struct DecodedStrips {
  strips: Vec<TileBand>,
  counts: Counts,
}

enum PreviousGrid {
  Use,
  Reset,
}

impl From<bool> for PreviousGrid {
  fn from(compatible: bool) -> Self {
    match compatible.then_some(()) {
      Some(()) => Self::Use,
      None => Self::Reset,
    }
  }
}

impl PreviousGrid {
  fn grid(self, previous: &Option<GridRef>) -> std::result::Result<Option<&MiGrid>, Never> {
    Ok(match self {
      Self::Use => previous.as_deref(),
      Self::Reset => None,
    })
  }
}

fn valid_size(size: Size<u32>) -> Result<()> {
  match (size.width <= MAX_FRAME_SIDE && size.height <= MAX_FRAME_SIDE).then_some(()) {
    Some(()) => Ok(()),
    None => Err(<crate::error::Error as ::core::convert::From<
      crate::error::Invalid,
    >>::from(Invalid::TooLarge {
      width: size.width,
      height: size.height,
    })),
  }
}

struct TileAxis {
  extent: u64,
  split: u32,
}

impl TileAxis {
  fn spans(self) -> std::result::Result<impl Iterator<Item = Range<u64>> + Clone, Never> {
    let Self { extent, split } = self;
    let count = 1u64.wrapping_shl(split);
    let units = extent.div_ceil(8);
    let edge = move |at: u64| {
      at.wrapping_mul(units)
        .wrapping_shr(split)
        .wrapping_shl(3)
        .min(extent)
    };
    Ok((0..count).map(move |index| edge(index)..edge(index.wrapping_add(1))))
  }
}

struct TileColumn<'a> {
  columns: Range<u64>,
  tiles: Vec<&'a [u8]>,
}

struct TilePayload<'a> {
  bytes: &'a [u8],
  offset: u64,
}

enum TileLength {
  Prefix,
  Remaining,
}

impl<'a> TilePayload<'a> {
  fn tile(&mut self, length: TileLength) -> Result<&'a [u8]> {
    let (prefix, rest) = match length {
      TileLength::Remaining => return Ok(std::mem::take(&mut self.bytes)),
      TileLength::Prefix => self
        .bytes
        .split_first_chunk::<4>()
        .ok_or(Invalid::TileBeyondData { size: self.offset })?,
    };
    let size = u32::from_be_bytes(*prefix);
    let Ok(length_2) = index(size);
    let (tile, tail) = rest
      .split_at_checked(length_2)
      .ok_or(Invalid::TileBeyondData {
        size: u64::from(size),
      })?;
    self.bytes = tail;
    self.offset = self.offset.wrapping_add(4).wrapping_add(u64::from(size));
    Ok(tile)
  }
}

fn compressed_header<'a>(
  data: &'a [u8],
  header: &FrameHeader,
) -> Result<(&'a [u8], TilePayload<'a>)> {
  let Ok(start) = index(header.uncompressed_size);
  let Ok(length) = index(header.header_size_in_bytes);
  let tail = data
    .get(start..)
    .ok_or(Invalid::CompressedHeaderBeyondData)?;
  let (compressed, bytes) = tail
    .split_at_checked(length)
    .ok_or(Invalid::CompressedHeaderBeyondData)?;
  let offset = header
    .uncompressed_size
    .wrapping_add(u64::from(header.header_size_in_bytes));
  Ok((compressed, TilePayload { bytes, offset }))
}

fn split_tiles<'a>(
  mut payload: TilePayload<'a>,
  header: &FrameHeader,
) -> Result<Vec<TileColumn<'a>>> {
  let columns = TileAxis {
    extent: u64::from(header.mi_cols),
    split: header.tile_cols_log2,
  };
  let Ok(spans) = columns.spans();
  let mut strips: Vec<_> = spans
    .map(|columns_2| TileColumn {
      columns: columns_2,
      tiles: Vec::new(),
    })
    .collect();
  let rows = 1u64.wrapping_shl(header.tile_rows_log2);
  let last_row = rows.wrapping_sub(1);
  let Ok(last_column) = fixed_index(strips.len().saturating_sub(1));

  for row in 0..rows {
    for (column, strip) in (0u64..).zip(&mut strips) {
      let length = match (row == last_row && column == last_column).then_some(()) {
        Some(()) => TileLength::Remaining,
        None => TileLength::Prefix,
      };
      strip.tiles.push({
        let checked_value = payload.tile(length)?;
        checked_value
      });
    }
  }

  Ok(strips)
}

fn segment_quantizers(
  header: &FrameHeader,
  segmentation: &Segmentation,
) -> std::result::Result<[[[i32; 2]; 2]; 8], Never> {
  let depth = u64::from(header.color.bit_depth.wrapping_sub(8).wrapping_shr(1));
  let table = depth.wrapping_mul(256);
  let base = i32::from(header.base_q_idx);
  let address = |value: i32| -> std::result::Result<u64, Never> {
    let Ok(index) = wrapped::<_, u64>(value.clamp(0, 255));
    Ok(table.wrapping_add(index))
  };
  let dc = |value| {
    let Ok(at) = address(value);
    read(&DC_QLOOKUP, at)
  };
  let ac = |value| {
    let Ok(at) = address(value);
    read(&AC_QLOOKUP, at)
  };
  let mut output = [[[0; 2]; 2]; 8];

  for (segment, quantizers) in (0u8..).zip(&mut output) {
    let Ok(feature) = segmentation.feature(segment, SEG_LVL_ALT_Q);
    let q = match (feature, segmentation.values) {
      (Some(value), FeatureValues::Absolute) => i32::from(value).clamp(0, 255),
      (Some(value), FeatureValues::Delta) => base.wrapping_add(i32::from(value)).clamp(0, 255),
      (None, _) => base,
    };
    let Ok(y_dc) = dc(q.wrapping_add(i32::from(header.delta_q_y_dc)));
    let Ok(y_ac) = ac(q);
    let Ok(uv_dc) = dc(q.wrapping_add(i32::from(header.delta_q_uv_dc)));
    let Ok(uv_ac) = ac(q.wrapping_add(i32::from(header.delta_q_uv_ac)));
    *quantizers = [[y_dc, y_ac], [uv_dc, uv_ac]];
  }

  Ok(output)
}

impl Decoder {
  pub fn new() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(threads) = default_threads();
    Self::for_threads(threads)
  }

  pub fn with_threads(
    threads: ::console_core_api_compatibility::NativeSize,
  ) -> std::result::Result<Self, Never> {
    let Ok(count) = fixed_index(threads);
    Self::for_threads(count)
  }

  fn for_threads(threads: u64) -> std::result::Result<Self, Never> {
    Ok(Decoder {
      st: HeaderState::default(),
      slots: Default::default(),
      prev_mi: None,
      prev_seg_ids: Vec::new(),
      last_size: None,
      last_visibility: crate::header::Visibility::Hidden,
      stats: DecodeStats::default(),
      threads: threads.max(1),
      bufs: SharedPools::default(),
      draft: DecodeQuality::Full,
      pending: VecDeque::new(),
      #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
      jobs: VecDeque::new(),
    })
  }

  pub fn stats(&self) -> core::result::Result<DecodeStats, console_core_never::Never> {
    Ok(self.stats.clone())
  }

  pub fn threads(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    return index(self.threads);

    #[cfg(not(all(feature = "threads", not(target_arch = "wasm32"))))]
    Ok(1)
  }

  pub fn set_quality(
    &mut self,
    quality: DecodeQuality,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.draft = quality;

    Ok(())
  }

  pub fn decode(&mut self, data: &[u8], pts: i64) -> Result<Vec<Picture>> {
    let Ok(frames) = split_superframe(data);

    match ::core::primitive::bool::then_some(frames.len() > 1, ()) {
      ::core::option::Option::Some(()) => {
        {
          let numeric_amount = 1;
          let numeric_target = &mut (self.stats.superframes);
          *numeric_target = u64::wrapping_add(*numeric_target, numeric_amount);
        };
      }
      ::core::option::Option::None => {}
    }

    for f in frames {
      match ::core::primitive::bool::then_some(f.is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      self.decode_frame(f, pts)?;
    }

    let Ok(drain) = self.drain(Drain::Ready);

    Ok(drain)
  }

  pub fn flush(&mut self) -> std::result::Result<Vec<Picture>, Never> {
    let Ok(output) = self.drain(Drain::All);
    let Ok(()) = self.join_jobs();
    Ok(output)
  }

  pub fn reset(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      self.st = HeaderState::default();
      self.slots = Default::default();
      self.prev_mi = None;
      self.prev_seg_ids = Vec::new();
      self.last_size = None;
      self.last_visibility = crate::header::Visibility::Hidden;
      self.pending.clear();
    };

    Ok(())
  }

  fn drain(&mut self, requested: Drain) -> std::result::Result<Vec<Picture>, Never> {
    let max_lag = self.threads.wrapping_div(2).clamp(2, 6);
    let mut output = Vec::new();
    let Ok(initial) = fixed_index(self.pending.len());

    for _ in 0..initial {
      let Ok(pending) = fixed_index(self.pending.len());
      let mode = match (requested == Drain::All || pending > max_lag).then_some(()) {
        Some(()) => Drain::All,
        None => Drain::Ready,
      };

      let front = {
        let optional_front = self.pending.pop_front();

        match optional_front {
          Some(matched_front) => matched_front,
          ::core::option::Option::None => break,
        }
      };

      match front {
        Pending::Ready(picture) => output.push(picture),
        #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
        Pending::Task(rx) => {
          let Ok(delivery) = receive_picture(&rx, mode);

          match delivery {
            PictureDelivery::Picture(picture) => output.push(picture),
            PictureDelivery::Waiting => {
              self.pending.push_front(Pending::Task(rx));
              break;
            }
            PictureDelivery::Ended => {}
          }
        }
        Pending::Existing(frame, pts, quality) => {
          let Ok(completeness) = frame.completeness();

          match (mode, completeness) {
            (Drain::Ready, crate::frame::Completeness::Incomplete) => {
              self
                .pending
                .push_front(Pending::Existing(frame, pts, quality));
              break;
            }
            _ => {
              let Ok(picture) = picture_of(&frame, pts, quality);
              output.push(picture);
            }
          }
        }
      }
    }

    Ok(output)
  }

  fn decode_frame(&mut self, data: &[u8], pts: i64) -> Result<()> {
    let refs = self
      .slots
      .each_ref()
      .map(|slot| slot.as_ref().map(|frame| frame.info.size));
    let parsed = parse_uncompressed(data, &mut self.st, &refs)?;
    let mut header = match parsed {
      Parsed::Existing { slot } => return self.show_existing(slot, pts),
      Parsed::Frame(header) => *header,
    };
    valid_size(header.size)?;
    let Ok(previous) = self.prepare_frame(&header);
    let (compressed, payload) = compressed_header(data, &header)?;
    let Ok(context_slot) = index(header.frame_context_idx);
    let mut context = match self.st.contexts.get(context_slot) {
      Some(found) => found.clone(),
      None => FrameContext::default(),
    };
    let Ok(()) = parse_compressed(compressed, &mut header, &mut context);
    let Ok(()) = self.count_frame(&header);
    let counting = match (header.resilience, header.adaptation) {
      (Resilience::Resilient, _) | (_, Adaptation::Parallel) => ProbabilityCounts::Ignore,
      (Resilience::Dependent, Adaptation::Backward) => ProbabilityCounts::Collect,
    };
    let Ok(segment_quantizers) = segment_quantizers(&header, &self.st.seg);
    let refs_2 = self.frame_references(&header)?;
    let Ok(prev_mi) = previous.grid(&self.prev_mi);
    let shared = FrameShared {
      h: &header,
      fc: &context,
      seg: &self.st.seg,
      prev_seg_ids: &self.prev_seg_ids,
      prev_mi,
      refs: refs_2,
      seg_q: segment_quantizers,
      counting,
      pools: &self.bufs,
    };
    let columns = split_tiles(payload, &header)?;
    let Ok(decoded_columns) = self.decode_columns(&columns, &shared);
    let DecodedStrips { mut strips, counts } = self.collect_strips(decoded_columns, counting)?;
    let dimensions = MiPlace {
      row: u64::from(header.mi_rows),
      col: u64::from(header.mi_cols),
    };
    let Ok((mi, segments)) = assemble_mi(&mut strips, dimensions, &self.bufs);
    let grid = GridRef::new(mi);
    let Ok(frame) = self.start_post(&header, &grid, strips, pts);
    let Ok(()) = self.save_context(&header, context, &counts, counting);
    let Ok(()) = self.store_references(&header, frame, grid, segments);
    Ok(())
  }

  fn show_existing(&mut self, slot: u8, pts: i64) -> Result<()> {
    self.stats.show_existing = self.stats.show_existing.wrapping_add(1);
    let Ok(at) = index(slot);
    let frame = self
      .slots
      .get(at)
      .and_then(Option::as_ref)
      .ok_or(MissingReference::ShownEmptySlot { slot })?;
    self.stats.shown = self.stats.shown.wrapping_add(1);
    self.pending.push_back(Pending::Existing(
      FrameRef::clone(frame),
      pts,
      DecodeQuality::Full,
    ));
    Ok(())
  }

  fn prepare_frame(&mut self, header: &FrameHeader) -> std::result::Result<PreviousGrid, Never> {
    let size_changed = self.last_size != Some(header.size);
    let dimensions = MiPlace {
      row: u64::from(header.mi_rows),
      col: u64::from(header.mi_cols),
    };
    let area = dimensions.row.wrapping_mul(dimensions.col);
    let Ok(segment_count) = index(area.min(MAX_MI_COUNT));

    match (size_changed && self.last_size.is_some()).then_some(()) {
      Some(()) => self.stats.size_changes = self.stats.size_changes.wrapping_add(1),
      None => {}
    }

    match (self.st.segment_map_reset == SegmentMapReset::Reset
      || size_changed
      || self.prev_seg_ids.len() != segment_count)
      .then_some(())
    {
      Some(()) => {
        let Ok(segments) = filled::<_, 4194304>(area, 0);
        self.prev_seg_ids = segments;
        self.st.segment_map_reset = SegmentMapReset::Preserve;
      }
      None => {}
    }

    let previous = PreviousGrid::from(
      !size_changed
        && self.last_visibility == Visibility::Shown
        && header.resilience == Resilience::Dependent
        && header.kind == FrameKind::Inter
        && self
          .prev_mi
          .as_ref()
          .is_some_and(|grid| grid.rows == dimensions.row && grid.cols == dimensions.col),
    );
    self.last_size = Some(header.size);
    self.last_visibility = header.visibility;
    Ok(previous)
  }

  fn frame_references<'a>(&'a self, header: &FrameHeader) -> Result<[Option<RefUse<'a>>; 3]> {
    match header.kind {
      FrameKind::Key | FrameKind::IntraOnly => Ok([None, None, None]),
      FrameKind::Inter => {
        let [a, b, c] = header.ref_frame_idx;
        let first = self.frame_reference(header, a)?;
        let second = self.frame_reference(header, b)?;
        let third = self.frame_reference(header, c)?;
        Ok([first, second, third])
      }
    }
  }

  fn frame_reference<'a>(&'a self, header: &FrameHeader, slot: u8) -> Result<Option<RefUse<'a>>> {
    let Ok(at) = index(slot);
    let frame = self
      .slots
      .get(at)
      .and_then(Option::as_deref)
      .ok_or(MissingReference::EmptySlot { slot })?;
    let reference = frame.info;

    match (reference.bit_depth == header.color.bit_depth
      && reference.subsampling == header.color.subsampling)
      .then_some(())
    {
      Some(()) => {}
      None => {
        return Err(<crate::error::Error as ::core::convert::From<
          crate::error::Invalid,
        >>::from(Invalid::ReferenceFormat));
      }
    }

    let w = u64::from(header.size.width);
    let h = u64::from(header.size.height);
    let rw = u64::from(reference.size.width);
    let rh = u64::from(reference.size.height);

    match (w.wrapping_mul(2) >= rw
      && h.wrapping_mul(2) >= rh
      && w <= rw.wrapping_mul(16)
      && h <= rh.wrapping_mul(16))
    .then_some(())
    {
      Some(()) => {
        let Ok(x_scale) = wrapped::<_, i32>(rw.wrapping_shl(14).wrapping_div(w.max(1)));
        let Ok(y_scale) = wrapped::<_, i32>(rh.wrapping_shl(14).wrapping_div(h.max(1)));
        Ok(Some(RefUse {
          frame,
          x_scale,
          y_scale,
          x_step: x_scale.wrapping_mul(16).wrapping_shr(14),
          y_step: y_scale.wrapping_mul(16).wrapping_shr(14),
        }))
      }
      None => Ok(None),
    }
  }

  fn decode_columns(
    &self,
    columns: &[TileColumn<'_>],
    shared: &FrameShared<'_>,
  ) -> std::result::Result<Vec<(TileBand, Option<Error>)>, Never> {
    let h = shared.h;
    let axis = TileAxis {
      extent: u64::from(h.mi_rows),
      split: h.tile_rows_log2,
    };
    let Ok(row_bounds) = axis.spans();
    let decode = |column: &TileColumn<'_>| {
      let Ok(mut tile) = TileDecoder::new(shared, column.columns.clone());

      match column.columns.is_empty().then_some(()) {
        Some(()) => {}
        None => {
          for (data, rows) in column.tiles.iter().zip(row_bounds.clone()) {
            let Ok(()) = tile.decode_tile(data, rows);
          }
        }
      }

      (tile.strip, tile.error)
    };

    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    match (self.threads > 1 && columns.len() > 1).then_some(()) {
      Some(()) => {
        let Ok(width) = console_core_number_conversion::fitted::<_, u32>(self.threads);
        return Ok(
          match console_concurrency::map_with_threads(width, columns, decode) {
            Ok(decoded) => decoded,
            Err(panic) => std::panic::resume_unwind(panic),
          },
        );
      }
      None => {}
    }

    Ok(columns.iter().map(decode).collect())
  }

  fn collect_strips(
    &mut self,
    decoded: Vec<(TileBand, Option<Error>)>,
    mode: ProbabilityCounts,
  ) -> Result<DecodedStrips> {
    let mut counts = Counts::default();
    let mut strips = Vec::with_capacity(decoded.len());
    let mut error = None;

    for (strip, failed) in decoded {
      match (error.as_ref(), failed) {
        (None, Some(fault)) => error = Some(fault),
        _ => {}
      }

      match mode {
        ProbabilityCounts::Collect => {
          let Ok(()) = counts.add(&strip.counts);
        }
        ProbabilityCounts::Ignore => {}
      }

      self.stats.compound_blocks = self
        .stats
        .compound_blocks
        .wrapping_add(strip.compound_blocks);
      self.stats.scaled_ref_blocks = self
        .stats
        .scaled_ref_blocks
        .wrapping_add(strip.scaled_blocks);
      self.stats.intra_blocks = self.stats.intra_blocks.wrapping_add(strip.intra_blocks);
      self.stats.inter_blocks = self.stats.inter_blocks.wrapping_add(strip.inter_blocks);
      strips.push(strip);
    }

    match error {
      Some(fault) => {
        let Ok(()) = recycle_strips(strips, &self.bufs);
        Err(fault)
      }
      None => Ok(DecodedStrips { strips, counts }),
    }
  }

  fn start_post(
    &mut self,
    header: &FrameHeader,
    grid: &GridRef,
    strips: Vec<TileBand>,
    pts: i64,
  ) -> std::result::Result<FrameRef, Never> {
    let info = FrameInfo {
      size: header.size,
      subsampling: header.color.subsampling,
      bit_depth: header.color.bit_depth,
      color_space: header.color.color_space,
      color_range: header.color.color_range,
      render: header.render,
      kind: header.kind,
    };
    let Ok(buffer) = Frame::new(info, SharedPools::clone(&self.bufs));
    let frame = FrameRef::new(buffer);
    let quality = match (
      self.draft,
      header.refresh_frame_flags,
      header.visibility,
      self.st.lf.level,
    ) {
      (DecodeQuality::Draft, 0, Visibility::Shown, 1..) => DecodeQuality::Draft,
      _ => DecodeQuality::Full,
    };
    self.stats.draft_frames = self
      .stats
      .draft_frames
      .wrapping_add(u64::from(quality == DecodeQuality::Draft));
    let lf = match (self.st.lf.level, quality) {
      (1.., DecodeQuality::Full) => {
        let Ok(filter) = LfFrame::new(
          &self.st.lf,
          &self.st.seg,
          MiPlace {
            row: grid.rows,
            col: grid.cols,
          },
          header.color.subsampling,
          header.color.bit_depth,
        );
        Some(filter)
      }
      _ => None,
    };
    let out = match header.visibility {
      Visibility::Shown => Some((pts, quality)),
      Visibility::Hidden => None,
    };
    let job = PostJob {
      frame: FrameRef::clone(&frame),
      strips,
      mi: GridRef::clone(grid),
      lf,
      out,
      pools: SharedPools::clone(&self.bufs),
    };
    let Ok(()) = self.run_post(job);
    Ok(frame)
  }

  fn save_context(
    &mut self,
    header: &FrameHeader,
    updated: FrameContext,
    counts: &Counts,
    mode: ProbabilityCounts,
  ) -> std::result::Result<(), Never> {
    let Ok(at) = index(header.frame_context_idx);
    let saved = match self.st.contexts.get_mut(at) {
      Some(saved) => saved,
      None => return Ok(()),
    };

    let context = match mode {
      ProbabilityCounts::Ignore => updated,
      ProbabilityCounts::Collect => {
        let mut adapted = saved.clone();
        adapted.tx = updated.tx;
        adapted.skip = updated.skip;
        let coefficient_update = match (header.kind, self.st.last_kind) {
          (FrameKind::Inter, FrameKind::Key) => 128,
          _ => 112,
        };
        let Ok(()) = adapt_coef_probs(&mut adapted, counts, coefficient_update);

        match header.kind {
          FrameKind::Inter => {
            adapted.tx = saved.tx;
            adapted.skip = saved.skip;
            let Ok(()) = adapt_noncoef_probs(&mut adapted, counts, header);
          }
          FrameKind::Key | FrameKind::IntraOnly => {}
        }

        adapted
      }
    };

    match header.refresh {
      ContextRefresh::Retain => {}
      ContextRefresh::Refresh => *saved = context,
    }

    Ok(())
  }

  fn store_references(
    &mut self,
    header: &FrameHeader,
    frame: FrameRef,
    grid: GridRef,
    segments: Vec<u8>,
  ) -> std::result::Result<(), Never> {
    match (self.st.seg.status, self.st.seg.map) {
      (SegmentationStatus::Enabled, MapUpdate::Explicit | MapUpdate::Temporal) => {
        self.prev_seg_ids = segments
      }
      _ => {}
    }

    for (bit, slot) in (0u32..).zip(&mut self.slots) {
      match header.refresh_frame_flags.wrapping_shr(bit) & 1 {
        0 => {}
        _ => *slot = Some(FrameRef::clone(&frame)),
      }
    }

    match self.prev_mi.replace(grid).and_then(GridRef::into_inner) {
      Some(old) => {
        let Ok(()) = self.bufs.mi.put(old.mi);
      }
      None => {}
    }

    let total = match header.visibility {
      Visibility::Shown => &mut self.stats.shown,
      Visibility::Hidden => &mut self.stats.hidden,
    };
    *total = total.wrapping_add(1);
    Ok(())
  }

  fn run_post(&mut self, job: PostJob) -> std::result::Result<(), Never> {
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    match (self.threads > 1).then_some(()) {
      Some(()) => return self.run_threaded_post(job),
      None => {}
    }

    self.run_serial_post(job)
  }

  fn run_serial_post(&mut self, job: PostJob) -> std::result::Result<(), Never> {
    let Ok(output) = job.run();

    match output {
      Some(picture) => self.pending.push_back(Pending::Ready(picture)),
      None => {}
    }

    Ok(())
  }

  #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
  fn run_threaded_post(&mut self, job: PostJob) -> std::result::Result<(), Never> {
    let Ok(()) = self.trim_jobs();
    let delivery = match job.out {
      Some(_) => OutputDelivery::Shown,
      None => OutputDelivery::Hidden,
    };
    let (send_picture, receive_picture) = std::sync::mpsc::sync_channel(1);
    let (send_job, receive_job) = std::sync::mpsc::sync_channel(1);
    let started = std::thread::Builder::new()
      .name(String::from("vp9-post"))
      .spawn(move || post_worker(receive_job, send_picture));

    let worker = match started {
      Ok(worker) => worker,
      Err(fault) => {
        console_core_logging::warn!(
          "VP9 post worker could not start, finishing on the caller thread: {fault}"
        );
        return self.run_serial_post(job);
      }
    };

    match send_job.send(job) {
      Ok(()) => {
        self.jobs.push_back(worker);

        match delivery {
          OutputDelivery::Shown => self.pending.push_back(Pending::Task(receive_picture)),
          OutputDelivery::Hidden => {}
        }

        Ok(())
      }
      Err(std::sync::mpsc::SendError(unsent)) => {
        let Ok(()) = join_post_worker(worker);
        self.run_serial_post(unsent)
      }
    }
  }

  #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
  fn trim_jobs(&mut self) -> std::result::Result<(), Never> {
    let max_jobs = self.threads.wrapping_div(3).clamp(2, 6);
    let Ok(initial_jobs) = fixed_index(self.jobs.len());

    for _ in 0..initial_jobs {
      let finished = self.jobs.front().is_some_and(|job| job.is_finished());
      let Ok(active) = fixed_index(self.jobs.len());

      match (finished || active >= max_jobs).then_some(()) {
        Some(()) => {
          let job = {
            let optional_job = self.jobs.pop_front();

            match optional_job {
              Some(matched_job) => matched_job,
              ::core::option::Option::None => break,
            }
          };

          let Ok(()) = join_post_worker(job);
        }
        None => break,
      }
    }

    Ok(())
  }

  fn join_jobs(&mut self) -> std::result::Result<(), Never> {
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    for job in self.jobs.drain(..) {
      let Ok(()) = join_post_worker(job);
    }

    Ok(())
  }

  fn count_frame(&mut self, header: &FrameHeader) -> std::result::Result<(), Never> {
    let stats = &mut self.stats;
    stats.frames = stats.frames.wrapping_add(1);
    let kind = match header.kind {
      FrameKind::Key => &mut stats.key_frames,
      FrameKind::IntraOnly => &mut stats.intra_only,
      FrameKind::Inter => &mut stats.inter_frames,
    };
    *kind = kind.wrapping_add(1);
    let resilient = header.resilience == Resilience::Resilient;
    let parallel = header.adaptation == Adaptation::Parallel;
    let Ok(quantization) = header.quantization();
    stats.error_resilient = stats.error_resilient.wrapping_add(u64::from(resilient));
    stats.no_backward_adaptation = stats
      .no_backward_adaptation
      .wrapping_add(u64::from(resilient || parallel));
    stats.lossless_frames = stats.lossless_frames.wrapping_add(u64::from(
      quantization == crate::transform::Quantization::Lossless,
    ));
    stats.max_tile_cols = stats
      .max_tile_cols
      .max(1u32.wrapping_shl(header.tile_cols_log2));
    stats.max_tile_rows = stats
      .max_tile_rows
      .max(1u32.wrapping_shl(header.tile_rows_log2));
    stats.segmentation_frames = stats
      .segmentation_frames
      .wrapping_add(u64::from(self.st.seg.status == SegmentationStatus::Enabled));
    stats.tx_select_frames = stats
      .tx_select_frames
      .wrapping_add(u64::from(header.tx_mode == TX_MODE_SELECT));

    match header.kind {
      FrameKind::Inter => {
        stats.switchable_interp_frames = stats
          .switchable_interp_frames
          .wrapping_add(u64::from(header.interp_filter == SWITCHABLE));
        stats.high_precision_mv_frames = stats.high_precision_mv_frames.wrapping_add(u64::from(
          header.precision == crate::header::Precision::High,
        ));
        stats.compound_frames = stats
          .compound_frames
          .wrapping_add(u64::from(header.reference_mode != 0));
      }
      FrameKind::Key | FrameKind::IntraOnly => {}
    }

    let depth = u64::from(header.color.bit_depth.wrapping_sub(8).wrapping_shr(1));
    let Ok(depth_slot) = index(depth);
    let Ok(profile_slot) = index(header.profile);

    for slot in [
      stats.bit_depths.get_mut(depth_slot),
      stats.profiles.get_mut(profile_slot),
    ]
    .into_iter()
    .flatten()
    {
      *slot = slot.wrapping_add(1);
    }

    Ok(())
  }
}

impl Drop for Decoder {
  fn drop(&mut self) {
    let Ok(()) = self.join_jobs();
  }
}

#[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
enum OutputDelivery {
  Shown,
  Hidden,
}

#[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
fn post_worker(
  input: std::sync::mpsc::Receiver<PostJob>,
  output: std::sync::mpsc::SyncSender<Picture>,
) -> std::result::Result<(), Never> {
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the caller owns the single job sender and either sends the job immediately after spawn or drops the sender"
    )
  )]
  let job = match input.recv() {
    Ok(job) => job,
    Err(_caller_dropped_before_submitting) => return Ok(()),
  };
  let Ok(rendered) = job.run();

  match rendered {
    Some(picture) => match output.send(picture) {
      Ok(()) => {}
      Err(std::sync::mpsc::SendError(discarded_after_reset)) => drop(discarded_after_reset),
    },
    None => {}
  }

  Ok(())
}

#[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
fn join_post_worker(
  job: std::thread::JoinHandle<std::result::Result<(), Never>>,
) -> std::result::Result<(), Never> {
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the decoder owns this finite post job and drains it in submission order; PublishGuard publishes unfinished reference bands during unwind"
    )
  )]
  let finished = job.join();

  match finished {
    Ok(Ok(())) => {}
    Err(fault) => {
      let detail = fault
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| fault.downcast_ref::<String>().map(String::as_str));

      match detail {
        Some(message) => console_core_logging::error!("VP9 post worker failed: {message}"),
        None => console_core_logging::error!("VP9 post worker failed"),
      }
    }
  }

  Ok(())
}

#[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
enum PictureDelivery {
  Picture(Picture),
  Waiting,
  Ended,
}

#[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
fn receive_picture(
  receive: &std::sync::mpsc::Receiver<Picture>,
  mode: Drain,
) -> std::result::Result<PictureDelivery, Never> {
  Ok(match mode {
    Drain::All => {
      #[cfg_attr(
        dylint_lib = "explicit057_no_unbounded_wait",
        expect(
          explicit057_no_unbounded_wait,
          reason = "the finite post worker owns the only sender; its completion or unwind closes the channel, and its reference rows come only from already-submitted frames"
        )
      )]
      let received = receive.recv();

      match received {
        Ok(picture) => PictureDelivery::Picture(picture),
        Err(_worker_finished_without_output) => PictureDelivery::Ended,
      }
    }
    Drain::Ready => match receive.try_recv() {
      Ok(picture) => PictureDelivery::Picture(picture),
      Err(std::sync::mpsc::TryRecvError::Empty) => PictureDelivery::Waiting,
      Err(std::sync::mpsc::TryRecvError::Disconnected) => PictureDelivery::Ended,
    },
  })
}

fn recycle_strips(
  strips: Vec<TileBand>,
  pools: &Pools,
) -> core::result::Result<(), console_core_never::Never> {
  {
    for s in strips {
      for p in s.planes {
        let Ok(()) = pools.samples.put(p);
      }

      let Ok(()) = pools.mi.put(s.mi);
    }
  };

  Ok(())
}

struct PostJob {
  frame: FrameRef,
  strips: Vec<TileBand>,
  mi: GridRef,
  lf: Option<LfFrame>,

  out: Option<(i64, DecodeQuality)>,
  pools: SharedPools,
}

struct PublishGuard<'a>(&'a Frame);

impl Drop for PublishGuard<'_> {
  fn drop(&mut self) {
    let Ok(sb_rows) = self.0.sb_rows();

    for r in 0..sb_rows {
      let Ok(publication) = self.0.publication(r);

      match ::core::primitive::bool::then_some(
        publication == crate::frame::Publication::Waiting,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = self.0.publish(r, {
            let Ok(explicit_value) = self.0.new_band();
            explicit_value
          });
        }
        ::core::option::Option::None => {}
      }
    }
  }
}

impl PostJob {
  fn assemble_band(
    &self,
    output: &mut Band,
    band_row: u64,
  ) -> core::result::Result<(), console_core_never::Never> {
    for p in 0u64..3 {
      let Ok(layout) = self.frame.layout(p);
      let Ok(dest) = output.plane_mut(p);
      let Ok(plane) = index(p);

      'over_strip_2: for strip in &self.strips {
        let Ok(stride) = read(&strip.strides, p);
        let Ok(first_x) = read(&strip.x_off, p);
        let width = stride.min(layout.stride.saturating_sub(first_x));
        let samples = match strip.planes.get(plane) {
          Some(samples) => samples,
          None => continue 'over_strip_2,
        };

        for row in 0..layout.band_rows {
          let y = band_row.wrapping_mul(layout.band_rows).wrapping_add(row);
          let from = y.wrapping_mul(stride);
          let to = BAND_PAD
            .wrapping_add(row)
            .wrapping_mul(layout.stride)
            .wrapping_add(first_x);
          let Ok(source) = span(samples, from..from.wrapping_add(width));
          let Ok(destination) = span_mut(dest, to..to.wrapping_add(width));

          for (out, &sample) in destination.iter_mut().zip(source) {
            *out = sample;
          }
        }
      }
    }

    Ok(())
  }

  fn run(self) -> std::result::Result<Option<Picture>, Never> {
    let frame = &self.frame;
    let _guard = PublishGuard(frame);
    let mut output = self.out.map(|(pts, quality)| {
      let Ok(builder) = PictureBuilder::new(frame, pts, quality);
      builder
    });
    let Ok(rows) = frame.sb_rows();
    let mut held = None;

    for row in 0..rows {
      let Ok(mut band) = frame.new_band();
      let Ok(()) = self.assemble_band(&mut band, row);

      match &self.lf {
        Some(filter) => {
          let Ok(()) = self.filter_band(&mut band, held.as_mut(), row, filter);
        }
        None => {}
      }

      match held.replace(band) {
        Some(previous) => {
          let Ok(()) = publish_band(frame, row.wrapping_sub(1), previous, output.as_mut());
        }
        None => {}
      }
    }

    match held {
      Some(last) => {
        let Ok(()) = publish_band(frame, rows.wrapping_sub(1), last, output.as_mut());
      }
      None => {}
    }

    let Ok(()) = recycle_strips(self.strips, &self.pools);
    Ok(output.map(|picture| {
      let Ok(finished) = picture.finish();
      finished
    }))
  }

  fn filter_band(
    &self,
    band: &mut Band,
    mut previous: Option<&mut Band>,
    row: u64,
    filter: &LfFrame,
  ) -> std::result::Result<(), Never> {
    let frame = &self.frame;

    match previous.as_deref() {
      Some(previous_2) => {
        let Ok(()) = transfer_padding(frame, previous_2, band, PaddingTransfer::IntoCurrent);
      }
      None => {}
    }

    let [a, b, c] = &mut band.planes;
    let Ok(y) = band_view(frame, a, crate::frame::Line { plane: 0, y: row });
    let Ok(u) = band_view(frame, b, crate::frame::Line { plane: 1, y: row });
    let Ok(v) = band_view(frame, c, crate::frame::Line { plane: 2, y: row });
    let mut views = [y, u, v];

    for col in 0..filter.grid.col.div_ceil(8) {
      let cell = MiPlace {
        row: row.wrapping_mul(8),
        col: col.wrapping_mul(8),
      };
      let Ok(()) = filter_superblock(&mut views, &self.mi, filter, cell);
    }

    match previous.take() {
      Some(previous_3) => {
        transfer_padding(frame, band, previous_3, PaddingTransfer::IntoPrevious)
      }
      None => Ok(()),
    }
  }
}

fn publish_band(
  frame: &Frame,
  row: u64,
  band: Band,
  output: Option<&mut PictureBuilder>,
) -> std::result::Result<(), Never> {
  match output {
    Some(picture) => {
      let Ok(()) = picture.add(frame, row, &band);
    }
    None => {}
  }

  frame.publish(row, band)
}

enum PaddingTransfer {
  IntoCurrent,
  IntoPrevious,
}

fn transfer_padding(
  frame: &Frame,
  from: &Band,
  to: &mut Band,
  direction: PaddingTransfer,
) -> std::result::Result<(), Never> {
  for ((source, destination), layout) in from.planes.iter().zip(&mut to.planes).zip(frame.layouts)
  {
    let visible_rows = layout.band_rows.wrapping_mul(layout.stride);
    let padding = BAND_PAD.wrapping_mul(layout.stride);
    let (read_start, write_start) = match direction {
      PaddingTransfer::IntoCurrent => (visible_rows, 0),
      PaddingTransfer::IntoPrevious => (0, visible_rows),
    };
    let Ok(source_2) = span(source, read_start..read_start.wrapping_add(padding));
    let Ok(destination_2) = span_mut(destination, write_start..write_start.wrapping_add(padding));

    for (out, &sample) in destination_2.iter_mut().zip(source_2) {
      *out = sample;
    }
  }

  Ok(())
}

fn band_view<'a>(
  frame: &Frame,
  samples: &'a mut [u16],
  line: crate::frame::Line,
) -> std::result::Result<PlaneView<'a>, Never> {
  let Ok(layout) = frame.layout(line.plane);

  Ok(match line.y {
    0 => {
      let Ok(data) = crate::places::after_mut(samples, BAND_PAD.wrapping_mul(layout.stride));
      PlaneView {
        data,
        stride: layout.stride,
        ox: 0,
        oy: 0,
      }
    }
    row => PlaneView {
      data: samples,
      stride: layout.stride,
      ox: 0,
      oy: row.wrapping_mul(layout.band_rows).wrapping_sub(BAND_PAD),
    },
  })
}

fn assemble_mi(
  strips: &mut [TileBand],
  dimensions: MiPlace,
  pools: &Pools,
) -> core::result::Result<(MiGrid, Vec<u8>), console_core_never::Never> {
  let MiPlace {
    row: rows,
    col: cols,
  } = dimensions;
  let area = rows.wrapping_mul(cols);
  let Ok(mut mi) = pools.mi.take(area, MiInfo::BLANK);
  let Ok(mut seg) = filled::<_, 4194304>(area, 0);

  for strip in strips {
    let width = strip.columns.end.wrapping_sub(strip.columns.start);

    for row in 0..rows {
      let source = row.wrapping_mul(width);
      let destination = row.wrapping_mul(cols).wrapping_add(strip.columns.start);
      let from = source..source.wrapping_add(width);
      let to = destination..destination.wrapping_add(width);
      let Ok(mi_source) = span(&strip.mi, from.clone());
      let Ok(mi_destination) = span_mut(&mut mi, to.clone());
      let Ok(id_source) = span(&strip.seg_ids, from);
      let Ok(id_destination) = span_mut(&mut seg, to);

      for (next, &old) in mi_destination.iter_mut().zip(mi_source) {
        *next = old;
      }

      for (next, &old) in id_destination.iter_mut().zip(id_source) {
        *next = old;
      }
    }

    let Ok(()) = pools.mi.put(std::mem::take(&mut strip.mi));
  }

  Ok((MiGrid { cols, rows, mi }, seg))
}

struct PictureBuilder {
  planes: [Plane; 3],
  info: FrameInfo,
  chroma: Size<u64>,
  pts: i64,
  quality: DecodeQuality,
}

impl PictureBuilder {
  fn new(frame: &Frame, pts: i64, quality: DecodeQuality) -> std::result::Result<Self, Never> {
    let planes = frame.layouts.map(|layout| {
      let area = layout.visible.width.wrapping_mul(layout.visible.height);

      match frame.info.bit_depth {
        8 => {
          let Ok(samples) = filled::<_, 268435456>(area, 0);
          Plane::U8(samples)
        }
        _ => {
          let Ok(samples) = filled::<_, 268435456>(area, 0);
          Plane::U16(samples)
        }
      }
    });
    let [_, chroma, _] = frame.layouts;
    Ok(Self {
      planes,
      info: frame.info,
      chroma: chroma.visible,
      pts,
      quality,
    })
  }

  fn add(
    &mut self,
    frame: &Frame,
    band_number: u64,
    band: &Band,
  ) -> std::result::Result<(), Never> {
    for ((target, samples), layout) in self.planes.iter_mut().zip(&band.planes).zip(frame.layouts)
    {
      let width = layout.visible.width;
      let start = band_number.wrapping_mul(layout.band_rows);
      let end = start
        .wrapping_add(layout.band_rows)
        .min(layout.visible.height);

      for y in start..end {
        let row = y
          .wrapping_sub(start)
          .wrapping_add(BAND_PAD)
          .wrapping_mul(layout.stride);
        let Ok(source) = span(samples, row..row.wrapping_add(width));
        let offset = y.wrapping_mul(width);
        let visible = offset..offset.wrapping_add(width);

        match target {
          Plane::U8(samples_2) => {
            let Ok(destination) = span_mut(samples_2, visible);

            for (out, &sample) in destination.iter_mut().zip(source) {
              let Ok(value) = wrapped::<_, u8>(sample);
              *out = value;
            }
          }
          Plane::U16(samples_3) => {
            let Ok(destination) = span_mut(samples_3, visible);

            for (out, &sample) in destination.iter_mut().zip(source) {
              *out = sample;
            }
          }
        }
      }
    }

    Ok(())
  }

  fn finish(self) -> std::result::Result<Picture, Never> {
    let info = self.info;
    let [y, u, v] = self.planes;
    let Ok(chroma_width) = wrapped::<_, u32>(self.chroma.width);
    let Ok(chroma_height) = wrapped::<_, u32>(self.chroma.height);
    let Ok(y_stride) = index(info.size.width);
    let Ok(uv_stride) = index(self.chroma.width);
    Ok(Picture {
      width: info.size.width,
      height: info.size.height,
      chroma_width,
      chroma_height,
      bit_depth: u32::from(info.bit_depth),
      subsampling_x: info.subsampling.x != 0,
      subsampling_y: info.subsampling.y != 0,
      y,
      u,
      v,
      y_stride,
      uv_stride,
      pts: self.pts,
      key: info.kind == FrameKind::Key,
      intra_only: info.kind == FrameKind::IntraOnly,
      color: ColorInfo {
        color_space: info.color_space,
        full_range: info.color_range == SampleRange::Full,
      },
      render_width: info.render.width,
      render_height: info.render.height,
      draft: self.quality == DecodeQuality::Draft,
    })
  }
}

fn picture_of(
  frame: &Frame,
  pts: i64,
  quality: DecodeQuality,
) -> std::result::Result<Picture, Never> {
  let Ok(mut picture) = PictureBuilder::new(frame, pts, quality);
  let Ok(rows) = frame.sb_rows();

  for row in 0..rows {
    let Ok(band) = frame.band(row);

    match band {
      Some(band_2) => {
        let Ok(()) = picture.add(frame, row, band_2);
      }
      None => {}
    }
  }

  picture.finish()
}
