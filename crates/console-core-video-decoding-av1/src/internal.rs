use crate::include::common::bitdepth::BPC;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::bitdepth::BitDepth8;
use crate::include::common::bitdepth::BitDepth16;
use crate::include::dav1d::common::Rav1dDataProps;
use crate::include::dav1d::data::Rav1dData;
use crate::include::dav1d::dav1d::Rav1dDecodeFrameType;
use crate::include::dav1d::dav1d::Rav1dEventFlags;
use crate::include::dav1d::dav1d::Rav1dInloopFilterType;
use crate::include::dav1d::headers::DRav1d;
use crate::include::dav1d::headers::Dav1dFrameHeader;
use crate::include::dav1d::headers::Dav1dSequenceHeader;
use crate::include::dav1d::headers::Rav1dContentLightLevel;
use crate::include::dav1d::headers::Rav1dFrameHeader;
use crate::include::dav1d::headers::Rav1dITUTT35;
use crate::include::dav1d::headers::Rav1dMasteringDisplay;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dSequenceHeader;
use crate::include::dav1d::headers::Rav1dWarpedMotionParams;
use crate::include::dav1d::picture::Rav1dPicAllocator;
use crate::include::dav1d::picture::Rav1dPicture;
use crate::src::align::Align16;
use crate::src::align::Align64;
use crate::src::align::AlignedVec64;
use crate::src::align::ArrayDefault;
use crate::src::cdef::Rav1dCdefDSPContext;
use crate::src::cdf::CdfContext;
use crate::src::cdf::CdfThreadContext;
use crate::src::cpu::CpuFlags;
use crate::src::cpu::rav1d_get_cpu_flags;
use crate::src::disjoint_mut::DisjointImmutGuard;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::disjoint_mut::DisjointMutArcSlice;
use crate::src::disjoint_mut::DisjointMutGuard;
use crate::src::env::BlockContext;
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::filmgrain::GRAIN_HEIGHT;
use crate::src::filmgrain::GRAIN_WIDTH;
use crate::src::filmgrain::Rav1dFilmGrainDSPContext;
use crate::src::ipred::Rav1dIntraPredDSPContext;
use crate::src::itx::Rav1dInvTxfmDSPContext;
use crate::src::levels::Av1Block;
use crate::src::levels::Filter2d;
use crate::src::levels::Mv;
use crate::src::levels::SegmentId;
use crate::src::levels::TxfmType;
use crate::src::levels::WHT_WHT;
use crate::src::lf_mask::Av1Filter;
use crate::src::lf_mask::Av1FilterLUT;
use crate::src::lf_mask::Av1Restoration;
use crate::src::lf_mask::Av1RestorationUnit;
use crate::src::log::Rav1dLogger;
use crate::src::loopfilter::Rav1dLoopFilterDSPContext;
use crate::src::looprestoration::Rav1dLoopRestorationDSPContext;
use crate::src::lr_apply::LrRestorePlanes;
use crate::src::managed::Strictness;
use crate::src::mc::Rav1dMCDSPContext;
use crate::src::msac::MsacContext;
use crate::src::msac::Rav1dMsacDSPContext;
use crate::src::pal::Rav1dPalDSPContext;
use crate::src::picture::PictureFlags;
use crate::src::picture::Rav1dThreadPicture;
use crate::src::recon::BackupIpredEdgeFn;
use crate::src::recon::CopyPalBlockFn;
use crate::src::recon::FilterSbrowFn;
use crate::src::recon::ReadCoefBlocksFn;
use crate::src::recon::ReadPalFn;
use crate::src::recon::ReconBInterFn;
use crate::src::recon::ReconBIntraFn;
use crate::src::recon::rav1d_backup_ipred_edge;
use crate::src::recon::rav1d_copy_pal_block;
use crate::src::recon::rav1d_filter_sbrow;
use crate::src::recon::rav1d_filter_sbrow_cdef;
use crate::src::recon::rav1d_filter_sbrow_deblock_cols;
use crate::src::recon::rav1d_filter_sbrow_deblock_rows;
use crate::src::recon::rav1d_filter_sbrow_lr;
use crate::src::recon::rav1d_filter_sbrow_resize;
use crate::src::recon::rav1d_read_coef_blocks;
use crate::src::recon::rav1d_read_pal_uv;
use crate::src::recon::rav1d_read_pal_y;
use crate::src::recon::rav1d_recon_b_inter;
use crate::src::recon::rav1d_recon_b_intra;
use crate::src::refmvs::Rav1dRefmvsDSPContext;
use crate::src::refmvs::RefMvsFrame;
use crate::src::refmvs::RefMvsTemporalBlock;
use crate::src::refmvs::RefmvsTile;
use crate::src::refmvs::read_spatial_block;
use crate::src::relaxed_atomic::RelaxedAtomic;
use crate::src::thread_task::Rav1dTaskIndex;
use crate::src::thread_task::Rav1dTasks;
use atomig::Atom;
use atomig::Atomic;
use console_core_geometry::{Point, Size};
use console_core_synchronization::nonpoison::{Mutex, RwLock, RwLockReadGuard};
use console_core_video_av1_digital_signal_processing::{MotionCompensation, MotionScratch};
use std::ffi::c_int;
use std::ffi::c_uint;
use std::mem;
use std::ops::Deref;
use std::ops::Range;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar};
use zerocopy::FromBytes;
use zerocopy::FromZeros;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

#[derive(Default)]
pub struct Rav1dDSPContext {
  pub pal: Rav1dPalDSPContext,
  pub refmvs: Rav1dRefmvsDSPContext,
  pub msac: Rav1dMsacDSPContext,
}

impl Rav1dDSPContext {
  pub const fn default() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let Ok(pal) = Rav1dPalDSPContext::default();
      let Ok(refmvs) = Rav1dRefmvsDSPContext::default();
      let Ok(msac) = Rav1dMsacDSPContext::default();

      Self { pal, refmvs, msac }
    })
  }

  pub const fn new(flags: CpuFlags) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let Ok(pal) = Rav1dPalDSPContext::new(flags);
      let Ok(refmvs) = Rav1dRefmvsDSPContext::new(flags);
      let Ok(msac) = Rav1dMsacDSPContext::new(flags);

      Self { pal, refmvs, msac }
    })
  }

  pub fn get() -> core::result::Result<&'static Self, console_core_never::Never> {
    Ok({
      #[cfg_attr(
        dylint_lib = "explicit044_no_ambient_value",
        expect(
          explicit044_no_ambient_value,
          reason = "the shared table publishes dispatch function pointers once and the initialized context is immutable"
        )
      )]
      #[cfg_attr(
        dylint_lib = "explicit068_no_global_mutable_state",
        expect(
          explicit068_no_global_mutable_state,
          reason = "the shared table publishes dispatch function pointers once and the initialized context is immutable"
        )
      )]
      static DSP: OnceLock<Rav1dDSPContext> = OnceLock::new();
      DSP.get_or_init(|| {
        let Ok(flags) = rav1d_get_cpu_flags();

        let Ok(rav1d_dsp_context) = Self::new(flags);

        rav1d_dsp_context
      })
    })
  }
}

impl Default for &'static Rav1dDSPContext {
  fn default() -> Self {
    {
      let Ok(explicit_value) = Rav1dDSPContext::get();
      explicit_value
    }
  }
}

pub(crate) struct Rav1dBitDepthDSPContext {
  pub fg: Rav1dFilmGrainDSPContext,
  pub ipred: Rav1dIntraPredDSPContext,
  pub mc: Rav1dMCDSPContext,
  pub itx: Rav1dInvTxfmDSPContext,
  pub lf: Rav1dLoopFilterDSPContext,
  pub cdef: Rav1dCdefDSPContext,
  pub lr: Rav1dLoopRestorationDSPContext,
}

