//! 100% Safe Rust API for rav1d-safe decoder
//!
//! This module provides a fully safe, zero-copy API for decoding AV1 video.
//! It wraps the internal `rav1d` decoder with type-safe, lifetime-safe abstractions.
//!
//! # Features
//!
//! - **100% Safe Rust** - No `unsafe` code in this module
//! - **Zero-Copy** - Direct access to decoded pixel data without copying
//! - **Type Safety** - Enums and strong types instead of raw integers
//! - **HDR Support** - Full access to HDR10, HLG metadata
//! - **Multi-threaded** - Configurable thread pool for parallel decoding
//!
//! # Example
//!
//! ```no_run
//! use console_core_video_decoding_av1::{Decoder, Error, Frame, Planes};
//!
//! fn show_frame(obu_data: &[u8]) -> Result<(), Error> {
//!     let mut decoder = Decoder::new().map_err(|fault| fault.decompose().0)?;
//!     let ready = decoder.decode(obu_data).map_err(|fault| fault.decompose().0)?;
//!
//!     match ready {
//!         Some(frame) => show_luma(&frame),
//!         None => Ok(()),
//!     }
//! }
//!
//! fn show_luma(frame: &Frame) -> Result<(), Error> {
//!     let planes = frame.planes()?;
//!
//!     match planes {
//!         Planes::Depth8(planes) => {
//!             let Ok(y_plane) = planes.y();
//!             let Ok(rows) = y_plane.rows();
//!
//!             for row in rows {
//!                 println!("8-bit row with {} pixels", row.len());
//!             }
//!         },
//!         Planes::Depth16(planes) => {
//!             let Ok(y_plane) = planes.y();
//!             let Ok(rows) = y_plane.rows();
//!
//!             for row in rows {
//!                 println!("High-bit-depth row with {} pixels", row.len());
//!             }
//!         },
//!     }
//!
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_parallel_execution::{Serial, Worker, Workers};
use console_core_pixel_buffers::{RowLayout, RowLayoutError};
use std::marker::PhantomData;

use crate::include::common::bitdepth::{BitDepth, BitDepth8, BitDepth16};
use crate::include::dav1d::data::Rav1dData;
use crate::include::dav1d::dav1d::{Rav1dDecodeFrameType, Rav1dInloopFilterType, Rav1dSettings};
use crate::include::dav1d::headers::{
  Rav1dColorPrimaries, Rav1dMatrixCoefficients, Rav1dPixelLayout, Rav1dTransferCharacteristics,
};
use crate::include::dav1d::picture::{
  Rav1dPicture, Rav1dPictureData, Rav1dPictureDataComponentInner,
};
use crate::src::c_arc::CArc;
use crate::src::disjoint_mut::DisjointImmutGuard;
use crate::src::error::Rav1dError;
use crate::src::internal::{DecoderStopState, DecoderStopToken, Rav1dContext};
use std::ops::Deref;
use std::sync::Arc;

pub use enough::{Stop, StopReason, Unstoppable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  InvalidSettings(&'static str),
  InitFailed,
  OutOfMemory,
  InvalidData,
  NeedMoreData,

  Cancelled,
  Other(String),
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidSettings(msg) => write!(f, "invalid settings: {}", msg),
      Self::InitFailed => write!(f, "decoder initialization failed"),
      Self::OutOfMemory => write!(f, "out of memory"),
      Self::InvalidData => write!(f, "invalid data"),
      Self::NeedMoreData => write!(f, "need more data"),
      Self::Cancelled => write!(f, "decode cancelled via stop token"),
      Self::Other(msg) => write!(f, "decode error: {}", msg),
    }
  }
}

impl std::error::Error for Error {}

impl From<Rav1dError> for Error {
  fn from(err: Rav1dError) -> Self {
    match err {
      Rav1dError::EAGAIN => Self::NeedMoreData,
      Rav1dError::ENOMEM => Self::OutOfMemory,
      Rav1dError::EINVAL => Self::InvalidData,
      Rav1dError::ECANCELED => Self::Cancelled,
      Rav1dError::EGeneric => Self::Other("generic error".to_string()),
      Rav1dError::ENOENT | Rav1dError::EIO | Rav1dError::ERANGE | Rav1dError::ENOPROTOOPT => {
        Self::Other(format!("{:?}", err))
      }
    }
  }
}

