//! Forward decoder diagnostics when the logging feature is enabled.

use console_core_never::Never;
use core::fmt::Arguments;

pub(crate) fn debug(message: Arguments<'_>) -> Result<(), Never> {
  #[cfg(feature = "logging")]
  console_core_logging::debug!("{}", message);
  #[cfg(not(feature = "logging"))]
  let _ = message;
  Ok(())
}

pub(crate) fn trace(message: Arguments<'_>) -> Result<(), Never> {
  #[cfg(feature = "logging")]
  console_core_logging::trace!("{}", message);
  #[cfg(not(feature = "logging"))]
  let _ = message;
  Ok(())
}

pub(crate) fn warn(message: Arguments<'_>) -> Result<(), Never> {
  #[cfg(feature = "logging")]
  console_core_logging::warn!("{}", message);
  #[cfg(not(feature = "logging"))]
  let _ = message;
  Ok(())
}
