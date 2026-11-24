#![cfg_attr(not(feature = "c-ffi"), forbid(unsafe_code))]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "bitdepth_8")]
use crate::include::common::bitdepth::BitDepth8;
#[cfg(feature = "bitdepth_16")]
use crate::include::common::bitdepth::BitDepth16;
use crate::include::common::validate::validate_input;
use crate::include::dav1d::data::Rav1dData;
use crate::include::dav1d::dav1d::Rav1dDecodeFrameType;
use crate::include::dav1d::dav1d::Rav1dInloopFilterType;
use crate::include::dav1d::dav1d::Rav1dSettings;
use crate::include::dav1d::headers::Rav1dFilmGrainData;
use crate::include::dav1d::picture::Rav1dPicture;
use crate::src::cpu::rav1d_init_cpu;
use crate::src::decode::rav1d_decode_frame_exit;
use crate::src::error::Rav1dError::EAGAIN;
use crate::src::error::Rav1dError::EGeneric;
use crate::src::error::Rav1dError::EINVAL;
use crate::src::error::Rav1dResult;
use crate::src::extensions::OptionError as _;
use crate::src::fg_apply;
use crate::src::internal::Rav1dBitDepthDSPContext;
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dContextFrameThread;
use crate::src::internal::Rav1dContextTaskThread;
use crate::src::internal::Rav1dContextTaskType;
use crate::src::internal::Rav1dFrameContext;
use crate::src::internal::Rav1dState;
use crate::src::internal::Rav1dTaskContext;
use crate::src::internal::Rav1dTaskContextTaskThread;
use crate::src::internal::TaskThreadData;
use crate::src::internal::{FrameVisibilityPolicy, GrainOutputPolicy, LayerOutputPolicy};
use crate::src::iter::wrapping_iter;
use crate::src::log::Rav1dLog as _;
use crate::src::log::Rav1dLogger;
use crate::src::managed::Strictness;
use crate::src::mem::try_arc;
use crate::src::obu::rav1d_parse_obus;
use crate::src::picture::TemporalUnitChange;
use crate::src::picture::rav1d_picture_alloc_copy;
use crate::src::thread_task::rav1d_task_delayed_fg;
use crate::src::thread_task::rav1d_worker_task;
use crate::src::thread_task::{FRAME_ERROR, advance_completed_frame};
use console_core_parallel_execution::{Worker, Workers};
use console_core_synchronization::nonpoison::{Mutex, wait_while};
use std::cmp;
#[cfg(feature = "c-ffi")]
use std::ffi::CStr;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

#[cold]
fn init_internal() -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(()) = rav1d_init_cpu();
  };

  Ok(())
}

#[cfg(feature = "c-ffi")]
pub(crate) const DAV1D_VERSION: &CStr = c"966d63c1";

#[cfg(feature = "c-ffi")]
pub const DAV1D_API_VERSION_MAJOR: u8 = 7;
#[cfg(feature = "c-ffi")]
pub const DAV1D_API_VERSION_MINOR: u8 = 0;
#[cfg(feature = "c-ffi")]
pub const DAV1D_API_VERSION_PATCH: u8 = 0;

impl Default for Rav1dSettings {
  fn default() -> Self {
    Self {
      n_threads: 0,
      max_frame_delay: 0,
      apply_grain: true,
      operating_point: 0,
      all_layers: true,

      frame_size_limit: 120_000_000,
      allocator: Default::default(),
      logger: Some(Rav1dLogger::default()),

      strictness: Strictness::Lenient,
      output_invisible_frames: false,
      inloop_filters: Rav1dInloopFilterType::all(),
      decode_frame_type: Rav1dDecodeFrameType::All,
    }
  }
}

struct NumThreads {
  n_tc: u32,
  n_fc: u32,
}

#[cold]
fn get_num_threads(
  s: &Rav1dSettings,
  workers: &impl Workers,
) -> core::result::Result<NumThreads, console_core_never::Never> {
  let n_tc = match s.n_threads.cast_unsigned() {
    0 => {
      let Ok(available) = workers.parallelism();

      available.get().min(256)
    }
    requested => requested,
  };

  #[cfg(feature = "unchecked")]
  let n_fc = match s.max_frame_delay.cast_unsigned() {
    0 => {
      let root = n_tc.isqrt();
      let round_up = u32::from(root.saturating_mul(root) < n_tc);

      root.saturating_add(round_up).min(8)
    }
    requested => requested.min(n_tc),
  };
  #[cfg(not(feature = "unchecked"))]
  let n_fc = 1;

  Ok(NumThreads { n_fc, n_tc })
}