pub type Result<T, E = whereat::At<Error>> = std::result::Result<T, E>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Strictness {
  Lenient,

  #[default]
  Strict,
}

#[derive(Clone, Debug)]
#[non_exhaustive]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "grain synthesis, layer copying, invisible frame output and the legacy strictness flag are independent public settings"
  )
)]
pub struct Settings {
  pub threads: u32,

  pub apply_grain: bool,

  pub frame_size_limit: u32,

  pub all_layers: bool,

  pub operating_point: u8,

  pub output_invisible_frames: bool,

  pub inloop_filters: InloopFilters,

  pub decode_frame_type: DecodeFrameType,

  pub max_frame_delay: u32,

  #[deprecated(since = "0.6.0", note = "use `strictness`")]
  pub strict_std_compliance: bool,

  pub strictness: Strictness,

  pub cpu_level: CpuLevel,
}

impl Settings {
  #[expect(
    deprecated,
    reason = "existing callers can still request strict decoding through the legacy settings flag"
  )]
  pub(crate) fn effective_strictness(
    &self,
  ) -> core::result::Result<Strictness, console_core_never::Never> {
    Ok({
      let legacy = match ::core::primitive::bool::then_some(self.strict_std_compliance, ()) {
        ::core::option::Option::Some(()) => Strictness::Strict,
        ::core::option::Option::None => Strictness::Lenient,
      };
      self.strictness.max(legacy)
    })
  }
}

impl Default for Settings {
  #[expect(
    deprecated,
    reason = "default settings still initialize the legacy public field"
  )]
  fn default() -> Self {
    let Ok(inloop_filters) = InloopFilters::all();

    Self {
      threads: 1,
      apply_grain: true,
      frame_size_limit: 120_000_000,
      all_layers: true,
      operating_point: 0,
      max_frame_delay: 0,
      output_invisible_frames: false,
      inloop_filters,
      decode_frame_type: DecodeFrameType::All,
      strict_std_compliance: false,
      strictness: Strictness::default(),
      cpu_level: CpuLevel::Native,
    }
  }
}

impl From<Settings> for Rav1dSettings {
  fn from(settings: Settings) -> Self {
    let Ok(strictness) = settings.effective_strictness();
    let Ok(n_threads) = console_core_number_conversion::wrapped::<_, i32>(settings.threads);
    let Ok(max_frame_delay) =
      console_core_number_conversion::wrapped::<_, i32>(settings.max_frame_delay);

    Self {
      n_threads,
      max_frame_delay,
      apply_grain: settings.apply_grain,
      operating_point: settings.operating_point,
      all_layers: settings.all_layers,
      frame_size_limit: settings.frame_size_limit,
      allocator: Default::default(),
      logger: None,
      strictness,
      output_invisible_frames: settings.output_invisible_frames,
      inloop_filters:
        <crate::include::dav1d::dav1d::Rav1dInloopFilterType as ::core::convert::From<
          crate::src::managed::InloopFilters,
        >>::from(settings.inloop_filters),
      decode_frame_type:
        <crate::include::dav1d::dav1d::Rav1dDecodeFrameType as ::core::convert::From<
          crate::src::managed::DecodeFrameType,
        >>::from(settings.decode_frame_type),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InloopFilters {
  bits: u8,
}

impl InloopFilters {
  pub const DEBLOCK: Self = Self {
    bits: Rav1dInloopFilterType::DEBLOCK.bits(),
  };
  pub const CDEF: Self = Self {
    bits: Rav1dInloopFilterType::CDEF.bits(),
  };
  pub const RESTORATION: Self = Self {
    bits: Rav1dInloopFilterType::RESTORATION.bits(),
  };

  pub const fn all() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        bits: Rav1dInloopFilterType::DEBLOCK.bits()
          | Rav1dInloopFilterType::CDEF.bits()
          | Rav1dInloopFilterType::RESTORATION.bits(),
      }
    })
  }

  pub const fn none() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { bits: 0 })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public filter-set query returns its coverage enum directly"
    )
  )]
  pub const fn coverage(&self, other: Self) -> FilterCoverage {
    match other.bits & !self.bits {
      0 => FilterCoverage::Included,
      _ => FilterCoverage::Absent,
    }
  }

  pub const fn union(self, other: Self) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        bits: self.bits | other.bits,
      }
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterCoverage {
  Included,
  Absent,
}

