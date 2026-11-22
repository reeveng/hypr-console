//! Logging macros that forward diagnostics when logging is enabled.

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the optional interpreter logger retains warning arguments while compiling out the sink when logging is disabled"
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