#[cfg(feature = "c-ffi")]
#[cold]
pub(crate) fn rav1d_get_frame_delay(
  s: &Rav1dSettings,
  workers: &impl Workers,
) -> Rav1dResult<u32> {
  validate_input!((s.n_threads >= 0 && s.n_threads <= 256, EINVAL))?;
  validate_input!((s.max_frame_delay >= 0 && s.max_frame_delay <= 256, EINVAL))?;
  let Ok(NumThreads { n_tc: _, n_fc }) = get_num_threads(s, workers);
  Ok(n_fc)
}

#[cold]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the decoder handle and scheduler are retained while work runs, and C picture releases may outlive the decoder through their own pool references"
  )
)]
pub(crate) fn rav1d_open(
  s: &Rav1dSettings,
  workers: &impl Workers,
) -> Rav1dResult<(Arc<Rav1dContext>, Vec<Worker>)> {
  let Ok(()) = init_internal();

  validate_input!((s.n_threads >= 0 && s.n_threads <= 256, EINVAL))?;
  validate_input!((s.max_frame_delay >= 0 && s.max_frame_delay <= 256, EINVAL))?;
  validate_input!((s.operating_point <= 31, EINVAL))?;

  #[cfg(feature = "c-ffi")]
  {
    let Ok(kind) = s.allocator.allocation_kind();
    validate_input!((
      kind == crate::src::picture::AllocationKind::Custom || s.allocator.cookie.is_none(),
      EINVAL
    ))?;
  }

  let Ok(frame_size_limit) = frame_size_limit_for_platform(s);

  let Ok(NumThreads { n_tc, n_fc }) = get_num_threads(s, workers);

  crate::include::dav1d::picture::set_tile_threading(n_tc > 1);

  rav1d_disjoint_mut::set_parallelism({
    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(n_tc);
    numeric_value
  });

  let task_thread = TaskThreadData {
    cur: crate::src::relaxed_atomic::RelaxedAtomic::from(n_fc),
    ..Default::default()
  };
  let Ok(fc) = frame_contexts(n_fc);

  let state = Mutex::new(Rav1dState {
    frame_thread: Rav1dContextFrameThread {
      out_delayed: match ::core::primitive::bool::then_some(n_fc > 1, ()) {
        ::core::option::Option::Some(()) => (0..n_fc).map(|_| Default::default()).collect(),
        ::core::option::Option::None => Box::new([]),
      },
      ..Default::default()
    },
    ..Default::default()
  });

  let tc = task_contexts(n_tc)?;
  #[cfg(feature = "c-ffi")]
  let Ok(allocator) = allocator_with_pool(s);
  #[cfg(not(feature = "c-ffi"))]
  let allocator = s.allocator.clone();

  let context = try_arc(Rav1dContext {
    allocator,
    logger: s.logger.clone(),
    grain_output: GrainOutputPolicy::from(s.apply_grain),
    operating_point: s.operating_point,
    layer_output: LayerOutputPolicy::from(s.all_layers),
    frame_size_limit,
    strictness: s.strictness,
    frame_visibility: FrameVisibilityPolicy::from(s.output_invisible_frames),
    inloop_filters: s.inloop_filters,
    decode_frame_type: s.decode_frame_type,
    fc,
    task_thread,
    state,
    tc,
    #[cfg(feature = "c-ffi")]
    c_workers: Default::default(),
    flush: Default::default(),
    dsp: Default::default(),
    stop: Default::default(),
  })?;
  let handles = start_decoder_workers(&context, workers)?;

  Ok((context, handles))
}

#[cfg(feature = "c-ffi")]
fn allocator_with_pool(
  settings: &Rav1dSettings,
) -> Result<crate::include::dav1d::picture::Rav1dPicAllocator, console_core_never::Never> {
  let mut allocator = settings.allocator.clone();

  let Ok(kind) = allocator.allocation_kind();

  match kind {
    crate::src::picture::AllocationKind::Default => {
      allocator.default_pool = Some(Default::default())
    }
    crate::src::picture::AllocationKind::Custom => {}
  }

  Ok(allocator)
}