impl From<InloopFilters> for Rav1dInloopFilterType {
  fn from(filters: InloopFilters) -> Self {
    Self::from_bits_retain(filters.bits)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeFrameType {
  All,

  Reference,

  Intra,

  Key,
}

impl From<DecodeFrameType> for Rav1dDecodeFrameType {
  fn from(ft: DecodeFrameType) -> Self {
    match ft {
      DecodeFrameType::All => Self::All,
      DecodeFrameType::Reference => Self::Reference,
      DecodeFrameType::Intra => Self::Intra,
      DecodeFrameType::Key => Self::Key,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[derive(Default)]
pub enum CpuLevel {
  Scalar,

  X86V2,

  X86V3,

  X86V4,

  Neon,

  NeonDotprod,

  NeonI8mm,

  #[default]
  Native,
}

impl CpuLevel {
  pub const fn to_mask(self) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      match self {
        Self::Scalar => 0,

        Self::X86V2 => (1 << 0) | (1 << 1) | (1 << 2),
        Self::X86V3 => (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3),
        Self::X86V4 => (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4),

        Self::Neon => 1 << 0,
        Self::NeonDotprod => (1 << 0) | (1 << 1),
        Self::NeonI8mm => (1 << 0) | (1 << 1) | (1 << 2),

        Self::Native => u32::MAX,
      }
    })
  }

  pub fn platform_levels() -> core::result::Result<&'static [CpuLevel], console_core_never::Never>
  {
    Ok({
      #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
      {
        &[
          CpuLevel::Scalar,
          CpuLevel::X86V2,
          CpuLevel::X86V3,
          CpuLevel::X86V4,
          CpuLevel::Native,
        ]
      }

      #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
      {
        &[
          CpuLevel::Scalar,
          CpuLevel::Neon,
          CpuLevel::NeonDotprod,
          CpuLevel::NeonI8mm,
          CpuLevel::Native,
        ]
      }

      #[cfg(not(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
      )))]
      {
        &[CpuLevel::Scalar, CpuLevel::Native]
      }
    })
  }

  pub const fn name(self) -> core::result::Result<&'static str, console_core_never::Never> {
    Ok({
      match self {
        Self::Scalar => "scalar",
        Self::X86V2 => "x86-64-v2",
        Self::X86V3 => "x86-64-v3",
        Self::X86V4 => "x86-64-v4",
        Self::Neon => "neon",
        Self::NeonDotprod => "neon-dotprod",
        Self::NeonI8mm => "neon-i8mm",
        Self::Native => "native",
      }
    })
  }
}

impl std::fmt::Display for CpuLevel {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Ok(name) = self.name();

    f.write_str(name)
  }
}

pub struct Decoder {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the decoder keeps the context live for its decode workers and joins those workers on drop"
    )
  )]
  ctx: Arc<Rav1dContext>,
  worker_handles: Vec<Worker>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelAccess {
  Checked,
  Unchecked,
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this public const query returns the build's immutable pixel access mode"
  )
)]
pub const fn pixel_access() -> PixelAccess {
  #[cfg(feature = "unchecked")]
  return PixelAccess::Unchecked;

  #[cfg(not(feature = "unchecked"))]
  return PixelAccess::Checked;
}

impl Decoder {
  pub fn new() -> Result<Self> {
    Self::with_settings(Settings::default())
  }

  pub fn with_settings(settings: Settings) -> Result<Self> {
    match settings.threads {
      0 | 1 => Self::with_workers(settings, &Serial),
      _requested => Err(
        <::whereat::At<crate::src::managed::Error> as ::core::convert::From<
          crate::src::managed::Error,
        >>::from(Error::InvalidSettings(
          "parallel decoding requires a worker executor",
        )),
      ),
    }
  }

