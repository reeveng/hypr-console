//! Top-level decoder: NAL dispatch, picture boundaries, POC / RPS / DPB management and output.
//!
//! The calling thread parses parameter sets and slice headers and does all POC, RPS and DPB
//! bookkeeping. The CTU layer of each picture runs as a job (on a thread pool with the `threads`
//! feature) that publishes finished CTB rows into the picture's shared [`Frame`]; jobs of later pictures
//! block per row on the reference data they read, so several pictures decode concurrently.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard};

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_video_decoding_bitstream::{annexb_nals, length_prefixed_nals, unescape_rbsp};

use crate::dpb::{
  Dpb, MissingFrames, Output, OutputMeta, OutputSelection, PocState, RaslPolicy, RefPicSet,
  build_ref_lists,
};
use crate::error::{Error, Result, ensure, invalid};
use crate::params::{Layout, PictureTool, Pps, ScalingList, Sps, Vps, Vui};
use crate::picture::{Completeness, Crop, Format, Frame, FrameRef, Identity, Publication};
use crate::slice::{
  NalHeader, NalKind, RandomAccess, SliceHeader, SliceType, SublayerReference, nal_type,
};
use crate::slicedec::{F_INTRA, F_SKIP, PicState, SliceDecoder, SliceJob};
use crate::{ColorInfo, DecodeQuality, Picture, Plane};

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "finite picture jobs use immutable parameter and layout snapshots while the parser can replace its current parameter sets"
  )
)]
pub(crate) type ParameterSnapshot<T> = Arc<T>;
pub(crate) type ScalingMatrices = ParameterSnapshot<Vec<Vec<u8>>>;

struct PendingPicture {
  job: PictureJob,
  last_header: SliceHeader,
  first_header: SliceHeader,
  rps: RefPicSet,
  output: OutputSelection,
  meta: OutputMeta,
}

struct PictureJob {
  frame: FrameRef,
  sps: ParameterSnapshot<Sps>,
  pps: ParameterSnapshot<Pps>,
  layout: ParameterSnapshot<Layout>,
  scaling: Option<ScalingMatrices>,
  slices: Vec<SliceJob>,
  quality: DecodeQuality,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecodeStats {
  pub pictures: u64,
  pub slices_i: u64,
  pub slices_p: u64,
  pub slices_b: u64,
  pub dependent_slices: u64,
  pub idr: u64,
  pub cra: u64,
  pub skipped_rasl: u64,
  pub intra_4x4_blocks: u64,
  pub skip_4x4_blocks: u64,
  pub inter_4x4_blocks: u64,
  pub weighted_slices: u64,
  pub tmvp_slices: u64,
  pub sao_slices: u64,
  pub deblock_disabled_slices: u64,
  pub tiles_pictures: u64,
  pub wpp_pictures: u64,
  pub scaling_list_pictures: u64,
  pub transform_skip_pictures: u64,
  pub long_term_slices: u64,
  pub missing_refs: u64,

  pub draft_pictures: u64,
}

#[derive(Default)]
struct Progress {
  error: Mutex<Option<Error>>,
  stats: Mutex<DecodeStats>,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the parser and finite picture jobs update counters and report the first available error through synchronized progress state"
  )
)]
type JobProgress = Arc<Progress>;

struct CachedLayout {
  pps: ParameterSnapshot<Pps>,
  sps: ParameterSnapshot<Sps>,
  layout: ParameterSnapshot<Layout>,
  scaling: Option<ScalingMatrices>,
}

enum Sequence {
  FirstPicture,
  Decoding,
  AfterEnd,
}

pub struct Decoder {
  vpss: [Option<Vps>; 16],
  spss: [Option<ParameterSnapshot<Sps>>; 16],
  ppss: [Option<ParameterSnapshot<Pps>>; 64],
  nal_length_size: Option<u8>,
  dpb: Dpb,
  poc_state: PocState,
  pending: Option<PendingPicture>,
  next_id: u32,
  active_sps: Option<ParameterSnapshot<Sps>>,
  sequence: Sequence,
  rasl: RaslPolicy,
  layout_cache: VecDeque<CachedLayout>,
  out_queue: VecDeque<Output>,
  progress: JobProgress,
  #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
  pool: Option<console_concurrency::FifoPool>,
  in_flight: VecDeque<FrameRef>,
  max_in_flight: u64,
  draft: DecodeQuality,
}