impl Rav1dBitDepthDSPContext {
  pub const fn _default<BD: BitDepth>() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let Ok(fg) = Rav1dFilmGrainDSPContext::default::<BD>();
      let Ok(ipred) = Rav1dIntraPredDSPContext::default::<BD>();
      let Ok(mc) = Rav1dMCDSPContext::default::<BD>();
      let Ok(itx) = Rav1dInvTxfmDSPContext::default::<BD>();
      let Ok(lf) = Rav1dLoopFilterDSPContext::default::<BD>();
      let Ok(cdef) = Rav1dCdefDSPContext::default::<BD>();
      let Ok(lr) = Rav1dLoopRestorationDSPContext::default::<BD>();

      Self {
        fg,
        ipred,
        mc,
        itx,
        lf,
        cdef,
        lr,
      }
    })
  }

  pub const fn new<BD: BitDepth>(
    flags: CpuFlags,
    bpc: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let Ok(fg) = Rav1dFilmGrainDSPContext::new::<BD>(flags);
      let Ok(ipred) = Rav1dIntraPredDSPContext::new::<BD>(flags);
      let Ok(mc) = Rav1dMCDSPContext::new::<BD>(flags);
      let Ok(itx) = Rav1dInvTxfmDSPContext::new::<BD>(flags, bpc);
      let Ok(lf) = Rav1dLoopFilterDSPContext::new::<BD>(flags);
      let Ok(cdef) = Rav1dCdefDSPContext::new::<BD>(flags);
      let Ok(lr) = Rav1dLoopRestorationDSPContext::new::<BD>(flags, bpc);

      Self {
        fg,
        ipred,
        mc,
        itx,
        lf,
        cdef,
        lr,
      }
    })
  }

  fn for_8bit() -> Result<&'static Self, console_core_never::Never> {
    #[cfg_attr(
      dylint_lib = "explicit044_no_ambient_value",
      expect(
        explicit044_no_ambient_value,
        reason = "this once-initialized table shares the immutable 8-bit dispatch function pointers"
      )
    )]
    #[cfg_attr(
      dylint_lib = "explicit068_no_global_mutable_state",
      expect(
        explicit068_no_global_mutable_state,
        reason = "this once-initialized table shares the immutable 8-bit dispatch function pointers"
      )
    )]
    static BPC8: OnceLock<Rav1dBitDepthDSPContext> = OnceLock::new();
    Ok(BPC8.get_or_init(|| {
      let Ok(flags) = rav1d_get_cpu_flags();
      let Ok(context) = Self::new::<BitDepth8>(flags, 8);
      context
    }))
  }

  pub fn get(bpc: u8) -> core::result::Result<Option<&'static Self>, console_core_never::Never> {
    Ok({
      #[cfg_attr(
        dylint_lib = "explicit044_no_ambient_value",
        expect(
          explicit044_no_ambient_value,
          reason = "this once-initialized table shares the immutable 10-bit dispatch function pointers"
        )
      )]
      #[cfg_attr(
        dylint_lib = "explicit068_no_global_mutable_state",
        expect(
          explicit068_no_global_mutable_state,
          reason = "this once-initialized table shares the immutable 10-bit dispatch function pointers"
        )
      )]
      static BPC10: OnceLock<Rav1dBitDepthDSPContext> = OnceLock::new();
      #[cfg_attr(
        dylint_lib = "explicit044_no_ambient_value",
        expect(
          explicit044_no_ambient_value,
          reason = "this once-initialized table shares the immutable 12-bit dispatch function pointers"
        )
      )]
      #[cfg_attr(
        dylint_lib = "explicit068_no_global_mutable_state",
        expect(
          explicit068_no_global_mutable_state,
          reason = "this once-initialized table shares the immutable 12-bit dispatch function pointers"
        )
      )]
      static BPC12: OnceLock<Rav1dBitDepthDSPContext> = OnceLock::new();
      Some(match bpc {
        8 => {
          let Ok(context) = Self::for_8bit();
          context
        }
        10 => BPC10.get_or_init(|| {
          let Ok(flags) = rav1d_get_cpu_flags();

          let Ok(rav1d_bit_depth_dsp_context) = Self::new::<BitDepth16>(flags, bpc);

          rav1d_bit_depth_dsp_context
        }),
        12 => BPC12.get_or_init(|| {
          let Ok(flags) = rav1d_get_cpu_flags();

          let Ok(rav1d_bit_depth_dsp_context) = Self::new::<BitDepth16>(flags, bpc);

          rav1d_bit_depth_dsp_context
        }),
        _ => return Ok(None),
      })
    })
  }
}

impl Default for &'static Rav1dBitDepthDSPContext {
  fn default() -> Self {
    let Ok(context) = Rav1dBitDepthDSPContext::for_8bit();
    context
  }
}

#[derive(Clone, Default)]
pub(crate) struct Rav1dTileGroupHeader {
  pub start: c_int,
  pub end: c_int,
}

#[derive(Clone, Default)]
pub(crate) struct Rav1dTileGroup {
  pub data: Rav1dData,
  pub hdr: Rav1dTileGroupHeader,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum TaskType {
  #[default]
  Initial = 0b00000,
  InitCdf = 0b00011,
  TileEntropy = 0b00101,
  EntropyProgress = 0b00110,
  TileReconstruction = 0b01001,
  DeblockCols = 0b01010,
  DeblockRows = 0b01100,
  Cdef = 0b01111,
  SuperResolution = 0b10001,
  LoopRestoration = 0b10010,
  ReconstructionProgress = 0b10100,
}

#[derive(Default)]
#[repr(C)]
pub(crate) struct Rav1dContextFrameThread {
  pub out_delayed: Box<[Rav1dThreadPicture]>,
  pub next: c_uint,
}

pub type GrainLut<Entry> =
  [[Entry; match console_core_number_conversion::const_u8_index(GRAIN_WIDTH) {
    Ok(width) => width,
  }]; match console_core_number_conversion::const_u8_index(GRAIN_HEIGHT.wrapping_add(1)) {
    Ok(height) => height,
  }];

pub struct GrainBD<BD: BitDepth> {
  pub grain_lut: Align16<[GrainLut<BD::Entry>; 3]>,

  pub scaling: Align64<[BD::Scaling; 3]>,
}

impl<BD: BitDepth> Default for GrainBD<BD> {
  fn default() -> Self {
    Self {
      grain_lut: ArrayDefault::default(),
      scaling: ArrayDefault::default(),
    }
  }
}

pub enum Grain {
  #[cfg(feature = "bitdepth_8")]
  Bpc8(GrainBD<BitDepth8>),
  #[cfg(feature = "bitdepth_16")]
  Bpc16(GrainBD<BitDepth16>),
}

impl Default for Grain {
  fn default() -> Self {
    #[cfg(feature = "bitdepth_8")]
    {
      Self::Bpc8(Default::default())
    }

    #[cfg(all(not(feature = "bitdepth_8"), feature = "bitdepth_16"))]
    {
      Self::Bpc16(Default::default())
    }
    #[cfg(not(any(feature = "bitdepth_8", feature = "bitdepth_16")))]
    compile_error!("No bitdepths enabled");
  }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum DelayedGrainStage {
  #[default]
  Idle,
  Prepare,
  Apply,
}

#[derive(Default)]
#[repr(C)]
pub(crate) struct TaskThreadDataDelayedFg {
  pub in_0: Rav1dPicture,
  pub out: Rav1dPicture,
  pub stage: DelayedGrainStage,
  pub grain: Grain,
  pub row_failed: AtomicBool,
}

#[derive(Default)]
#[repr(C)]
pub(crate) struct TaskThreadData {
  pub lock: Mutex<()>,
  pub cond: Condvar,
  pub first: AtomicU32,
  pub cur: RelaxedAtomic<u32>,

