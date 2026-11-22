#[cfg(feature = "c-ffi")]
use crate::include::dav1d::common::Dav1dDataProps;
use crate::include::dav1d::common::Rav1dDataProps;
use crate::src::c_arc::CArc;
#[cfg(feature = "c-ffi")]
use crate::src::c_arc::RawCArc;
#[cfg(feature = "c-ffi")]
use console_core_api_compatibility::NativeSize;
#[cfg(feature = "c-ffi")]
use std::ptr::NonNull;

#[cfg(feature = "c-ffi")]
#[derive(Default)]
#[repr(C)]
pub struct Dav1dData {
  pub data: Option<NonNull<u8>>,
  pub sz: NativeSize,
  pub r#ref: Option<RawCArc<[u8]>>,
  pub m: Dav1dDataProps,
}

#[derive(Clone, Default)]
#[repr(C)]
pub(crate) struct Rav1dData {
  pub data: Option<CArc<[u8]>>,
  pub m: Rav1dDataProps,
}

#[cfg(feature = "c-ffi")]
impl From<Dav1dData> for Rav1dData {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C packet transfers the outstanding owned reference produced by CArc::into_raw"
    )
  )]
  fn from(value: Dav1dData) -> Self {
    let Dav1dData {
      data: _,
      sz: _,
      r#ref,
      m,
    } = value;
    Self {
      data: r#ref.map(|ref_2| {
        // SAFETY: `r#ref` is a [`RawCArc`] originally from [`CArc`].
        unsafe {
          let Ok(c_arc) = CArc::from_raw(ref_2);
          c_arc
        }
      }),
      m: <crate::include::dav1d::common::Rav1dDataProps as ::core::convert::From<
        crate::include::dav1d::common::Dav1dDataProps,
      >>::from(m),
    }
  }
}

#[cfg(feature = "c-ffi")]
impl From<Rav1dData> for Dav1dData {
  fn from(value: Rav1dData) -> Self {
    let Rav1dData { data, m } = value;
    Self {
      data: data
        .as_ref()
        .map(|data_2| NonNull::from(data_2.as_ref()).cast()),
      sz: match data.as_ref().map(|data_3| data_3.len()) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
      },
      r#ref: data.map(|data_4| {
        let Ok(raw) = data_4.into_raw();

        raw
      }),
      m: <crate::include::dav1d::common::Dav1dDataProps as ::core::convert::From<
        crate::include::dav1d::common::Rav1dDataProps,
      >>::from(m),
    }
  }
}