  pub fn with_workers(settings: Settings, workers: &impl Workers) -> Result<Self> {
    let Ok(mask) = settings.cpu_level.to_mask();
    let Ok(()) = crate::src::cpu::rav1d_set_cpu_flags_mask(mask);

    let rav1d_settings: Rav1dSettings =
      <crate::include::dav1d::dav1d::Rav1dSettings as ::core::convert::From<
        crate::src::managed::Settings,
      >>::from(settings);
    let (ctx, worker_handles) =
      crate::src::lib::rav1d_open(&rav1d_settings, workers).map_err(|_| Error::InitFailed)?;
    Ok(Self {
      ctx,
      worker_handles,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the public cancellation API accepts a stop token shared with the caller that requests cancellation"
    )
  )]
  pub fn set_stop(
    &mut self,
    stop: Option<Arc<dyn Stop>>,
  ) -> core::result::Result<(), console_core_never::Never> {
    self
      .ctx
      .set_stop(stop.map(|retained| DecoderStopToken { retained }))
  }

  fn classify_decode_error(
    &self,
    e: Rav1dError,
  ) -> core::result::Result<whereat::At<Error>, console_core_never::Never> {
    let stop = self.ctx.stop.lock().clone();

    Ok(match stop.map(|token| token.state()) {
      Some(Ok(DecoderStopState::Requested)) => whereat::at(Error::Cancelled),
      Some(Ok(DecoderStopState::Running)) | None => whereat::at(Error::from(e)),
    })
  }

  pub fn decode(&mut self, data: &[u8]) -> Result<Option<Frame>> {
    let mut rav1d_data = match ::core::primitive::bool::then_some(!data.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let owned = data.to_vec().into_boxed_slice();
        let Ok(cbox) = crate::src::c_box::CBox::from_box(owned);
        let carc = CArc::wrap(cbox).map_err(|_| Error::OutOfMemory)?;
        Rav1dData {
          data: Some(carc),
          m: Default::default(),
        }
      }
      ::core::option::Option::None => Rav1dData {
        data: None,
        m: Default::default(),
      },
    };

    crate::src::lib::rav1d_send_data(&self.ctx, &mut rav1d_data).map_err(|e| {
      let Ok(classify_decode_error) = self.classify_decode_error(e);

      classify_decode_error
    })?;

    let mut pic = Rav1dPicture::default();

    match crate::src::lib::rav1d_get_picture(&self.ctx, &mut pic) {
      Ok(()) => Ok(Some(Frame { inner: pic })),
      Err(Rav1dError::EAGAIN) => Ok(None),
      Err(e) => {
        let Ok(classify_decode_error) = self.classify_decode_error(e);

        Err(classify_decode_error)
      }
    }
  }

  pub fn decode_stamped(&mut self, data: &[u8], timestamp: i64) -> Result<Option<Frame>> {
    let owned = data.to_vec().into_boxed_slice();
    let Ok(cbox) = crate::src::c_box::CBox::from_box(owned);
    let carc = CArc::wrap(cbox).map_err(|_| Error::OutOfMemory)?;
    let m = crate::include::dav1d::common::Rav1dDataProps {
      timestamp,
      ..Default::default()
    };
    let mut rav1d_data = Rav1dData {
      data: Some(carc),
      m,
    };

    crate::src::lib::rav1d_send_data(&self.ctx, &mut rav1d_data).map_err(|e| {
      let Ok(classify_decode_error) = self.classify_decode_error(e);

      classify_decode_error
    })?;

    let mut pic = Rav1dPicture::default();

    match crate::src::lib::rav1d_get_picture(&self.ctx, &mut pic) {
      Ok(()) => Ok(Some(Frame { inner: pic })),
      Err(Rav1dError::EAGAIN) => Ok(None),
      Err(e) => {
        let Ok(classify_decode_error) = self.classify_decode_error(e);

        Err(classify_decode_error)
      }
    }
  }

  pub fn get_frame(&mut self) -> Result<Option<Frame>> {
    let mut pic = Rav1dPicture::default();

    match crate::src::lib::rav1d_get_picture(&self.ctx, &mut pic) {
      Ok(()) => Ok(Some(Frame { inner: pic })),
      Err(Rav1dError::EAGAIN) => Ok(None),
      Err(e) => {
        let Ok(classify_decode_error) = self.classify_decode_error(e);

        Err(classify_decode_error)
      }
    }
  }

  pub fn flush(&mut self) -> Result<Vec<Frame>> {
    let mut frames = Vec::new();
    let mut next = self.get_frame();

    #[cfg_attr(
      dylint_lib = "explicit053_no_while",
      expect(
        explicit053_no_while,
        reason = "draining continues only for an available picture and retains the end-of-stream or error outcome for the caller"
      )
    )]
    while let Ok(Some(frame)) = next {
      frames.push(frame);
      next = self.get_frame();
    }

    let Ok(()) = crate::src::lib::rav1d_flush(&self.ctx);

    next.map(|_drained| frames)
  }
}