impl Default for Decoder {
  fn default() -> Self {
    let Ok(decoder) = Self::new();
    decoder
  }
}

fn default_threads() -> core::result::Result<u64, Never> {
  #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
  let width = match std::thread::available_parallelism() {
    Ok(available) => {
      let Ok(count) = fixed_index(available.get());
      count.min(16)
    }
    Err(_parallelism_unavailable) => 1,
  };
  #[cfg(not(all(feature = "threads", not(target_arch = "wasm32"))))]
  let width = 1;

  Ok(width)
}

impl Decoder {
  pub fn new() -> core::result::Result<Self, Never> {
    let Ok(width) = default_threads();
    let Ok(threads) = index(width);
    Self::with_threads(threads)
  }

  pub fn with_threads(
    threads: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Self, Never> {
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    let Ok(pool) = {
      let Ok(width) = console_core_number_conversion::fitted(threads);
      console_concurrency::FifoPool::for_threads("hevc", width)
    };
    let Ok(max_in_flight) = fixed_index(threads.max(1).wrapping_add(2));

    Ok(Self {
      vpss: [const { None }; 16],
      spss: [const { None }; 16],
      ppss: [const { None }; 64],
      nal_length_size: None,
      dpb: Dpb::default(),
      poc_state: PocState::default(),
      pending: None,
      next_id: 1,
      active_sps: None,
      sequence: Sequence::FirstPicture,
      rasl: RaslPolicy::Decode,
      layout_cache: VecDeque::new(),
      out_queue: VecDeque::new(),
      progress: JobProgress::new(Progress::default()),
      #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
      pool,
      in_flight: VecDeque::new(),
      max_in_flight,
      draft: DecodeQuality::Full,
    })
  }

  pub fn from_hvcc(hvcc: &[u8]) -> Result<Self> {
    let Ok(mut decoder) = Self::new();
    decoder.configure_hvcc(hvcc)?;
    Ok(decoder)
  }

  pub fn configure_hvcc(&mut self, hvcc: &[u8]) -> Result<()> {
    let (version, length_field, count, mut records) = match hvcc.split_at_checked(23) {
      Some(([version, .., length_field, count], records)) => {
        (*version, *length_field, *count, records)
      }
      Some(_) | None => return invalid("hvcC too short"),
    };
    ensure(version == 1, || {
      format!("unsupported hvcC version {version}")
    })?;
    self.nal_length_size = Some((length_field & 3).wrapping_add(1));

    for _ in 0..count {
      let nal_count = match records {
        [_, high, low, rest @ ..] => {
          records = rest;
          u16::from_be_bytes([*high, *low])
        }
        _ => return invalid("hvcC truncated"),
      };

      for _ in 0..nal_count {
        let (nal, rest) = hvcc_nal(records)?;
        self.handle_nal(nal, 0)?;
        records = rest;
      }
    }

    Ok(())
  }

  pub fn nal_length_size(
    &self,
  ) -> core::result::Result<Option<::console_core_api_compatibility::NativeSize>, Never> {
    Ok(self.nal_length_size.map(|width| {
      let Ok(native) = index(width);
      native
    }))
  }

  pub fn decode(&mut self, data: &[u8], pts: i64) -> Result<Vec<Picture>> {
    let nals = match self.nal_length_size {
      Some(width) => length_prefixed_nals(data, width)?,
      None => {
        let Ok(nals) = annexb_nals(data);
        nals
      }
    };
    let mut result = Ok(());

    for nal in nals {
      match self.handle_nal(nal, pts) {
        Err(error) => {
          result = Err(error);
          break;
        }
        Ok(()) => {}
      }
    }

    let Ok(()) = self.submit_pending();
    let Ok(error) = self.take_error();

    match error {
      Some(error_2) => result = result.and(Err(error_2)),
      None => {}
    }

    result?;

    let Ok(pics) = self.ready_pictures();
    Ok(pics)
  }

  fn ready_pictures(&mut self) -> core::result::Result<Vec<Picture>, Never> {
    let mut pictures = Vec::new();
    let max_queue = self.max_in_flight.wrapping_mul(2);
    let Ok(queued) = fixed_index(self.out_queue.len());

    for remaining in (1..=queued).rev() {
      let picture = match self.out_queue.front() {
        Some(picture) => picture,
        None => break,
      };
      let Ok(state) = picture.frame.completeness();

      match (state == Completeness::Pending && remaining <= max_queue).then_some(()) {
        Some(()) => break,
        None => {}
      }

      let picture_3 = match self.out_queue.pop_front() {
        Some(picture_2) => picture_2,
        None => break,
      };
      let Ok(picture_4) = make_picture(&picture_3);
      pictures.push(picture_4);
    }

    Ok(pictures)
  }

  pub fn flush(&mut self) -> core::result::Result<Vec<Picture>, Never> {
    let Ok(()) = self.submit_pending();
    let mut outs = Vec::new();
    let Ok(()) = self.dpb.flush(&mut outs);
    self.out_queue.extend(outs);
    let pics = self
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

    self.sequence = Sequence::FirstPicture;
    Ok(pics)
  }

  pub fn stats(&self) -> core::result::Result<DecodeStats, Never> {
    let Ok(stats) = progress_lock(&self.progress.stats);
    Ok(stats.clone())
  }

  pub fn set_quality(&mut self, quality: DecodeQuality) -> core::result::Result<(), Never> {
    self.draft = quality;
    Ok(())
  }

  pub fn take_error(&mut self) -> core::result::Result<Option<Error>, Never> {
    let Ok(mut first) = progress_lock(&self.progress.error);
    Ok(first.take())
  }

  fn handle_nal(&mut self, nal: &[u8], pts: i64) -> Result<()> {
    let payload = match nal {
      [_, _, payload @ ..] => payload,
      _ => return Ok(()),
    };
    let hdr = NalHeader::parse(nal)?;

    match hdr.layer_id {
      0 => {}
      _ => return Ok(()),
    }

    match hdr.nal_type {
      nal_type::VPS => {
        let Ok(rbsp) = unescape_rbsp(payload);
        let vps = Vps::parse(&rbsp)?;
        store_parameter(&mut self.vpss, u32::from(vps.id), vps)?;
      }
      nal_type::SPS => {
        let Ok(rbsp) = unescape_rbsp(payload);
        let sps = Sps::parse(&rbsp)?;
        store_parameter(&mut self.spss, sps.id, ParameterSnapshot::new(sps))?;
      }
      nal_type::PPS => {
        let Ok(rbsp) = unescape_rbsp(payload);
        let pps = Pps::parse(&rbsp)?;
        store_parameter(&mut self.ppss, pps.id, ParameterSnapshot::new(pps))?;
      }
      nal_type::EOS | nal_type::EOB => {
        let Ok(()) = self.submit_pending();

        match self.sequence {
          Sequence::FirstPicture => {}
          Sequence::Decoding | Sequence::AfterEnd => self.sequence = Sequence::AfterEnd,
        }
      }
      0..=9 | 16..=21 => self.handle_slice(payload, hdr, pts)?,
      _ => {}
    }

    Ok(())
  }

  fn layout_for(
    &mut self,
    pps: &ParameterSnapshot<Pps>,
    sps: &ParameterSnapshot<Sps>,
  ) -> Result<(ParameterSnapshot<Layout>, Option<ScalingMatrices>)> {
    match self.layout_cache.iter().find(|entry| {
      ParameterSnapshot::ptr_eq(&entry.pps, pps) && ParameterSnapshot::ptr_eq(&entry.sps, sps)
    }) {
      Some(cached) => {
        return Ok((
          ParameterSnapshot::clone(&cached.layout),
          cached.scaling.as_ref().map(ScalingMatrices::clone),
        ));
      }
      None => {}
    }

    let layout = Layout::new(sps, pps)?;
    let layout_2 = ParameterSnapshot::new(layout);
    let Ok(scaling) = scaling_matrices(pps, sps);

    match (self.layout_cache.len() > 8).then_some(()) {
      Some(()) => {
        self.layout_cache.pop_front();
      }
      None => {}
    }

    self.layout_cache.push_back(CachedLayout {
      pps: ParameterSnapshot::clone(pps),
      sps: ParameterSnapshot::clone(sps),
      layout: ParameterSnapshot::clone(&layout_2),
      scaling: scaling.as_ref().map(ScalingMatrices::clone),
    });

    Ok((layout_2, scaling))
  }

  fn parameters(&self, pps_id: u32) -> Result<(ParameterSnapshot<Pps>, ParameterSnapshot<Sps>)> {
    let pps = parameter(&self.ppss, pps_id, "PPS")?;
    let sps = parameter(&self.spss, pps.sps_id, "SPS")?;
    Ok((ParameterSnapshot::clone(pps), ParameterSnapshot::clone(sps)))
  }

  fn handle_slice(&mut self, payload: &[u8], hdr: NalHeader, pts: i64) -> Result<()> {
    let Ok(rbsp) = unescape_rbsp(payload);
    let first = rbsp.first().is_some_and(|byte| byte & 0x80 != 0);

    match first.then_some(()) {
      Some(()) => {
        let Ok(()) = self.submit_pending();
      }
      None => {}
    }

    let skip = matches!(hdr.kind(), NalKind::Rasl)
      && (self.rasl == RaslPolicy::Skip || matches!(self.sequence, Sequence::FirstPicture));

    match skip.then_some(()) {
      Some(()) => {
        let Ok(mut stats) = progress_lock(&self.progress.stats);
        stats.skipped_rasl = stats.skipped_rasl.wrapping_add(u64::from(first));
        return Ok(());
      }
      None => {}
    }

    let prev = self.pending.as_ref().map(|pending| &pending.last_header);
    let (header, pps, sps) = SliceHeader::parse(&rbsp, hdr, |id| self.parameters(id), prev)?;
    sps.check_supported()?;
    pps.check_supported()?;

    match (first, &self.pending) {
      (true, _) => self.start_picture(&header, &sps, &pps, pts)?,
      (false, Some(pending)) => {
        ensure(
          ParameterSnapshot::ptr_eq(&pending.job.pps, &pps),
          || "PPS changed within a picture",
        )?;
      }
      (false, None) => return invalid("slice segment without the first slice of its picture"),
    }

    let pending = match self.pending.as_mut() {
      Some(pending) => pending,
      None => return invalid("slice segment without the first slice of its picture"),
    };
    let refs = build_ref_lists(&header, &pending.rps)?;

    match header.dependent.then_some(()) {
      Some(()) => {}
      None => pending.last_header = header.clone(),
    }

    let Ok(mut stats) = progress_lock(&self.progress.stats);
    let Ok(()) = stats.count_slice(&header);
    pending.job.slices.push(SliceJob {
      sh: header,
      pps,
      sps,
      rbsp,
      refs,
    });

    Ok(())
  }

  fn start_picture(
    &mut self,
    sh: &SliceHeader,
    sps: &ParameterSnapshot<Sps>,
    pps: &ParameterSnapshot<Pps>,
    pts: i64,
  ) -> Result<()> {
    ensure(
      sh.first_slice_segment_in_pic && !sh.dependent,
      || "invalid first slice segment",
    )?;

    let format = Format::from(sps.as_ref());
    let Ok((poc, rps)) = self.picture_references(sh, sps, format);
    let (layout, scaling) = self.layout_for(pps, sps)?;
    let id = self.next_id;
    self.next_id = id.wrapping_add(1);
    let Ok(frame) = Frame::new(Identity { id, poc }, format);
    let kind = sh.nal.kind();
    let output = OutputSelection::from(
      sh.pic_output && !(matches!(kind, NalKind::Rasl) && self.rasl == RaslPolicy::Skip),
    );
    let quality = DecodeQuality::from(
      self.draft == DecodeQuality::Draft
        && matches!(sh.nal.sublayer_reference(), SublayerReference::NonReference)
        && u32::from(sh.nal.temporal_id) >= sps.max_sub_layers_minus1,
    );
    let Ok(meta) = output_meta(sps, pts, kind, quality);
    self.pending = Some(PendingPicture {
      job: PictureJob {
        frame: FrameRef::new(frame),
        sps: ParameterSnapshot::clone(sps),
        pps: ParameterSnapshot::clone(pps),
        layout,
        scaling,
        slices: Vec::new(),
        quality,
      },
      last_header: sh.clone(),
      first_header: sh.clone(),
      rps,
      output,
      meta,
    });
    self.sequence = Sequence::Decoding;
    let Ok(mut stats) = progress_lock(&self.progress.stats);
    let Ok(()) = stats.count_picture_tools(sps, pps, quality);
    Ok(())
  }

  fn picture_references(
    &mut self,
    sh: &SliceHeader,
    sps: &ParameterSnapshot<Sps>,
    format: Format,
  ) -> core::result::Result<(i32, RefPicSet), Never> {
    let mut outs = Vec::new();
    let changed = self.active_sps.as_ref().is_some_and(|previous| {
      previous.width != sps.width
        || previous.height != sps.height
        || previous.bit_depth_luma != sps.bit_depth_luma
        || previous.bit_depth_chroma != sps.bit_depth_chroma
        || previous.log2_ctb != sps.log2_ctb
    });

    match changed.then_some(()) {
      Some(()) => {
        let Ok(()) = self.dpb.flush(&mut outs);
      }
      None => {}
    }

    self.active_sps = Some(ParameterSnapshot::clone(sps));
    self.dpb.max_dec_pic_buffering = sps.max_dec_pic_buffering;
    self.dpb.max_num_reorder = sps.max_num_reorder;
    self.dpb.max_latency = match sps.max_latency_increase_plus1 {
      0 => 0,
      increase => sps.max_num_reorder.wrapping_add(increase).wrapping_sub(1),
    };

    let kind = sh.nal.kind();
    let Ok(rasl) = self.random_access(kind);
    let Ok(max_lsb) = sps.max_poc_lsb();
    let Ok(order) = self.poc_state.compute(sh, max_lsb, rasl);
    let Ok(template) = Frame::new(Identity { id: 0, poc: 0 }, format);
    let mut missing = MissingFrames {
      template,
      next_id: &mut self.next_id,
      made: 0,
    };
    let Ok(rps) = self.dpb.apply_rps(sh, order, rasl, &mut missing);

    {
      let Ok(mut stats) = progress_lock(&self.progress.stats);
      stats.missing_refs = stats.missing_refs.wrapping_add(missing.made);
    }

    match (rasl, &self.sequence) {
      (RaslPolicy::Skip, Sequence::Decoding | Sequence::AfterEnd) => {
        match (sh.no_output_of_prior_pics, kind) {
          (true, NalKind::RandomAccess(RandomAccess::Idr)) => {
            let Ok(()) = self.dpb.clear();
          }
          _ => {
            let Ok(()) = self.dpb.flush(&mut outs);
          }
        }
      }
      (RaslPolicy::Decode, _) | (RaslPolicy::Skip, Sequence::FirstPicture) => {
        let Ok(()) = self.dpb.bump_before_decode(&mut outs);
      }
    }

    self.out_queue.extend(outs);

    Ok((order.value, rps))
  }

  fn random_access(&mut self, kind: NalKind) -> core::result::Result<RaslPolicy, Never> {
    let rasl = match kind {
      NalKind::RandomAccess(RandomAccess::Idr | RandomAccess::Bla) => RaslPolicy::Skip,
      NalKind::RandomAccess(RandomAccess::Cra | RandomAccess::Reserved) => RaslPolicy::from(
        matches!(self.sequence, Sequence::FirstPicture | Sequence::AfterEnd),
      ),
      NalKind::Rasl | NalKind::Radl | NalKind::OtherVcl | NalKind::NonVcl => {
        return Ok(RaslPolicy::Decode);
      }
    };
    self.rasl = rasl;
    let Ok(mut stats) = progress_lock(&self.progress.stats);

    match kind {
      NalKind::RandomAccess(RandomAccess::Idr) => stats.idr = stats.idr.wrapping_add(1),
      NalKind::RandomAccess(RandomAccess::Bla | RandomAccess::Cra | RandomAccess::Reserved)
      | NalKind::Rasl
      | NalKind::Radl
      | NalKind::OtherVcl
      | NalKind::NonVcl => {
        stats.cra = stats.cra.wrapping_add(1);
      }
    }

    Ok(rasl)
  }

  fn submit_pending(&mut self) -> core::result::Result<(), Never> {
    let pending = match self.pending.take() {
      Some(pending) => pending,
      None => return Ok(()),
    };
    let frame = FrameRef::clone(&pending.job.frame);
    let poc = frame.poc;
    let Ok(()) = self.dispatch(pending.job);
    let Ok(()) = self.poc_state.update(&pending.first_header, poc);
    let mut outs = Vec::new();
    let Ok(()) = self.dpb.insert(
      Output {
        frame,
        poc,
        meta: pending.meta,
      },
      pending.output,
      &mut outs,
    );
    self.out_queue.extend(outs);

    Ok(())
  }

  fn dispatch(&mut self, job: PictureJob) -> core::result::Result<(), Never> {
    #[cfg(all(feature = "threads", not(target_arch = "wasm32")))]
    match &self.pool {
      Some(pool) => {
        self.in_flight.retain(|frame| {
          let Ok(state) = frame.completeness();
          state == Completeness::Pending
        });
        let Ok(count) = fixed_index(self.in_flight.len());
        let backpressure = count
          .saturating_add(1)
          .saturating_sub(self.max_in_flight.max(1));

        for _ in 0..backpressure {
          match self.in_flight.pop_front() {
            Some(previous) => {
              let Ok(()) = previous.wait_complete();
            }
            None => break,
          }
        }

        self.in_flight.push_back(FrameRef::clone(&job.frame));
        let progress = JobProgress::clone(&self.progress);
        let Ok(()) = pool.submit(move || {
          let Ok(()) = job.run(&progress);
        });
        return Ok(());
      }
      None => {}
    }

    job.run(&self.progress)
  }
}

fn hvcc_nal(records: &[u8]) -> Result<(&[u8], &[u8])> {
  let (size, payload) = match records {
    [high, low, payload @ ..] => (u16::from_be_bytes([*high, *low]), payload),
    _ => return invalid("hvcC truncated"),
  };
  let Ok(size_2) = index(size);

  match payload.split_at_checked(size_2) {
    Some(nal_and_rest) => Ok(nal_and_rest),
    None => invalid("hvcC truncated"),
  }
}

fn store_parameter<T>(slots: &mut [Option<T>], id: u32, parameter: T) -> Result<()> {
  let Ok(at) = index(id);

  match slots.get_mut(at) {
    Some(slot) => {
      *slot = Some(parameter);
      Ok(())
    }
    None => invalid("index or range outside the buffer"),
  }
}

fn parameter<'a, T>(slots: &'a [Option<T>], id: u32, label: &str) -> Result<&'a T> {
  let Ok(at) = index(id);

  match slots.get(at) {
    Some(Some(parameter)) => Ok(parameter),
    Some(None) => Err(Error::MissingParameterSet(format!("{label} {id}"))),
    None => invalid("index or range outside the buffer"),
  }
}