fn frame_size_limit_for_platform(
  settings: &Rav1dSettings,
) -> Result<u32, console_core_never::Never> {
  const MAXIMUM_SMALL_ADDRESS_SPACE: u32 = 67_108_864;
  let requested = settings.frame_size_limit;
  let small_address_space = cfg!(any(
    target_pointer_width = "16",
    target_pointer_width = "32"
  ));

  match (small_address_space && requested.wrapping_sub(1) >= MAXIMUM_SMALL_ADDRESS_SPACE)
    .then_some(())
  {
    Some(()) => {
      match requested {
        0 => {}
        1.. => {
          writeln!(
            settings.logger,
            "Frame size limit reduced from {} to {}.",
            requested, MAXIMUM_SMALL_ADDRESS_SPACE,
          );
        }
      }

      Ok(MAXIMUM_SMALL_ADDRESS_SPACE)
    }
    None => Ok(requested),
  }
}

fn frame_contexts(count: u32) -> Result<Box<[Rav1dFrameContext]>, console_core_never::Never> {
  Ok(
    (0..count)
      .map(|number| {
        let Ok(mut frame) = Rav1dFrameContext::default(number);
        frame.task_thread.finished = AtomicBool::new(true);
        frame.data.get_mut().lf.last_sharpness = u8::MAX;

        frame
      })
      .collect(),
  )
}

fn task_contexts(count: u32) -> Rav1dResult<Box<[Rav1dContextTaskThread]>> {
  let mut tasks = Vec::new();

  for _task_number in 0..count {
    let thread_data = Rav1dTaskContextTaskThread::default();
    let task = match count {
      1 => {
        let scratch = crate::src::internal::TaskContextScratch::allocate()?;
        let Ok(local) = Rav1dTaskContext::new(scratch);

        Rav1dContextTaskType::Single(Mutex::new(Box::new(local)))
      }
      _worker_count => Rav1dContextTaskType::Worker,
    };
    tasks.push(Rav1dContextTaskThread { task, thread_data });
  }

  Ok(tasks.into_boxed_slice())
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "a running decoder worker retains the decoder owning its scheduler and worker slot until it exits"
  )
)]
fn start_decoder_workers(
  context: &Arc<Rav1dContext>,
  workers: &impl Workers,
) -> Rav1dResult<Vec<Worker>> {
  let mut handles = Vec::new();

  for (index, task) in (0_u32..).zip(&context.tc) {
    match &task.task {
      Rav1dContextTaskType::Single(_local) => {}
      Rav1dContextTaskType::Worker => {
        let retained = Arc::clone(context);
        let started = workers.start(format!("rav1d-worker-{index}"), move || {
          let Ok(()) = rav1d_worker_task(&retained, index);
        });

        match started {
          Ok(worker) => handles.push(worker),
          Err(_unstarted) => {
            let Ok(()) = context.stop_workers(handles);

            return Err(EGeneric);
          }
        }
      }
    }
  }

  Ok(handles)
}

enum GrainPresence {
  Present,
  Absent,
}

impl From<bool> for GrainPresence {
  fn from(present: bool) -> Self {
    match present {
      true => Self::Present,
      false => Self::Absent,
    }
  }
}

impl Rav1dFilmGrainData {
  #[must_use]
  fn grain(&self) -> Result<GrainPresence, console_core_never::Never> {
    Ok(match (self.num_y_points, self.num_uv_points) {
      (0, [0, 0]) => {
        GrainPresence::from(self.clip_to_restricted_range && self.chroma_scaling_from_luma)
      }
      _ => GrainPresence::Present,
    })
  }
}

impl Rav1dPicture {
  #[must_use]
  fn grain(&self) -> Rav1dResult<GrainPresence> {
    let header = self.frame_hdr.as_ref().ok_or(EINVAL)?;
    let Ok(grain) = header.film_grain.data.grain();

    Ok(grain)
  }
}

#[derive(Clone, Copy)]
enum OutputLayer {
  Current,
  CachedSpatial,
}