impl Drop for Decoder {
  fn drop(&mut self) {
    let workers = std::mem::take(&mut self.worker_handles);
    let Ok(()) = self.ctx.stop_workers(workers);
  }
}

#[derive(Clone)]
pub struct Frame {
  inner: Rav1dPicture,
}

impl Frame {
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      let Ok(own_w) = console_core_number_conversion::wrapped::<_, u32>(self.inner.p.w);

      own_w
    })
  }

  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      let Ok(own_h) = console_core_number_conversion::wrapped::<_, u32>(self.inner.p.h);

      own_h
    })
  }

  pub fn bit_depth(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.inner.p.bpc)
  }

  pub fn pixel_layout(&self) -> core::result::Result<PixelLayout, console_core_never::Never> {
    Ok({
      <crate::src::managed::PixelLayout as ::core::convert::From<
        crate::include::dav1d::headers::Rav1dPixelLayout,
      >>::from(self.inner.p.layout)
    })
  }

  pub fn planes(&self) -> Result<Planes<'_>, Error> {
    let data = self.inner.data.as_deref().ok_or(Error::InvalidData)?;
    let Ok(bit_depth) = self.bit_depth();

    match bit_depth {
      8 => Ok(Planes::Depth8(PlaneSet {
        frame: self,
        data,
        depth: PhantomData,
      })),
      10 | 12 => Ok(Planes::Depth16(PlaneSet {
        frame: self,
        data,
        depth: PhantomData,
      })),
      _ => Err(Error::InvalidData),
    }
  }

  pub fn chroma_dimensions(&self) -> Result<Size<u32>, Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(layout) = self.pixel_layout();

    Ok(match layout {
      PixelLayout::I400 => Size {
        width: 0,
        height: 0,
      },
      PixelLayout::I420 => Size {
        width: width.div_ceil(2),
        height: height.div_ceil(2),
      },
      PixelLayout::I422 => Size {
        width: width.div_ceil(2),
        height,
      },
      PixelLayout::I444 => Size { width, height },
    })
  }

  pub fn color_info(&self) -> Result<ColorInfo, Error> {
    let seq_hdr = self.inner.seq_hdr.as_ref().ok_or(Error::InvalidData)?;

    Ok(ColorInfo {
      primaries: ColorPrimaries::from(seq_hdr.rav1d.pri),
      transfer_characteristics: TransferCharacteristics::from(seq_hdr.rav1d.trc),
      matrix_coefficients: MatrixCoefficients::from(seq_hdr.rav1d.mtrx),
      color_range: match seq_hdr.rav1d.color_range {
        0 => ColorRange::Limited,
        _ => ColorRange::Full,
      },
    })
  }

  pub fn content_light(
    &self,
  ) -> core::result::Result<Option<ContentLightLevel>, console_core_never::Never> {
    Ok({
      self
        .inner
        .content_light
        .as_ref()
        .map(|arc| ContentLightLevel {
          max_content_light_level: arc.max_content_light_level,
          max_frame_average_light_level: arc.max_frame_average_light_level,
        })
    })
  }

  pub fn mastering_display(
    &self,
  ) -> core::result::Result<Option<MasteringDisplay>, console_core_never::Never> {
    Ok({
      self
        .inner
        .mastering_display
        .as_ref()
        .map(|arc| MasteringDisplay {
          primaries: arc.primaries,
          white_point: arc.white_point,
          max_luminance: arc.max_luminance,
          min_luminance: arc.min_luminance,
        })
    })
  }

  pub fn timestamp(&self) -> core::result::Result<i64, console_core_never::Never> {
    Ok(self.inner.m.timestamp)
  }

  pub fn duration(&self) -> core::result::Result<i64, console_core_never::Never> {
    Ok(self.inner.m.duration)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelLayout {
  I400,

  I420,

  I422,

  I444,
}

impl From<Rav1dPixelLayout> for PixelLayout {
  fn from(layout: Rav1dPixelLayout) -> Self {
    match layout {
      Rav1dPixelLayout::I400 => Self::I400,
      Rav1dPixelLayout::I420 => Self::I420,
      Rav1dPixelLayout::I422 => Self::I422,
      Rav1dPixelLayout::I444 => Self::I444,
    }
  }
}

pub enum Planes<'a> {
  Depth8(Planes8<'a>),
  Depth16(Planes16<'a>),
}

pub type Planes8<'a> = PlaneSet<'a, BitDepth8>;
pub type Planes16<'a> = PlaneSet<'a, BitDepth16>;

pub struct PlaneSet<'a, Depth> {
  frame: &'a Frame,
  data: &'a Rav1dPictureData,
  depth: PhantomData<Depth>,
}

#[derive(Clone, Copy)]
enum Plane {
  Luma,
  Chroma(ChromaPlane),
}

#[derive(Clone, Copy)]
enum ChromaPlane {
  Blue,
  Red,
}

impl<'a, Depth: BitDepth> PlaneSet<'a, Depth> {
  pub fn y(&self) -> Result<PlaneView<'a, Depth::Pixel>, Never> {
    let Ok(width) = self.frame.width();
    let Ok(height) = self.frame.height();
    self.view(Plane::Luma, Size { width, height })
  }

  pub fn u(&self) -> Result<Option<PlaneView<'a, Depth::Pixel>>, Never> {
    self.chroma(ChromaPlane::Blue)
  }

  pub fn v(&self) -> Result<Option<PlaneView<'a, Depth::Pixel>>, Never> {
    self.chroma(ChromaPlane::Red)
  }

  fn chroma(&self, chroma: ChromaPlane) -> Result<Option<PlaneView<'a, Depth::Pixel>>, Never> {
    let Ok(layout) = self.frame.pixel_layout();

    match layout {
      PixelLayout::I400 => Ok(None),
      PixelLayout::I420 | PixelLayout::I422 | PixelLayout::I444 => {
        let Ok(dimensions) = self.frame.chroma_dimensions();
        let Ok(view) = self.view(Plane::Chroma(chroma), dimensions);

        Ok(Some(view))
      }
    }
  }

  fn view(&self, plane: Plane, size: Size<u32>) -> Result<PlaneView<'a, Depth::Pixel>, Never> {
    let [luma, blue, red] = &self.data.data;
    let [luma_byte_stride, chroma_byte_stride] = self.frame.inner.stride.map(|stride| {
      let Ok(bytes) = fitted::<_, u64>(stride);
      bytes
    });

    let (component, byte_stride) = match plane {
      Plane::Luma => (luma, luma_byte_stride),
      Plane::Chroma(ChromaPlane::Blue) => (blue, chroma_byte_stride),
      Plane::Chroma(ChromaPlane::Red) => (red, chroma_byte_stride),
    };
    let guard = component.slice::<Depth, _>(..);
    let Ok(sample_size) = fitted::<_, u64>(size_of::<Depth::Pixel>());
    let stride = match byte_stride.checked_div(sample_size) {
      Some(samples) => samples,
      None => 0,
    };
    let Ok(samples) = fitted::<_, u64>(guard.len());

    let buffer_height = match samples.checked_div(stride) {
      Some(rows) => rows,
      None => 0,
    };
    let Ok(available_rows) = fitted::<_, u32>(buffer_height);
    let height = size.height.min(available_rows);

    let row_size = Size {
      width: u64::from(size.width),
      height: u64::from(height),
    };
    let row_layout = match i64::try_from(stride) {
      Ok(row_stride) => match RowLayout::new(row_size, 0, row_stride) {
        Ok(layout) => Some(layout),
        Err(RowLayoutError::Empty | RowLayoutError::Stride | RowLayoutError::Bounds) => None,
      },
      Err(_outside_integer_range) => None,
    };
    Ok(PlaneView {
      guard,
      stride,
      size: Size { height, ..size },
      row_layout,
    })
  }
}

