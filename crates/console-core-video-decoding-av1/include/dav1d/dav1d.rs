#![forbid(unsafe_code)]
#[cfg(feature = "c-ffi")]
use crate::include::dav1d::picture::Dav1dPicAllocator;
use crate::include::dav1d::picture::Rav1dPicAllocator;
#[cfg(feature = "c-ffi")]
use crate::src::c_arc::RawArc;
use crate::src::error::Rav1dError;
#[cfg(feature = "c-ffi")]
use crate::src::internal::Rav1dContext;
#[cfg(feature = "c-ffi")]
pub use crate::src::log::Dav1dLogger;
use crate::src::log::Rav1dLogger;
use crate::src::managed::Strictness;
use std::ffi::c_int;
use std::ffi::c_uint;

#[cfg(feature = "c-ffi")]
pub type Dav1dContext = RawArc<Rav1dContext>;

#[cfg(feature = "c-ffi")]
pub type Dav1dRef = ();

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "bitflags generates primitive value accessors for the existing filter and event masks"
  )
)]
mod codec_flags {
  use bitflags::bitflags;

  bitflags! {
      #[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
      pub(crate) struct Rav1dInloopFilterType: u8 {
          const DEBLOCK = 1 << 1;
          const CDEF = 1 << 2;
          const RESTORATION = 1 << 3;
      }
  }

  bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub(crate) struct Rav1dEventFlags: u8 {
      #[doc = "The decoder started a new coded sequence or was flushed before decoding the picture."]
      const NEW_SEQUENCE = 1 << 0;
      #[doc = "The current coded sequence has new operating parameters."]
      const NEW_OP_PARAMS_INFO = 1 << 1;
    }
  }
}

pub(crate) use codec_flags::{Rav1dEventFlags, Rav1dInloopFilterType};

pub type Dav1dInloopFilterType = c_uint;
pub const DAV1D_INLOOPFILTER_ALL: Dav1dInloopFilterType = ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dInloopFilterType, Rav1dInloopFilterType::all().bits());
pub const DAV1D_INLOOPFILTER_NONE: Dav1dInloopFilterType = ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dInloopFilterType, Rav1dInloopFilterType::empty().bits());
pub const DAV1D_INLOOPFILTER_DEBLOCK: Dav1dInloopFilterType = ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dInloopFilterType, Rav1dInloopFilterType::DEBLOCK.bits());
pub const DAV1D_INLOOPFILTER_CDEF: Dav1dInloopFilterType = ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dInloopFilterType, Rav1dInloopFilterType::CDEF.bits());
pub const DAV1D_INLOOPFILTER_RESTORATION: Dav1dInloopFilterType = ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dInloopFilterType, Rav1dInloopFilterType::RESTORATION.bits());

impl From<Rav1dInloopFilterType> for Dav1dInloopFilterType {
  fn from(value: Rav1dInloopFilterType) -> Self {
    <::core::primitive::u32 as ::core::convert::From<::core::primitive::u8>>::from(value.bits())
  }
}

impl From<Dav1dInloopFilterType> for Rav1dInloopFilterType {
  fn from(value: Dav1dInloopFilterType) -> Self {
    Self::from_bits_retain(
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(value) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
    )
  }
}

pub type Dav1dDecodeFrameType = c_uint;
pub const DAV1D_DECODEFRAMETYPE_ALL: Dav1dDecodeFrameType = 0;
pub const DAV1D_DECODEFRAMETYPE_REFERENCE: Dav1dDecodeFrameType = 1;
pub const DAV1D_DECODEFRAMETYPE_INTRA: Dav1dDecodeFrameType = 2;
pub const DAV1D_DECODEFRAMETYPE_KEY: Dav1dDecodeFrameType = 3;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) enum Rav1dDecodeFrameType {
  #[default]
  All,
  Reference,
  Intra,
  Key,
}

impl From<Rav1dDecodeFrameType> for Dav1dDecodeFrameType {
  fn from(value: Rav1dDecodeFrameType) -> Self {
    match value {
      Rav1dDecodeFrameType::All => DAV1D_DECODEFRAMETYPE_ALL,
      Rav1dDecodeFrameType::Reference => DAV1D_DECODEFRAMETYPE_REFERENCE,
      Rav1dDecodeFrameType::Intra => DAV1D_DECODEFRAMETYPE_INTRA,
      Rav1dDecodeFrameType::Key => DAV1D_DECODEFRAMETYPE_KEY,
    }
  }
}

