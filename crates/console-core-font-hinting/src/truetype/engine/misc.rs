//! Miscellaneous instructions.
//!
//! Implements 3 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#miscellaneous-instructions>

use super::{Engine, OpResult};
use raw::types::F2Dot14;

impl Engine<'_> {
  pub(super) fn op_getinfo(&mut self) -> OpResult {
    use getinfo::*;
    let selector = self.value_stack.pop()?;
    let mut result = 0;

    match ::core::primitive::bool::then_some((selector & VERSION_SELECTOR_BIT) != 0, ()) {
      ::core::option::Option::Some(()) => {
        result = 40;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some((selector & GLYPH_ROTATED_SELECTOR_BIT) != 0
      && matches!(self.graphics.is_rotated, crate::Rotation::Rotated), ())
    {
      ::core::option::Option::Some(()) => {
        result |= GLYPH_ROTATED_RESULT_BIT;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some((selector & GLYPH_STRETCHED_SELECTOR_BIT) != 0
      && matches!(self.graphics.is_stretched, crate::Stretch::Stretched), ())
    {
      ::core::option::Option::Some(()) => {
        result |= GLYPH_STRETCHED_RESULT_BIT;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some((selector & FONT_VARIATIONS_SELECTOR_BIT) != 0 && self.axis_count != 0, ()) {
      ::core::option::Option::Some(()) => {
        result |= FONT_VARIATIONS_RESULT_BIT;
      }
      ::core::option::Option::None => {}
    }

    let is_smooth = matches!(self.graphics.target, crate::Target::Smooth { .. });
    let is_vertical_lcd = matches!(
      self.graphics.target,
      crate::Target::Smooth {
        mode: crate::SmoothMode::VerticalLcd,
        ..
      }
    );
    let symmetric_rendering = matches!(
      self.graphics.target,
      crate::Target::Smooth {
        symmetric_rendering: crate::SymmetricRendering::Enabled,
        ..
      }
    );
    let is_grayscale_cleartype = matches!(
      self.graphics.target,
      crate::Target::Smooth {
        mode: crate::SmoothMode::Normal | crate::SmoothMode::Light,
        ..
      }
    );

    match ::core::primitive::bool::then_some(is_smooth, ()) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some((selector & SUBPIXEL_HINTING_SELECTOR_BIT) != 0, ()) {
          ::core::option::Option::Some(()) => {
            result |= SUBPIXEL_HINTING_RESULT_BIT;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some((selector & VERTICAL_LCD_SELECTOR_BIT) != 0 && is_vertical_lcd, ()) {
          ::core::option::Option::Some(()) => {
            result |= VERTICAL_LCD_RESULT_BIT;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some((selector & SUBPIXEL_POSITIONED_SELECTOR_BIT) != 0, ()) {
          ::core::option::Option::Some(()) => {
            result |= SUBPIXEL_POSITIONED_RESULT_BIT;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some((selector & SYMMETRICAL_SMOOTHING_SELECTOR_BIT) != 0 && symmetric_rendering, ()) {
          ::core::option::Option::Some(()) => {
            result |= SYMMETRICAL_SMOOTHING_RESULT_BIT;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some((selector & GRAYSCALE_CLEARTYPE_SELECTOR_BIT) != 0 && is_grayscale_cleartype, ()) {
          ::core::option::Option::Some(()) => {
            result |= GRAYSCALE_CLEARTYPE_RESULT_BIT;
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    self.value_stack.push(result)
  }

  pub(super) fn op_getvariation(&mut self) -> OpResult {
    match self.axis_count {
      0 => return self.op_unknown(0x91),
      _ => {}
    }

    let Ok(axis_count) = console_core_number_conversion::index(self.axis_count);
    let Ok(actual_axes) = console_core_number_conversion::fitted::<_, u64>(self.coords.len());
    let missing = u64::from(self.axis_count).saturating_sub(actual_axes);
    let Ok(padding_count) = console_core_number_conversion::index(missing);
    let padding = core::iter::repeat_n(F2Dot14::default(), padding_count);

    for coord in self.coords.iter().copied().take(axis_count).chain(padding) {
      let Ok(bits) = coord.to_bits();
      self.value_stack.push(i32::from(bits))?;
    }

    Ok(())
  }

  pub(super) fn op_getdata(&mut self) -> OpResult {
    match ::core::primitive::bool::then_some(self.axis_count != 0, ()) {
      ::core::option::Option::Some(()) => self.value_stack.push(17),
      ::core::option::Option::None => self.op_unknown(0x92),
    }
  }
}

mod getinfo {

  pub const VERSION_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 0_u32);

  pub const GLYPH_ROTATED_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 1_u32);
  pub const GLYPH_ROTATED_RESULT_BIT: i32 = i32::wrapping_shl(1, 8_u32);

  pub const GLYPH_STRETCHED_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 2_u32);
  pub const GLYPH_STRETCHED_RESULT_BIT: i32 = i32::wrapping_shl(1, 9_u32);

  pub const FONT_VARIATIONS_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 3_u32);
  pub const FONT_VARIATIONS_RESULT_BIT: i32 = i32::wrapping_shl(1, 10_u32);

  pub const SUBPIXEL_HINTING_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 6_u32);
  pub const SUBPIXEL_HINTING_RESULT_BIT: i32 = i32::wrapping_shl(1, 13_u32);

  pub const VERTICAL_LCD_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 8_u32);
  pub const VERTICAL_LCD_RESULT_BIT: i32 = i32::wrapping_shl(1, 15_u32);

  pub const SUBPIXEL_POSITIONED_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 10_u32);
  pub const SUBPIXEL_POSITIONED_RESULT_BIT: i32 = i32::wrapping_shl(1, 17_u32);

  pub const SYMMETRICAL_SMOOTHING_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 11_u32);
  pub const SYMMETRICAL_SMOOTHING_RESULT_BIT: i32 = i32::wrapping_shl(1, 18_u32);

  pub const GRAYSCALE_CLEARTYPE_SELECTOR_BIT: i32 = i32::wrapping_shl(1, 12_u32);
  pub const GRAYSCALE_CLEARTYPE_RESULT_BIT: i32 = i32::wrapping_shl(1, 19_u32);
}

#[cfg(test)]
mod tests {
  use super::super::{Engine, HintErrorKind, MockEngine};
  use crate::{SmoothMode, Target};
  use raw::types::F2Dot14;
  use read_fonts::tables::glyf::bytecode::Opcode;

  #[test]
  fn getinfo_version_and_font_features() -> Result<(), crate::TestFailure> {
    use super::getinfo::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.getinfo_test(InfoCase {
      selector: VERSION_SELECTOR_BIT,
      expected: 40,
    })?;

    engine.getinfo_test(InfoCase {
      selector: GLYPH_ROTATED_SELECTOR_BIT,
      expected: 0,
    })?;

    engine.graphics.is_rotated = crate::Rotation::Rotated;
    engine.getinfo_test(InfoCase {
      selector: GLYPH_ROTATED_SELECTOR_BIT,
      expected: GLYPH_ROTATED_RESULT_BIT,
    })?;

    engine.getinfo_test(InfoCase {
      selector: GLYPH_STRETCHED_SELECTOR_BIT,
      expected: 0,
    })?;

    engine.graphics.is_stretched = crate::Stretch::Stretched;
    engine.getinfo_test(InfoCase {
      selector: GLYPH_STRETCHED_SELECTOR_BIT,
      expected: GLYPH_STRETCHED_RESULT_BIT,
    })?;

    engine.getinfo_test(InfoCase {
      selector: GLYPH_ROTATED_SELECTOR_BIT | GLYPH_STRETCHED_SELECTOR_BIT,
      expected: GLYPH_ROTATED_RESULT_BIT | GLYPH_STRETCHED_RESULT_BIT,
    })?;

    engine.getinfo_test(InfoCase {
      selector: FONT_VARIATIONS_SELECTOR_BIT,
      expected: 0,
    })?;

    engine.axis_count = 1;
    engine.getinfo_test(InfoCase {
      selector: FONT_VARIATIONS_SELECTOR_BIT,
      expected: FONT_VARIATIONS_RESULT_BIT,
    })?;

    Ok(())
  }

  #[test]
  fn getinfo_rendering_target() -> Result<(), crate::TestFailure> {
    use super::getinfo::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.graphics.target = Target::Mono;

    for selector in [
      SUBPIXEL_HINTING_SELECTOR_BIT,
      VERTICAL_LCD_SELECTOR_BIT,
      SUBPIXEL_POSITIONED_SELECTOR_BIT,
      SYMMETRICAL_SMOOTHING_SELECTOR_BIT,
      GRAYSCALE_CLEARTYPE_SELECTOR_BIT,
    ] {
      engine.getinfo_test(InfoCase {
        selector,
        expected: 0,
      })?;
    }

    engine.graphics.target = Target::default();

    for (selector, result) in [
      (
        GRAYSCALE_CLEARTYPE_SELECTOR_BIT,
        GRAYSCALE_CLEARTYPE_RESULT_BIT,
      ),
      (SUBPIXEL_HINTING_SELECTOR_BIT, SUBPIXEL_HINTING_RESULT_BIT),
      (
        SUBPIXEL_POSITIONED_SELECTOR_BIT,
        SUBPIXEL_POSITIONED_RESULT_BIT,
      ),
    ] {
      engine.getinfo_test(InfoCase {
        selector,
        expected: result,
      })?;
    }

    engine.graphics.target = Target::Smooth {
      mode: SmoothMode::VerticalLcd,
      linear_metrics: crate::LinearMetrics::Preserve,
      symmetric_rendering: crate::SymmetricRendering::Disabled,
    };
    engine.getinfo_test(InfoCase {
      selector: VERTICAL_LCD_SELECTOR_BIT,
      expected: VERTICAL_LCD_RESULT_BIT,
    })?;

    engine.getinfo_test(InfoCase {
      selector: SYMMETRICAL_SMOOTHING_SELECTOR_BIT,
      expected: 0,
    })?;

    engine.getinfo_test(InfoCase {
      selector: GRAYSCALE_CLEARTYPE_SELECTOR_BIT,
      expected: 0,
    })?;

    engine.graphics.target = Target::default();

    engine.getinfo_test(InfoCase {
      selector: SYMMETRICAL_SMOOTHING_SELECTOR_BIT,
      expected: SYMMETRICAL_SMOOTHING_RESULT_BIT,
    })?;

    Ok(())
  }

  #[test]
  fn getvariation() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    assert!(matches!(
      engine.op_getvariation(),
      Err(HintErrorKind::UnhandledOpcode(Opcode::GETVARIATION))
    ));

    engine.axis_count = 2;

    let coords = [
      ({
        let Ok(value) = F2Dot14::from_f32(-1.0);
        value
      }),
      ({
        let Ok(value) = F2Dot14::from_f32(0.5);
        value
      }),
      ({
        let Ok(value) = F2Dot14::from_f32(1.0);
        value
      }),
    ];
    let [first_coord, second_coord, _] = coords.map(|x| {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>({
        let Ok(value) = (x).to_bits();
        value
      });
      migrated_value
    });

    engine.coords = {
      let [_explicit_part @ .., _, _] = &coords;
      let _explicit_slice: &[_] = _explicit_part;
      _explicit_slice
    };
    engine.op_getvariation().required()?;
    assert_eq!(engine.value_stack.len(), Ok(2));
    let required = engine.value_stack.values().required()?;
    assert_eq!(required, &[first_coord, 0]);
    let Ok(()) = engine.value_stack.clear();

    engine.coords = {
      let [_explicit_part @ ..] = &coords;
      let _explicit_slice: &[_] = _explicit_part;
      _explicit_slice
    };
    engine.op_getvariation().required()?;
    assert_eq!(engine.value_stack.len(), Ok(2));
    let engine_required = engine.value_stack.values().required()?;
    assert_eq!(engine_required, &[first_coord, second_coord]);
    let Ok(()) = engine.value_stack.clear();

    engine.coords = {
      let [_explicit_part @ .., _] = &coords;
      let _explicit_slice: &[_] = _explicit_part;
      _explicit_slice
    };
    engine.op_getvariation().required()?;
    assert_eq!(engine.value_stack.len(), Ok(2));
    let selected_axis_values = engine.value_stack.values().required()?;
    assert_eq!(selected_axis_values, &[first_coord, second_coord]);

    Ok(())
  }

  #[test]
  fn getdata() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    assert!(matches!(
      engine.op_getdata(),
      Err(HintErrorKind::UnhandledOpcode(Opcode::GETDATA))
    ));

    engine.axis_count = 1;
    engine.op_getdata().required()?;

    {
      let queried_value = engine.value_stack.pop().required()?;
      assert_eq!(queried_value, 17);
    }

    Ok(())
  }

  struct InfoCase {
    selector: i32,
    expected: i32,
  }

  impl Engine<'_> {
    fn getinfo_test(
      &mut self,
      InfoCase { selector, expected }: InfoCase,
    ) -> Result<(), crate::TestFailure> {
      use crate::test_support::Required;

      self.value_stack.push(selector).required()?;
      self.op_getinfo().required()?;
      let actual = self.value_stack.pop().required()?;
      assert_eq!(actual, expected);

      Ok(())
    }
  }
}