fn scaling_matrices(
  pps: &Pps,
  sps: &Sps,
) -> core::result::Result<Option<ScalingMatrices>, Never> {
  match sps.scaling_list_enabled {
    PictureTool::Disabled => return Ok(None),
    PictureTool::Enabled => {}
  }

  let fallback;
  let list = match pps.scaling_list.as_ref().or(sps.scaling_list.as_ref()) {
    Some(list) => list,
    None => {
      let Ok(defaults) = ScalingList::default_lists();
      fallback = defaults;
      &fallback
    }
  };
  let mut matrices = Vec::with_capacity(24);

  for size in 0..4 {
    for matrix in 0..6 {
      let Ok(factor) = list.factor(size, matrix);
      matrices.push(factor);
    }
  }

  Ok(Some(ScalingMatrices::new(matrices)))
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "a guard covers only fixed counters, a progress snapshot or the first error; workers release it before decoding, waiting or publishing"
  )
)]
fn progress_lock<T>(progress: &Mutex<T>) -> core::result::Result<MutexGuard<'_, T>, Never> {
  Ok(match progress.lock() {
    Ok(progress_2) => progress_2,
    Err(poisoned) => poisoned.into_inner(),
  })
}

impl DecodeStats {
  fn count_slice(&mut self, header: &SliceHeader) -> core::result::Result<(), Never> {
    match header.slice_type {
      SliceType::I => self.slices_i = self.slices_i.wrapping_add(1),
      SliceType::P => self.slices_p = self.slices_p.wrapping_add(1),
      SliceType::B => self.slices_b = self.slices_b.wrapping_add(1),
    }

    self.dependent_slices = self
      .dependent_slices
      .wrapping_add(u64::from(header.dependent));
    self.weighted_slices = self
      .weighted_slices
      .wrapping_add(u64::from(header.pwt.is_some()));
    self.tmvp_slices = self
      .tmvp_slices
      .wrapping_add(u64::from(header.temporal_mvp));
    self.sao_slices = self
      .sao_slices
      .wrapping_add(u64::from(header.sao_luma || header.sao_chroma));
    self.deblock_disabled_slices = self
      .deblock_disabled_slices
      .wrapping_add(u64::from(header.deblocking_disabled));
    self.long_term_slices = self
      .long_term_slices
      .wrapping_add(u64::from(!header.lt.is_empty()));
    Ok(())
  }