impl From<bool> for OutputLayer {
  fn from(use_spatial_cache: bool) -> Self {
    match use_spatial_cache {
      true => Self::CachedSpatial,
      false => Self::Current,
    }
  }
}

#[must_use]
fn output_layer(
  c: &Rav1dContext,
  state: &Rav1dState,
) -> Result<OutputLayer, console_core_never::Never> {
  Ok(OutputLayer::from(
    c.layer_output == LayerOutputPolicy::HighestSpatial && state.max_spatial_id != 0,
  ))
}

fn output_image(c: &Rav1dContext, state: &mut Rav1dState, out: &mut Rav1dPicture) -> Rav1dResult {
  let Ok(layer) = output_layer(c, state);
  let picture = match layer {
    OutputLayer::Current => &mut state.out,
    OutputLayer::CachedSpatial => &mut state.cache,
  };

  let grain = match c.grain_output {
    GrainOutputPolicy::Synthesized => picture.p.grain(),
    GrainOutputPolicy::Original => Ok(GrainPresence::Absent),
  };
  let result = match grain {
    Ok(GrainPresence::Present) => rav1d_apply_grain(c, out, &picture.p),
    Ok(GrainPresence::Absent) => {
      *out = mem::take(&mut picture.p);
      Ok(())
    }
    Err(header_failure) => Err(header_failure),
  };

  let _ = mem::take(picture);

  match (layer, &state.out.p.data) {
    (OutputLayer::CachedSpatial, Some(_)) => state.cache = mem::take(&mut state.out),
    _ => {}
  }

  result
}

enum OutputPhase {
  Decoding,
  Draining,
}

enum PictureOutput {
  Ready,
  Pending,
}

fn picture_output(
  c: &Rav1dContext,
  state: &mut Rav1dState,
  phase: OutputPhase,
) -> Rav1dResult<PictureOutput> {
  match state.cached_error {
    Some(_pending_decode_error) => return Ok(PictureOutput::Ready),
    None => {}
  }

  let Ok(layer) = output_layer(c, state);

  Ok(match layer {
    OutputLayer::Current => match state.out.p.data {
      Some(_) => PictureOutput::Ready,
      None => PictureOutput::Pending,
    },
    OutputLayer::CachedSpatial => match (&state.out.p.data, &state.cache.p.data, phase) {
      (Some(_), Some(_), _) => {
        let cached_header = state.cache.p.frame_hdr.as_ref().ok_or(EINVAL)?;
        let cached_id = cached_header.spatial_id;
        let Ok(temporal_unit) = state.out.flags.temporal_unit();

        match (state.max_spatial_id.cmp(&cached_id), temporal_unit) {
          (std::cmp::Ordering::Equal, _) | (_, TemporalUnitChange::New) => PictureOutput::Ready,
          (
            std::cmp::Ordering::Less | std::cmp::Ordering::Greater,
            TemporalUnitChange::Unchanged,
          ) => {
            state.cache = mem::take(&mut state.out);
            PictureOutput::Pending
          }
        }
      }
      (None, Some(_), OutputPhase::Draining) => PictureOutput::Ready,
      (Some(_), None, _) => {
        state.cache = mem::take(&mut state.out);
        PictureOutput::Pending
      }
      (None, _, _) => PictureOutput::Pending,
    },
  })
}

enum DrainScan {
  NoFrames,
  FramesFound,
}