  pub reset_task_cur: Atomic<Option<std::num::NonZeroU32>>,
  pub cond_signaled: AtomicI32,
  pub delayed_fg_exec: RelaxedAtomic<i32>,
  pub delayed_fg_cond: Condvar,
  pub delayed_fg_progress: [AtomicI32; 2],
  pub delayed_fg: RwLock<TaskThreadDataDelayedFg>,

  pub panicked: AtomicBool,
}

pub(crate) struct ScratchParallelism {
  pub workers: u32,
  pub tiles: u32,
}

#[derive(Default)]
#[repr(C)]
pub(crate) struct Rav1dContextRefs {
  pub p: Rav1dThreadPicture,
  pub segmap: Option<DisjointMutArcSlice<SegmentId>>,
  pub refmvs: Option<DisjointMutArcSlice<RefMvsTemporalBlock>>,
  pub refpoc: [c_uint; 7],
}

pub(crate) enum Rav1dContextTaskType {
  Worker,

  Single(Mutex<Box<Rav1dTaskContext>>),
}

pub(crate) struct Rav1dContextTaskThread {
  pub task: Rav1dContextTaskType,

  pub thread_data: Rav1dTaskContextTaskThread,
}

pub(crate) enum FlushState {
  Flushed,
  Waiting,
}

impl From<bool> for FlushState {
  fn from(flushed: bool) -> Self {
    match flushed {
      true => Self::Flushed,
      false => Self::Waiting,
    }
  }
}

impl Rav1dContextTaskThread {
  pub(crate) fn flush_state(&self) -> Result<FlushState, console_core_never::Never> {
    let Ok(flushed) = self.thread_data.flushed.get();
    Ok(FlushState::from(flushed))
  }
}

#[derive(Default)]
#[repr(u8)]
pub(crate) enum DrainRequest {
  #[default]
  Idle,
  Requested,
}

#[derive(Clone)]
pub(crate) struct DecoderStopToken {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the managed API receives a caller-owned cancellation source and workers hold a snapshot while callbacks run outside the decoder lock"
    )
  )]
  pub(crate) retained: Arc<dyn enough::Stop>,
}

impl DecoderStopToken {
  pub(crate) fn check(&self) -> Result<(), enough::StopReason> {
    self.retained.check()
  }

  pub(crate) fn state(&self) -> Result<DecoderStopState, console_core_never::Never> {
    Ok(match self.retained.should_stop().then_some(()) {
      Some(()) => DecoderStopState::Requested,
      None => DecoderStopState::Running,
    })
  }
}

pub(crate) enum DecoderStopState {
  Running,
  Requested,
}

#[derive(Default)]
pub struct Rav1dState {
  pub(crate) tiles: Vec<Rav1dTileGroup>,
  pub(crate) n_tiles: c_int,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the parser retains immutable sequence metadata for subsequent frames while exported C pictures may retain the same descriptor"
    )
  )]
  pub(crate) seq_hdr: Option<Arc<DRav1d<Rav1dSequenceHeader, Dav1dSequenceHeader>>>,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the parser holds an immutable frame header while collecting tile groups and completed C pictures may retain it independently"
    )
  )]
  pub(crate) frame_hdr: Option<Arc<DRav1d<Rav1dFrameHeader, Dav1dFrameHeader>>>,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "published C pictures retain immutable content light metadata after the decoder receives a replacement"
    )
  )]
  pub(crate) content_light: Option<Arc<Rav1dContentLightLevel>>,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "published C pictures retain immutable mastering display metadata after the decoder receives a replacement"
    )
  )]
  pub(crate) mastering_display: Option<Arc<Rav1dMasteringDisplay>>,
  pub(crate) itut_t35: Vec<Rav1dITUTT35>,

  pub(crate) in_0: Rav1dData,
  pub(crate) out: Rav1dThreadPicture,
  pub(crate) cache: Rav1dThreadPicture,
  pub(crate) frame_thread: Rav1dContextFrameThread,

  pub(crate) refs: [Rav1dContextRefs; 8],
  pub(crate) cdf: [CdfThreadContext; 8],

  pub(crate) operating_point_idc: c_uint,
  pub(crate) max_spatial_id: u8,
  pub(crate) drain: DrainRequest,
  pub(crate) frame_flags: PictureFlags,
  pub(crate) event_flags: Rav1dEventFlags,
  pub(crate) cached_error_props: Rav1dDataProps,
  pub(crate) cached_error: Option<Rav1dError>,
}

#[derive(Default)]
#[repr(C)]
#[repr(align(64))]
pub struct Rav1dContext {
  pub(crate) state: Mutex<Rav1dState>,

  pub(crate) fc: Box<[Rav1dFrameContext]>,

  pub(crate) tc: Box<[Rav1dContextTaskThread]>,

  #[cfg(feature = "c-ffi")]
  pub(crate) c_workers: Mutex<Vec<console_core_parallel_execution::Worker>>,

  pub(crate) flush: AtomicBool,

  pub(crate) task_thread: TaskThreadData,

  pub dsp: &'static Rav1dDSPContext,
  pub motion_compensation: MotionCompensation,

  pub(crate) allocator: Rav1dPicAllocator,
  pub(crate) grain_output: GrainOutputPolicy,
  pub(crate) operating_point: u8,
  pub(crate) layer_output: LayerOutputPolicy,
  pub(crate) frame_size_limit: c_uint,
  pub(crate) strictness: Strictness,
  pub(crate) frame_visibility: FrameVisibilityPolicy,
  pub(crate) inloop_filters: Rav1dInloopFilterType,
  pub(crate) decode_frame_type: Rav1dDecodeFrameType,

  pub(crate) logger: Option<Rav1dLogger>,

  pub(crate) stop: Mutex<Option<DecoderStopToken>>,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(crate) enum GrainOutputPolicy {
  #[default]
  Original,
  Synthesized,
}

impl From<bool> for GrainOutputPolicy {
  fn from(apply_grain: bool) -> Self {
    match apply_grain.then_some(()) {
      Some(()) => Self::Synthesized,
      None => Self::Original,
    }
  }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(crate) enum LayerOutputPolicy {
  #[default]
  HighestSpatial,
  All,
}

impl From<bool> for LayerOutputPolicy {
  fn from(all_layers: bool) -> Self {
    match all_layers.then_some(()) {
      Some(()) => Self::All,
      None => Self::HighestSpatial,
    }
  }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(crate) enum FrameVisibilityPolicy {
  #[default]
  VisibleOnly,
  IncludeInvisible,
}

impl From<bool> for FrameVisibilityPolicy {
  fn from(include_invisible: bool) -> Self {
    match include_invisible.then_some(()) {
      Some(()) => Self::IncludeInvisible,
      None => Self::VisibleOnly,
    }
  }
}

impl Rav1dContext {
  pub(crate) fn set_stop(
    &self,
    stop: Option<DecoderStopToken>,
  ) -> core::result::Result<(), console_core_never::Never> {
    let previous = mem::replace(&mut *self.stop.lock(), stop);
    drop(previous);
    Ok(())
  }

  pub(crate) fn stop_token(&self) -> Result<Option<DecoderStopToken>, console_core_never::Never> {
    let snapshot = self.stop.lock().clone();
    Ok(snapshot.filter(|token| token.retained.may_stop()))
  }
}

#[derive(Default)]
#[repr(C)]
pub struct Rav1dTask {
  pub frame_idx: c_uint,
  pub tile_idx: c_uint,

  pub type_0: TaskType,

  pub sby: c_int,

  pub recon_progress: c_int,
  pub deblock_progress: c_int,
  pub deps_skip: RelaxedAtomic<i32>,

