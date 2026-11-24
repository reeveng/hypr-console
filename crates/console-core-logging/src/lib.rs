//! Library diagnostics carry a severity, the source module, and a message.
//! Hosted programs write warnings and errors to stderr by default, which
//! systemd services can collect in the journal. A host can install a sink and
//! choose its minimum severity at startup. The first installation wins.
//! Freestanding builds need a sink to emit diagnostics.

#![no_std]
#![cfg_attr(feature = "std", forbid(unsafe_code))]

#[cfg(any(feature = "std", test))]
extern crate std;

#[cfg(all(feature = "std", target_os = "linux"))]
pub mod orbit;

use core::fmt;
#[cfg(all(not(feature = "std"), target_has_atomic = "ptr"))]
use core::{
  ptr::{from_ref, null_mut},
  sync::atomic::{AtomicPtr, Ordering},
};

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
  Trace,
  Debug,
  Info,
  Warning,
  Error,
}

impl fmt::Display for Severity {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str(match self {
      Self::Trace => "trace",
      Self::Debug => "debug",
      Self::Info => "info",
      Self::Warning => "warning",
      Self::Error => "error",
    })
  }
}

pub struct Record<'a> {
  pub severity: Severity,
  pub module: &'static str,
  pub message: fmt::Arguments<'a>,
}

impl fmt::Display for Record<'_> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "{} {}: {}", self.severity, self.module, self.message)
  }
}

pub struct Sink {
  pub minimum: Severity,
  pub write: fn(Record<'_>),
}

#[cfg(all(feature = "std", target_has_atomic = "ptr"))]
#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "libraries emit diagnostics to the one sink the process host installs at startup"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the host installs one diagnostic destination for all the libraries in its process"
  )
)]
static INSTALLED: std::sync::OnceLock<&'static Sink> = std::sync::OnceLock::new();

#[cfg(all(not(feature = "std"), target_has_atomic = "ptr"))]
#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "libraries emit diagnostics to the one sink the process host installs at startup"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the host installs one diagnostic destination for all the libraries in its process"
  )
)]
static INSTALLED: AtomicPtr<Sink> = AtomicPtr::new(null_mut());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlreadyInstalled;

impl fmt::Display for AlreadyInstalled {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str("a logging sink is already installed")
  }
}

impl core::error::Error for AlreadyInstalled {}

#[cfg(all(feature = "std", target_has_atomic = "ptr"))]
pub fn install(sink: &'static Sink) -> Result<(), AlreadyInstalled> {
  INSTALLED.set(sink).map_err(|_installed| AlreadyInstalled)
}

#[cfg(all(not(feature = "std"), target_has_atomic = "ptr"))]
pub fn install(sink: &'static Sink) -> Result<(), AlreadyInstalled> {
  INSTALLED
    .compare_exchange(
      null_mut(),
      from_ref(sink).cast_mut(),
      Ordering::SeqCst,
      Ordering::SeqCst,
    )
    .map(|_empty| ())
    .map_err(|_installed| AlreadyInstalled)
}

#[cfg(feature = "std")]
const DEFAULT: Option<&Sink> = Some(&Sink {
  minimum: Severity::Warning,
  write: |record| {
    let Ok(()) = stderr(record);
  },
});

#[cfg(not(feature = "std"))]
const DEFAULT: Option<&Sink> = None;

#[cfg(feature = "std")]
fn stderr(record: Record<'_>) -> Result<(), Never> {
  use std::io::Write as _;

  match writeln!(std::io::stderr().lock(), "{record}") {
    Ok(()) => Ok(()),
    Err(_stderr_cannot_deliver_the_diagnostic) => Ok(()),
  }
}