fn drain_picture(
  decoder: &Rav1dContext,
  state: &mut Rav1dState,
  output: &mut Rav1dPicture,
) -> Rav1dResult {
  let mut drained = DrainScan::NoFrames;
  let Ok(frame_count) = console_core_number_conversion::wrapped::<_, u32>(decoder.fc.len());

  for _frame in &*decoder.fc {
    let next = state.frame_thread.next;
    let Ok(index) = console_core_number_conversion::index(next);
    let frame = decoder.fc.get(index).ok_or(EINVAL)?;
    let control = &decoder.task_thread;
    let waiting = control.lock.lock();
    let scheduler = wait_while(&frame.task_thread.cond, waiting, |_schedule| {
      !frame.task_thread.finished.load(Ordering::SeqCst)
        && !control.panicked.load(Ordering::SeqCst)
    });

    match frame
      .task_thread
      .finished
      .load(Ordering::SeqCst)
      .then_some(())
    {
      Some(()) => {}
      None => return Err(EGeneric),
    }

    let delayed = state
      .frame_thread
      .out_delayed
      .get_mut(index)
      .ok_or(EINVAL)?;
    let mut result = frame.task_thread.retval.try_lock().ok_or(EAGAIN)?;
    let has_pending_output =
      delayed.p.data.is_some() || frame.task_thread.error.load(Ordering::SeqCst) != 0;

    match has_pending_output.then_some(()) {
      Some(()) => {
        let Ok(()) = advance_completed_frame(control, frame_count);
        drained = DrainScan::FramesFound;
      }
      None => match drained {
        DrainScan::FramesFound => break,
        DrainScan::NoFrames => {}
      },
    }

    let following = next
      .wrapping_add(1)
      .checked_rem(frame_count)
      .ok_or(EINVAL)?;
    state.frame_thread.next = following;
    drop(scheduler);
    let fault = mem::take(&mut *result);
    drop(result);

    match fault {
      Some(decode_failure) => {
        state.cached_error_props = delayed.p.m.clone();
        drop(mem::take(delayed));

        return Err(decode_failure);
      }
      None => {}
    }

    match delayed.p.data {
      Some(_) => {
        let Ok(counters) = delayed.progress();
        let [_, final_progress] = counters.ok_or(EINVAL)?;
        let progress = final_progress.load(Ordering::Relaxed);
        let completed = mem::take(delayed);

        use crate::src::picture::FramePresentation;
        let visible = matches!(
          completed.presentation,
          FramePresentation::Visible | FramePresentation::VisibleShowable
        );

        match ((visible || decoder.frame_visibility == FrameVisibilityPolicy::IncludeInvisible)
          && progress != FRAME_ERROR)
          .then_some(())
        {
          Some(()) => {
            let flags = crate::include::dav1d::dav1d::Rav1dEventFlags::from(completed.flags);
            state.out = completed;
            state.event_flags |= flags;
          }
          None => drop(completed),
        }

        let pending = picture_output(decoder, state, OutputPhase::Decoding)?;

        match pending {
          PictureOutput::Ready => return output_image(decoder, state, output),
          PictureOutput::Pending => {}
        }
      }
      None => {}
    }
  }

  let pending = picture_output(decoder, state, OutputPhase::Draining)?;

  match pending {
    PictureOutput::Ready => output_image(decoder, state, output),
    PictureOutput::Pending => Err(EAGAIN),
  }
}

fn gen_picture(c: &Rav1dContext, state: &mut Rav1dState) -> Rav1dResult {
  let pending = picture_output(c, state, OutputPhase::Decoding)?;

  match pending {
    PictureOutput::Ready => {
      return Ok(());
    }
    PictureOutput::Pending => {}
  }

  let Rav1dData {
    data: queued,
    m: props,
  } = mem::take(&mut state.in_0);
  let mut packet = match queued {
    Some(input) => input,
    None => return Ok(()),
  };
  let Ok(packet_length) = console_core_number_conversion::fixed_index(packet.len());

  for _obu in 0..packet_length {
    let Ok(remaining) = console_core_number_conversion::fixed_index(packet.len());

    match remaining {
      0 => return Ok(()),
      1.. => {}
    }

    let parsed = match rav1d_parse_obus(c, state, &packet, &props) {
      Ok(consumed) => {
        packet.slice_in_place(consumed..remaining)?;

        Ok(consumed)
      }
      Err(fault) => Err(fault),
    };
    let output = picture_output(c, state, OutputPhase::Decoding)?;

    match output {
      PictureOutput::Ready => {
        match packet.is_empty().then_some(()) {
          Some(()) => {}
          None => {
            state.in_0 = Rav1dData {
              data: Some(packet),
              m: props,
            };
          }
        }

        return Ok(());
      }
      PictureOutput::Pending => {}
    }

    let consumed = parsed?;

    match consumed {
      0 => return Err(EINVAL),
      1.. => {}
    }
  }

  Ok(())
}