  fn count_picture_tools(
    &mut self,
    sps: &Sps,
    pps: &Pps,
    quality: DecodeQuality,
  ) -> core::result::Result<(), Never> {
    self.tiles_pictures = self
      .tiles_pictures
      .wrapping_add(u64::from(pps.tiles_enabled == PictureTool::Enabled));
    self.wpp_pictures = self
      .wpp_pictures
      .wrapping_add(u64::from(pps.entropy_coding_sync == PictureTool::Enabled));
    self.scaling_list_pictures = self
      .scaling_list_pictures
      .wrapping_add(u64::from(sps.scaling_list_enabled == PictureTool::Enabled));
    self.transform_skip_pictures = self
      .transform_skip_pictures
      .wrapping_add(u64::from(pps.transform_skip == PictureTool::Enabled));
    self.draft_pictures = self
      .draft_pictures
      .wrapping_add(u64::from(quality == DecodeQuality::Draft));
    Ok(())
  }
}

#[derive(Default)]
struct BlockCounts {
  intra: u64,
  skip: u64,
  inter: u64,
}

impl BlockCounts {
  fn read(picture: &PicState) -> core::result::Result<Self, Never> {
    let mut counts = Self::default();

    for block in &picture.blk {
      match (
        block.flags & F_INTRA != 0,
        block.flags & F_SKIP != 0,
        block.flags != 0,
      ) {
        (true, _, _) => counts.intra = counts.intra.wrapping_add(1),
        (false, true, _) => counts.skip = counts.skip.wrapping_add(1),
        (false, false, true) => counts.inter = counts.inter.wrapping_add(1),
        (false, false, false) => {}
      }
    }

    Ok(counts)
  }
}

