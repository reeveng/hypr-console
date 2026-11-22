#![deny(unsafe_op_in_unsafe_fn)]

use crate::include::dav1d::common::Rav1dDataProps;
use crate::include::dav1d::dav1d::Rav1dEventFlags;
use crate::include::dav1d::headers::DRav1d;
use crate::include::dav1d::headers::Dav1dFrameHeader;
use crate::include::dav1d::headers::Dav1dITUTT35;
use crate::include::dav1d::headers::Dav1dSequenceHeader;
use crate::include::dav1d::headers::Rav1dContentLightLevel;
use crate::include::dav1d::headers::Rav1dFrameHeader;
use crate::include::dav1d::headers::Rav1dITUTT35;
use crate::include::dav1d::headers::Rav1dMasteringDisplay;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dSequenceHeader;
#[cfg(feature = "c-ffi")]
use crate::include::dav1d::picture::Dav1dPicture;
use crate::include::dav1d::picture::RAV1D_PICTURE_ALIGNMENT;
use crate::include::dav1d::picture::Rav1dPicture;
use crate::include::dav1d::picture::Rav1dPictureParameters;
use crate::include::dav1d::picture::{PictureThreading, Rav1dPicAllocator};
#[cfg(feature = "c-ffi")]
use crate::src::error::Dav1dResult;
use crate::src::error::Rav1dError::{EGeneric, EINVAL, ENOMEM};
use crate::src::error::Rav1dResult;
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dFrameData;
use crate::src::log::Rav1dLog as _;
use crate::src::log::Rav1dLogger;
#[cfg(feature = "c-ffi")]
use crate::src::mem::MemPool;
#[cfg(feature = "c-ffi")]
use crate::src::send_sync_non_null::{Dav1dOpaquePointer, SendSyncNonNull};
use console_core_geometry::Size;
use console_core_never::Never;
use core::ops::{BitAndAssign, BitOr, BitOrAssign};
use std::ffi::c_int;
#[cfg(feature = "c-ffi")]
use std::mem;
#[cfg(feature = "c-ffi")]
use std::ptr::NonNull;
#[cfg(feature = "c-ffi")]
use std::ptr::fn_addr_eq;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicU32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct PictureFlags(u8);

pub(crate) enum TemporalUnitChange {
  Unchanged,
  New,
}

impl PictureFlags {
  const EMPTY: Self = Self(0);
  pub const NEW_SEQUENCE: Self = Self(1 << 0);
  pub const NEW_OP_PARAMS_INFO: Self = Self(1 << 1);
  pub const NEW_TEMPORAL_UNIT: Self = Self(1 << 2);

  pub(crate) fn temporal_unit(self) -> Result<TemporalUnitChange, Never> {
    Ok(match self.0 & Self::NEW_TEMPORAL_UNIT.0 {
      0 => TemporalUnitChange::Unchanged,
      _ => TemporalUnitChange::New,
    })
  }
}

impl BitOr for PictureFlags {
  type Output = Self;

  fn bitor(self, flags: Self) -> Self::Output {
    Self(self.0 | flags.0)
  }
}

impl BitOrAssign for PictureFlags {
  fn bitor_assign(&mut self, flags: Self) {
    self.0 |= flags.0;
  }
}

impl BitAndAssign for PictureFlags {
  fn bitand_assign(&mut self, flags: Self) {
    self.0 &= flags.0;
  }
}

impl From<PictureFlags> for Rav1dEventFlags {
  fn from(value: PictureFlags) -> Self {
    Self::from_bits_truncate(value.0)
  }
}

#[derive(Clone, Default)]
#[repr(C)]
pub(crate) struct Rav1dThreadPicture {
  pub p: Rav1dPicture,
  pub visible: bool,

  pub showable: bool,
  pub flags: PictureFlags,

  pub progress: Option<Arc<[AtomicU32; 2]>>,
}

#[cfg(feature = "c-ffi")]
struct MemPoolBuf<T> {
  pool: Arc<MemPool<T>>,
  buf: Vec<T>,
}

impl Rav1dPictureParameters {
  pub(crate) fn pic_len(&self, [y_stride, uv_stride]: [i64; 2]) -> Rav1dResult<[u64; 2]> {
    let height = u32::try_from(self.h).map_err(|_| ENOMEM)?;
    let padded_height = height.checked_add(127).ok_or(ENOMEM)?;
    let aligned_height = u64::from(padded_height & !127);
    let chroma_height = match self.layout {
      Rav1dPixelLayout::I420 => ::core::primitive::u64::wrapping_shr(aligned_height, 1_u32),
      Rav1dPixelLayout::I400 | Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => aligned_height,
    };
    let luma_bytes = y_stride.unsigned_abs().checked_mul(aligned_height).ok_or(ENOMEM)?;
    let chroma_bytes = uv_stride.unsigned_abs().checked_mul(chroma_height).ok_or(ENOMEM)?;
    Ok([luma_bytes, chroma_bytes])
  }

