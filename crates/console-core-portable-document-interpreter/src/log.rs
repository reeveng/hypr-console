//! Logging macros that evaluate arguments only when a sink accepts the message.
//! Builds with logging disabled still check the format strings and argument types.

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "selects whether warnings are emitted while preserving format checks and caller source information"
  )
)]
macro_rules! warn {
  ($($message:tt)+) => {{
    #[cfg(feature = "logging")]
    {
      ::console_core_logging::warn!($($message)+);
    }
    #[cfg(not(feature = "logging"))]
    if false {
      let _ = ::core::format_args!($($message)+);
    }
  }};
}

#[cfg(all(test, not(feature = "logging")))]
mod tests {
  use core::cell::Cell;

  #[test]
  fn disabled_logging_skips_arguments() {
    let evaluated = Cell::new(false);
    let argument = || {
      evaluated.set(true);
      "message"
    };

    warn!("{}", argument());
    assert!(!evaluated.get());
  }
}