  pub next: Atomic<Rav1dTaskIndex>,
}

impl Rav1dTask {
  pub fn init(frame_idx: c_uint) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        type_0: TaskType::Initial,
        frame_idx,
        sby: 0,
        deblock_progress: 0,
        recon_progress: 0,
        ..Default::default()
      }
    })
  }

  pub fn next(&self) -> core::result::Result<Rav1dTaskIndex, console_core_never::Never> {
    Ok(self.next.load(Ordering::SeqCst))
  }

  pub fn set_next(
    &self,
    next: Rav1dTaskIndex,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      self.next.store(next, Ordering::SeqCst)
    };

    Ok(())
  }
}

impl Rav1dTask {
  pub fn without_next(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        frame_idx: self.frame_idx,
        tile_idx: self.tile_idx,
        type_0: self.type_0,
        sby: self.sby,
        recon_progress: self.recon_progress,
        deblock_progress: self.deblock_progress,
        deps_skip: self.deps_skip.clone(),
        next: Default::default(),
      }
    })
  }
}

#[derive(Default)]
#[repr(C)]
pub struct ScalableMotionParams {
  pub scale: c_int,
  pub step: c_int,
}

pub(crate) struct Rav1dFrameContextBdFn {
  pub stitch_recon_band: fn(&Rav1dFrameData, &mut Rav1dTaskContext) -> Rav1dResult,
  pub recon_b_intra: ReconBIntraFn,
  pub recon_b_inter: ReconBInterFn,
  pub filter_sbrow:
    fn(&Rav1dContext, &Rav1dFrameData, &mut Rav1dTaskContext, c_int) -> Rav1dResult,
  pub filter_sbrow_deblock_cols: FilterSbrowFn,
  pub filter_sbrow_deblock_rows:
    fn(&Rav1dContext, &Rav1dFrameData, &mut Rav1dTaskContext, c_int) -> Rav1dResult,
  pub filter_sbrow_cdef:
    fn(&Rav1dContext, &Rav1dFrameData, &mut Rav1dTaskContext, c_int) -> Rav1dResult,
  pub filter_sbrow_resize:
    fn(&Rav1dContext, &Rav1dFrameData, &mut Rav1dTaskContext, c_int) -> Rav1dResult,
  pub filter_sbrow_lr:
    fn(&Rav1dContext, &Rav1dFrameData, &mut Rav1dTaskContext, c_int) -> Rav1dResult,
  pub backup_ipred_edge: BackupIpredEdgeFn,
  pub read_coef_blocks: ReadCoefBlocksFn,
  pub copy_pal_block: CopyPalBlockFn,
  pub read_pal_y: ReadPalFn,
  pub read_pal_uv: ReadPalFn,
}

impl Rav1dFrameContextBdFn {
  pub const fn new<BD: BitDepth>() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        stitch_recon_band: crate::src::owned_recon::stitch_sbrow::<BD>,
        recon_b_inter: rav1d_recon_b_inter::<BD>,
        recon_b_intra: rav1d_recon_b_intra::<BD>,
        filter_sbrow: rav1d_filter_sbrow::<BD>,
        filter_sbrow_deblock_cols: rav1d_filter_sbrow_deblock_cols::<BD>,
        filter_sbrow_deblock_rows: rav1d_filter_sbrow_deblock_rows::<BD>,
        filter_sbrow_cdef: rav1d_filter_sbrow_cdef::<BD>,
        filter_sbrow_resize: rav1d_filter_sbrow_resize::<BD>,
        filter_sbrow_lr: rav1d_filter_sbrow_lr::<BD>,
        backup_ipred_edge: rav1d_backup_ipred_edge::<BD>,
        read_coef_blocks: rav1d_read_coef_blocks::<BD>,
        copy_pal_block: rav1d_copy_pal_block::<BD>,
        read_pal_y: rav1d_read_pal_y::<BD>,
        read_pal_uv: rav1d_read_pal_uv::<BD>,
      }
    })
  }

  pub const fn get(bpc: BPC) -> core::result::Result<&'static Self, console_core_never::Never> {
    Ok({
      const BPC8: Rav1dFrameContextBdFn = {
        let Ok(explicit_value) = Rav1dFrameContextBdFn::new::<BitDepth8>();
        explicit_value
      };
      const BPC16: Rav1dFrameContextBdFn = {
        let Ok(explicit_value) = Rav1dFrameContextBdFn::new::<BitDepth16>();
        explicit_value
      };

      match bpc {
        BPC::BPC8 => &BPC8,
        BPC::BPC16 => &BPC16,
      }
    })
  }
}

#[derive(Clone, Copy, Atom)]
pub struct CodedBlockInfo(i16);

impl CodedBlockInfo {
  pub const INITIAL: Self = Self(0);

  const TXTP_BITS: u32 = TxfmType::BITS.wrapping_sub(WHT_WHT.leading_zeros());
  const TXTP_MASK: i16 = 1i16.wrapping_shl(Self::TXTP_BITS).wrapping_sub(1);

  pub const fn eob(&self) -> Result<i16, console_core_never::Never> {
    Ok(self.0.wrapping_shr(Self::TXTP_BITS))
  }

  pub const fn txtp(&self) -> Result<TxfmType, console_core_never::Never> {
    Ok(
      console_core_number_conversion::const_wrapped_integer!(signed i16 => TxfmType, self.0 & Self::TXTP_MASK),
    )
  }

  pub const fn new(eob: i16, kind: TxfmType) -> Rav1dResult<Self> {
    let position = eob.wrapping_shl(Self::TXTP_BITS);
    let transform =
      console_core_number_conversion::const_wrapped_integer!(unsigned u8 => i16, kind);

    match (
      position.wrapping_shr(Self::TXTP_BITS) == eob,
      transform <= Self::TXTP_MASK,
    ) {
      (true, true) => Ok(Self(position | transform)),
      (false, _) | (_, false) => Err(Rav1dError::EINVAL),
    }
  }
}

#[derive(Default)]
#[repr(C)]
pub struct Pal {
  data: DisjointMut<AlignedVec64<u8>>,
}

pub(crate) enum PalettePixels {
  Present,
  Empty,
}

type PalArray<BD> = [[<BD as BitDepth>::Pixel; 8]; 3];

impl Pal {
  pub fn try_resize(&mut self, entries: u64) -> Rav1dResult {
    let length = entries.checked_mul(24).ok_or(Rav1dError::ENOMEM)?;
    let bytes = console_core_number_conversion::checked_index(length)
      .map_err(|_outside_allocation| Rav1dError::ENOMEM)?;
    self
      .data
      .try_resize(bytes, 0)
      .map_err(|_allocation| Rav1dError::ENOMEM)
  }

  pub(crate) fn pixels(&self) -> Result<PalettePixels, console_core_never::Never> {
    Ok(match self.data.len() {
      0 => PalettePixels::Empty,
      _ => PalettePixels::Present,
    })
  }

  fn checked_position<BD: BitDepth>(&self, record: u64) -> Rav1dResult<u64> {
    let Ok(storage_bytes) = console_core_number_conversion::fixed_index(self.data.len());
    let Ok(record_bytes) =
      console_core_number_conversion::fixed_index(mem::size_of::<PalArray<BD>>());
    let records = storage_bytes
      .checked_div(record_bytes)
      .ok_or(Rav1dError::EINVAL)?;

    match record.cmp(&records) {
      core::cmp::Ordering::Less => Ok(record),
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => Err(Rav1dError::EINVAL),
    }
  }

