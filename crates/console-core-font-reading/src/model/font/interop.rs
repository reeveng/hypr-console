//! Font model interop for Skrifa and HarfRust.
//!
//! This should not be used directly.

use crate::model::font::Font;
use alloc::boxed::Box;
use core::any::Any;

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "console-core-text-shaping downcasts the borrowed cache value returned by this cross-crate interop function"
  )
)]
pub fn _get_or_init_shaping_data(
  font: &Font,
  f: impl FnOnce() -> Box<dyn Any + Send + Sync>,
) -> &dyn Any {
  &**font.0.shaping_data.get_or_init(|| f())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn recover_shaping_data() -> ::core::result::Result<(), crate::test_support::Failure> {
    struct ShapingData {
      arg: i32,
    }

    let font = Font::new(font_test_data::AHEM, 0)?;
    let data = _get_or_init_shaping_data(&font, || Box::new(ShapingData { arg: -42 }));
    let actual =
      crate::test_support::fixture_value(data.downcast_ref::<ShapingData>(), "fixture value")?;
    assert_eq!(actual.arg, -42);
    ::core::result::Result::Ok(())
  }
}
