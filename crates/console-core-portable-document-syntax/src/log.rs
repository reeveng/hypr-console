//! Logging macros that forward diagnostics when logging is enabled.

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "Selects whether debug diagnostics are emitted while keeping argument types and caller source information"
  )
)]
macro_rules! debug {
    ($fmt:literal $(, $($arg:expr),* $(,)?)?) => {{
        #[cfg(feature = "logging")]
        {
            ::console_core_logging::debug!($fmt $(, $($arg),*)?);
        }
        #[cfg(not(feature = "logging"))]
        {
            $($(let _ = &$arg;)*)?
        }
    }};
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "Selects whether error diagnostics are emitted while keeping argument types and caller source information"
  )
)]
macro_rules! error {
    ($fmt:literal $(, $($arg:expr),* $(,)?)?) => {{
        #[cfg(feature = "logging")]
        {
            ::console_core_logging::error!($fmt $(, $($arg),*)?);
        }
        #[cfg(not(feature = "logging"))]
        {
            $($(let _ = &$arg;)*)?
        }
    }};
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "Selects whether warn diagnostics are emitted while keeping argument types and caller source information"
  )
)]
macro_rules! warn {
    ($fmt:literal $(, $($arg:expr),* $(,)?)?) => {{
        #[cfg(feature = "logging")]
        {
            ::console_core_logging::warn!($fmt $(, $($arg),*)?);
        }
        #[cfg(not(feature = "logging"))]
        {
            $($(let _ = &$arg;)*)?
        }
    }};
}