#[doc(hidden)]
pub fn sink_for(severity: Severity) -> Result<Option<&'static Sink>, Never> {
  #[cfg(all(feature = "std", target_has_atomic = "ptr"))]
  let sink = INSTALLED.get().copied().or(DEFAULT);
  #[cfg(all(not(feature = "std"), target_has_atomic = "ptr"))]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "install only publishes immutable static sinks and never replaces an installed pointer"
    )
  )]
  let sink = {
    let installed = INSTALLED.load(Ordering::SeqCst);
    // SAFETY: install only publishes shared references to immutable static sinks.
    // A published sink is never replaced or freed.
    unsafe { installed.as_ref() }.or(DEFAULT)
  };
  #[cfg(not(target_has_atomic = "ptr"))]
  let sink = DEFAULT;

  Ok(sink.filter(|destination| severity >= destination.minimum))
}

#[doc(hidden)]
pub fn emit(sink: &Sink, record: Record<'_>) -> Result<(), Never> {
  #[cfg(all(feature = "std", target_os = "linux"))]
  match record.severity {
    Severity::Error => {
      let Ok(()) = orbit::keep_error(record.module, record.message);
    }
    Severity::Trace | Severity::Debug | Severity::Info | Severity::Warning => {}
  }

  (sink.write)(record);

  Ok(())
}

#[macro_export]
#[doc(hidden)]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one expansion captures the caller's module and evaluates formatting arguments only after the sink accepts the severity"
  )
)]
macro_rules! report {
  ($severity:ident, $($message:tt)+) => {{
    let severity = $crate::Severity::$severity;
    let Ok(destination) = $crate::sink_for(severity);
    match destination {
      Some(sink) => {
        let Ok(()) = $crate::emit(sink, $crate::Record {
          severity,
          module: ::core::module_path!(),
          message: ::core::format_args!($($message)+),
        });
      }
      None => {},
    }
  }};
}

#[macro_export]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "supplies Trace severity while preserving the caller's module and deferred formatting"
  )
)]
macro_rules! trace {
  ($($message:tt)+) => { $crate::report!(Trace, $($message)+) };
}

#[macro_export]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "supplies Debug severity while preserving the caller's module and deferred formatting"
  )
)]
macro_rules! debug {
  ($($message:tt)+) => { $crate::report!(Debug, $($message)+) };
}

#[macro_export]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "supplies Info severity while preserving the caller's module and deferred formatting"
  )
)]
macro_rules! info {
  ($($message:tt)+) => { $crate::report!(Info, $($message)+) };
}

#[macro_export]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "supplies Warning severity while preserving the caller's module and deferred formatting"
  )
)]
macro_rules! warn {
  ($($message:tt)+) => { $crate::report!(Warning, $($message)+) };
}

#[macro_export]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "supplies Error severity while preserving the caller's module and deferred formatting"
  )
)]
macro_rules! error {
  ($($message:tt)+) => { $crate::report!(Error, $($message)+) };
}

#[cfg(all(test, target_has_atomic = "ptr"))]
mod tests {
  use super::*;
  use core::cell::Cell;
  use std::string::ToString;

  std::thread_local! {
    static RECEIVED: Cell<bool> = const { Cell::new(false) };
  }

  #[test]
  fn a_sink_receives_the_source_severity_and_message_at_its_threshold() {
    static SINK: Sink = Sink {
      minimum: Severity::Warning,
      write: |record| {
        assert_eq!(record.severity, Severity::Warning);
        assert_eq!(record.module, module_path!());
        assert_eq!(record.message.to_string(), "diagnostic 42");
        RECEIVED.set(true);
      },
    };

    let Ok(verbose) = sink_for(Severity::Info);
    let Ok(warnings) = sink_for(Severity::Warning);
    assert!(verbose.is_none());
    assert_eq!(warnings.is_some(), cfg!(feature = "std"));

    assert_eq!(install(&SINK), Ok(()));
    assert_eq!(install(&SINK), Err(AlreadyInstalled));

    let evaluated = Cell::new(false);
    debug!("{:?}", evaluated.set(true));
    assert!(!evaluated.get());

    warn!("diagnostic {}", 42);
    assert!(RECEIVED.get());
  }
}
