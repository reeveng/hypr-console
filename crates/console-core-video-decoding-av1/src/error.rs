#![forbid(unsafe_code)]
use std::ffi::c_int;
use std::ffi::c_uint;
use strum::FromRepr;

#[cfg(all(
  feature = "c-ffi",
  any(
    all(unix, not(any(target_os = "l4re", target_os = "qurt"))),
    target_os = "wasi"
  )
))]
mod native {
  use rustix::io::Errno;
  use std::ffi::c_int;

  pub const ENOENT: c_int = Errno::NOENT.raw_os_error();
  pub const EIO: c_int = Errno::IO.raw_os_error();
  pub const EAGAIN: c_int = Errno::AGAIN.raw_os_error();
  pub const ENOMEM: c_int = Errno::NOMEM.raw_os_error();
  pub const EINVAL: c_int = Errno::INVAL.raw_os_error();
  pub const ERANGE: c_int = Errno::RANGE.raw_os_error();
  pub const ENOPROTOOPT: c_int = Errno::NOPROTOOPT.raw_os_error();
  pub const ECANCELED: c_int = Errno::CANCELED.raw_os_error();
}

#[cfg(all(
  feature = "c-ffi",
  any(
    windows,
    target_os = "hermit",
    target_os = "l4re",
    target_os = "qurt",
    target_os = "solid_asp3",
    target_os = "teeos",
    all(target_arch = "wasm32", target_os = "unknown")
  )
))]
mod native {
  use std::ffi::c_int;

  pub const ENOENT: c_int = c_int::from_le_bytes([super::MISSING_ERRNO, 0, 0, 0]);
  pub const EIO: c_int = c_int::from_le_bytes([super::IO_ERRNO, 0, 0, 0]);
  pub const EAGAIN: c_int = c_int::from_le_bytes([super::AGAIN_ERRNO, 0, 0, 0]);
  pub const ENOMEM: c_int = c_int::from_le_bytes([super::MEMORY_ERRNO, 0, 0, 0]);
  pub const EINVAL: c_int = c_int::from_le_bytes([super::INVALID_ERRNO, 0, 0, 0]);
  pub const ERANGE: c_int = c_int::from_le_bytes([super::RANGE_ERRNO, 0, 0, 0]);

  #[cfg(windows)]
  pub const ENOPROTOOPT: c_int = 123;
  #[cfg(windows)]
  pub const ECANCELED: c_int = 105;

  #[cfg(not(windows))]
  pub const ENOPROTOOPT: c_int = c_int::from_le_bytes([super::PROTOCOL_ERRNO, 0, 0, 0]);
  #[cfg(not(windows))]
  pub const ECANCELED: c_int = c_int::from_le_bytes([super::CANCELED_ERRNO, 0, 0, 0]);
}

const GENERIC_ERRNO: u8 = 1;
const MISSING_ERRNO: u8 = 2;
const IO_ERRNO: u8 = 5;
const AGAIN_ERRNO: u8 = 11;
const MEMORY_ERRNO: u8 = 12;
const INVALID_ERRNO: u8 = 22;
const RANGE_ERRNO: u8 = 34;
const PROTOCOL_ERRNO: u8 = 92;
const CANCELED_ERRNO: u8 = 125;

#[derive(Clone, Copy, PartialEq, Eq, FromRepr, Debug)]
#[repr(u8)]
#[non_exhaustive]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the public Rust discriminants preserve the existing errno mapping used at the dav1d boundary"
  )
)]
pub enum Rav1dError {
  EGeneric = GENERIC_ERRNO,

  ENOENT = MISSING_ERRNO,
  EIO = IO_ERRNO,
  EAGAIN = AGAIN_ERRNO,
  ENOMEM = MEMORY_ERRNO,
  EINVAL = INVALID_ERRNO,
  ERANGE = RANGE_ERRNO,
  ENOPROTOOPT = PROTOCOL_ERRNO,

  ECANCELED = CANCELED_ERRNO,
}

impl Rav1dError {
  const fn compatibility_errno(self) -> Result<c_int, console_core_never::Never> {
    let byte = match self {
      Self::EGeneric => GENERIC_ERRNO,
      Self::ENOENT => MISSING_ERRNO,
      Self::EIO => IO_ERRNO,
      Self::EAGAIN => AGAIN_ERRNO,
      Self::ENOMEM => MEMORY_ERRNO,
      Self::EINVAL => INVALID_ERRNO,
      Self::ERANGE => RANGE_ERRNO,
      Self::ENOPROTOOPT => PROTOCOL_ERRNO,
      Self::ECANCELED => CANCELED_ERRNO,
    };

    Ok(c_int::from_le_bytes([byte, 0, 0, 0]))
  }

  #[inline]
  pub const fn errno(self) -> core::result::Result<c_int, console_core_never::Never> {
    Ok({
      #[cfg(feature = "c-ffi")]
      {
        match self {
          Self::EGeneric => match self.compatibility_errno() {
            Ok(generic) => generic,
          },
          Self::ENOENT => native::ENOENT,
          Self::EIO => native::EIO,
          Self::EAGAIN => native::EAGAIN,
          Self::ENOMEM => native::ENOMEM,
          Self::EINVAL => native::EINVAL,
          Self::ERANGE => native::ERANGE,
          Self::ENOPROTOOPT => native::ENOPROTOOPT,
          Self::ECANCELED => native::ECANCELED,
        }
      }

      #[cfg(not(feature = "c-ffi"))]
      {
        match self.compatibility_errno() {
          Ok(compatible) => compatible,
        }
      }
    })
  }