pub type PlaneView8<'a> = PlaneView<'a, u8>;
pub type PlaneView16<'a> = PlaneView<'a, u16>;

pub struct PlaneView<'a, Pixel> {
  guard: DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, [Pixel]>,
  stride: u64,
  size: Size<u32>,
  row_layout: Option<RowLayout>,
}

impl<'a, Pixel: Copy> PlaneView<'a, Pixel> {
  pub fn row(&self, y: u32) -> Result<Option<&[Pixel]>, Never> {
    Ok(match &self.row_layout {
      Some(layout) => match layout.row(&self.guard, u64::from(y)) {
        Ok(row) => Some(row),
        Err(RowLayoutError::Empty | RowLayoutError::Stride | RowLayoutError::Bounds) => None,
      },
      None => None,
    })
  }

  pub fn pixel(&self, position: Point<u32>) -> Result<Option<Pixel>, Never> {
    let Ok(column) = index(position.x);
    let Ok(found) = self.row(position.y);

    Ok(match found {
      Some(row) => row.get(column).copied(),
      None => None,
    })
  }

  pub fn rows(&self) -> Result<impl Iterator<Item = &[Pixel]> + '_, Never> {
    let rows = match &self.row_layout {
      Some(layout) => match layout.rows(&self.guard) {
        Ok(rows) => Some(rows),
        Err(RowLayoutError::Empty | RowLayoutError::Stride | RowLayoutError::Bounds) => None,
      },
      None => None,
    };