  pub(crate) fn checked_index<BD: BitDepth>(
    &self,
    record: u64,
  ) -> Rav1dResult<DisjointImmutGuard<'_, AlignedVec64<u8>, PalArray<BD>>> {
    let position = self.checked_position::<BD>(record)?;
    let Ok(index) = console_core_number_conversion::index(position);
    Ok(self.data.element_as(index))
  }

  pub(crate) fn checked_index_mut<BD: BitDepth>(
    &self,
    record: u64,
  ) -> Rav1dResult<DisjointMutGuard<'_, AlignedVec64<u8>, PalArray<BD>>> {
    let position = self.checked_position::<BD>(record)?;
    let Ok(index) = console_core_number_conversion::index(position);
    Ok(self.data.mut_element_as(index))
  }
}

#[derive(Default)]
#[repr(C)]
pub struct Rav1dFrameContextFrameThread {
  pub next_tile_row: [RelaxedAtomic<i32>; 2],

  pub b: DisjointMut<Vec<Av1Block>>,

  pub cbi: Vec<RelaxedAtomic<CodedBlockInfo>>,

  pub pal: Pal,

  pub pal_idx: DisjointMut<AlignedVec64<u8>>,

  pub cf: DisjointMut<AlignedVec64<u8>>,

  pub tile_start_off: Vec<u32>,
}

#[derive(Default)]
pub(crate) struct TxLpfRightEdge {
  inner: DisjointMut<Vec<u8>>,
}

pub(crate) struct RightEdgeRows<T> {
  pub(crate) luma: T,
  pub(crate) chroma: T,
}

pub(crate) struct RightEdgePixels<'a> {
  pub(crate) start: u64,
  pub(crate) values: &'a [u8],
}

impl TxLpfRightEdge {
  pub(crate) fn resize(&mut self, capacity: u64) -> Rav1dResult {
    let bytes = capacity.checked_mul(64).ok_or(Rav1dError::ENOMEM)?;
    let length = console_core_number_conversion::checked_index(bytes)
      .map_err(|_outside_allocation| Rav1dError::ENOMEM)?;
    self
      .inner
      .try_resize(length, 0)
      .map_err(|_allocation| Rav1dError::ENOMEM)
  }

  fn buffer_ranges(
    &self,
    rows: RightEdgeRows<Range<u64>>,
  ) -> Rav1dResult<RightEdgeRows<Range<u64>>> {
    let Ok(length) = console_core_number_conversion::fixed_index(self.inner.len());
    let half = length.wrapping_shr(1);

    match (rows.luma.end <= half
      && rows.chroma.end <= half
      && rows.luma.start <= rows.luma.end
      && rows.chroma.start <= rows.chroma.end)
      .then_some(())
    {
      Some(()) => Ok(RightEdgeRows {
        luma: rows.luma,
        chroma: half.wrapping_add(rows.chroma.start)..half.wrapping_add(rows.chroma.end),
      }),
      None => Err(Rav1dError::EINVAL),
    }
  }

  pub(crate) fn get<'a>(
    &'a self,
    rows: RightEdgeRows<Range<u64>>,
  ) -> Rav1dResult<(
    impl 'a + Deref<Target = [u8]>,
    impl 'a + Deref<Target = [u8]>,
  )> {
    let ranges = self.buffer_ranges(rows)?;
    let Ok(luma) = console_core_number_conversion::indices(ranges.luma);
    let Ok(chroma) = console_core_number_conversion::indices(ranges.chroma);
    Ok((self.inner.index(luma), self.inner.index(chroma)))
  }

  pub(crate) fn store(&self, pixels: RightEdgeRows<RightEdgePixels<'_>>) -> Rav1dResult {
    let range =
      |RightEdgePixels { start, values }: &RightEdgePixels<'_>| -> Rav1dResult<Range<u64>> {
        let Ok(length) = console_core_number_conversion::fixed_index(values.len());
        let end = start.checked_add(length).ok_or(Rav1dError::EINVAL)?;
        Ok(*start..end)
      };
    let luma = range(&pixels.luma)?;
    let chroma = range(&pixels.chroma)?;
    let ranges = self.buffer_ranges(RightEdgeRows { luma, chroma })?;

    for (range_2, values) in [
      (ranges.luma, pixels.luma.values),
      (ranges.chroma, pixels.chroma.values),
    ] {
      let Ok(native_range) = console_core_number_conversion::indices(range_2);
      let mut destination = self.inner.index_mut(native_range);

      for (output, &value) in destination.iter_mut().zip(values) {
        *output = value;
      }
    }

    Ok(())
  }
}

#[repr(C)]
pub struct Rav1dFrameContextLf {
  pub level: Vec<AtomicU8>,
  pub mask: Vec<Av1Filter>,
  pub lr_mask: Vec<Av1Restoration>,
  pub lim_lut: Align16<Av1FilterLUT>,
  pub lvl: [Align16<[[[u8; 2]; 8]; 4]>; 8],
  pub last_sharpness: u8,
  pub tx_lpf_right_edge: TxLpfRightEdge,

  pub cdef_line_buf: DisjointMut<AlignedVec64<u8>>,
  pub lr_line_buf: DisjointMut<AlignedVec64<u8>>,
  pub cdef_line: [[u64; 3]; 2],
  pub cdef_lpf_line: [u64; 3],
  pub lr_lpf_line: [u64; 3],

  pub start_of_tile_row: Vec<u8>,
  pub restore_planes: LrRestorePlanes,
}

impl Default for Rav1dFrameContextLf {
  fn default() -> Self {
    use crate::src::disjoint_mut::dm_new;
    Self {
      level: Vec::new(),
      mask: Default::default(),
      lr_mask: Default::default(),
      lim_lut: Default::default(),
      lvl: Default::default(),
      last_sharpness: Default::default(),
      tx_lpf_right_edge: Default::default(),
      cdef_line_buf: ({
        let Ok(explicit_value) = dm_new(Default::default());
        explicit_value
      }),
      lr_line_buf: ({
        let Ok(explicit_value) = dm_new(Default::default());
        explicit_value
      }),
      cdef_line: Default::default(),
      cdef_lpf_line: Default::default(),
      lr_lpf_line: Default::default(),
      start_of_tile_row: Default::default(),
      restore_planes: Default::default(),
    }
  }
}

#[derive(Default)]
#[repr(C)]
pub(crate) struct Rav1dFrameContextTaskThread {
  pub lock: Mutex<()>,
  pub cond: Condvar,
  pub tasks: Rav1dTasks,
  pub init_done: AtomicI32,
  pub done: [AtomicI32; 2],
  pub retval: Mutex<Option<Rav1dError>>,
  pub finished: AtomicBool,
  pub update_set: RelaxedAtomic<bool>,
  pub error: AtomicI32,
  pub task_counter: AtomicI32,
}

#[derive(Default)]
pub(crate) struct Rav1dFrameContextFrameThreadProgress {
  pub entropy: AtomicI32,
  pub deblock: AtomicI32,
  pub frame: RwLock<Vec<AtomicU32>>,
  pub copy_lpf: RwLock<Vec<AtomicU32>>,
}

pub(crate) struct Rav1dFrameContext {
  pub index: u32,

  pub data: RwLock<Rav1dFrameData>,
  pub in_cdf: RwLock<CdfThreadContext>,
  pub task_thread: Rav1dFrameContextTaskThread,
  pub frame_thread_progress: Rav1dFrameContextFrameThreadProgress,
}

impl Rav1dFrameContext {
  pub fn default(index: u32) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        index,
        data: Default::default(),
        in_cdf: Default::default(),
        task_thread: Default::default(),
        frame_thread_progress: Default::default(),
      }
    })
  }

  pub fn in_cdf(&self) -> Rav1dResult<RwLockReadGuard<'_, CdfThreadContext>> {
    self.in_cdf.try_read().ok_or(Rav1dError::EINVAL)
  }
}