impl TryFrom<Dav1dDecodeFrameType> for Rav1dDecodeFrameType {
  type Error = Rav1dError;

  fn try_from(value: Dav1dDecodeFrameType) -> Result<Self, Self::Error> {
    match value {
      DAV1D_DECODEFRAMETYPE_ALL => Ok(Self::All),
      DAV1D_DECODEFRAMETYPE_REFERENCE => Ok(Self::Reference),
      DAV1D_DECODEFRAMETYPE_INTRA => Ok(Self::Intra),
      DAV1D_DECODEFRAMETYPE_KEY => Ok(Self::Key),
      _ => Err(Rav1dError::EINVAL),
    }
  }
}

pub type Dav1dEventFlags = c_uint;
pub const DAV1D_EVENT_FLAG_NEW_SEQUENCE: Dav1dEventFlags = ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dEventFlags, Rav1dEventFlags::NEW_SEQUENCE.bits());
pub const DAV1D_EVENT_FLAG_NEW_OP_PARAMS_INFO: Dav1dEventFlags = ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dEventFlags, Rav1dEventFlags::NEW_OP_PARAMS_INFO.bits());

impl From<Rav1dEventFlags> for Dav1dEventFlags {
  fn from(value: Rav1dEventFlags) -> Self {
    <::core::primitive::u32 as ::core::convert::From<::core::primitive::u8>>::from(value.bits())
  }
}

impl From<Dav1dEventFlags> for Rav1dEventFlags {
  fn from(value: Dav1dEventFlags) -> Self {
    Self::from_bits_retain(
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(value) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
    )
  }
}

#[cfg(feature = "c-ffi")]
#[repr(C)]
pub struct Dav1dSettings {
  pub n_threads: c_int,
  pub max_frame_delay: c_int,
  pub apply_grain: c_int,
  pub operating_point: c_int,
  pub all_layers: c_int,
  pub frame_size_limit: c_uint,
  pub allocator: Dav1dPicAllocator,
  pub logger: Dav1dLogger,
  pub strict_std_compliance: c_int,
  pub output_invisible_frames: c_int,
  pub inloop_filters: Dav1dInloopFilterType,
  pub decode_frame_type: Dav1dDecodeFrameType,
  pub reserved: [u8; 16],
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the settings ABI includes the target's native allocator and logger pointer fields"
  )
)]
mod settings_abi {
  use super::Dav1dSettings;
  use console_core_api_compatibility::NativeSize;
  use core::mem::{align_of, offset_of, size_of};

  const POINTER_BYTES: NativeSize = size_of::<*const ()>();
  const SETTINGS_ALIGN: NativeSize = align_of::<*const ()>();
  const ALLOCATOR: NativeSize = 24;
  const LOGGER: NativeSize = ALLOCATOR.wrapping_add(POINTER_BYTES.wrapping_mul(3));
  const STRICTNESS: NativeSize = LOGGER.wrapping_add(POINTER_BYTES.wrapping_mul(2));
  const INVISIBLE: NativeSize = STRICTNESS.wrapping_add(4);
  const FILTERS: NativeSize = INVISIBLE.wrapping_add(4);
  const FRAME_TYPE: NativeSize = FILTERS.wrapping_add(4);
  const RESERVED: NativeSize = FRAME_TYPE.wrapping_add(4);
  const SETTINGS_BYTES: NativeSize = RESERVED.wrapping_add(16);

  const _: () = {
    assert!(size_of::<Dav1dSettings>() == SETTINGS_BYTES);
    assert!(align_of::<Dav1dSettings>() == SETTINGS_ALIGN);
    assert!(offset_of!(Dav1dSettings, n_threads) == 0);
    assert!(offset_of!(Dav1dSettings, max_frame_delay) == 4);
    assert!(offset_of!(Dav1dSettings, apply_grain) == 8);
    assert!(offset_of!(Dav1dSettings, operating_point) == 12);
    assert!(offset_of!(Dav1dSettings, all_layers) == 16);
    assert!(offset_of!(Dav1dSettings, frame_size_limit) == 20);
    assert!(offset_of!(Dav1dSettings, allocator) == ALLOCATOR);
    assert!(offset_of!(Dav1dSettings, logger) == LOGGER);
    assert!(offset_of!(Dav1dSettings, strict_std_compliance) == STRICTNESS);
    assert!(offset_of!(Dav1dSettings, output_invisible_frames) == INVISIBLE);
    assert!(offset_of!(Dav1dSettings, inloop_filters) == FILTERS);
    assert!(offset_of!(Dav1dSettings, decode_frame_type) == FRAME_TYPE);
    assert!(offset_of!(Dav1dSettings, reserved) == RESERVED);
  };
}