  pub(crate) fn default_strides(&self) -> Rav1dResult<[i64; 2]> {
    let width = u32::try_from(self.w).map_err(|_| ENOMEM)?;
    let padded_width = width.checked_add(127).ok_or(ENOMEM)?;
    let aligned_width = u64::from(padded_width & !127);
    let luma_bytes = ::core::primitive::u64::wrapping_shl(aligned_width, u32::from(self.bpc > 8));
    let chroma_bytes = match self.layout {
      Rav1dPixelLayout::I400 => None,
      Rav1dPixelLayout::I444 => Some(luma_bytes),
      Rav1dPixelLayout::I420 | Rav1dPixelLayout::I422 => Some(::core::primitive::u64::wrapping_shr(luma_bytes, 1_u32)),
    };
    let padded_stride = |row_bytes: u64| -> Rav1dResult<i64> {
      let length = match row_bytes & 1023 {
        0 => {
          let Ok(alignment) = console_core_number_conversion::fixed_index(RAV1D_PICTURE_ALIGNMENT);
          row_bytes.checked_add(alignment).ok_or(ENOMEM)?
        }
        _ => row_bytes,
      };
      i64::try_from(length).map_err(|_| ENOMEM)
    };
    let luma = padded_stride(luma_bytes)?;
    let chroma = match chroma_bytes {
      Some(bytes) => padded_stride(bytes)?,
      None => 0,
    };
    Ok([luma, chroma])
  }
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C callback borrows initialized picture fields and writes into the caller's allocation slots"
  )
)]
unsafe extern "C" fn dav1d_default_picture_alloc(
  picture: *mut Dav1dPicture,
  _cookie: Option<Dav1dOpaquePointer>,
) -> Dav1dResult {
  // SAFETY: The allocator callback supplies an initialized, uniquely writable C picture.
  let status: Rav1dResult = match unsafe { picture.as_mut() } {
    Some(output) => {
      let Ok(pool) = MemPool::new();
      Rav1dPicAllocator::default_picture_alloc_with_pool(output, Arc::new(pool))
    }
    None => Err(EINVAL),
  };
  Dav1dResult::from(status)
}

#[cfg(feature = "c-ffi")]
impl Rav1dPicAllocator {
  pub(crate) fn default_picture_alloc_with_pool(
    picture: &mut Dav1dPicture,
    pool: Arc<MemPool<u8>>,
  ) -> Rav1dResult {
    let parameters = Rav1dPictureParameters::try_from(picture.p)?;
    let strides = parameters.default_strides()?;
    let [luma_stride, chroma_stride] = strides;
    let y_stride = console_core_number_conversion::checked_offset(luma_stride).map_err(|_| ENOMEM)?;
    let uv_stride = console_core_number_conversion::checked_offset(chroma_stride).map_err(|_| ENOMEM)?;
    let [luma_bytes, chroma_bytes] = parameters.pic_len(strides)?;
    let total_bytes = chroma_bytes.checked_mul(2)
      .and_then(|both_chroma_planes| luma_bytes.checked_add(both_chroma_planes))
      .ok_or(ENOMEM)?;
    let Ok(alignment) = console_core_number_conversion::fixed_index(RAV1D_PICTURE_ALIGNMENT);
    let capacity = total_bytes.checked_add(alignment).ok_or(ENOMEM)?;
    let buf = pool.pop_init(capacity, 0)?;
    let mut allocation = Box::new(MemPoolBuf { pool, buf });
    let Ok(prefix) = console_core_number_conversion::fixed_index(allocation.buf.as_ptr().align_offset(RAV1D_PICTURE_ALIGNMENT));
    let end = prefix.checked_add(total_bytes).ok_or(ENOMEM)?;
    let start = console_core_number_conversion::checked_index(prefix).map_err(|_| ENOMEM)?;
    let finish = console_core_number_conversion::checked_index(end).map_err(|_| ENOMEM)?;
    let pixels = allocation.buf.get_mut(start..finish).ok_or(ENOMEM)?;
    let Ok(chroma_start) = console_core_number_conversion::index(luma_bytes);
    let Ok(v_start) = console_core_number_conversion::index(chroma_bytes);
    let (y, chroma) = pixels.split_at_mut_checked(chroma_start).ok_or(ENOMEM)?;
    let (u, v) = chroma.split_at_mut_checked(v_start).ok_or(ENOMEM)?;
    let data = [y, u, v].map(|plane| match plane {
      [] => None,
      _ => Some(NonNull::from(plane).cast()),
    });
    let Ok(owner) = SendSyncNonNull::from_box(allocation);
    let Ok(cookie) = owner.cast();

    picture.stride = [y_stride, uv_stride];
    picture.data = data;
    picture.allocator_data = Some(cookie);
    Ok(())
  }
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the default picture release callback takes the matching allocated buffer back from its C cookie"
  )
)]
unsafe extern "C" fn dav1d_default_picture_release(
  picture: *mut Dav1dPicture,
  _cookie: Option<Dav1dOpaquePointer>,
) {
  // SAFETY: The release callback receives exclusive access to the matching allocation fields.
  let cookie = match unsafe { picture.as_mut() }.and_then(|output| output.allocator_data.take()) {
    Some(owner) => owner,
    None => return,
  };
  let Ok(raw_owner) = cookie.cast::<MemPoolBuf<u8>>();
  // SAFETY: The matching default allocator transferred this Box to the release callback.
  let Ok(allocation) = unsafe { raw_owner.into_box() };
  let MemPoolBuf { pool, buf } = *allocation;
  let Ok(()) = pool.push(buf);
}