#[derive(Default)]
#[repr(C)]
pub(crate) struct Rav1dFrameData {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "frame workers read the same immutable sequence descriptor published with C pictures that can outlive this frame slot"
    )
  )]
  pub seq_hdr: Option<Arc<DRav1d<Rav1dSequenceHeader, Dav1dSequenceHeader>>>,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "frame workers read the same immutable frame descriptor published with C pictures that can outlive this frame slot"
    )
  )]
  pub frame_hdr: Option<Arc<DRav1d<Rav1dFrameHeader, Dav1dFrameHeader>>>,
  pub refp: [Rav1dThreadPicture; 7],

  pub cur: Rav1dPicture,

  pub sr_cur: Rav1dThreadPicture,
  pub mvs: Option<DisjointMutArcSlice<RefMvsTemporalBlock>>,
  pub ref_mvs: [Option<DisjointMutArcSlice<RefMvsTemporalBlock>>; 7],
  pub cur_segmap: Option<DisjointMutArcSlice<SegmentId>>,
  pub prev_segmap: Option<DisjointMutArcSlice<SegmentId>>,
  pub refpoc: [c_uint; 7],
  pub refrefpoc: [[c_uint; 7]; 7],
  pub gmv_warp_allowed: [u8; 7],
  pub out_cdf: CdfThreadContext,
  pub tiles: Vec<Rav1dTileGroup>,

  pub reconstruction_storage: crate::src::owned_recon::ReconstructionStorage,

  pub svc: [[ScalableMotionParams; 2]; 7],
  pub resize_step: [c_int; 2],
  pub resize_start: [c_int; 2],

  pub ts: Vec<Rav1dTileState>,
  pub dsp: &'static Rav1dBitDepthDSPContext,
  pub motion_compensation: MotionCompensation,

  pub ipred_edge: DisjointMut<AlignedVec64<u8>>,
  pub ipred_edge_off: u64,

  pub b4_stride: u32,
  pub w4: c_int,
  pub h4: c_int,
  pub bw: c_int,
  pub bh: c_int,
  pub sb128w: c_int,
  pub sb128h: c_int,
  pub sbh: c_int,
  pub sb_shift: c_int,
  pub sb_step: c_int,
  pub sr_sb128w: c_int,

  pub dq: [[[RelaxedAtomic<u16>; 2]; 3]; {
    match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
      Ok(extent) => extent,
    }
  }],

  pub qm: [[Option<&'static [u8]>; 3]; 19],
  pub a: Vec<BlockContext>,
  pub rf: RefMvsFrame,
  pub jnt_weights: [[u8; 7]; 7],
  pub bitdepth_max: c_int,

  pub frame_thread: Rav1dFrameContextFrameThread,
  pub lf: Rav1dFrameContextLf,
  pub lowest_pixel_mem: DisjointMut<Vec<[[c_int; 2]; 7]>>,
}

pub(crate) struct ChromaNeighborPrediction {
  pub position: Bxy,
  pub vector: Mv,
  pub reference: u8,
}

pub(crate) struct ChromaMotionNeighbors {
  pub left: Option<ChromaNeighborPrediction>,
  pub above: Option<ChromaNeighborPrediction>,
  pub diagonal: Option<ChromaNeighborPrediction>,
}

enum ChromaNeighborRequirement {
  Read,
  Unused,
}

impl From<bool> for ChromaNeighborRequirement {
  fn from(required: bool) -> Self {
    match required.then_some(()) {
      Some(()) => Self::Read,
      None => Self::Unused,
    }
  }
}

impl Rav1dFrameData {
  pub(crate) fn chroma_motion_neighbors(
    &self,
    task: &Rav1dTaskContext,
    size: Size<i32>,
  ) -> Rav1dResult<Option<ChromaMotionNeighbors>> {
    let (needs_left, needs_above) = match self.cur.p.layout {
      Rav1dPixelLayout::I420 => (size.width == 1, size.height == 1),
      Rav1dPixelLayout::I422 => (size.width == 1, false),
      Rav1dPixelLayout::I400 | Rav1dPixelLayout::I444 => return Ok(None),
    };
    let position = task.b;
    let row = 5_i32.wrapping_add(position.y & 31);
    let read = |required: ChromaNeighborRequirement,
                neighbor: Bxy|
     -> Rav1dResult<Option<ChromaNeighborPrediction>> {
      match required {
        ChromaNeighborRequirement::Read => {
          let ring_row = row.wrapping_add(neighbor.y.wrapping_sub(position.y));
          let block = read_spatial_block(
            &self.rf.r,
            &task.rt.r,
            Point {
              x: neighbor.x,
              y: ring_row,
            },
          )?;
          let [first_reference, _] = block.r#ref.r#ref;
          let [vector, _] = block.mv.mv;
          Ok(
            match u8::try_from(i16::from(first_reference).wrapping_sub(1)) {
              ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
              ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
            }
            .map(|reference| ChromaNeighborPrediction {
              position: neighbor,
              vector,
              reference,
            }),
          )
        }
        ChromaNeighborRequirement::Unused => Ok(None),
      }
    };
    let previous_x = position.x.wrapping_sub(1);
    let previous_y = position.y.wrapping_sub(1);
    let left = read(
      ChromaNeighborRequirement::from(needs_left),
      Bxy {
        x: previous_x,
        y: position.y,
      },
    )?;
    let above = read(
      ChromaNeighborRequirement::from(needs_above),
      Bxy {
        x: position.x,
        y: previous_y,
      },
    )?;
    let diagonal = read(
      ChromaNeighborRequirement::from(needs_left && needs_above),
      Bxy {
        x: previous_x,
        y: previous_y,
      },
    )?;
    let uses_neighbors = (needs_left || needs_above)
      && (!needs_left || left.is_some())
      && (!needs_above || above.is_some())
      && (!(needs_left && needs_above) || diagonal.is_some());
    Ok(uses_neighbors.then_some(ChromaMotionNeighbors {
      left,
      above,
      diagonal,
    }))
  }

  pub(crate) fn tile(&self, position: u32) -> Rav1dResult<&Rav1dTileState> {
    let Ok(index) = console_core_number_conversion::index(position);
    self.ts.get(index).ok_or(Rav1dError::EINVAL)
  }

  pub(crate) fn above_context(&self, position: u32) -> Rav1dResult<&BlockContext> {
    let Ok(index) = console_core_number_conversion::index(position);
    self.a.get(index).ok_or(Rav1dError::EINVAL)
  }

  pub(crate) fn lowest_pixel_position(
    &self,
    tile: &Rav1dTileState,
    row: i32,
  ) -> Rav1dResult<u64> {
    let relative = u64::try_from(row).map_err(|_outside_tile| Rav1dError::EINVAL)?;
    let offset = tile
      .lowest_pixel
      .checked_add(relative)
      .ok_or(Rav1dError::EINVAL)?;
    let Ok(length) = console_core_number_conversion::fixed_index(self.lowest_pixel_mem.len());

    match offset.cmp(&length) {
      core::cmp::Ordering::Less => Ok(offset),
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => Err(Rav1dError::EINVAL),
    }
  }

  pub(crate) fn loop_filter_mask(&self, position: Option<u32>) -> Rav1dResult<&Av1Filter> {
    let block = position.ok_or(Rav1dError::EINVAL)?;
    let Ok(index) = console_core_number_conversion::index(block);
    self.lf.mask.get(index).ok_or(Rav1dError::EINVAL)
  }

  pub(crate) fn coded_block_position(&self, position: Bxy) -> Rav1dResult<u64> {
    let column = u32::try_from(position.x).map_err(|_outside_frame| Rav1dError::EINVAL)?;
    let row = u32::try_from(position.y).map_err(|_outside_frame| Rav1dError::EINVAL)?;
    let offset = u64::from(row)
      .wrapping_mul(u64::from(self.b4_stride))
      .wrapping_add(u64::from(column));
    let Ok(blocks) = console_core_number_conversion::fixed_index(self.frame_thread.b.len());

    match (column < self.b4_stride && offset < blocks).then_some(()) {
      Some(()) => Ok(offset),
      None => Err(Rav1dError::EINVAL),
    }
  }

  pub fn configure_scratch_parallelism(
    &mut self,
    ScratchParallelism { workers, tiles }: ScratchParallelism,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(threads) = console_core_number_conversion::index(workers);
      let Ok(tile_count) = console_core_number_conversion::index(tiles);

      for buffer in [
        &mut self.ipred_edge,
        &mut self.lf.cdef_line_buf,
        &mut self.lf.lr_line_buf,
        &mut self.frame_thread.pal.data,
        &mut self.frame_thread.pal_idx,
        &mut self.frame_thread.cf,
      ] {
        buffer.configure_parallelism(threads, tile_count);
      }

      self.rf.r.configure_parallelism(threads, tile_count);
      self.rf.rp_proj.configure_parallelism(threads, tile_count);
      self
        .frame_thread
        .b
        .configure_parallelism(threads, tile_count);
      self
        .lowest_pixel_mem
        .configure_parallelism(threads, tile_count);
      self
        .lf
        .tx_lpf_right_edge
        .inner
        .configure_parallelism(threads, tile_count);
    };

    Ok(())
  }

  pub fn bd_fn(
    &self,
  ) -> core::result::Result<&'static Rav1dFrameContextBdFn, console_core_never::Never> {
    Ok({
      let bpc = BPC::from_bitdepth_max(self.bitdepth_max);

      {
        let Ok(explicit_value) = Rav1dFrameContextBdFn::get(bpc);
        explicit_value
      }
    })
  }

  pub fn frame_hdr(&self) -> Rav1dResult<&Rav1dFrameHeader> {
    let header = self.frame_hdr.as_ref().ok_or(Rav1dError::EINVAL)?;
    Ok(header)
  }

  pub fn seq_hdr(&self) -> Rav1dResult<&Rav1dSequenceHeader> {
    let header = self.seq_hdr.as_ref().ok_or(Rav1dError::EINVAL)?;
    Ok(header)
  }
}

