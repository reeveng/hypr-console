//! Various types of number parsers.

use super::error::Error;
use crate::Cursor;
use types::Fixed;

#[inline]
pub(crate) fn parse_int(cursor: &mut Cursor, b0: u8) -> Result<i32, Error> {
  Ok(match b0 {
    32..=246 => ::core::primitive::i32::wrapping_sub(
      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(b0);
        migrated_value
      },
      139,
    ),
    247..=250 => ::core::primitive::i32::wrapping_add(
      ::core::primitive::i32::wrapping_add(
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_sub(
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(b0);
              migrated_value
            },
            247,
          ),
          256,
        ),
        {
          let read = cursor.read::<u8>()?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(read);
          migrated_value
        },
      ),
      108,
    ),
    251..=254 => ::core::primitive::i32::wrapping_sub(
      ::core::primitive::i32::wrapping_sub(
        ::core::primitive::i32::wrapping_mul(
          ::core::primitive::i32::wrapping_neg(::core::primitive::i32::wrapping_sub(
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(b0);
              migrated_value
            },
            251,
          )),
          256,
        ),
        {
          let read = cursor.read::<u8>()?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(read);
          migrated_value
        },
      ),
      108,
    ),
    28 => {
      let read = cursor.read::<i16>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(read);
      migrated_value
    }
    29 => cursor.read::<i32>()?,
    _ => {
      return Err(Error::InvalidNumber);
    }
  })
}

const BCD_OVERFLOW: Fixed = {
  let Ok(value) = Fixed::from_bits(0x7FFFFFFF);
  value
};

const BCD_UNDERFLOW: Fixed = Fixed::ZERO;

const BCD_NUMBER_LIMIT: i32 = 0xCCCCCCC;

const BCD_INTEGER_LIMIT: i32 = 0x7FFF;

pub(crate) const BCD_POWER_TENS: [i32; 10] = [
  1, 10, 100, 1000, 10000, 100000, 1000000, 10000000, 100000000, 1000000000,
];

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct BcdComponents {
  error: Option<Fixed>,
  number: i32,
  sign: i32,
  exponent: i32,
  exponent_add: i32,
  integer_len: i32,
  fraction_len: i32,
}

#[derive(Clone, Copy)]
enum BcdPhase {
  Integer,
  Fraction,
  PositiveExponent,
  NegativeExponent,
}

enum BcdRead {
  Continue,
  Finish,
  Saturated(BcdComponents),
}

impl BcdComponents {
  pub(crate) fn parse(cursor: &mut Cursor) -> Result<Self, Error> {
    let mut phase = BcdPhase::Integer;
    let mut value = Self {
      sign: 1,
      ..Self::default()
    };
    let Ok(remaining) = cursor.remaining_bytes();

    'outer: for _ in 0..=remaining {
      let packed = cursor.read::<u8>()?;

      for nibble in [packed.wrapping_shr(4) & 0xF, packed & 0xF] {
        match value.read_nibble(&mut phase, nibble) {
          Ok(BcdRead::Continue) => {}
          Ok(BcdRead::Finish) => break 'outer,
          Ok(BcdRead::Saturated(replacement)) => return Ok(replacement),
        }
      }
    }

    match phase {
      BcdPhase::NegativeExponent => value.exponent = value.exponent.wrapping_neg(),
      BcdPhase::Integer | BcdPhase::Fraction | BcdPhase::PositiveExponent => {}
    }