#[cfg(feature = "c-ffi")]
impl Default for Rav1dPicAllocator {
  fn default() -> Self {
    let Ok(mem_pool) = MemPool::new();

    Self {
      cookie: None,
      default_pool: Some(Arc::new(mem_pool)),
      // SAFETY: `dav1d_default_picture_alloc` requires `p_c` be from a `&mut Dav1dPicture`,
      // `Self::alloc_picture_callback` safety preconditions guarantee that.
      alloc_picture_callback: dav1d_default_picture_alloc,
      // SAFETY: `dav1d_default_picture_release` requires `p` be from a `&mut Dav1dPicture`
      // initialized by `dav1d_default_picture_alloc`.
      // Since these `fn`s are private and we only use them here, that is guaranteed.
      release_picture_callback: dav1d_default_picture_release,
    }
  }
}

#[cfg(feature = "c-ffi")]
#[derive(PartialEq, Eq)]
pub enum AllocationKind {
  Default,
  Custom,
}

#[cfg(feature = "c-ffi")]
impl From<bool> for AllocationKind {
  fn from(default: bool) -> Self {
    match default {
      true => Self::Default,
      false => Self::Custom,
    }
  }
}

#[cfg(feature = "c-ffi")]
impl Rav1dPicAllocator {
  #[must_use]
  pub fn allocation_kind(&self) -> AllocationKind {
    match self.default_pool {
      Some(_) => return AllocationKind::Default,
      None => {}
    }

    let default_alloc: unsafe extern "C" fn(
      *mut Dav1dPicture,
      Option<Dav1dOpaquePointer>,
    ) -> Dav1dResult = dav1d_default_picture_alloc;
    let default_release: unsafe extern "C" fn(*mut Dav1dPicture, Option<Dav1dOpaquePointer>) =
      dav1d_default_picture_release;
    let alloc = fn_addr_eq(self.alloc_picture_callback, default_alloc);
    let release = fn_addr_eq(self.release_picture_callback, default_release);
    AllocationKind::from(alloc && release)
  }
}

struct PictureAllocation {
  size: Size<i32>,
  sequence_header: Option<Arc<DRav1d<Rav1dSequenceHeader, Dav1dSequenceHeader>>>,
  frame_header: Option<Arc<DRav1d<Rav1dFrameHeader, Dav1dFrameHeader>>>,
  bit_depth: u8,
  threading: Option<PictureThreading>,
}

fn picture_alloc_with_edges(
  logger: &Option<Rav1dLogger>,
  allocator: &Rav1dPicAllocator,
  output: &mut Rav1dPicture,
  request: PictureAllocation,
) -> Rav1dResult {
  match output.data {
    Some(_) => {
      writeln!(logger, "Picture already allocated!");
      return Err(EGeneric);
    }
    None => {}
  }

  match request.bit_depth {
    1..=16 => {}
    _ => return Err(EINVAL),
  }

  let sequence = request.sequence_header.ok_or(EINVAL)?;
  let mut picture = allocator.alloc_picture_data(request.size.width, request.size.height, sequence, request.frame_header)?;

  match request.threading {
    Some(policy) => {
      let data = picture.data.as_mut().and_then(Arc::get_mut).ok_or(EGeneric)?;

      for plane in &mut data.data {
        let Ok(()) = plane.set_threading_policy(policy);
      }
    }
    None => {}
  }

  *output = picture;
  Ok(())
}