#[derive(Default)]
#[repr(C)]
pub struct Rav1dTileStateTiling {
  pub col_start: i32,
  pub col_end: i32,
  pub row_start: i32,
  pub row_end: i32,

  pub col: i32,
  pub row: i32,
}

#[derive(Default)]
#[repr(C)]
pub struct Rav1dTileStateFrameThread {
  pub pal_idx: RelaxedAtomic<u32>,

  pub cbi_idx: RelaxedAtomic<u32>,

  pub cf: RelaxedAtomic<u32>,
}

#[derive(Default)]
#[repr(C, align(32))]
pub struct Rav1dTileStateContext {
  pub cdf: CdfContext,
  pub msac: MsacContext,
}

#[derive(Default)]
#[repr(C, align(32))]
pub struct Rav1dTileState {
  pub context: Mutex<Rav1dTileStateContext>,

  pub tiling: Rav1dTileStateTiling,

  pub progress: [AtomicI32; 2],
  pub frame_thread: [Rav1dTileStateFrameThread; 2],

  pub lowest_pixel: u64,

  pub dqmem: [[[RelaxedAtomic<u16>; 2]; 3]; {
    match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
      Ok(extent) => extent,
    }
  }],

  pub dq: RelaxedAtomic<TileStateRef>,
  pub last_qidx: RelaxedAtomic<u8>,
  pub last_delta_lf: RelaxedAtomic<[i8; 4]>,
  pub lflvlmem: RwLock<[Align16<[[[u8; 2]; 8]; 4]>; 8]>,
  pub lflvl: RelaxedAtomic<TileStateRef>,

  pub lr_ref: RwLock<[Av1RestorationUnit; 3]>,
}

#[derive(Clone, Copy, Default)]
#[repr(u8)]
pub enum TileStateRef {
  #[default]
  Frame,
  Local,
}

impl Atom for TileStateRef {
  type Repr = u8;

  fn pack(self) -> Self::Repr {
    match self {
      Self::Frame => 0,
      Self::Local => 1,
    }
  }

  fn unpack(src: Self::Repr) -> Self {
    match src {
      1 => Self::Local,
      _ => Self::Frame,
    }
  }
}

fn scratch_prefix<T: FromBytes + KnownLayout + Immutable>(bytes: &[u8]) -> Rav1dResult<&T> {
  let (view, _remaining) =
    T::ref_from_prefix(bytes).map_err(|_invalid_scratch| Rav1dError::EINVAL)?;
  Ok(view)
}

fn scratch_prefix_mut<T: FromBytes + KnownLayout + IntoBytes>(
  bytes: &mut [u8],
) -> Rav1dResult<&mut T> {
  let (view, _remaining) =
    T::mut_from_prefix(bytes).map_err(|_invalid_scratch| Rav1dError::EINVAL)?;
  Ok(view)
}

pub type CoefficientScratch<T> = [T; 32 * 32];

#[derive(FromZeros)]
#[repr(align(64))]
pub struct Cf(CoefficientScratch<i32>);

impl Cf {
  pub fn select<BD: BitDepth>(&self) -> Rav1dResult<&CoefficientScratch<BD::Coef>> {
    scratch_prefix(IntoBytes::as_bytes(&self.0))
  }

  pub fn select_mut<BD: BitDepth>(&mut self) -> Rav1dResult<&mut CoefficientScratch<BD::Coef>> {
    scratch_prefix_mut(IntoBytes::as_mut_bytes(&mut self.0))
  }
}

#[derive(FromZeros)]
pub struct AlPal([u16; 8 * 3 * 32 * 2]);

impl AlPal {
  pub fn select_mut<BD: BitDepth>(
    &mut self,
  ) -> core::result::Result<&mut [[[[BD::Pixel; 8]; 3]; 32]; 2], Rav1dError> {
    scratch_prefix_mut(IntoBytes::as_mut_bytes(&mut self.0))
  }
}

pub const MAX_COMPOUND_SAMPLES: u16 = 128 * 128;
type CompoundSamples<T> =
  [T; match console_core_number_conversion::const_u16_index(MAX_COMPOUND_SAMPLES) {
    Ok(samples) => samples,
  }];
pub type CompoundPrediction = CompoundSamples<i16>;
pub type SegmentMask = CompoundSamples<u8>;

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct ScratchCompinter {
  pub compinter: [CompoundPrediction; 2],
  pub seg_mask: SegmentMask,
}

pub type OverlapPrediction<T> = [T; 128 * 32];

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct ScratchLapInter([u8; mem::size_of::<ScratchCompinter>()]);

impl ScratchLapInter {
  pub fn lap_mut<BD: BitDepth>(
    &mut self,
  ) -> core::result::Result<&mut OverlapPrediction<BD::Pixel>, Rav1dError> {
    scratch_prefix_mut(&mut self.0)
  }

  pub fn inter_mut(&mut self) -> core::result::Result<&mut ScratchCompinter, Rav1dError> {
    scratch_prefix_mut(&mut self.0)
  }
}

pub type EmulatedEdge<T> = [T; 320 * (256 + 7)];

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct ScratchEmuEdge {
  pixels: [u8; mem::size_of::<EmulatedEdge<u16>>()],
  motion: MotionScratch,
}

impl ScratchEmuEdge {
  pub fn buf_mut<BD: BitDepth>(
    &mut self,
  ) -> core::result::Result<&mut EmulatedEdge<BD::Pixel>, Rav1dError> {
    scratch_prefix_mut(&mut self.pixels)
  }