    Ok(value)
  }

  fn read_nibble(
    &mut self,
    phase: &mut BcdPhase,
    nibble: u8,
  ) -> Result<BcdRead, console_core_never::Never> {
    match (*phase, nibble) {
      (BcdPhase::Integer, digit @ 0..=9) => {
        let Ok(()) = self.append_integer_digit(digit);
      }
      (BcdPhase::Fraction, digit @ 0..=9) => {
        let Ok(()) = self.append_fraction_digit(digit);
      }
      (BcdPhase::Integer, 0xE) => self.sign = -1,
      (BcdPhase::Integer, 0xA) => *phase = BcdPhase::Fraction,
      (BcdPhase::Integer | BcdPhase::Fraction, 0xB) => *phase = BcdPhase::PositiveExponent,
      (BcdPhase::Integer | BcdPhase::Fraction, 0xC) => *phase = BcdPhase::NegativeExponent,
      (BcdPhase::PositiveExponent | BcdPhase::NegativeExponent, digit @ 0..=9) => {
        let Ok(exceptional) = self.append_exponent_digit(digit, *phase);

        match exceptional {
          Some(replacement) => return Ok(BcdRead::Saturated(replacement)),
          None => {}
        }
      }
      (_phase, _terminator) => return Ok(BcdRead::Finish),
    }

    Ok(BcdRead::Continue)
  }

  fn append_integer_digit(&mut self, digit: u8) -> Result<(), console_core_never::Never> {
    match (self.number >= BCD_NUMBER_LIMIT).then_some(()) {
      Some(()) => {
        self.exponent_add = self.exponent_add.wrapping_add(1);
        return Ok(());
      }
      None => {}
    }

    match (digit == 0 && self.number == 0).then_some(()) {
      Some(()) => {}
      None => {
        self.number = self.number.wrapping_mul(10).wrapping_add(i32::from(digit));
        self.integer_len = self.integer_len.wrapping_add(1);
      }
    }

    Ok(())
  }

  fn append_fraction_digit(&mut self, digit: u8) -> Result<(), console_core_never::Never> {
    match (digit == 0 && self.number == 0).then_some(()) {
      Some(()) => {
        self.exponent_add = self.exponent_add.wrapping_sub(1);
        return Ok(());
      }
      None => {}
    }

    match (self.number < BCD_NUMBER_LIMIT && self.fraction_len < 9).then_some(()) {
      Some(()) => {
        self.number = self.number.wrapping_mul(10).wrapping_add(i32::from(digit));
        self.fraction_len = self.fraction_len.wrapping_add(1);
      }
      None => {}
    }

    Ok(())
  }

  fn append_exponent_digit(
    &mut self,
    digit: u8,
    phase: BcdPhase,
  ) -> Result<Option<Self>, console_core_never::Never> {
    Ok(match self.exponent.cmp(&1000) {
      core::cmp::Ordering::Greater => {
        let limit = match phase {
          BcdPhase::NegativeExponent => BCD_UNDERFLOW,
          BcdPhase::Integer | BcdPhase::Fraction | BcdPhase::PositiveExponent => BCD_OVERFLOW,
        };
        Some(Self::from(limit))
      }
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {
        self.exponent = self
          .exponent
          .wrapping_mul(10)
          .wrapping_add(i32::from(digit));
        None
      }
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit008_no_bool_param",
    expect(
      explicit008_no_bool_param,
      reason = "CFF DICT Token::Operand exposes BcdComponents and this existing Boolean selects the BlueScale conversion"
    )
  )]
  pub fn value(
    &self,
    scale_by_1000: bool,
  ) -> core::result::Result<Fixed, console_core_never::Never> {
    match self.error {
      Some(error) => {
        return Ok(error);
      }
      None => {}
    }

    let mut number = self.number;

    match ::core::primitive::bool::then_some(number == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(Fixed::ZERO);
      }
      ::core::option::Option::None => {}
    }

    let mut exponent = self.exponent;
    let mut integer_len = self.integer_len;
    let mut fraction_len = self.fraction_len;

    match ::core::primitive::bool::then_some(scale_by_1000, ()) {
      ::core::option::Option::Some(()) => {
        match (
          ::core::primitive::i32::wrapping_add(3, self.exponent_add),
          &mut exponent,
        ) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };
      }
      ::core::option::Option::None => {
        match (self.exponent_add, &mut exponent) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };
      }
    }

    match (exponent, &mut integer_len) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    match (exponent, &mut fraction_len) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::i32::wrapping_sub(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    match ::core::primitive::bool::then_some(integer_len > 5, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(BCD_OVERFLOW);
      }
      ::core::option::Option::None => {}
    }

    const MIN_INTEGER_LENGTH: i32 = -5;

    match ::core::primitive::bool::then_some(integer_len < MIN_INTEGER_LENGTH, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(BCD_UNDERFLOW);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(integer_len < 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_20) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
            ::core::primitive::i32::wrapping_neg(integer_len),
          );
          migrated_value
        });

        let divisor = match BCD_POWER_TENS.get(value_20).copied() {
          Some(divisor) => divisor,
          None => return Ok(BCD_UNDERFLOW),
        };
        number = match number.checked_div(divisor) {
          Some(quotient) => quotient,
          None => return Ok(BCD_UNDERFLOW),
        };

        match (integer_len, &mut fraction_len) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(fraction_len == 10, ()) {
      ::core::option::Option::Some(()) => {
        number = ::core::primitive::i32::wrapping_div(number, 10);
        fraction_len = ::core::primitive::i32::wrapping_sub(fraction_len, 1);
      }
      ::core::option::Option::None => {}
    }

    let mut result = match ::core::primitive::bool::then_some(fraction_len > 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_21) = console_core_number_conversion::index({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(fraction_len);
          migrated_value
        });

        let b = match BCD_POWER_TENS.get(value_21).copied() {
          Some(divisor) => divisor,
          None => return Ok(BCD_UNDERFLOW),
        };

        let leading_digits = match number.checked_div(b) {
          Some(quotient) => quotient,
          None => return Ok(BCD_UNDERFLOW),
        };

        match (leading_digits > BCD_INTEGER_LIMIT).then_some(()) {
          ::core::option::Option::Some(()) => 0,
          ::core::option::Option::None => {
            let Ok(value_1) = Fixed::from_bits(number);
            let Ok(value_2) = Fixed::from_bits(b);

            let Ok(value_17) = (value_1 / value_2).to_bits();

            value_17
          }
        }
      }
      ::core::option::Option::None => {
        let Ok(value_22) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
            ::core::primitive::i32::wrapping_neg(fraction_len),
          );
          migrated_value
        });

        let factor = match BCD_POWER_TENS.get(value_22).copied() {
          Some(factor) => factor,
          None => return Ok(BCD_OVERFLOW),
        };
        number = number.wrapping_mul(factor);

        match ::core::primitive::bool::then_some(number > BCD_INTEGER_LIMIT, ()) {
          ::core::option::Option::Some(()) => {
            return Ok(BCD_OVERFLOW);
          }
          ::core::option::Option::None => ::core::primitive::i32::wrapping_shl(number, 16_u32),
        }
      }
    };

    match ::core::primitive::bool::then_some(scale_by_1000, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_3) = Fixed::from_bits(result);
        let Ok(value_4) = Fixed::from_i32(1000);

        let Ok(value_19) = (value_3 / value_4).to_bits();

        result = value_19;
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_5) = Fixed::from_bits(::core::primitive::i32::wrapping_mul(result, self.sign));

    Ok(value_5)
  }

  #[must_use]
  pub(crate) fn dynamically_scaled_value(
    &self,
  ) -> Result<(Fixed, i32), console_core_never::Never> {
    match self.error {
      Some(error) => return Ok((error, 0)),
      None => {}
    }

    match self.number {
      0 => return Ok((Fixed::ZERO, 0)),
      _nonzero => {}
    }

    let magnitude = BcdMagnitude {
      digits: self.number,
      exponent: self
        .exponent
        .wrapping_add(self.exponent_add)
        .wrapping_add(self.integer_len),
      fraction_len: self.fraction_len.wrapping_add(self.integer_len),
    };
    let Ok(outcome) = match (magnitude.fraction_len <= 5).then_some(()) {
      Some(()) => magnitude.scale_short(),
      None => magnitude.scale_long(),
    };
    let (unsigned, scaling) = match outcome {
      BcdScaled::Value {
        magnitude: magnitude_2,
        scaling,
      } => (magnitude_2, scaling),
      BcdScaled::Overflow => return Ok((BCD_OVERFLOW, 0)),
      BcdScaled::Underflow => return Ok((BCD_UNDERFLOW, 0)),
    };
    let Ok(bits) = unsigned.to_bits();
    let Ok(signed) = Fixed::from_bits(bits.wrapping_mul(self.sign));

    Ok((signed, scaling))
  }
}