pub(crate) fn rav1d_send_data(c: &Rav1dContext, in_0: &mut Rav1dData) -> Rav1dResult {
  let mut state = c.state.try_lock().ok_or(EAGAIN)?;

  match &in_0.data {
    Some(data) => {
      validate_input!((!data.is_empty(), EINVAL))?;
      let layout =
        core::alloc::Layout::array::<u8>(data.len()).map_err(|_outside_address_space| EINVAL);
      let _checked_layout = validate_input!(layout)?;
      state.drain = crate::src::internal::DrainRequest::Idle;
    }
    None => {}
  }

  match ::core::primitive::bool::then_some(state.in_0.data.is_some(), ()) {
    ::core::option::Option::Some(()) => {
      return Err(EAGAIN);
    }
    ::core::option::Option::None => {}
  }

  state.in_0 = in_0.clone();
  let res = gen_picture(c, &mut state);

  match res {
    ::core::result::Result::Ok(_) => {
      let _ = mem::take(in_0);
    }
    Err(fault) => return Err(fault),
  }

  res
}

pub(crate) fn rav1d_get_picture(c: &Rav1dContext, out: &mut Rav1dPicture) -> Rav1dResult {
  let mut state = c.state.try_lock().ok_or(EAGAIN)?;
  use crate::src::internal::DrainRequest;

  let drain = mem::replace(&mut state.drain, DrainRequest::Requested);
  gen_picture(c, &mut state)?;
  mem::take(&mut state.cached_error).err_or(())?;

  let phase = match c.fc.len() {
    1 => OutputPhase::Draining,
    _ => OutputPhase::Decoding,
  };

  let pending = picture_output(c, &mut state, phase)?;

  match pending {
    PictureOutput::Ready => {
      return output_image(c, &mut state, out);
    }
    PictureOutput::Pending => {}
  }

  match (c.fc.len(), drain) {
    (2.., DrainRequest::Requested) => return drain_picture(c, &mut state, out),
    _ => {}
  }

  Err(EAGAIN)
}

pub(crate) fn rav1d_apply_grain(
  c: &Rav1dContext,
  out: &mut Rav1dPicture,
  in_0: &Rav1dPicture,
) -> Rav1dResult {
  let grain = in_0.grain()?;

  match grain {
    GrainPresence::Absent => {
      *out = in_0.clone();
      return Ok(());
    }
    GrainPresence::Present => {}
  }

  let allocated = rav1d_picture_alloc_copy(&c.logger, out, in_0.p.w, in_0);

  match allocated {
    Ok(()) => {}
    Err(fault) => {
      drop(mem::take(out));

      return Err(fault);
    }
  }

  let bit_depth = out.p.bpc;
  let Ok(depth_context) = Rav1dBitDepthDSPContext::get(bit_depth);
  let dsp = match depth_context {
    Some(context) => context,
    None => {
      drop(mem::take(out));

      return Err(EINVAL);
    }
  };

  let applied = match c.tc.len() {
    2.. => {
      let result = rav1d_task_delayed_fg(c, out, in_0);

      match c.task_thread.panicked.load(Ordering::SeqCst).then_some(()) {
        Some(()) => Err(EGeneric),
        None => result,
      }
    }
    _single_worker => match bit_depth {
      #[cfg(feature = "bitdepth_8")]
      8 => fg_apply::rav1d_apply_grain::<BitDepth8>(&dsp.fg, out, in_0),
      #[cfg(feature = "bitdepth_16")]
      10 | 12 => fg_apply::rav1d_apply_grain::<BitDepth16>(&dsp.fg, out, in_0),
      _unsupported_depth => Err(EINVAL),
    },
  };

  match applied {
    Ok(()) => {}
    Err(error) => {
      drop(mem::take(out));
      return Err(error);
    }
  }

  Ok(())
}

