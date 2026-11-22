#![cfg_attr(not(feature = "c-ffi"), forbid(unsafe_code))]
#[cfg(feature = "c-ffi")]
use crate::include::common::validate::validate_input;
use crate::include::dav1d::common::Rav1dDataProps;
use crate::include::dav1d::data::Rav1dData;
use crate::src::c_arc::CArc;
#[cfg(feature = "c-ffi")]
use crate::src::c_box::CBox;
#[cfg(feature = "c-ffi")]
use crate::src::c_box::FnFree;
#[cfg(feature = "c-ffi")]
use crate::src::c_box::Free;
#[cfg(feature = "c-ffi")]
use crate::src::error::Rav1dError::EINVAL;
#[cfg(feature = "c-ffi")]
use crate::src::error::Rav1dResult;
#[cfg(feature = "c-ffi")]
use crate::src::send_sync_non_null::Dav1dOpaquePointer;
#[cfg(feature = "c-ffi")]
use std::ptr::NonNull;

impl From<CArc<[u8]>> for Rav1dData {
  fn from(data: CArc<[u8]>) -> Self {
    let Ok(size) = console_core_number_conversion::fixed_index(data.len());
    Self {
      data: Some(data),
      m: Rav1dDataProps {
        size,
        ..Default::default()
      },
    }
  }
}

impl Rav1dData {
  #[cfg(feature = "c-ffi")]
  pub fn create(size: u64) -> Rav1dResult<Self> {
    let data = CArc::zeroed_slice(size)?;
    Ok(
      <crate::include::dav1d::data::Rav1dData as ::core::convert::From<
        crate::src::c_arc::CArc<[::core::primitive::u8]>,
      >>::from(data),
    )
  }

  #[cfg(feature = "c-ffi")]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this boundary transfers an initialized packet buffer together with its C deallocation callback"
    )
  )]
  pub unsafe fn wrap(
    data: NonNull<[u8]>,
    free_callback: Option<FnFree>,
    cookie: Option<Dav1dOpaquePointer>,
  ) -> Rav1dResult<Self> {
    let free = validate_input!(free_callback.ok_or(EINVAL))?;
    let free_2 = Free { free, cookie };

    // SAFETY: Preconditions delegate to `CBox::from_c`'s safety.
    let data_2 = unsafe {
      let Ok(c_box) = CBox::from_c(data, free_2);
      c_box
    };

    let data_3 = CArc::wrap(data_2)?;
    Ok(
      <crate::include::dav1d::data::Rav1dData as ::core::convert::From<
        crate::src::c_arc::CArc<[::core::primitive::u8]>,
      >>::from(data_3),
    )
  }

  #[cfg(feature = "c-ffi")]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this boundary transfers the initialized user-data byte with its matched C ownership callback"
    )
  )]
  pub unsafe fn wrap_user_data(
    &mut self,
    user_data: NonNull<u8>,
    free_callback: Option<FnFree>,
    cookie: Option<Dav1dOpaquePointer>,
  ) -> Rav1dResult {
    let free = validate_input!(free_callback.ok_or(EINVAL))?;
    let free_2 = Free { free, cookie };

    // SAFETY: Preconditions delegate to `CBox::from_c`'s safety.
    let user_data_2 = unsafe {
      let Ok(c_box) = CBox::from_c(user_data, free_2);
      c_box
    };

    let user_data_3 = CArc::wrap(user_data_2)?;
    self.m.user_data = Some(user_data_3);
    Ok(())
  }
}

impl AsRef<[u8]> for Rav1dData {
  fn as_ref(&self) -> &[u8] {
    match &self.data {
      Some(data) => data.as_ref(),
      None => &[],
    }
  }
}