struct BcdMagnitude {
  digits: i32,
  exponent: i32,
  fraction_len: i32,
}

enum BcdScaled {
  Value { magnitude: Fixed, scaling: i32 },
  Underflow,
  Overflow,
}

impl BcdMagnitude {
  fn scale_short(self) -> Result<BcdScaled, console_core_never::Never> {
    let BcdMagnitude {
      mut digits,
      exponent,
      fraction_len,
    } = self;

    match digits.cmp(&BCD_INTEGER_LIMIT) {
      core::cmp::Ordering::Greater => {
        let Ok(magnitude) = Fixed::from_bits(digits);
        let Ok(ten) = Fixed::from_bits(10);
        return Ok(BcdScaled::Value {
          magnitude: magnitude / ten,
          scaling: exponent.wrapping_sub(fraction_len).wrapping_add(1),
        });
      }
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
    }

    let target_digits = exponent.min(5);
    let shift = target_digits.wrapping_sub(fraction_len);
    let mut scaling = exponent.wrapping_sub(fraction_len);

    match (exponent > 0 && shift > 0).then_some(()) {
      Some(()) => {
        let Ok(position) = console_core_number_conversion::index(shift.unsigned_abs());
        let factor = match BCD_POWER_TENS.get(position) {
          Some(factor) => *factor,
          None => return Ok(BcdScaled::Overflow),
        };
        scaling = exponent.wrapping_sub(target_digits);
        digits = digits.wrapping_mul(factor);

        match digits.cmp(&BCD_INTEGER_LIMIT) {
          core::cmp::Ordering::Greater => {
            digits = digits.wrapping_div(10);
            scaling = scaling.wrapping_add(1);
          }
          core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
        }
      }
      None => {}
    }

    let Ok(magnitude) = Fixed::from_bits(digits.wrapping_shl(16));
    Ok(BcdScaled::Value { magnitude, scaling })
  }

