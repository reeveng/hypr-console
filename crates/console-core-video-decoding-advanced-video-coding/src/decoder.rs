//! Top-level decoder: NAL dispatch, picture boundaries, POC, DPB and output.
//!
//! The calling thread parses headers and does all DPB / reference list bookkeeping (which only needs
//! header information). Macroblock decoding of each picture runs as a job — on a thread pool with the
//! `threads` feature — that publishes finished macroblock rows into the picture's shared [`Frame`];
//! jobs of later pictures block per row on the reference data they need, so several pictures decode
//! concurrently (frame-level parallelism).

use crate::dpb::{Dpb, FrameNumbers, Output, OutputMeta, PictureOutput, ReferenceLimits};
use crate::error::{Error, Result, ensure, invalid, unsupported};
use crate::params::{EntropyCoding, Pps, PredictionWeighting, Sps};
use crate::picture::{
  CroppedPlanes, Frame, FrameRef, MacroblockPrediction, MbKind, MbState, Planes, Publication,
  RefPic, TransformSize, retain_frame, share_frame,
};
use crate::slice::{
  NalHeader, PictureRefresh, Poc, PocState, ReferenceReset, SliceHeader, SliceType, nal_type,
};
use crate::slicedec::{PicState, RowFlush, SliceDecoder};
use crate::transform::LevelScale;
use crate::{ColorInfo, DecodeQuality, Picture};
use console_core_api_compatibility::NativeSize;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_video_decoding_bitstream::{
  BitReader, annexb_nals, length_prefixed_nals, unescape_rbsp,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The decoder cache and independently queued jobs keep immutable header generations alive"
  )
)]
struct HeaderSnapshot<T>(Arc<T>);

impl<T> HeaderSnapshot<T> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "A parsed header revision becomes the immutable snapshot queued with each dependent slice"
    )
  )]
  fn new(value: T) -> core::result::Result<Self, Never> {
    Ok(Self(Arc::new(value)))
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Scale matrices stay shared by slices with identical parameter revisions"
    )
  )]
  fn from_box(value: Box<T>) -> core::result::Result<Self, Never> {
    Ok(Self(Arc::from(value)))
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Pointer identity distinguishes redefinitions of the same parameter identifier"
    )
  )]
  fn revision(&self, other: &Self) -> core::result::Result<HeaderRevision, Never> {
    Ok(
      match ::core::primitive::bool::then_some(Arc::ptr_eq(&self.0, &other.0), ()) {
        ::core::option::Option::Some(()) => HeaderRevision::Same,
        ::core::option::Option::None => HeaderRevision::Different,
      },
    )
  }
}

impl<T> Clone for HeaderSnapshot<T> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Each queued slice explicitly retains the immutable parameter-set revision"
    )
  )]
  fn clone(&self) -> Self {
    Self(Arc::clone(&self.0))
  }
}

impl<T> core::ops::Deref for HeaderSnapshot<T> {
  type Target = T;

  fn deref(&self) -> &T {
    &self.0
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeaderRevision {
  Same,
  Different,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Queued frame workers retain the shared stats, error slot, and reusable picture storage through job completion"
  )
)]
#[derive(Clone, Default)]
struct WorkerState(Arc<Shared>);

impl core::ops::Deref for WorkerState {
  type Target = Shared;

  fn deref(&self) -> &Shared {
    &self.0
  }
}