pub(crate) fn rav1d_flush(
  c: &Rav1dContext,
) -> core::result::Result<(), console_core_never::Never> {
  {
    #[cfg_attr(
      dylint_lib = "explicit004_no_panic",
      expect(
        explicit004_no_panic,
        reason = "flush requires exclusive decoder API access, and decoder workers never take the API state lock"
      )
    )]
    let state = &mut *c.state.try_lock().unwrap();

    let old_state = mem::take(state);
    state.tiles = old_state.tiles;
    state.n_tiles = old_state.n_tiles;
    state.frame_thread = old_state.frame_thread;
    state.operating_point_idc = old_state.operating_point_idc;
    state.max_spatial_id = old_state.max_spatial_id;
    state.frame_flags = old_state.frame_flags;
    state.event_flags = old_state.event_flags;

    match ::core::primitive::bool::then_some(c.fc.len() == 1 && c.tc.len() == 1, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    c.flush.store(true, Ordering::SeqCst);

    match ::core::primitive::bool::then_some(c.tc.len() > 1, ()) {
      ::core::option::Option::Some(()) => {
        let mut task_thread_lock = c.task_thread.lock.lock();

        for task in c.tc.iter() {
          task_thread_lock = wait_while(&task.thread_data.cond, task_thread_lock, |_scheduler| {
            matches!(
              task.flush_state(),
              Ok(crate::src::internal::FlushState::Waiting)
            )
          });
        }

        for fc in c.fc.iter() {
          let Ok(()) = fc.task_thread.tasks.clear();
        }

        c.task_thread.first.store(0, Ordering::SeqCst);
        let Ok(()) = c.task_thread.cur.set({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(c.fc.len());
          numeric_value
        });
        c.task_thread.reset_task_cur.store(None, Ordering::SeqCst);
        c.task_thread.cond_signaled.store(0, Ordering::SeqCst);
      }
      ::core::option::Option::None => {}
    }

    match c.fc.len() {
      2.. => {
        let Ok(()) = discard_flushed_frames(c, state);
      }
      _single_frame => {}
    }

    c.flush.store(false, Ordering::SeqCst);
  };

  Ok(())
}

fn discard_flushed_frames(
  c: &Rav1dContext,
  state: &mut Rav1dState,
) -> Result<(), console_core_never::Never> {
  let Ok(frames) = wrapping_iter(c.fc.iter(), u64::from(state.frame_thread.next));

  for fc in frames {
    let _flushed_frame_result = rav1d_decode_frame_exit(c, fc, Err(EGeneric));
    #[cfg_attr(
      dylint_lib = "explicit004_no_panic",
      expect(
        explicit004_no_panic,
        reason = "all worker tasks have acknowledged flush before their frame result slots are reset"
      )
    )]
    let mut frame_result = fc.task_thread.retval.try_lock().unwrap();
    *frame_result = None;
    drop(frame_result);

    let Ok(delayed_slot) = console_core_number_conversion::index(fc.index);

    match state.frame_thread.out_delayed.get_mut(delayed_slot) {
      Some(delayed) => match delayed.p.frame_hdr {
        Some(_) => drop(mem::take(delayed)),
        None => {}
      },
      None => console_core_logging::error!(
        "AV1 flush found an invalid delayed frame index: {}",
        fc.index
      ),
    }
  }

  state.frame_thread.next = 0;

  Ok(())
}

#[cfg(any(feature = "c-ffi", test))]
#[cold]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "closing consumes the C handle's final external reference after joining workers that owned temporary context references"
  )
)]
pub(crate) fn rav1d_close(
  c: Arc<Rav1dContext>,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let c_2 = &*c;
    let Ok(()) = rav1d_flush(c_2);
    #[cfg(feature = "c-ffi")]
    let workers = mem::take(&mut *c_2.c_workers.lock());
    #[cfg(not(feature = "c-ffi"))]
    let workers = Vec::new();
    let Ok(()) = c_2.stop_workers(workers);
  };

  Ok(())
}

impl Rav1dContext {
  pub(crate) fn stop_workers(
    &self,
    workers: Vec<Worker>,
  ) -> Result<(), console_core_never::Never> {
    let Ok(()) = self.tell_worker_threads_to_die();

    for worker in workers {
      match worker.join() {
        Ok(()) => {}
        Err(join_failure) => {
          console_core_logging::error!("AV1 worker failed to join: {join_failure:?}")
        }
      }
    }

    Ok(())
  }

  fn tell_worker_threads_to_die(&self) -> core::result::Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(self.tc.is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let ttd = &self.task_thread;
      let _task_thread_lock = ttd.lock.lock();

      for tc in self.tc.iter() {
        let Ok(()) = tc.thread_data.die.set(true);
      }

      ttd.cond.notify_all();
    };

    Ok(())
  }
}