  fn scale_long(self) -> Result<BcdScaled, console_core_never::Never> {
    let position = self.fraction_len.wrapping_sub(5).unsigned_abs();
    let Ok(index) = console_core_number_conversion::index(position);
    let divisor = match BCD_POWER_TENS.get(index) {
      Some(divisor) => *divisor,
      None => return Ok(BcdScaled::Underflow),
    };
    let leading_digits = match self.digits.checked_div(divisor) {
      Some(quotient) => quotient,
      None => return Ok(BcdScaled::Underflow),
    };
    let (divisor_2, fraction_digits) = match leading_digits.cmp(&BCD_INTEGER_LIMIT) {
      core::cmp::Ordering::Greater => {
        let Ok(next_index) = console_core_number_conversion::index(position.wrapping_add(1));

        match BCD_POWER_TENS.get(next_index) {
          Some(scaled_divisor) => (*scaled_divisor, 4),
          None => return Ok(BcdScaled::Underflow),
        }
      }
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => (divisor, 5),
    };

    let Ok(numerator) = Fixed::from_bits(self.digits);
    let Ok(denominator) = Fixed::from_bits(divisor_2);
    Ok(BcdScaled::Value {
      magnitude: numerator / denominator,
      scaling: self.exponent.wrapping_sub(fraction_digits),
    })
  }
}

impl From<Fixed> for BcdComponents {
  fn from(value: Fixed) -> Self {
    Self {
      error: Some(value),
      ..Default::default()
    }
  }
}