  #[inline]
  pub const fn from_errno(
    errno: c_int,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok({
      #[cfg(feature = "c-ffi")]
      {
        match errno {
          1 => Some(Self::EGeneric),
          native::ENOENT => Some(Self::ENOENT),
          native::EIO => Some(Self::EIO),
          native::EAGAIN => Some(Self::EAGAIN),
          native::ENOMEM => Some(Self::ENOMEM),
          native::EINVAL => Some(Self::EINVAL),
          native::ERANGE => Some(Self::ERANGE),
          native::ENOPROTOOPT => Some(Self::ENOPROTOOPT),
          native::ECANCELED => Some(Self::ECANCELED),
          _ => None,
        }
      }

      #[cfg(not(feature = "c-ffi"))]
      {
        match errno {
          0..=255 => {
            let [low, _, _, _] = errno.to_le_bytes();

            Self::from_repr(low)
          }
          i32::MIN..=-1 | 256..=i32::MAX => None,
        }
      }
    })
  }
}

#[cfg(all(feature = "c-ffi", target_os = "linux"))]
const _: () = {
  let Ok(enoent) = Rav1dError::ENOENT.compatibility_errno();
  assert!(enoent == native::ENOENT);
  let Ok(eio) = Rav1dError::EIO.compatibility_errno();
  assert!(eio == native::EIO);
  let Ok(eagain) = Rav1dError::EAGAIN.compatibility_errno();
  assert!(eagain == native::EAGAIN);
  let Ok(enomem) = Rav1dError::ENOMEM.compatibility_errno();
  assert!(enomem == native::ENOMEM);
  let Ok(einval) = Rav1dError::EINVAL.compatibility_errno();
  assert!(einval == native::EINVAL);
  let Ok(erange) = Rav1dError::ERANGE.compatibility_errno();
  assert!(erange == native::ERANGE);
  let Ok(enoprotoopt) = Rav1dError::ENOPROTOOPT.compatibility_errno();
  assert!(enoprotoopt == native::ENOPROTOOPT);
  let Ok(ecanceled) = Rav1dError::ECANCELED.compatibility_errno();
  assert!(ecanceled == native::ECANCELED);
};

pub type Rav1dResult<T = ()> = Result<T, Rav1dError>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct Dav1dResult(pub c_int);

const _: () = {
  use core::mem::{align_of, offset_of, size_of};

  assert!(size_of::<Dav1dResult>() == 4);
  assert!(align_of::<Dav1dResult>() == 4);
  assert!(offset_of!(Dav1dResult, 0) == 0);
};

impl From<Rav1dResult> for Dav1dResult {
  #[inline]
  fn from(value: Rav1dResult) -> Self {
    Dav1dResult(
      (match value {
        Ok(()) => 0,
        Err(e) => {
          let Ok(errno) = e.errno();

          errno
        }
      })
      .wrapping_neg(),
    )
  }
}

impl From<Rav1dResult<c_uint>> for Dav1dResult {
  #[inline]
  fn from(value: Rav1dResult<c_uint>) -> Self {
    Dav1dResult(match value {
      Ok(value_2) => {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(value_2);
        numeric_value
      }
      Err(e) => {
        let Ok(errno) = e.errno();

        errno
      }
    })
  }
}

impl TryFrom<Dav1dResult> for Rav1dResult {
  type Error = Dav1dResult;

  #[inline]
  fn try_from(value: Dav1dResult) -> Result<Self, Self::Error> {
    match value.0 {
      0 => Ok(Ok(())),
      e => {
        let Ok(rav1d_error) = Rav1dError::from_errno((e).wrapping_neg());
        let e_2 = rav1d_error.ok_or(value)?;
        Ok(Err(e_2))
      }
    }
  }
}

#[cfg(test)]
mod errno_tests {
  use super::*;

  const ALL: [Rav1dError; 9] = [
    Rav1dError::EGeneric,
    Rav1dError::ENOENT,
    Rav1dError::EIO,
    Rav1dError::EAGAIN,
    Rav1dError::ENOMEM,
    Rav1dError::EINVAL,
    Rav1dError::ERANGE,
    Rav1dError::ENOPROTOOPT,
    Rav1dError::ECANCELED,
  ];

  #[test]
  fn errno_round_trips_on_this_platform() {
    for e in ALL {
      let Ok(errno) = e.errno();
      assert!(errno > 0, "{e:?} must map to a positive errno");
      let Ok(e_errno) = e.errno();
      let Ok(rav1d_error) = Rav1dError::from_errno(e_errno);
      assert_eq!(rav1d_error, Some(e), "{e:?}");
      assert_eq!(
        Rav1dResult::try_from(Dav1dResult::from(Err::<(), _>(e))),
        Ok(Err(e)),
        "{e:?} through Dav1dResult"
      );
    }

    assert_eq!(Rav1dResult::try_from(Dav1dResult(0)), Ok(Ok(())));
    let Ok(rav1d_error) = Rav1dError::from_errno(0);
    assert_eq!(rav1d_error, None);
    assert_eq!(
      ({
        let Ok(explicit_value) = Rav1dError::from_errno(-1);
        explicit_value
      }),
      None
    );
  }

  #[cfg(feature = "c-ffi")]
  #[test]
  fn errno_is_the_platform_value_under_c_ffi() {
    let Ok(errno) = Rav1dError::EAGAIN.errno();
    assert_eq!(errno, native::EAGAIN);
    let Ok(enoprotoopt_errno) = Rav1dError::ENOPROTOOPT.errno();
    assert_eq!(enoprotoopt_errno, native::ENOPROTOOPT);
    let Ok(ecanceled_errno) = Rav1dError::ECANCELED.errno();
    assert_eq!(ecanceled_errno, native::ECANCELED);
    assert_eq!(
      Dav1dResult::from(Err::<(), _>(Rav1dError::EAGAIN)).0,
      (native::EAGAIN).wrapping_neg()
    );
  }
}