impl PictureJob {
  fn run(self, progress: &Progress) -> core::result::Result<(), Never> {
    let Self {
      frame,
      pps,
      sps,
      layout,
      scaling,
      slices,
      quality,
    } = self;
    let Ok(mut pic) = PicState::new(FrameRef::clone(&frame), &sps, pps, layout, scaling);
    pic.draft = quality;
    let decode = (&mut pic, &slices);
    let result = catch_unwind(AssertUnwindSafe(move || {
      let (picture, slices_2) = decode;
      let Ok(error) = decode_slices(picture, slices_2);
      error
    }));
    let error = match result {
      Ok(error) => error,
      Err(_worker_panicked) => {
        let Ok(()) = recover_rows(&frame);
        Some(Error::Invalid(
          <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from(
            "internal error while decoding a picture",
          ),
        ))
      }
    };

    let Ok(counts) = BlockCounts::read(&pic);

    {
      let Ok(mut stats) = progress_lock(&progress.stats);
      stats.pictures = stats.pictures.wrapping_add(1);
      stats.intra_4x4_blocks = stats.intra_4x4_blocks.wrapping_add(counts.intra);
      stats.skip_4x4_blocks = stats.skip_4x4_blocks.wrapping_add(counts.skip);
      stats.inter_4x4_blocks = stats.inter_4x4_blocks.wrapping_add(counts.inter);
    }

    match error {
      Some(error_2) => {
        let Ok(mut first) = progress_lock(&progress.error);
        first.get_or_insert(error_2);
      }
      None => {}
    }

    Ok(())
  }
}