impl Shared {
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "The statistics lock covers a snapshot or counter merge and is never held during frame or row waits"
    )
  )]
  fn with_stats<T>(
    &self,
    update: impl FnOnce(&mut DecodeStats) -> T,
  ) -> core::result::Result<T, Never> {
    let mut stats = match self.stats.lock() {
      Ok(stats) => stats,
      Err(poisoned) => poisoned.into_inner(),
    };
    Ok(update(&mut stats))
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "The worker error lock only guards the first pending error and is released before caller or decoder work"
    )
  )]
  fn error_slot(&self) -> core::result::Result<std::sync::MutexGuard<'_, Option<Error>>, Never> {
    Ok(match self.error.lock() {
      Ok(slot) => slot,
      Err(poisoned) => poisoned.into_inner(),
    })
  }

  fn remember_error(&self, error: Error) -> core::result::Result<(), Never> {
    let Ok(mut slot) = self.error_slot();
    slot.get_or_insert(error);
    Ok(())
  }

  fn take_error(&self) -> core::result::Result<Option<Error>, Never> {
    let Ok(mut slot) = self.error_slot();
    Ok(slot.take())
  }

  fn reuse_picture(&self, frame: &Frame) -> core::result::Result<Option<PicState>, Never> {
    let Ok(rows) = frame.mb_h();
    let picture = match self.pool.try_lock() {
      Ok(mut pool) => pool.pop(),
      Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner().pop(),
      Err(std::sync::TryLockError::WouldBlock) => None,
    };
    Ok(picture.filter(|picture_2| picture_2.mb_w == frame.mb_w && picture_2.mb_h == rows))
  }

  fn recycle_picture(&self, picture: PicState) -> core::result::Result<(), Never> {
    let guard = match self.pool.try_lock() {
      Ok(pool) => pool,
      Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
      Err(std::sync::TryLockError::WouldBlock) => return Ok(()),
    };
    let mut pool = guard;

    match ::core::primitive::bool::then_some(pool.len() < 4, ()) {
      ::core::option::Option::Some(()) => {
        pool.push(picture);
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }
}

fn last_row_status(frame: &Frame) -> core::result::Result<Publication, Never> {
  let Ok(rows) = frame.mb_h();
  frame.publication(rows.wrapping_sub(1))
}

struct SliceJob {
  sh: SliceHeader,
  pps: HeaderSnapshot<Pps>,
  sps: HeaderSnapshot<Sps>,
  rbsp: Vec<u8>,
  refs: [Vec<RefPic>; 2],
  ls: HeaderSnapshot<LevelScale>,
}

struct PendingPic {
  frame: FrameRef,
  sps: HeaderSnapshot<Sps>,
  first: SliceHeader,
  poc: Poc,
  pts: i64,
  refresh: PictureRefresh,
  slices: Vec<SliceJob>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecodeStats {
  pub pictures: u64,
  pub slices_cavlc: u64,
  pub slices_cabac: u64,
  pub slices_i: u64,
  pub slices_p: u64,
  pub slices_b: u64,
  pub mb_i4x4: u64,
  pub mb_i8x8: u64,
  pub mb_i16x16: u64,
  pub mb_pcm: u64,
  pub mb_p_skip: u64,
  pub mb_b_skip: u64,
  pub mb_b_direct16x16: u64,
  pub mb_inter: u64,

  pub mb_inter_8x8_transform: u64,
  pub mmco_ops: u64,
  pub long_term_marks: u64,
  pub frame_num_gaps: u64,
  pub weighted_slices: u64,
  pub temporal_direct_slices: u64,

  pub draft_pictures: u64,
}

#[derive(Default)]
struct Shared {
  error: Mutex<Option<Error>>,
  stats: Mutex<DecodeStats>,

  pool: Mutex<Vec<PicState>>,
}

pub struct Decoder {
  spss: Vec<Option<HeaderSnapshot<Sps>>>,
  ppss: Vec<Option<HeaderSnapshot<Pps>>>,
  nal_length_size: Option<u8>,
  dpb: Dpb,
  poc_state: PocState,
  prev_ref_frame_num: u32,
  pending: Option<PendingPic>,
  next_id: u32,
  active_sps: Option<HeaderSnapshot<Sps>>,
  ls_cache: Vec<(HeaderSnapshot<Pps>, HeaderSnapshot<LevelScale>)>,

  out_queue: VecDeque<Output>,
  shared: WorkerState,
  #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
  pool: Option<console_concurrency::FifoPool>,
  in_flight: VecDeque<FrameRef>,
  max_in_flight: u64,
  threads: u64,

  draft: DecodeQuality,
}

impl Default for Decoder {
  fn default() -> Self {
    let Ok(decoder) = Self::new();

    decoder
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PictureBoundary {
  New,
  Continuing,
}

fn picture_boundary(
  prev: &SliceHeader,
  sh: &SliceHeader,
  sps: &Sps,
) -> core::result::Result<PictureBoundary, Never> {
  match ::core::primitive::bool::then_some(
    sh.frame_num != prev.frame_num
      || sh.pps_id != prev.pps_id
      || sh.field_pic != prev.field_pic
      || sh.bottom_field != prev.bottom_field
      || (sh.nal_ref_idc == 0) != (prev.nal_ref_idc == 0)
      || sh.refresh != prev.refresh
      || (sh.refresh == PictureRefresh::Idr
        && prev.refresh == PictureRefresh::Idr
        && sh.idr_pic_id != prev.idr_pic_id),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(PictureBoundary::New);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    sps.pic_order_cnt_type == 0
      && (sh.pic_order_cnt_lsb != prev.pic_order_cnt_lsb
        || sh.delta_pic_order_cnt_bottom != prev.delta_pic_order_cnt_bottom),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(PictureBoundary::New);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    sps.pic_order_cnt_type == 1 && sh.delta_pic_order_cnt != prev.delta_pic_order_cnt,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(PictureBoundary::New);
    }
    ::core::option::Option::None => {}
  }

  Ok(PictureBoundary::Continuing)
}

fn ensure_supported_slice(header: &SliceHeader, pps: &Pps) -> Result<()> {
  match ::core::primitive::bool::then_some(pps.num_slice_groups > 1, ()) {
    ::core::option::Option::Some(()) => {
      return unsupported("slice groups (FMO)");
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(header.field_pic, ()) {
    ::core::option::Option::Some(()) => {
      return unsupported("field pictures");
    }
    ::core::option::Option::None => {}
  }

  match header.slice_type {
    SliceType::Sp | SliceType::Si => unsupported("SP/SI slices"),
    SliceType::I | SliceType::P | SliceType::B => Ok(()),
  }
}

fn ensure_slice_references(kind: SliceType, references: &[Vec<RefPic>; 2]) -> Result<()> {
  let [first, second] = references;

  match kind {
    SliceType::P | SliceType::Sp | SliceType::B => {
      ensure(
        !first.is_empty(),
        || "no reference pictures available for inter slice",
      )?;
    }
    SliceType::I | SliceType::Si => {}
  }

  match ::core::primitive::bool::then_some(kind == SliceType::B, ()) {
    ::core::option::Option::Some(()) => {
      ensure(!second.is_empty(), || "empty RefPicList1 in B slice")?;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn count_slice(
  stats: &mut DecodeStats,
  header: &SliceHeader,
  pps: &Pps,
) -> core::result::Result<(), Never> {
  match pps.entropy_coding_mode {
    EntropyCoding::Cabac => stats.slices_cabac = stats.slices_cabac.wrapping_add(1),
    EntropyCoding::Cavlc => stats.slices_cavlc = stats.slices_cavlc.wrapping_add(1),
  }

  match header.slice_type {
    SliceType::I | SliceType::Si => stats.slices_i = stats.slices_i.wrapping_add(1),
    SliceType::P | SliceType::Sp => stats.slices_p = stats.slices_p.wrapping_add(1),
    SliceType::B => stats.slices_b = stats.slices_b.wrapping_add(1),
  }

  let weighted = (matches!(pps.weighted_pred, PredictionWeighting::Weighted)
    && matches!(header.slice_type, SliceType::P | SliceType::Sp))
    || (pps.weighted_bipred_idc != 0 && header.slice_type == SliceType::B);
  let temporal = header.slice_type == SliceType::B && !header.direct_spatial_mv_pred;
  let Ok(operations) = fixed_index(header.mmcos.len());
  let Ok(long_term) = fixed_index(
    header
      .mmcos
      .iter()
      .filter(|operation| matches!(operation.op, 3 | 6))
      .count(),
  );
  stats.weighted_slices = stats.weighted_slices.wrapping_add(u64::from(weighted));
  stats.temporal_direct_slices = stats
    .temporal_direct_slices
    .wrapping_add(u64::from(temporal));
  stats.mmco_ops = stats.mmco_ops.wrapping_add(operations);
  stats.long_term_marks = stats
    .long_term_marks
    .wrapping_add(long_term)
    .wrapping_add(u64::from(header.long_term_reference));
  Ok(())
}

pub fn default_threads()
-> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never> {
  Ok({
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    {
      match std::thread::available_parallelism() {
        Ok(available) => available.get().min(16),
        Err(_parallelism_unknown) => 1,
      }
    }

    #[cfg(not(all(feature = "threads", not(target_arch = "wasm32"))))]
    {
      1
    }
  })
}

impl Decoder {
  pub fn new() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(decoder) = Self::with_threads(match default_threads() {
      ::core::result::Result::Ok(checked_value) => checked_value,
    });

    Ok(decoder)
  }

  pub fn with_threads(
    threads: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(()) = crate::cavlc::init_tables();
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    let Ok(pool) = {
      let Ok(width) = console_core_number_conversion::fitted(threads);
      console_concurrency::FifoPool::for_threads("h264", width)
    };
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    let worker_count = match pool {
      Some(_) => {
        let Ok(workers) = fixed_index(threads);
        workers
      }
      None => 1_u64,
    };
    #[cfg(not(all(feature = "threads", not(target_arch = "wasm32"))))]
    let worker_count = 1;

    let Ok(requested) = fixed_index(threads);
    Ok(Decoder {
      spss: vec![None; 32],
      ppss: vec![None; 256],
      nal_length_size: None,
      dpb: ({
        let Ok(explicit_value) = Dpb::new();
        explicit_value
      }),
      poc_state: PocState::default(),
      prev_ref_frame_num: 0,
      pending: None,
      next_id: 1,
      active_sps: None,
      ls_cache: Vec::new(),
      out_queue: VecDeque::new(),
      shared: WorkerState::default(),
      #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
      pool,
      in_flight: VecDeque::new(),
      max_in_flight: requested.max(1).saturating_add(2),
      threads: worker_count,
      draft: DecodeQuality::Full,
    })
  }

  pub fn threads(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    index(self.threads)
  }

  pub fn set_quality(
    &mut self,
    quality: DecodeQuality,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.draft = quality;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "Preserve the public decoder quality query signature for downstream callers"
    )
  )]
  #[must_use]
  pub fn quality(&self) -> DecodeQuality {
    self.draft
  }

  pub fn from_avcc(avcc: &[u8]) -> Result<Self> {
    let Ok(mut d) = Decoder::new();
    d.configure_avcc(avcc)?;
    Ok(d)
  }

  pub fn configure_avcc(&mut self, avcc: &[u8]) -> Result<()> {
    ensure(avcc.len() >= 7, || "avcC too short")?;
    let prefix = match avcc.first_chunk::<6>() {
      Some(prefix) => prefix,
      None => return invalid("avcC too short"),
    };
    let &[version, _, _, _, framing, sps_count] = prefix;
    ensure(version == 1, || {
      format!("unsupported avcC version {version}")
    })?;
    self.nal_length_size = Some((framing & 3).wrapping_add(1));
    let remainder = match avcc.get(6..) {
      Some(bytes) => bytes,
      None => return invalid("avcC truncated"),
    };
    let after_sps = self.read_avcc_sets(remainder, sps_count & 0x1f)?;
    let (pps_count, after_count) = match after_sps.split_first() {
      Some((&count, bytes)) => (count, bytes),
      None => return invalid("avcC truncated"),
    };
    let _extensions = self.read_avcc_sets(after_count, pps_count)?;
    Ok(())
  }

  fn read_avcc_sets<'a>(&mut self, mut bytes: &'a [u8], count: u8) -> Result<&'a [u8]> {
    for _ in 0..count {
      let (length, payload) = match bytes.split_first_chunk::<2>() {
        Some((size, rest)) => (u16::from_be_bytes(*size), rest),
        None => return invalid("avcC truncated"),
      };
      let Ok(position) = index(length);
      let (nal, remaining) = match payload.split_at_checked(position) {
        Some(split) => split,
        None => return invalid("avcC truncated"),
      };
      self.handle_nal(nal, 0)?;
      bytes = remaining;
    }

    Ok(bytes)
  }

  pub fn nal_length_size(&self) -> core::result::Result<Option<NativeSize>, Never> {
    Ok(self.nal_length_size.map(|length| {
      let Ok(size) = index(length);
      size
    }))
  }

  pub fn decode(&mut self, data: &[u8], pts: i64) -> Result<Vec<Picture>> {
    let nals = match self.nal_length_size {
      Some(length) => length_prefixed_nals(data, length)?,
      None => {
        let Ok(nals) = annexb_nals(data);
        nals
      }
    };
    let mut result = Ok(());

    for nal in nals {
      match self.handle_nal(nal, pts) {
        Ok(()) => {}
        Err(error) => {
          result = Err(error);
          break;
        }
      }
    }

    let Ok(()) = self.submit_pending();
    let Ok(worker_error) = self.take_error();
    result?;

    match worker_error {
      Some(error) => return Err(error),
      None => {}
    }

    let Ok(pictures) = self.ready_pictures();
    Ok(pictures)
  }

  fn ready_pictures(&mut self) -> core::result::Result<Vec<Picture>, Never> {
    let queue_limit = self.max_in_flight.saturating_mul(2);
    let Ok(count) = fixed_index(self.out_queue.len());
    let mut pictures = Vec::new();

    for _ in 0..count {
      let front = match self.out_queue.front() {
        Some(front) => front,
        None => break,
      };
      let Ok(ready) = last_row_status(&front.frame);
      let Ok(remaining) = fixed_index(self.out_queue.len());

      match ::core::primitive::bool::then_some(
        ready == Publication::Pending && remaining <= queue_limit,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      match self.out_queue.pop_front() {
        Some(output) => {
          let Ok(decoded) = make_picture(&output);
          pictures.push(decoded);
        }
        None => break,
      }
    }

    Ok(pictures)
  }

  pub fn flush(&mut self) -> core::result::Result<Vec<Picture>, Never> {
    let Ok(()) = self.submit_pending();
    let mut outputs = Vec::new();
    let Ok(()) = self.dpb.flush(&mut outputs);
    let Ok(()) = self.emit(outputs);
    let pictures = self
      .out_queue
      .drain(..)
      .map(|output| {
        let Ok(picture) = make_picture(&output);
        picture
      })
      .collect();

    for frame in self.in_flight.drain(..) {
      let Ok(()) = frame.wait_complete();
    }

    Ok(pictures)
  }

  pub fn stats(&self) -> core::result::Result<DecodeStats, Never> {
    self.shared.with_stats(|stats| stats.clone())
  }

  pub fn take_error(&mut self) -> core::result::Result<Option<Error>, Never> {
    self.shared.take_error()
  }

  fn handle_nal(&mut self, nal: &[u8], pts: i64) -> Result<()> {
    let (&first, payload) = match nal.split_first() {
      Some(parts) => parts,
      None => return Ok(()),
    };
    let header = NalHeader::parse(first)?;

    match header.nal_unit_type {
      nal_type::SPS => {
        let Ok(rbsp) = unescape_rbsp(payload);
        let sps = Sps::parse(&rbsp)?;
        let Ok(position) = index(sps.id);
        let slot = match self.spss.get_mut(position) {
          Some(slot) => slot,
          None => return invalid("SPS identifier outside the parameter set table"),
        };
        let Ok(version) = HeaderSnapshot::new(sps);
        *slot = Some(version);
      }
      nal_type::PPS => {
        let Ok(rbsp) = unescape_rbsp(payload);
        let sequences: Vec<Option<Sps>> = self
          .spss
          .iter()
          .map(|entry| entry.as_ref().map(|sps| (**sps).clone()))
          .collect();
        let pps = Pps::parse(&rbsp, &sequences)?;
        let Ok(position) = index(pps.id);
        let slot = match self.ppss.get_mut(position) {
          Some(slot) => slot,
          None => return invalid("PPS identifier outside the parameter set table"),
        };
        let Ok(version) = HeaderSnapshot::new(pps);
        *slot = Some(version);
      }
      nal_type::SLICE | nal_type::IDR => self.handle_slice(payload, header, pts)?,
      nal_type::SLICE_DPA | nal_type::SLICE_DPB | nal_type::SLICE_DPC => {
        return unsupported("data partitioning (Extended profile)");
      }
      nal_type::END_SEQ | nal_type::END_STREAM => {
        let Ok(()) = self.submit_pending();
      }
      _ => {}
    }

    Ok(())
  }

  fn level_scale(
    &mut self,
    pps: &HeaderSnapshot<Pps>,
  ) -> core::result::Result<HeaderSnapshot<LevelScale>, console_core_never::Never> {
    match self
      .ls_cache
      .iter()
      .find(|(p, _)| matches!(p.revision(pps), Ok(HeaderRevision::Same)))
    {
      Some((_, ls)) => {
        return Ok(ls.clone());
      }
      ::core::option::Option::None => {}
    }

    let Ok(level_scale) = LevelScale::new(&pps.scaling);
    let Ok(ls) = HeaderSnapshot::from_box(level_scale);
    self.ls_cache.retain(|(p, _)| {
      self
        .ppss
        .iter()
        .flatten()
        .any(|q| matches!(p.revision(q), Ok(HeaderRevision::Same)))
    });
    self.ls_cache.push((pps.clone(), ls.clone()));

    Ok(ls)
  }

  fn parameter_sets(&self, id: u32) -> Result<(&HeaderSnapshot<Pps>, &HeaderSnapshot<Sps>)> {
    let Ok(pps_position) = index(id);
    let pps = match self.ppss.get(pps_position).and_then(Option::as_ref) {
      Some(pps) => pps,
      None => return Err(Error::MissingParameterSet(format!("PPS {id}"))),
    };
    let Ok(sps_position) = index(pps.sps_id);
    let sps = match self.spss.get(sps_position).and_then(Option::as_ref) {
      Some(sps) => sps,
      None => return Err(Error::MissingParameterSet(format!("SPS {}", pps.sps_id))),
    };
    Ok((pps, sps))
  }

  fn start_slice_picture(
    &mut self,
    header: &SliceHeader,
    sps: &HeaderSnapshot<Sps>,
    pts: i64,
  ) -> Result<()> {
    let start = match &self.pending {
      None => true,
      Some(existing) => {
        let Ok(boundary) = picture_boundary(&existing.first, header, sps);
        let Ok(revision) = existing.sps.revision(sps);
        boundary == PictureBoundary::New
          || revision == HeaderRevision::Different
          || header.first_mb_in_slice == 0
      }
    };

    match ::core::primitive::bool::then_some(start, ()) {
      ::core::option::Option::None => return Ok(()),
      ::core::option::Option::Some(()) => {}
    }

    let Ok(()) = self.submit_pending();

    match ::core::primitive::bool::then_some(
      header.first_mb_in_slice != 0 && self.active_sps.is_none(),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return invalid("stream does not start with the first slice of a picture");
      }
      ::core::option::Option::None => {}
    }

    self.start_picture(header, sps, pts)
  }

  fn handle_slice(&mut self, bytes: &[u8], nal: NalHeader, pts: i64) -> Result<()> {
    let Ok(rbsp) = unescape_rbsp(bytes);
    let (header, _, _) = SliceHeader::parse(&rbsp, nal, |id| {
      let (pps, sps) = self.parameter_sets(id)?;
      Ok((&**pps, &**sps))
    })?;
    let (pps_ref, sps_ref) = self.parameter_sets(header.pps_id)?;
    let (pps, sps) = (pps_ref.clone(), sps_ref.clone());
    sps.check_supported()?;
    ensure_supported_slice(&header, &pps)?;

    match header.redundant_pic_cnt {
      0 => {}
      _ => {
        return Ok(());
      }
    }

    self.start_slice_picture(&header, &sps, pts)?;
    let Ok(level_scale) = self.level_scale(&pps);

    let pending = match self.pending.as_mut() {
      Some(pending) => pending,
      None => return invalid("slice without a started picture"),
    };
    let Ok(poc) = pending.poc.frame();
    let Ok(max_frame_num) = sps.max_frame_num();
    let refs = self.dpb.build_ref_lists(&header, poc, max_frame_num)?;
    ensure_slice_references(header.slice_type, &refs)?;

    let job = SliceJob {
      sh: header,
      pps,
      sps,
      rbsp,
      refs,
      ls: level_scale,
    };
    let Ok(()) = self.shared.with_stats(|stats| {
      let Ok(()) = count_slice(stats, &job.sh, &job.pps);
    });
    pending.slices.push(job);
    Ok(())
  }

  fn configure_dpb(&mut self, sps: &HeaderSnapshot<Sps>) -> core::result::Result<(), Never> {
    let Ok(width) = sps.width();
    let Ok(height) = sps.height();
    let Ok(max_frames) =
      ::console_core_number_conversion::fixed_index(match sps.max_dpb_frames() {
        ::core::result::Result::Ok(max_frames) => max_frames,
      });
    let changed = match &self.active_sps {
      Some(previous) => {
        let Ok(last_width) = previous.width();
        let Ok(last_height) = previous.height();
        let Ok(last_frames) =
          ::console_core_number_conversion::fixed_index(match previous.max_dpb_frames() {
            ::core::result::Result::Ok(last_frames) => last_frames,
          });
        (
          width,
          height,
          match ::console_core_number_conversion::index(max_frames) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        ) != (
          last_width,
          last_height,
          match ::console_core_number_conversion::index(last_frames) {
            ::core::result::Result::Ok(native_position) => native_position,
          },
        )
      }
      None => false,
    };

    match ::core::primitive::bool::then_some(changed, ()) {
      ::core::option::Option::Some(()) => {
        let mut outputs = Vec::new();
        let Ok(()) = self.dpb.flush(&mut outputs);
        let Ok(()) = self.emit(outputs);
        self.dpb.entries.clear();
      }
      ::core::option::Option::None => {}
    }

    self.active_sps = Some(sps.clone());
    let Ok(capacity) =
      wrapped::<_, u32>(match ::console_core_number_conversion::index(max_frames) {
        ::core::result::Result::Ok(native_position) => native_position,
      });
    let Ok(reorder_count) =
      ::console_core_number_conversion::fixed_index(match sps.max_num_reorder_frames() {
        ::core::result::Result::Ok(reorder_count) => reorder_count,
      });
    let Ok(reorder_limit) = wrapped::<_, u32>(
      match ::console_core_number_conversion::index(reorder_count) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    );
    self.dpb.capacity = capacity;
    self.dpb.max_reorder = reorder_limit.min(capacity);
    Ok(())
  }

  fn start_picture(
    &mut self,
    header: &SliceHeader,
    sps: &HeaderSnapshot<Sps>,
    pts: i64,
  ) -> Result<()> {
    let Ok(()) = self.configure_dpb(sps);
    let Ok(height) = sps.frame_height_in_mbs();
    let blocks = Size {
      width: sps.pic_width_in_mbs,
      height,
    };

    match header.refresh {
      PictureRefresh::Idr => {
        let mut outputs = Vec::new();
        let Ok(()) = self.dpb.idr(header.prior_pictures, &mut outputs);
        let Ok(()) = self.emit(outputs);
        self.prev_ref_frame_num = 0;
      }
      PictureRefresh::NonIdr => {
        let Ok(()) = self.fill_gap(header, sps, pts, blocks);
      }
    }

    let Ok(poc) = self.poc_state.compute(header, sps);
    let frame_poc = match header.reference_reset() {
      ReferenceReset::Reset => 0,
      ReferenceReset::Retain => {
        let Ok(number) = poc.frame();
        number
      }
    };
    let Ok(id) = self.next_picture_id();
    let Ok(frame) = Frame::new(id, frame_poc, blocks);
    let Ok(shared_frame) = share_frame(frame);
    self.pending = Some(PendingPic {
      frame: shared_frame,
      sps: sps.clone(),
      first: header.clone(),
      poc,
      pts,
      refresh: header.refresh,
      slices: Vec::new(),
    });
    Ok(())
  }

  fn next_picture_id(&mut self) -> core::result::Result<u32, Never> {
    let id = self.next_id;
    self.next_id = self.next_id.wrapping_add(1);
    Ok(id)
  }

  fn fill_gap(
    &mut self,
    header: &SliceHeader,
    sps: &Sps,
    pts: i64,
    blocks: Size<u32>,
  ) -> core::result::Result<(), Never> {
    let Ok(modulus) = sps.max_frame_num();
    let mut missing = match self.prev_ref_frame_num.wrapping_add(1).checked_rem(modulus) {
      Some(number) => number,
      None => return Ok(()),
    };

    match ::core::primitive::bool::then_some(
      header.frame_num == self.prev_ref_frame_num || header.frame_num == missing,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.shared.with_stats(|stats| {
      stats.frame_num_gaps = stats.frame_num_gaps.wrapping_add(1);
    });
    let Ok(meta) = output_meta(sps, pts, PictureRefresh::NonIdr);
    let last = self
      .dpb
      .entries
      .iter()
      .filter(|entry| entry.output != PictureOutput::Gap)
      .max_by_key(|entry| entry.frame.id)
      .map(|entry| {
        let Ok(frame) = retain_frame(&entry.frame);
        frame
      });
    let last_matching = last.as_deref().filter(|frame| {
      let Ok(height) = frame.mb_h();
      frame.mb_w == blocks.width && height == blocks.height
    });
    let dimensions = Size {
      width: blocks.width.wrapping_mul(16),
      height: blocks.height.wrapping_mul(16),
    };

    for _ in 0..modulus {
      match ::core::primitive::bool::then_some(missing == header.frame_num, ()) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      let Ok(id) = self.next_picture_id();
      let Ok(planes) = gap_planes(dimensions, last_matching);
      let Ok(frame) = Frame::from_planes(id, 0, &planes);
      let Ok(()) = self.poc_state.update_gap_frame(missing, sps);
      let poc = frame.poc;
      let Ok(shared_frame) = share_frame(frame);
      let output = Output {
        poc,
        frame: shared_frame,
        meta,
      };
      let Ok(()) = self.dpb.store_gap(
        FrameNumbers {
          current: missing,
          modulus,
        },
        sps.max_num_ref_frames,
        output,
      );
      missing = match missing.wrapping_add(1).checked_rem(modulus) {
        Some(number) => number,
        None => break,
      };
    }

    let previous = header.frame_num.wrapping_add(modulus).wrapping_sub(1);

    match previous.checked_rem(modulus) {
      Some(number) => self.prev_ref_frame_num = number,
      None => {}
    }

    Ok(())
  }

  fn submit_pending(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let p = match self.pending.take() {
        Some(p) => p,
        None => return Ok(()),
      };
      let PendingPic {
        frame,
        sps,
        first,
        poc,
        pts,
        refresh,
        slices,
      } = p;
      let draft = match first.nal_ref_idc {
        0 => self.draft,
        _ => DecodeQuality::Full,
      };
      let Ok(queued) = retain_frame(&frame);
      let Ok(()) = self.dispatch(queued, slices, draft);
      let Ok(()) = self.poc_state.update(&first, &poc);

      match ::core::primitive::bool::then_some(first.nal_ref_idc != 0, ()) {
        ::core::option::Option::Some(()) => {
          self.prev_ref_frame_num = match first.reference_reset() {
            ReferenceReset::Reset => 0,
            ReferenceReset::Retain => first.frame_num,
          };
        }
        ::core::option::Option::None => {}
      }

      let Ok(base_meta) = output_meta(&sps, pts, refresh);
      let meta = OutputMeta {
        quality: draft,
        ..base_meta
      };
      let poc_2 = frame.poc;
      let Ok(modulus) = sps.max_frame_num();
      let mut outs = Vec::new();
      let Ok(()) = self.dpb.store_picture(
        &first,
        Output {
          frame,
          poc: poc_2,
          meta,
        },
        ReferenceLimits {
          modulus,
          max_pictures: sps.max_num_ref_frames,
        },
        &mut outs,
      );
      let Ok(()) = self.emit(outs);
    };

    Ok(())
  }

  fn dispatch(
    &mut self,
    frame: FrameRef,
    slices: Vec<SliceJob>,
    quality: DecodeQuality,
  ) -> core::result::Result<(), Never> {
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    match &self.pool {
      Some(pool) => {
        let Ok(()) = throttle_frames(&mut self.in_flight, self.max_in_flight);
        let Ok(queued) = retain_frame(&frame);
        self.in_flight.push_back(queued);
        let shared = self.shared.clone();
        return pool.submit(move || {
          let Ok(()) = run_job(frame, slices, &shared, quality);
        });
      }
      None => {}
    }

    run_job(frame, slices, &self.shared, quality)
  }

  fn emit(&mut self, outs: Vec<Output>) -> core::result::Result<(), console_core_never::Never> {
    {
      self.out_queue.extend(outs);
    };

    Ok(())
  }
}

#[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
fn throttle_frames(
  frames: &mut VecDeque<FrameRef>,
  maximum: u64,
) -> core::result::Result<(), Never> {
  frames.retain(|frame| matches!(last_row_status(frame), Ok(Publication::Pending)));
  let Ok(count) = fixed_index(frames.len());
  let excess = count.saturating_add(1).saturating_sub(maximum);

  for _ in 0..excess {
    match frames.pop_front() {
      Some(frame) => {
        let Ok(()) = frame.wait_complete();
      }
      None => break,
    }
  }

  Ok(())
}

fn gap_planes(
  dimensions: Size<u32>,
  previous: Option<&Frame>,
) -> core::result::Result<Planes, Never> {
  match previous {
    Some(frame) => {
      let Ok(CroppedPlanes { y, cb, cr }) = frame.copy_cropped([
        0,
        0,
        u64::from(dimensions.width),
        u64::from(dimensions.height),
      ]);
      Ok(Planes {
        y,
        cb,
        cr,
        width: dimensions.width,
        height: dimensions.height,
        cwidth: dimensions.width.wrapping_shr(1),
      })
    }
    None => Planes::gray(dimensions),
  }
}

struct DecodedFrame {
  picture: PicState,
  error: Option<Error>,
}

fn decode_picture(
  mut picture: PicState,
  slices: Vec<SliceJob>,
) -> core::result::Result<DecodedFrame, Never> {
  let mut error = None;

  for slice in &slices {
    match decode_slice(slice, &mut picture) {
      Ok(()) => {}
      Err(fault) => {
        error.get_or_insert(fault);
      }
    }
  }

  let Ok(()) = picture.flush_rows(RowFlush::Finish);
  Ok(DecodedFrame { picture, error })
}

fn recover_picture(frame: &FrameRef) -> core::result::Result<DecodedFrame, Never> {
  let Ok(planes) = Planes::gray(Size {
    width: frame.width,
    height: frame.height,
  });
  let Ok(rows) = frame.mb_h();
  let blank: Vec<_> = (0..frame.mb_w.wrapping_mul(rows).min(1_048_576))
    .map(|_| MbState::default())
    .collect();

  for row in 0..rows {
    let Ok(state) = frame.publication(row);

    match state {
      Publication::Pending => {
        let Ok(samples) = Frame::make_row(&planes, u64::from(row), &blank, &|_, _, _| None);
        let Ok(()) = frame.publish(row, samples);
      }
      Publication::Published => {}
    }
  }

  let Ok(recovered) = retain_frame(frame);
  let Ok(picture) = PicState::new(recovered);
  let error = Error::Invalid("internal error while decoding a picture".to_owned());
  Ok(DecodedFrame {
    picture,
    error: Some(error),
  })
}

fn count_picture(
  stats: &mut DecodeStats,
  blocks: &[MbState],
  quality: DecodeQuality,
) -> core::result::Result<(), Never> {
  stats.pictures = stats.pictures.wrapping_add(1);
  stats.draft_pictures = stats
    .draft_pictures
    .wrapping_add(u64::from(quality == DecodeQuality::Draft));

  for block in blocks {
    let counter = match block.kind {
      MbKind::I4x4 => &mut stats.mb_i4x4,
      MbKind::I8x8 => &mut stats.mb_i8x8,
      MbKind::I16x16 => &mut stats.mb_i16x16,
      MbKind::IPcm => &mut stats.mb_pcm,
      MbKind::PSkip => &mut stats.mb_p_skip,
      MbKind::BSkip => &mut stats.mb_b_skip,
      MbKind::BDirect16x16 => &mut stats.mb_b_direct16x16,
      MbKind::Inter => &mut stats.mb_inter,
      MbKind::None => continue,
    };
    *counter = counter.wrapping_add(1);
    let inter_8x8 = block.transform_size == TransformSize::EightByEight
      && matches!(
        MacroblockPrediction::from(block.kind),
        MacroblockPrediction::Inter
      );
    stats.mb_inter_8x8_transform = stats
      .mb_inter_8x8_transform
      .wrapping_add(u64::from(inter_8x8));
  }

  Ok(())
}

fn run_job(
  frame: FrameRef,
  slices: Vec<SliceJob>,
  shared: &Shared,
  quality: DecodeQuality,
) -> core::result::Result<(), Never> {
  let Ok(reuse) = shared.reuse_picture(&frame);
  let mut picture = match reuse {
    Some(mut picture) => {
      let Ok(reused_frame) = retain_frame(&frame);
      let Ok(()) = picture.reset(reused_frame);
      picture
    }
    None => {
      let Ok(new_frame) = retain_frame(&frame);
      let Ok(picture) = PicState::new(new_frame);
      picture
    }
  };
  picture.skip_deblock = quality == DecodeQuality::Draft;
  let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
    decode_picture(picture, slices)
  }));
  let Ok(decoded) = match execution {
    Ok(result) => result,
    Err(_picture_panic) => recover_picture(&frame),
  };
  let DecodedFrame {
    picture: completed,
    error,
  } = decoded;
  let Ok(()) = shared.with_stats(|stats| {
    let Ok(()) = count_picture(stats, &completed.mbs, quality);
  });

  match error {
    Some(error_2) => {
      let Ok(()) = shared.remember_error(error_2);
    }
    ::core::option::Option::None => {}
  }

  shared.recycle_picture(completed)
}

