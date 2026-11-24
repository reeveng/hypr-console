//! Arithmetic shared by the desktop calculator and the native calculator executable.

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the calculator operates on dimensionless real numbers"
  )
)]
pub(crate) struct Number(pub(crate) f64);

pub(crate) struct Operands {
  pub(crate) left: Number,
  pub(crate) right: Number,
}

pub(crate) struct UnsupportedOperator;

pub(crate) fn calculate(operands: Operands, operator: u8) -> Result<Number, UnsupportedOperator> {
  let Operands {
    left: Number(left),
    right: Number(right),
  } = operands;
  let answer = match operator {
    b'+' => left + right,
    b'-' => left - right,
    b'*' => left * right,
    b'/' => left / right,
    _other => return Err(UnsupportedOperator),
  };

  Ok(Number(answer))
}
