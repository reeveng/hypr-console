use crate::src::c_arc::CArc;
#[cfg(feature = "c-ffi")]
use crate::src::c_arc::RawCArc;
#[cfg(feature = "c-ffi")]
use console_core_api_compatibility::NativeSize;
#[cfg(feature = "c-ffi")]
use console_core_number_conversion::{fixed_index, index};
#[cfg(feature = "c-ffi")]
use std::ptr::NonNull;

#[cfg(feature = "c-ffi")]
#[derive(Default)]
#[repr(C)]
pub struct Dav1dUserData {
  pub data: Option<NonNull<u8>>,
  pub r#ref: Option<RawCArc<u8>>,
}

pub(crate) type Rav1dUserData = Option<CArc<u8>>;

#[cfg(feature = "c-ffi")]
impl From<Dav1dUserData> for Rav1dUserData {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C user-data object hands back the outstanding owned handle from CArc::into_raw"
    )
  )]
  fn from(value: Dav1dUserData) -> Self {
    let Dav1dUserData { data: _, r#ref } = value;
    r#ref.map(|ref_2| {
      // SAFETY: `r#ref` came from `CArc::into_raw`.
      unsafe {
        let Ok(c_arc) = CArc::from_raw(ref_2);
        c_arc
      }
    })
  }
}

#[cfg(feature = "c-ffi")]
impl From<Rav1dUserData> for Dav1dUserData {
  fn from(value: Rav1dUserData) -> Self {
    Self {
      data: value
        .as_ref()
        .map(|user_data| NonNull::from(user_data.as_ref())),
      r#ref: value.map(|user_data| {
        let Ok(raw) = user_data.into_raw();

        raw
      }),
    }
  }
}

#[cfg(feature = "c-ffi")]
#[derive(Default)]
#[repr(C)]
pub struct Dav1dDataProps {
  pub timestamp: i64,
  pub duration: i64,
  pub offset: i64,
  pub size: NativeSize,
  pub user_data: Dav1dUserData,
}

#[derive(Clone)]
#[repr(C)]
pub(crate) struct Rav1dDataProps {
  pub timestamp: i64,
  pub duration: i64,
  pub offset: i64,
  pub size: u64,
  pub user_data: Rav1dUserData,
}

impl Default for Rav1dDataProps {
  fn default() -> Self {
    Self {
      timestamp: i64::MIN,
      duration: 0,
      offset: -1,
      size: 0,
      user_data: Default::default(),
    }
  }
}

#[cfg(feature = "c-ffi")]
impl Rav1dDataProps {
  #[doc = "\n# Safety\nThe C record must hold a live user-data reference, when present, until this call returns."]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C caller owns the user-data handle for the lifetime of the metadata borrow"
    )
  )]
  pub(crate) unsafe fn from_borrowed(
    properties: &Dav1dDataProps,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(size) = fixed_index(properties.size);
    let user_data = properties.user_data.r#ref.as_ref().map(|handle| {
      // SAFETY: The C record retains its live reference until the retain returns.
      let Ok(retained) = unsafe { CArc::borrow_clone(handle) };

      retained
    });

    Ok(Self {
      timestamp: properties.timestamp,
      duration: properties.duration,
      offset: properties.offset,
      size,
      user_data,
    })
  }
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the C packet layout follows the target pointer width and the C ABI alignment for its 64-bit timestamps"
  )
)]
mod data_abi {
  use super::{Dav1dDataProps, Dav1dUserData, NativeSize};
  use super::super::data::Dav1dData;
  use core::mem::{align_of, offset_of, size_of};

  const POINTER_BYTES: NativeSize = size_of::<*const u8>();
  const PROPS_ALIGN: NativeSize = align_of::<i64>();
  const USER_DATA_OFFSET: NativeSize = 24usize.wrapping_add(POINTER_BYTES);
  const PROPS_BYTES: NativeSize = USER_DATA_OFFSET
    .wrapping_add(POINTER_BYTES.wrapping_mul(2))
    .next_multiple_of(PROPS_ALIGN);
  const PACKET_REF: NativeSize = POINTER_BYTES.wrapping_mul(2);
  const PACKET_PROPERTIES: NativeSize = POINTER_BYTES
    .wrapping_mul(3)
    .next_multiple_of(PROPS_ALIGN);
  const PACKET_BYTES: NativeSize = PACKET_PROPERTIES.wrapping_add(PROPS_BYTES);

  const _: () = {
    assert!(size_of::<Dav1dUserData>() == POINTER_BYTES.wrapping_mul(2));
    assert!(align_of::<Dav1dUserData>() == align_of::<*const u8>());
    assert!(offset_of!(Dav1dUserData, data) == 0);
    assert!(offset_of!(Dav1dUserData, r#ref) == POINTER_BYTES);

    assert!(PROPS_ALIGN >= align_of::<*const u8>());
    assert!(size_of::<Dav1dDataProps>() == PROPS_BYTES);
    assert!(align_of::<Dav1dDataProps>() == PROPS_ALIGN);
    assert!(offset_of!(Dav1dDataProps, timestamp) == 0);
    assert!(offset_of!(Dav1dDataProps, duration) == 8);
    assert!(offset_of!(Dav1dDataProps, offset) == 16);
    assert!(offset_of!(Dav1dDataProps, size) == 24);
    assert!(offset_of!(Dav1dDataProps, user_data) == USER_DATA_OFFSET);

    assert!(align_of::<Dav1dData>() == PROPS_ALIGN);
    assert!(offset_of!(Dav1dData, data) == 0);
    assert!(offset_of!(Dav1dData, sz) == POINTER_BYTES);
    assert!(offset_of!(Dav1dData, r#ref) == PACKET_REF);
    assert!(offset_of!(Dav1dData, m) == PACKET_PROPERTIES);
    assert!(size_of::<Dav1dData>() == PACKET_BYTES);
  };
}

#[cfg(feature = "c-ffi")]
impl From<Dav1dDataProps> for Rav1dDataProps {
  fn from(value: Dav1dDataProps) -> Self {
    let Ok(size) = fixed_index(value.size);
    let Dav1dDataProps {
      timestamp,
      duration,
      offset,
      size: _,
      user_data,
    } = value;
    Self {
      timestamp,
      duration,
      offset,
      size,
      user_data: Rav1dUserData::from(user_data),
    }
  }
}

#[cfg(feature = "c-ffi")]
impl From<Rav1dDataProps> for Dav1dDataProps {
  fn from(value: Rav1dDataProps) -> Self {
    let Rav1dDataProps {
      timestamp,
      duration,
      offset,
      size,
      user_data,
    } = value;
    Self {
      timestamp,
      duration,
      offset,
      size: match index(size) {
        Ok(c_size) => c_size,
      },
      user_data: Dav1dUserData::from(user_data),
    }
  }
}