fn decode_slices(
  picture: &mut PicState,
  slices: &[SliceJob],
) -> core::result::Result<Option<Error>, Never> {
  let mut first = None;

  for slice in slices {
    match SliceDecoder::decode(picture, slice) {
      Err(error) => {
        first.get_or_insert(error);
      }
      Ok(()) => {}
    }
  }

  let Ok(()) = crate::filter::finish(picture);
  Ok(first)
}

fn recover_rows(frame: &Frame) -> core::result::Result<(), Never> {
  let Ok(rows) = frame.num_rows();

  for row in 0..rows {
    let Ok(publication) = frame.publication(row);

    match publication {
      Publication::Pending => {
        let Ok(neutral) = frame.neutral_row(row);
        let Ok(()) = frame.publish(row, neutral);
      }
      Publication::Published => {}
    }
  }

  Ok(())
}

fn output_meta(
  sps: &Sps,
  pts: i64,
  kind: NalKind,
  draft: DecodeQuality,
) -> core::result::Result<OutputMeta, console_core_never::Never> {
  let defaults = Vui::default();
  let vui = match &sps.vui {
    Some(vui) => vui,
    None => &defaults,
  };

  let Ok(crop) = sps.crop_rect();

  Ok(OutputMeta {
    pts,
    kind,
    crop,
    color: ColorInfo {
      full_range: vui.full_range,
      primaries: vui.colour_primaries,
      transfer: vui.transfer_characteristics,
      matrix: vui.matrix_coefficients,
    },
    sar: vui.sar,
    bit_depth: sps.bit_depth_luma,
    draft,
  })
}

