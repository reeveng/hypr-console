#[cfg(feature = "images")]
#[inline(always)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Rounding changes the numeric value's precision while its caller supplies the unit"
  )
)]
pub(crate) fn round_f32(x: f32) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      x.round()
    }

    #[cfg(not(feature = "std"))]
    {
      match x >= 0.0 {
        true => {
          let Ok(rounded) = floor_f32(x + 0.5);
          rounded
        }
        false => {
          let Ok(rounded) = floor_f32(-x + 0.5);
          -rounded
        }
      }
    }
  })
}

#[cfg(all(feature = "images", not(feature = "std")))]
#[inline(always)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "This helper preserves the caller's units while finding an integral floating point value"
  )
)]
fn floor_f32(x: f32) -> Result<f32, console_core_never::Never> {
  let Ok(integer) = console_core_number_conversion::toward_zero_i32(f64::from(x));
  let Ok(nearest) = console_core_number_conversion::nearest_f32_from_i64(i64::from(integer));
  Ok(match x < nearest {
    true => nearest - 1.0,
    false => nearest,
  })
}

#[inline(always)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The truncation applies equally to PDF numbers with or without units"
  )
)]
pub(crate) fn trunc_f64(x: f64) -> core::result::Result<f64, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      x.trunc()
    }

    #[cfg(not(feature = "std"))]
    {
      use console_core_number_conversion::Float;

      let Ok(integer) = console_core_number_conversion::toward_zero_i64(x);
      let Ok(widened) = integer.float();
      widened
    }
  })
}

#[inline(always)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Exponentiation here computes decimal powers for the PDF numeric parser"
  )
)]
pub(crate) fn powi_f64(x: f64, n: u32) -> core::result::Result<f64, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      x.powi({
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(n);
        numeric_value
      })
    }

    #[cfg(not(feature = "std"))]
    {
      let mut result = 1.0;
      let mut base = x;
      let mut exp = n;

      for _ in 0..u32::BITS {
        match exp {
          0 => break,
          1.. => {}
        }

        match exp & 1 {
          1 => result *= base,
          _ => {}
        }

        base *= base;
        exp = exp.wrapping_shr(1);
      }

      result
    }
  })
}