#[repr(C)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "grain synthesis, layer copying, invisible frame output and logging are independent decoder options"
  )
)]
pub(crate) struct Rav1dSettings {
  pub n_threads: c_int,
  pub max_frame_delay: c_int,
  pub apply_grain: bool,
  pub operating_point: u8,
  pub all_layers: bool,
  pub frame_size_limit: c_uint,
  pub allocator: Rav1dPicAllocator,
  pub logger: Option<Rav1dLogger>,
  #[doc = "A nonzero C strictness value requests strict validation. Zero uses dav1d's lenient default."]
  pub strictness: Strictness,
  pub output_invisible_frames: bool,
  pub inloop_filters: Rav1dInloopFilterType,
  pub decode_frame_type: Rav1dDecodeFrameType,
}

#[cfg(feature = "c-ffi")]
impl Dav1dSettings {
  pub(crate) fn into_rust(self) -> Result<Rav1dSettings, Rav1dError> {
    let Dav1dSettings {
      n_threads,
      max_frame_delay,
      apply_grain,
      operating_point,
      all_layers,
      frame_size_limit,
      allocator,
      logger,
      strict_std_compliance,
      output_invisible_frames,
      inloop_filters,
      decode_frame_type,
      reserved: _,
    } = self;
    let operating_index =
      u8::try_from(operating_point).map_err(|_invalid_index| Rav1dError::EINVAL)?;
    let picture_allocator = Rav1dPicAllocator::try_from(allocator)?;
    let frame_type = Rav1dDecodeFrameType::try_from(decode_frame_type)?;

    Ok(Rav1dSettings {
      n_threads,
      max_frame_delay,
      apply_grain: apply_grain != 0,
      operating_point: operating_index,
      all_layers: all_layers != 0,
      frame_size_limit,
      allocator: picture_allocator,
      logger: Option::<Rav1dLogger>::from(logger),
      strictness: match strict_std_compliance {
        0 => Strictness::Lenient,
        _ => Strictness::Strict,
      },
      output_invisible_frames: output_invisible_frames != 0,
      inloop_filters: Rav1dInloopFilterType::from(inloop_filters),
      decode_frame_type: frame_type,
    })
  }
}

#[cfg(feature = "c-ffi")]
impl From<Rav1dSettings> for Dav1dSettings {
  fn from(value: Rav1dSettings) -> Self {
    let Rav1dSettings {
      n_threads,
      max_frame_delay,
      apply_grain,
      operating_point,
      all_layers,
      frame_size_limit,
      allocator,
      logger,
      strictness,
      output_invisible_frames,
      inloop_filters,
      decode_frame_type,
    } = value;
    Self {
      n_threads,
      max_frame_delay,
      apply_grain: <c_int as ::core::convert::From<::core::primitive::bool>>::from(apply_grain),
      operating_point:
        <::core::primitive::i32 as ::core::convert::From<::core::primitive::u8>>::from(
          operating_point,
        ),
      all_layers: <c_int as ::core::convert::From<::core::primitive::bool>>::from(all_layers),
      frame_size_limit,
      allocator: <crate::include::dav1d::picture::Dav1dPicAllocator as ::core::convert::From<
        crate::include::dav1d::picture::Rav1dPicAllocator,
      >>::from(allocator),
      logger: <crate::src::log::Dav1dLogger as ::core::convert::From<
        ::core::option::Option<crate::src::log::Rav1dLogger>,
      >>::from(logger),
      strict_std_compliance: <c_int as ::core::convert::From<::core::primitive::bool>>::from(
        strictness >= Strictness::Strict,
      ),
      output_invisible_frames: <c_int as ::core::convert::From<::core::primitive::bool>>::from(
        output_invisible_frames,
      ),
      inloop_filters: <::core::primitive::u32 as ::core::convert::From<
        crate::include::dav1d::dav1d::Rav1dInloopFilterType,
      >>::from(inloop_filters),
      decode_frame_type: <::core::primitive::u32 as ::core::convert::From<
        crate::include::dav1d::dav1d::Rav1dDecodeFrameType,
      >>::from(decode_frame_type),
      reserved: Default::default(),
    }
  }
}