fn decode_slice(s: &SliceJob, pic: &mut PicState) -> Result<()> {
  let mut sd = SliceDecoder::new(&s.sh, &s.pps, &s.sps, pic, &s.refs, &s.ls)?;

  match s.pps.entropy_coding_mode {
    EntropyCoding::Cabac => sd.decode_cabac(&s.rbsp),
    EntropyCoding::Cavlc => {
      let Ok(mut r) = BitReader::new(&s.rbsp);
      let Ok(header_bits) = console_core_number_conversion::wrapped::<_, u64>(s.sh.header_bits);
      let Ok(()) = r.seek_bits(header_bits);
      sd.decode_cavlc(&mut r)
    }
  }
}

fn output_meta(
  sps: &Sps,
  pts: i64,
  refresh: PictureRefresh,
) -> core::result::Result<OutputMeta, console_core_never::Never> {
  let fallback = crate::params::Vui::default();
  let vui = match &sps.vui {
    Some(vui) => vui,
    None => &fallback,
  };

  let Ok(crop) = sps.crop_rect();

  Ok(OutputMeta {
    pts,
    refresh,
    crop,
    color: ColorInfo {
      full_range: vui.full_range,
      primaries: vui.colour_primaries,
      transfer: vui.transfer_characteristics,
      matrix: vui.matrix_coefficients,
    },
    sar: vui.sar,
    quality: DecodeQuality::Full,
  })
}

fn make_picture(o: &Output) -> core::result::Result<Picture, console_core_never::Never> {
  let (crop_x, crop_y, width, height) = o.meta.crop;
  let Ok(CroppedPlanes { y, cb: u, cr: v }) = o.frame.copy_cropped([
    u64::from(crop_x),
    u64::from(crop_y),
    u64::from(width),
    u64::from(height),
  ]);
  let chroma_width = width.div_ceil(2);
  let chroma_height = height.div_ceil(2);
  let Ok(y_stride) = console_core_number_conversion::index(width);
  let Ok(uv_stride) = console_core_number_conversion::index(chroma_width);

  Ok(Picture {
    width,
    height,
    chroma_width,
    chroma_height,
    y,
    u,
    v,
    y_stride,
    uv_stride,
    pts: o.meta.pts,
    poc: o.poc,
    key: o.meta.refresh == PictureRefresh::Idr,
    color: o.meta.color,
    sar: o.meta.sar,
    draft: o.meta.quality == DecodeQuality::Draft,
  })
}