    Ok(rows.into_iter().flatten())
  }

  pub fn as_slice(&self) -> Result<&[Pixel], Never> {
    Ok(self.guard.deref())
  }

  pub fn width(&self) -> Result<u32, Never> {
    Ok(self.size.width)
  }

  pub fn height(&self) -> Result<u32, Never> {
    Ok(self.size.height)
  }

  pub fn stride(&self) -> Result<u64, Never> {
    Ok(self.stride)
  }
}

#[derive(Clone, Copy, Debug)]
pub struct ColorInfo {
  pub primaries: ColorPrimaries,
  pub transfer_characteristics: TransferCharacteristics,
  pub matrix_coefficients: MatrixCoefficients,
  pub color_range: ColorRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the public discriminants preserve the AV1 CICP color primaries values"
  )
)]
pub enum ColorPrimaries {
  Unknown = 0,
  BT709 = 1,
  Unspecified = 2,
  BT470M = 4,
  BT470BG = 5,
  BT601 = 6,
  SMPTE240 = 7,
  Film = 8,
  BT2020 = 9,
  XYZ = 10,
  SMPTE431 = 11,
  SMPTE432 = 12,
  EBU3213 = 22,
}

impl From<Rav1dColorPrimaries> for ColorPrimaries {
  fn from(pri: Rav1dColorPrimaries) -> Self {
    match pri.0 {
      1 => Self::BT709,
      4 => Self::BT470M,
      5 => Self::BT470BG,
      6 => Self::BT601,
      7 => Self::SMPTE240,
      8 => Self::Film,
      9 => Self::BT2020,
      10 => Self::XYZ,
      11 => Self::SMPTE431,
      12 => Self::SMPTE432,
      22 => Self::EBU3213,
      _ => Self::Unspecified,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the public discriminants preserve the AV1 CICP transfer characteristic values"
  )
)]
pub enum TransferCharacteristics {
  Reserved = 0,
  BT709 = 1,
  Unspecified = 2,
  BT470M = 4,
  BT470BG = 5,
  BT601 = 6,
  SMPTE240 = 7,
  Linear = 8,
  Log100 = 9,
  Log100Sqrt10 = 10,
  IEC61966 = 11,
  BT1361 = 12,
  SRGB = 13,
  BT2020_10bit = 14,
  BT2020_12bit = 15,

  SMPTE2084 = 16,
  SMPTE428 = 17,

  HLG = 18,
}

