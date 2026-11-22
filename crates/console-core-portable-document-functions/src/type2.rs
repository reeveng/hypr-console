use crate::{Clamper, PdfSample, Values};
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Number;
use console_core_portable_document_syntax::object::dict::keys::{C0, C1, N};

#[derive(Debug)]
pub(crate) struct Type2 {
  c0: Values,
  c1: Values,
  clamper: Clamper,
  n: Exponent,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the N entry of a PDF exponential function is a dimensionless power"
  )
)]
#[derive(Debug)]
struct Exponent(f32);

impl Type2 {
  pub(crate) fn new(
    dict: &Dict<'_>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(explicit_value) = dict.get::<Values>(C0);
    let c0 = match explicit_value {
      Some(c0) => c0,
      None => vec![0.0],
    };
    let Ok(explicit_value_2) = dict.get::<Values>(C1);
    let c1 = match explicit_value_2 {
      Some(c1) => c1,
      None => vec![1.0],
    };
    let Ok(explicit_value_3) = Clamper::new(dict);
    let clamper = match explicit_value_3 {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(explicit_value_4) = dict.get::<Number>(N);
    let exponent = match explicit_value_4 {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(wide) = exponent.as_f64();
    let Ok(n) = console_core_number_conversion::narrow_f32(wide);

    Ok(Some(Self { c0, c1, clamper, n: Exponent(n) }))
  }

  pub(crate) fn eval(
    &self,
    PdfSample(input): PdfSample,
  ) -> core::result::Result<Values, console_core_never::Never> {
    let mut input_2 = [input];
    let Ok(()) = self.clamper.clamp_input(&mut input_2);
    let [clamped] = input_2;

    let mut out = self
      .c0
      .iter()
      .zip(self.c1.iter())
      .map(|(c0, c1)| *c0 + clamped.powf(self.n.0) * (*c1 - *c0))
      .collect::<Vec<_>>();

    let Ok(()) = self.clamper.clamp_output(&mut out);

    Ok(out)
  }
}

#[cfg(test)]
mod tests {
  use crate::Function;

  use console_core_portable_document_syntax::object::Object;
  use console_core_portable_document_syntax::object::{Dict, FromBytes};

  #[test]
  fn simple() -> Result<(), crate::TestFailure> {
    let dictionary = Dict::from_bytes(
      b"<<
              /FunctionType 2
              /Domain [ 0  1 ]
              /C0 [ 0 20  ]
              /C1 [ 30 -50 ]
              /N 1
            >>",
    )
    .ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Function::new(&Object::Dict(dictionary));
    let func = made.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(func.eval(vec![0.0]), Ok(Some(vec![0.0, 20.0])));
    assert_eq!(func.eval(vec![0.5]), Ok(Some(vec![15.0, -15.0])));
    assert_eq!(func.eval(vec![1.0]), Ok(Some(vec![30.0, -50.0])));

    Ok(())
  }

  #[test]
  fn with_exponent() -> Result<(), crate::TestFailure> {
    let dictionary = Dict::from_bytes(
      b"<<
              /FunctionType 2
              /Domain [ 0  1 ]
              /C0 [ 0  ]
              /C1 [ 30 ]
              /N 2
            >>",
    )
    .ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Function::new(&Object::Dict(dictionary));
    let func = made.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(func.eval(vec![0.5]), Ok(Some(vec![7.5])));

    Ok(())
  }

  #[test]
  fn clamp_domain() -> Result<(), crate::TestFailure> {
    let dictionary = Dict::from_bytes(
      b"<<
              /FunctionType 2
              /Domain [ 0.2  0.8 ]
              /C0 [ 0  ]
              /C1 [ 30 ]
              /N 2
            >>",
    )
    .ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Function::new(&Object::Dict(dictionary));
    let func = made.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(func.eval(vec![0.0]), func.eval(vec![0.2]));
    assert_eq!(func.eval(vec![-10.]), func.eval(vec![0.2]));
    assert_eq!(func.eval(vec![0.8]), func.eval(vec![0.8]));
    assert_eq!(func.eval(vec![1.2]), func.eval(vec![1.0]));

    Ok(())
  }

  #[test]
  fn clamp_range() -> Result<(), crate::TestFailure> {
    let dictionary = Dict::from_bytes(
      b"<<
              /FunctionType 2
              /Domain [ 0.0  1.0 ]
              /Range [10.0 20.0]
              /C0 [ 0  ]
              /C1 [ 30 ]
              /N 1
            >>",
    )
    .ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Function::new(&Object::Dict(dictionary));
    let func = made.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(func.eval(vec![0.0]), Ok(Some(vec![10.0])));
    assert_eq!(func.eval(vec![0.5]), Ok(Some(vec![15.0])));
    assert_eq!(func.eval(vec![1.0]), Ok(Some(vec![20.0])));

    Ok(())
  }
}