pub(crate) fn parse_fixed_dynamic(cursor: &mut Cursor) -> Result<(Fixed, i32), Error> {
  let b0 = cursor.read::<u8>()?;

  match b0 {
    30 => {
      let parse = BcdComponents::parse(cursor)?;

      let Ok(dynamically_scaled_value) = parse.dynamically_scaled_value();

      Ok(dynamically_scaled_value)
    }
    28 | 29 | 32..=254 => {
      let num = parse_int(cursor, b0)?;

      match ::core::primitive::bool::then_some(num > BCD_INTEGER_LIMIT, ()) {
        ::core::option::Option::Some(()) => {
          const MIN_SCALE_DIGITS: u32 = 5;
          let digits = num.ilog10().saturating_add(1).max(MIN_SCALE_DIGITS);
          let initial_exponent = digits.saturating_sub(MIN_SCALE_DIGITS);
          let Ok(index) = console_core_number_conversion::index(initial_exponent);
          let base_divisor = BCD_POWER_TENS
            .get(index)
            .copied()
            .ok_or(Error::InvalidNumber)?;
          let exponent = match num.wrapping_sub(base_divisor).cmp(&BCD_INTEGER_LIMIT) {
            core::cmp::Ordering::Greater => initial_exponent.saturating_add(1),
            core::cmp::Ordering::Equal | core::cmp::Ordering::Less => initial_exponent,
          };
          let Ok(scale_index) = console_core_number_conversion::index(exponent);
          let divisor = BCD_POWER_TENS
            .get(scale_index)
            .copied()
            .ok_or(Error::InvalidNumber)?;
          let Ok(fixed) = Fixed::from_bits(num);
          let Ok(scale) = Fixed::from_bits(divisor);
          let Ok(signed_exponent) = console_core_number_conversion::wrapped::<_, i32>(exponent);
          Ok((fixed / scale, signed_exponent))
        }
        ::core::option::Option::None => {
          let Ok(value_16) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(num, 16_u32));

          Ok((value_16, 0))
        }
      }
    }
    _ => Err(Error::InvalidNumber),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::FontData;

  #[test]
  fn int_operands() -> ::core::result::Result<(), crate::ps::error::Error> {
    let Ok(empty) = FontData::new(&[]);
    let Ok(min_byte) = FontData::new(&[0]);
    let Ok(max_byte) = FontData::new(&[255]);

    let asserted_value = parse_int(
      &mut ({
        let Ok(value) = empty.cursor();
        value
      }),
      32,
    )?;
    assert_eq!(asserted_value, -107);
    let asserted_value_2 = parse_int(
      &mut ({
        let Ok(value) = empty.cursor();
        value
      }),
      246,
    )?;
    assert_eq!(asserted_value_2, 107);

    let asserted_value_3 = parse_int(
      &mut ({
        let Ok(value) = min_byte.cursor();
        value
      }),
      247,
    )?;
    assert_eq!(asserted_value_3, 108);
    let asserted_value_4 = parse_int(
      &mut ({
        let Ok(value) = max_byte.cursor();
        value
      }),
      250,
    )?;
    assert_eq!(asserted_value_4, 1131);

    let asserted_value_5 = parse_int(
      &mut ({
        let Ok(value) = min_byte.cursor();
        value
      }),
      251,
    )?;
    assert_eq!(asserted_value_5, -108);
    let asserted_value_6 = parse_int(
      &mut ({
        let Ok(value) = max_byte.cursor();
        value
      }),
      254,
    )?;
    assert_eq!(asserted_value_6, -1131);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn binary_coded_decimal_operands() -> ::core::result::Result<(), crate::ps::error::Error> {
    let Ok(bytes) = FontData::new(&[0xe2, 0xa2, 0x5f]);
    assert_eq!(
      ({
        let Ok(explicit_value) = match BcdComponents::parse(
          &mut ({
            let Ok(value) = bytes.cursor();
            value
          }),
        ) {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) => {
            return ::core::result::Result::Err(read_error);
          }
        }
        .value(false);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = Fixed::from_f64(-2.25);
        explicit_value
      })
    );
    let Ok(bytes_2) = FontData::new(&[0x0a, 0x14, 0x05, 0x41, 0xc3, 0xff]);
    assert_eq!(
      ({
        let Ok(explicit_value) = match BcdComponents::parse(
          &mut ({
            let Ok(value) = bytes_2.cursor();
            value
          }),
        ) {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) => {
            return ::core::result::Result::Err(read_error);
          }
        }
        .value(false);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = Fixed::from_f64(0.140541E-3);
        explicit_value
      })
    );

    let Ok(bytes_3) = FontData::new(&[0x37, 0x5c, 0x4f]);
    assert_eq!(
      ({
        let Ok(explicit_value) = match BcdComponents::parse(
          &mut ({
            let Ok(value) = bytes_3.cursor();
            value
          }),
        ) {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) => {
            return ::core::result::Result::Err(read_error);
          }
        }
        .value(false);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = Fixed::from_f64(0.0370025634765625);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn scaled_binary_coded_decimal_operands() -> ::core::result::Result<(), crate::ps::error::Error>
  {
    let Ok(bytes) = FontData::new(&[0xA, 0x06, 0x25, 0xf]);
    assert_eq!(
      ({
        let Ok(explicit_value) = match BcdComponents::parse(
          &mut ({
            let Ok(value) = bytes.cursor();
            value
          }),
        ) {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) => {
            return ::core::result::Result::Err(read_error);
          }
        }
        .value(true);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = Fixed::from_f64(0.0625);
        explicit_value
      })
    );

    let Ok(bytes_2) = FontData::new(&[0x37, 0x5c, 0x4f]);
    assert_eq!(
      ({
        let Ok(explicit_value) = match BcdComponents::parse(
          &mut ({
            let Ok(value) = bytes_2.cursor();
            value
          }),
        ) {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) => {
            return ::core::result::Result::Err(read_error);
          }
        }
        .value(true);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = Fixed::from_f64(0.037506103515625);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn dynamically_scaled_binary_coded_decimal_operands()
  -> ::core::result::Result<(), crate::ps::error::Error> {
    let Ok(bytes) = FontData::new(&[0xA, 0x06, 0x25, 0xf]);
    let Ok(dynamically_scaled_value) = match BcdComponents::parse(
      &mut ({
        let Ok(value) = bytes.cursor();
        value
      }),
    ) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(read_error),
    }
    .dynamically_scaled_value();
    assert_eq!(
      dynamically_scaled_value,
      (
        {
          let Ok(value) = Fixed::from_f64(6250.0);
          value
        },
        -5
      )
    );

    let Ok(bytes_2) = FontData::new(&[0x37, 0x5c, 0x4f]);
    assert_eq!(
      ({
        let Ok(explicit_value) = match BcdComponents::parse(
          &mut ({
            let Ok(value) = bytes_2.cursor();
            value
          }),
        ) {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) => {
            return ::core::result::Result::Err(read_error);
          }
        }
        .dynamically_scaled_value();
        explicit_value
      }),
      (
        {
          let Ok(value) = Fixed::from_f64(375.0);
          value
        },
        -4
      )
    );

    let Ok(bytes_3) = FontData::new(&[0xa0, 0x1, 0x95, 0x31, 0x25, 0xff]);
    assert_eq!(
      ({
        let Ok(explicit_value) = match BcdComponents::parse(
          &mut ({
            let Ok(value) = bytes_3.cursor();
            value
          }),
        ) {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) => {
            return ::core::result::Result::Err(read_error);
          }
        }
        .dynamically_scaled_value();
        explicit_value
      }),
      (
        {
          let Ok(value) = Fixed::from_bits(1280000000);
          value
        },
        -7
      )
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn blue_scale_fraction_length_of_0() -> ::core::result::Result<(), crate::ps::error::Error> {
    let Ok(bytes) = FontData::new(&[0x37, 0xC3, 0xFF]);
    assert_eq!(
      ({
        let Ok(explicit_value) = match BcdComponents::parse(
          &mut ({
            let Ok(value) = bytes.cursor();
            value
          }),
        ) {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) => {
            return ::core::result::Result::Err(read_error);
          }
        }
        .value(true);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = Fixed::from_f64(0.0370025634765625);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }
}