impl From<Rav1dTransferCharacteristics> for TransferCharacteristics {
  fn from(trc: Rav1dTransferCharacteristics) -> Self {
    match trc.0 {
      1 => Self::BT709,
      4 => Self::BT470M,
      5 => Self::BT470BG,
      6 => Self::BT601,
      7 => Self::SMPTE240,
      8 => Self::Linear,
      9 => Self::Log100,
      10 => Self::Log100Sqrt10,
      11 => Self::IEC61966,
      12 => Self::BT1361,
      13 => Self::SRGB,
      14 => Self::BT2020_10bit,
      15 => Self::BT2020_12bit,
      16 => Self::SMPTE2084,
      17 => Self::SMPTE428,
      18 => Self::HLG,
      _ => Self::Unspecified,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the public discriminants preserve the AV1 CICP matrix coefficient values"
  )
)]
pub enum MatrixCoefficients {
  Identity = 0,
  BT709 = 1,
  Unspecified = 2,
  Reserved = 3,
  FCC = 4,
  BT470BG = 5,
  BT601 = 6,
  SMPTE240 = 7,
  YCgCo = 8,
  BT2020NCL = 9,
  BT2020CL = 10,
  SMPTE2085 = 11,
  ChromaDerivedNCL = 12,
  ChromaDerivedCL = 13,
  ICtCp = 14,
}

impl From<Rav1dMatrixCoefficients> for MatrixCoefficients {
  fn from(mtrx: Rav1dMatrixCoefficients) -> Self {
    match mtrx.0 {
      0 => Self::Identity,
      1 => Self::BT709,
      4 => Self::FCC,
      5 => Self::BT470BG,
      6 => Self::BT601,
      7 => Self::SMPTE240,
      8 => Self::YCgCo,
      9 => Self::BT2020NCL,
      10 => Self::BT2020CL,
      11 => Self::SMPTE2085,
      12 => Self::ChromaDerivedNCL,
      13 => Self::ChromaDerivedCL,
      14 => Self::ICtCp,
      _ => Self::Unspecified,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorRange {
  Limited,

  Full,
}

#[derive(Clone, Copy, Debug)]
pub struct ContentLightLevel {
  pub max_content_light_level: u16,

  pub max_frame_average_light_level: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct MasteringDisplay {
  pub primaries: [[u16; 2]; 3],

  pub white_point: [u16; 2],

  pub max_luminance: u32,

  pub min_luminance: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimaryColorIndexError;

impl std::fmt::Display for PrimaryColorIndexError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("primary color index must be between 0 and 2")
  }
}

impl std::error::Error for PrimaryColorIndexError {}

impl MasteringDisplay {
  pub fn max_luminance_nits(&self) -> core::result::Result<f64, console_core_never::Never> {
    Ok(f64::from(self.max_luminance) / 10000.0)
  }

  pub fn min_luminance_nits(&self) -> core::result::Result<f64, console_core_never::Never> {
    Ok(f64::from(self.min_luminance) / 10000.0)
  }

  pub fn primary_chromaticity(
    &self,
    index: ::console_core_api_compatibility::NativeSize,
  ) -> Result<[f64; 2], PrimaryColorIndexError> {
    let &[x, y] = self.primaries.get(index).ok_or(PrimaryColorIndexError)?;

    Ok([f64::from(x) / 50000.0, f64::from(y) / 50000.0])
  }

  pub fn white_point_chromaticity(
    &self,
  ) -> core::result::Result<[f64; 2], console_core_never::Never> {
    Ok({
      [
        f64::from(match self.white_point {
          [element, ..] => element,
        }) / 50000.0,
        f64::from(match self.white_point {
          [_, element, ..] => element,
        }) / 50000.0,
      ]
    })
  }
}

pub fn enabled_features() -> core::result::Result<String, console_core_never::Never> {
  let features = [
    #[cfg(feature = "c-ffi")]
    "c-ffi",
    #[cfg(feature = "unchecked")]
    "unchecked",
    #[cfg(feature = "bitdepth_8")]
    "bitdepth_8",
    #[cfg(feature = "bitdepth_16")]
    "bitdepth_16",
    #[cfg(feature = "c-ffi")]
    "safety:c-ffi",
    #[cfg(all(feature = "unchecked", not(feature = "c-ffi")))]
    "safety:unchecked",
    #[cfg(not(any(feature = "unchecked", feature = "c-ffi")))]
    "safety:forbid-unsafe",
  ];

  Ok(features.join(", "))
}

#[cfg(test)]
#[path = "managed/frame_tile_tests.rs"]
mod frame_tile_tests;
