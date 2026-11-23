#[cfg(feature = "c-ffi")]
use crate::src::send_sync_non_null::Dav1dOpaquePointer;
#[cfg(feature = "c-ffi")]
use std::ffi::c_char;
use std::fmt;
#[cfg(feature = "c-ffi")]
use std::fmt::Write as _;
#[cfg(feature = "c-ffi")]
use std::io::Write as _;
#[cfg(feature = "c-ffi")]
use std::io::stdout;
#[cfg(feature = "c-ffi")]
use std::ptr::fn_addr_eq;

#[cfg(feature = "c-ffi")]
pub type Dav1dLoggerCallback =
  unsafe extern "C" fn(cookie: Option<Dav1dOpaquePointer>, fmt: *const c_char, args: ...);

#[cfg(feature = "c-ffi")]
#[derive(Clone)]
#[repr(C)]
pub struct Dav1dLogger {
  cookie: Option<Dav1dOpaquePointer>,

  callback: Option<Dav1dLoggerCallback>,
}

#[cfg(feature = "c-ffi")]
impl Dav1dLogger {
  #[doc = "# Safety"]
  #[doc = "The callback must accept the cookie and the arguments for the C `%c` format."]
  pub const unsafe fn new(
    cookie: Option<Dav1dOpaquePointer>,
    callback: Option<Dav1dLoggerCallback>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { cookie, callback })
  }
}

#[cfg(feature = "c-ffi")]
impl fmt::Write for Dav1dLogger {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the validated C logger and cookie receive one promoted character with a constant format"
    )
  )]
  fn write_str(&mut self, s: &str) -> fmt::Result {
    let callback = self.callback.ok_or(fmt::Error)?;

    let fmt = c"%c";

    for &byte in s.as_bytes() {
      let character = i32::from(byte);

      // SAFETY:
      //
      // The first argument is `self.cookie`
      // and the rest are safe to call `printf` with,
      // as required by [`Self::new`].
      unsafe { callback(self.cookie, fmt.as_ptr(), character) };
    }

    Ok(())
  }
}

#[derive(Clone, Default)]
pub(crate) enum Rav1dLogger {
  #[cfg(feature = "c-ffi")]
  Dav1d(Dav1dLogger),
  #[cfg(feature = "c-ffi")]
  Stdout,
  #[default]
  Stderr,
}

pub trait Rav1dLog {
  fn write_fmt(&self, args: fmt::Arguments);
}

impl Rav1dLog for Rav1dLogger {
  #[inline(never)]
  fn write_fmt(&self, args: fmt::Arguments) {
    match self {
      #[cfg(feature = "c-ffi")]
      Self::Dav1d(dav1d) => match dav1d.clone().write_fmt(args) {
        Ok(()) => {}
        Err(error) => {
          console_core_logging::warn!("AV1 C logger could not deliver {args}: {error}")
        }
      },
      #[cfg(feature = "c-ffi")]
      Self::Stdout => match stdout().write_fmt(args) {
        Ok(()) => {}
        Err(error) => {
          console_core_logging::warn!("AV1 stdout logger could not deliver {args}: {error}")
        }
      },
      Self::Stderr => console_core_logging::warn!("{args}"),
    }
  }
}

impl Rav1dLog for Option<Rav1dLogger> {
  #[inline(always)]
  fn write_fmt(&self, args: fmt::Arguments) {
    match self {
      Some(logger) => {
        logger.write_fmt(args);
      }
      None => {}
    }
  }
}

#[cfg(feature = "c-ffi")]
mod marker {
  use super::*;

  type Callback = extern "C" fn(cookie: Option<Dav1dOpaquePointer>, fmt: *const c_char);

  #[cfg_attr(
    dylint_lib = "explicit059_no_unchecked_api",
    allow(
      explicit059_no_unchecked_api,
      reason = "dav1d's logger is a C variadic function pointer, which no checked spelling can name; both callbacks spell the arguments they take and read no others"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C variadic ABI uses the same first two arguments and the stream selectors read no variadic arguments"
    )
  )]
  const fn cast(
    callback: Callback,
  ) -> core::result::Result<Dav1dLoggerCallback, console_core_never::Never> {
    Ok({
      // SAFETY: It should always be safe to ignore variadic args.
      // Declaring a variadic `fn` is unstable, though, which is why we avoid that.
      unsafe { std::mem::transmute(callback) }
    })
  }

  pub const STDOUT: Dav1dLoggerCallback = {
    #[cfg_attr(
      dylint_lib = "explicit004_no_panic",
      expect(
        explicit004_no_panic,
        reason = "invoking the stdout selector violates its C ABI contract because the decoder recognizes its unique address and selects the stdout writer"
      )
    )]
    extern "C" fn stdout(_cookie: Option<Dav1dOpaquePointer>, _fmt: *const c_char) {
      unimplemented!(stringify!(stdout));
    }

    {
      let Ok(explicit_value) = cast(stdout);
      explicit_value
    }
  };

  pub const STDERR: Dav1dLoggerCallback = {
    #[cfg_attr(
      dylint_lib = "explicit004_no_panic",
      expect(
        explicit004_no_panic,
        reason = "invoking the stderr selector violates its C ABI contract because the decoder recognizes its unique address and selects the process diagnostic sink"
      )
    )]
    extern "C" fn stderr(_cookie: Option<Dav1dOpaquePointer>, _fmt: *const c_char) {
      unimplemented!(stringify!(stderr));
    }

    {
      let Ok(explicit_value) = cast(stderr);
      explicit_value
    }
  };
}

#[cfg(feature = "c-ffi")]
impl From<Dav1dLogger> for Option<Rav1dLogger> {
  fn from(logger: Dav1dLogger) -> Self {
    let Dav1dLogger { cookie, callback } = logger;
    let callback_2 = callback?;
    let streams = [
      (marker::STDOUT, Rav1dLogger::Stdout),
      (marker::STDERR, Rav1dLogger::Stderr),
    ];

    Some(
      match streams
        .into_iter()
        .find(|(address, _)| fn_addr_eq(callback_2, *address))
      {
        Some((_, stream)) => stream,
        None => Rav1dLogger::Dav1d(Dav1dLogger {
          cookie,
          callback: Some(callback_2),
        }),
      },
    )
  }
}

#[cfg(feature = "c-ffi")]
impl From<Option<Rav1dLogger>> for Dav1dLogger {
  fn from(logger: Option<Rav1dLogger>) -> Self {
    let cookie = match &logger {
      Some(Rav1dLogger::Dav1d(dav1d)) => dav1d.cookie,
      ::core::option::Option::None | ::core::option::Option::Some(_) => None,
    };
    let callback = logger.and_then(|logger_2| match logger_2 {
      Rav1dLogger::Dav1d(dav1d) => dav1d.callback,
      Rav1dLogger::Stdout => Some(marker::STDOUT),
      Rav1dLogger::Stderr => Some(marker::STDERR),
    });
    Self { cookie, callback }
  }
}