fn make_picture(o: &Output) -> core::result::Result<Picture, console_core_never::Never> {
  let (cx, cy, cw, ch) = o.meta.crop;
  let origin = Point {
    x: u64::from(cx),
    y: u64::from(cy),
  };
  let crop = Crop {
    origin,
    size: Size {
      width: u64::from(cw),
      height: u64::from(ch),
    },
  };
  let Ok([y, u, v]) = o.frame.copy_cropped(crop);
  let (ccw, cch) = (cw.div_ceil(2), ch.div_ceil(2));
  let bd = o.meta.bit_depth;
  let conv = |p: Vec<u16>| match ::core::primitive::bool::then_some(bd <= 8, ()) {
    ::core::option::Option::Some(()) => Plane::U8(
      p.into_iter()
        .map(|v_2| {
          let Ok(numeric_value) = wrapped::<_, u8>(v_2);
          numeric_value
        })
        .collect(),
    ),
    ::core::option::Option::None => Plane::U16(p),
  };

  let Ok(y_stride) = console_core_number_conversion::index(cw);
  let Ok(uv_stride) = console_core_number_conversion::index(ccw);

  Ok(Picture {
    width: cw,
    height: ch,
    chroma_width: ccw,
    chroma_height: cch,
    bit_depth: bd,
    y: conv(y),
    u: conv(u),
    v: conv(v),
    y_stride,
    uv_stride,
    pts: o.meta.pts,
    poc: o.poc,
    key: matches!(o.meta.kind, NalKind::RandomAccess(_)),
    color: o.meta.color,
    sar: o.meta.sar,
    draft: o.meta.draft == DecodeQuality::Draft,
  })
}