pub fn rav1d_picture_copy_props(
  p: &mut Rav1dPicture,
  content_light: Option<Arc<Rav1dContentLightLevel>>,
  mastering_display: Option<Arc<Rav1dMasteringDisplay>>,
  itut_t35: Arc<DRav1d<Box<[Rav1dITUTT35]>, Box<[Dav1dITUTT35]>>>,
  props: Rav1dDataProps,
) -> core::result::Result<(), console_core_never::Never> {
  {
    p.m = props;
    p.content_light = content_light;
    p.mastering_display = mastering_display;
    p.itut_t35 = itut_t35;
  };

  Ok(())
}

#[derive(PartialEq, Eq)]
pub(crate) enum OutputScope {
  VisibleFrames,
  AllFrames,
}

impl From<bool> for OutputScope {
  fn from(include_invisible: bool) -> Self {
    match include_invisible {
      true => Self::AllFrames,
      false => Self::VisibleFrames,
    }
  }
}

pub(crate) struct ThreadPictureRequest<'a> {
  pub(crate) content_light: Option<Arc<Rav1dContentLightLevel>>,
  pub(crate) mastering_display: Option<Arc<Rav1dMasteringDisplay>>,
  pub(crate) max_spatial_id: u8,
  pub(crate) frame_flags: &'a mut PictureFlags,
  pub(crate) bit_depth: u8,
  pub(crate) itut_t35: Arc<Mutex<Vec<Rav1dITUTT35>>>,
}

pub(crate) fn rav1d_thread_picture_alloc(
  decoder: &Rav1dContext,
  frame: &mut Rav1dFrameData,
  request: ThreadPictureRequest<'_>,
) -> Rav1dResult {
  let ThreadPictureRequest { content_light, mastering_display, max_spatial_id, frame_flags, bit_depth, mut itut_t35 } = request;
  let p = &mut frame.sr_cur;
  let have_frame_mt = decoder.fc.len() > 1;
  let output_scope = OutputScope::from(decoder.output_invisible_frames);
  let frame_header = frame.frame_hdr.as_ref().ok_or(EGeneric)?;
  let frame_hdr = &***frame_header;
  let first_tile = frame.tiles.first().ok_or(EINVAL)?;
  let immutable_metadata = Rav1dITUTT35::to_immut(&mut itut_t35)?;
  let [_, width] = frame_hdr.size.width;
  let tiles = u64::from(frame_hdr.tiling.cols).wrapping_mul(u64::from(frame_hdr.tiling.rows));
  let Ok(workers) = console_core_number_conversion::fixed_index(decoder.tc.len());
  let counts = crate::include::dav1d::picture::PictureThreadingCounts { workers, tiles };
  let Ok(policy) = PictureThreading::new(counts);
  picture_alloc_with_edges(&decoder.logger, &decoder.allocator, &mut p.p, PictureAllocation {
    size: Size { width, height: frame_hdr.size.height },
    sequence_header: frame.seq_hdr.clone(),
    frame_header: frame.frame_hdr.clone(),
    bit_depth,
    threading: Some(policy),
  })?;

  let Ok(()) = rav1d_picture_copy_props(
    &mut p.p,
    content_light,
    mastering_display,
    immutable_metadata,
    first_tile.data.m.clone(),
  );

  let flags_mask = match ::core::primitive::bool::then_some((frame_hdr.show_frame != 0 || output_scope == OutputScope::AllFrames)
    && max_spatial_id == frame_hdr.spatial_id, ())
  {
    ::core::option::Option::Some(()) => PictureFlags::EMPTY,
    ::core::option::Option::None => PictureFlags::NEW_SEQUENCE | PictureFlags::NEW_OP_PARAMS_INFO,
  };
  p.flags = *frame_flags;
  *frame_flags &= flags_mask;
  p.visible = frame_hdr.show_frame != 0;
  p.showable = frame_hdr.showable_frame != 0;
  p.progress = match ::core::primitive::bool::then_some(have_frame_mt, ()) {
    ::core::option::Option::Some(()) => Some(Default::default()),
    ::core::option::Option::None => None,
  };
  Ok(())
}

pub(crate) fn rav1d_picture_alloc_copy(
  logger: &Option<Rav1dLogger>,
  dst: &mut Rav1dPicture,
  w: c_int,
  src: &Rav1dPicture,
) -> Rav1dResult {
  let source = src.data.as_ref().ok_or(EINVAL)?;
  let [luma, _, _] = &source.data;
  let Ok(threading) = luma.threading_policy();
  picture_alloc_with_edges(logger, &source.allocator, dst, PictureAllocation {
    size: Size { width: w, height: src.p.h },
    sequence_header: src.seq_hdr.clone(),
    frame_header: src.frame_hdr.clone(),
    bit_depth: src.p.bpc,
    threading,
  })?;

  let Ok(()) = rav1d_picture_copy_props(
    dst,
    src.content_light.clone(),
    src.mastering_display.clone(),
    src.itut_t35.clone(),
    src.m.clone(),
  );
  Ok(())
}