  pub fn motion_and_edges<BD: BitDepth>(
    &mut self,
  ) -> Rav1dResult<(&mut MotionScratch, &mut EmulatedEdge<BD::Pixel>)> {
    let edges = scratch_prefix_mut(&mut self.pixels)?;
    Ok((&mut self.motion, edges))
  }
}

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct ScratchInter {
  pub lap_inter: ScratchLapInter,
  pub emu_edge: ScratchEmuEdge,
}

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct ScratchLevelsPal([u8; 32 * 34]);

impl ScratchLevelsPal {
  pub fn levels_mut(
    &mut self,
  ) -> core::result::Result<&mut [u8; 32 * 34], console_core_never::Never> {
    Ok(&mut self.0)
  }
}

pub type InterIntraPrediction<T> = [T; 64 * 64];

#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct ScratchInterIntraBuf([InterIntraPrediction<u16>; 2]);

impl ScratchInterIntraBuf {
  pub fn buf_mut<BD: BitDepth>(
    &mut self,
  ) -> core::result::Result<&mut InterIntraPrediction<BD::Pixel>, Rav1dError> {
    scratch_prefix_mut(IntoBytes::as_mut_bytes(&mut self.0))
  }
}

pub type IntraEdgeSamples<T> = [T; 257];

#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct ScratchEdgeBuf([u8; mem::size_of::<IntraEdgeSamples<u16>>() + 62]);

impl ScratchEdgeBuf {
  pub fn buf_mut<BD: BitDepth>(&mut self) -> Rav1dResult<&mut IntraEdgeSamples<BD::Pixel>> {
    scratch_prefix_mut(&mut self.0)
  }
}

#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(16))]
pub struct ScratchPalBuf([u8; 8 * 3 * 2]);

impl ScratchPalBuf {
  pub fn buf<BD: BitDepth>(&self) -> core::result::Result<&[[BD::Pixel; 8]; 3], Rav1dError> {
    scratch_prefix(&self.0)
  }

  pub fn buf_mut<BD: BitDepth>(
    &mut self,
  ) -> core::result::Result<&mut [[BD::Pixel; 8]; 3], Rav1dError> {
    scratch_prefix_mut(&mut self.0)
  }
}

#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct ScratchInterIntraEdgePal {
  pub interintra: ScratchInterIntraBuf,
  pub edge: ScratchEdgeBuf,
  pub pal: ScratchPalBuf,

  _padding: [u8; 16],
}

#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct ScratchAcTxtpMap([u8; mem::size_of::<CoefficientScratch<i16>>()]);

impl ScratchAcTxtpMap {
  pub fn ac_mut(&mut self) -> Rav1dResult<&mut CoefficientScratch<i16>> {
    scratch_prefix_mut(&mut self.0)
  }

  pub fn txtp_map(&self) -> Rav1dResult<&CoefficientScratch<TxfmType>> {
    scratch_prefix(&self.0)
  }

  pub fn txtp_map_mut(&mut self) -> Rav1dResult<&mut CoefficientScratch<TxfmType>> {
    scratch_prefix_mut(&mut self.0)
  }
}

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct ScratchInterIntra {
  pub levels_pal: ScratchLevelsPal,
  pub ac_txtp_map: ScratchAcTxtpMap,
  pub pal_idx_y: [u8; 32 * 64],
  pub pal_idx_uv: [u8; 64 * 64],
  pub interintra_edge_pal: ScratchInterIntraEdgePal,
}

#[derive(FromZeros)]
#[repr(C, align(64))]
pub struct TaskContextScratch([u8; mem::size_of::<ScratchInter>()]);

const _: () = assert!(mem::align_of::<TaskContextScratch>() >= mem::align_of::<ScratchInter>());
const _: () =
  assert!(mem::align_of::<TaskContextScratch>() >= mem::align_of::<ScratchInterIntra>());

const _: () = {
  use crate::include::dav1d::picture::DAV1D_PICTURE_ALIGNMENT as PIC_ALIGN;
  assert!(mem::align_of::<ScratchEmuEdge>() >= PIC_ALIGN);
  assert!(mem::align_of::<ScratchLapInter>() >= PIC_ALIGN);
  assert!(mem::align_of::<ScratchInterIntraBuf>() >= PIC_ALIGN);

  assert!(mem::align_of::<ScratchInter>() >= PIC_ALIGN);
  assert!(mem::align_of::<ScratchInterIntraEdgePal>() >= PIC_ALIGN);
  assert!(mem::align_of::<ScratchInterIntra>() >= PIC_ALIGN);
  assert!(mem::align_of::<TaskContextScratch>() >= PIC_ALIGN);
};

impl TaskContextScratch {
  pub(crate) fn allocate() -> Rav1dResult<Box<Self>> {
    Self::new_box_zeroed().map_err(|_allocation| Rav1dError::ENOMEM)
  }

  pub fn inter_mut(&mut self) -> core::result::Result<&mut ScratchInter, Rav1dError> {
    scratch_prefix_mut(&mut self.0)
  }

  pub fn inter_intra(&self) -> core::result::Result<&ScratchInterIntra, Rav1dError> {
    scratch_prefix(&self.0)
  }

  pub fn inter_intra_mut(&mut self) -> core::result::Result<&mut ScratchInterIntra, Rav1dError> {
    scratch_prefix_mut(&mut self.0)
  }
}

#[derive(Default)]
#[repr(C)]
pub struct Rav1dTaskContextFrameThread {
  pub pass: c_int,
}

#[derive(Default)]
#[repr(C)]
pub(crate) struct Rav1dTaskContextTaskThread {
  pub cond: Condvar,
  pub flushed: RelaxedAtomic<bool>,
  pub die: RelaxedAtomic<bool>,
}

#[derive(Clone, Copy, Default)]
pub struct Bxy {
  pub x: c_int,
  pub y: c_int,
}

#[repr(C)]
pub(crate) struct Rav1dTaskContext {
  pub tile_index: u32,
  pub b: Bxy,
  pub l: BlockContext,
  pub above_index: u32,
  pub rt: RefmvsTile,
  pub cf: Cf,
  pub al_pal: AlPal,
  pub pal_sz_uv: [[u8; 32]; 2],

  pub scratch: Box<TaskContextScratch>,

  pub recon_band: crate::src::owned_recon::ReconBand,

  pub warpmv: Rav1dWarpedMotionParams,

  pub lf_mask: Option<u32>,
  pub top_pre_cdef_toggle: c_int,
  pub cur_sb_cdef_idx: u8,

  pub tl_4x4_filter: Filter2d,
  pub frame_thread: Rav1dTaskContextFrameThread,
}

impl Rav1dTaskContext {
  pub(crate) fn new(scratch: Box<TaskContextScratch>) -> Result<Self, console_core_never::Never> {
    Ok({
      Self {
        tile_index: Default::default(),
        b: Default::default(),
        l: Default::default(),
        above_index: Default::default(),
        rt: Default::default(),
        cf: FromZeros::new_zeroed(),
        al_pal: FromZeros::new_zeroed(),
        pal_sz_uv: Default::default(),
        scratch,
        recon_band: Default::default(),
        warpmv: Default::default(),
        lf_mask: Default::default(),
        top_pre_cdef_toggle: Default::default(),
        cur_sb_cdef_idx: Default::default(),
        tl_4x4_filter: Default::default(),
        frame_thread: Default::default(),
      }
    })
  }
}

#[cfg(test)]
mod task_ctx_size {
  use super::*;

  #[test]
  fn task_context_stays_stack_light() {
    const MAX_STACK_BYTES: u64 = 48 * 1024;
    let Ok(bytes) =
      console_core_number_conversion::fixed_index(core::mem::size_of::<Rav1dTaskContext>());
    eprintln!("Rav1dTaskContext = {bytes}");
    assert!(
      bytes < MAX_STACK_BYTES,
      "Rav1dTaskContext grew to {bytes} B — inline buffers belong in \
             `scratch`-style zeroed boxes (see issue #15)"
    );
  }
}
