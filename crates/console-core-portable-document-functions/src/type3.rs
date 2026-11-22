use crate::{Clamper, Function, Interpolation, PdfSample, TupleVec, interpolate};
use console_core_portable_document_syntax::object::Array;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Object;
use console_core_portable_document_syntax::object::dict::keys::{BOUNDS, ENCODE, FUNCTIONS};

enum BoundsPosition {
  Inside,
  Outside,
}

impl From<bool> for BoundsPosition {
  fn from(outside: bool) -> Self {
    match bool::then_some(outside, ()) {
      Some(()) => Self::Outside,
      None => Self::Inside,
    }
  }
}

#[derive(Debug)]
pub(crate) struct Type3 {
  pub(crate) functions: Vec<Function>,
  bounds: Vec<f32>,
  encode: TupleVec,
  pub(crate) clamper: Clamper,
}

impl Type3 {
  pub(crate) fn read<'a>(
    dict: &Dict<'a>,
  ) -> core::result::Result<Option<(Self, Vec<Object<'a>>)>, console_core_never::Never> {
    let Ok(explicit_value) = Clamper::new(dict);
    let clamper = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(listed) = dict.get::<Array<'a>>(FUNCTIONS);
    let functions = match listed {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(inner) = functions.iter::<Object<'a>>();
    let domain = *match clamper.domain.first() {
      Some(value) => value,
      None => return Ok(None),
    };
    let mut bounds = vec![domain.0 - 0.0001];
    let Ok(explicit_value_2) = dict.get::<Array<'_>>(BOUNDS);

    match explicit_value_2 {
      Some(a) => {
        let Ok(iter) = a.iter::<f32>();
        bounds.extend(iter);
      }
      None => {}
    }

    bounds.push(domain.1 + 0.0001);

    let Ok(explicit_value_4) = dict.get::<TupleVec>(ENCODE);
    let encode = match explicit_value_4 {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(Some((
      Self {
        functions: Vec::new(),
        clamper,
        bounds,
        encode,
      },
      inner.collect(),
    )))
  }

  pub(crate) fn select(
    &self,
    PdfSample(input): PdfSample,
  ) -> core::result::Result<Option<(&Function, f32)>, console_core_never::Never> {
    let mut input_2 = [input];
    let Ok(()) = self.clamper.clamp_input(&mut input_2);
    let [clamped] = input_2;
    let Ok(explicit_value) = find_interval(&self.bounds, PdfSample(clamped));
    let interval = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };
    let following = match interval.checked_add(1) {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(at) = console_core_number_conversion::index(interval);
    let Ok(next) = console_core_number_conversion::index(following);
    let bounds_i = *match self.bounds.get(next) {
      Some(value) => value,
      None => return Ok(None),
    };
    let bounds_i_minus_1 = *match self.bounds.get(at) {
      Some(value) => value,
      None => return Ok(None),
    };
    let encoding = match self.encode.get(at) {
      Some(value) => value,
      None => return Ok(None),
    };
    let function = match self.functions.get(at) {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(encoded) = interpolate(
      PdfSample(clamped),
      Interpolation {
        domain: [bounds_i_minus_1, bounds_i],
        range: [encoding.0, encoding.1],
      },
    );

    Ok(Some((function, encoded)))
  }
}

fn find_interval(
  bounds: &[f32],
  PdfSample(x): PdfSample,
) -> core::result::Result<Option<u32>, console_core_never::Never> {
  let first = *match bounds.first() {
    Some(value) => value,
    None => return Ok(None),
  };
  let last = *match bounds.last() {
    Some(value) => value,
    None => return Ok(None),
  };

  match BoundsPosition::from(x < first || x >= last) {
    BoundsPosition::Outside => return Ok(None),
    BoundsPosition::Inside => {}
  }

  let Ok(above) =
    console_core_number_conversion::fitted::<_, u32>(bounds.partition_point(|bound| *bound <= x));

  Ok(above.checked_sub(1))
}

#[cfg(test)]
mod tests {
  use crate::Function;
  use console_core_portable_document_syntax::object::FromBytes;
  use console_core_portable_document_syntax::object::Object;

  #[test]
  fn simple() -> Result<(), crate::TestFailure> {
    let data = b"<<
  /FunctionType 3
  /Domain [-7 7]
  /Functions [
    << /FunctionType 2
       /Domain [0 1]
       /C0 [0.5 0.5 0.5]
       /C1 [0.5 0.5 0.5]
       /N 1
    >>
    << /FunctionType 2
       /Domain [0 1]
       /C0 [0.7 0.7 0.7]
       /C1 [0.7 0.7 0.7]
       /N 1
    >>
  ]
  /Bounds [0]
  /Encode [0 1 0 1]
>>";

    let dict = Object::from_bytes(data).ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Function::new(&dict);
    let function = made.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(function.eval(vec![-7.0]), Ok(Some(vec![0.5, 0.5, 0.5])));
    assert_eq!(function.eval(vec![-3.0]), Ok(Some(vec![0.5, 0.5, 0.5])));
    assert_eq!(function.eval(vec![-0.5]), Ok(Some(vec![0.5, 0.5, 0.5])));
    assert_eq!(function.eval(vec![0.0]), Ok(Some(vec![0.7, 0.7, 0.7])));
    assert_eq!(function.eval(vec![7.0]), Ok(Some(vec![0.7, 0.7, 0.7])));

    Ok(())
  }

  #[test]
  fn stitching_inside_stitching_clamps_each_range_on_the_way_out()
  -> Result<(), crate::TestFailure> {
    let data = b"<<
  /FunctionType 3
  /Domain [0 1]
  /Range [0 0.6]
  /Bounds [0.5]
  /Encode [0 1 0 1]
  /Functions [
    << /FunctionType 3
       /Domain [0 1]
       /Range [0 0.5]
       /Bounds [0.5]
       /Encode [0 1 0 1]
       /Functions [
         << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>
         << /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [1] /N 1 >>
       ]
    >>
    << /FunctionType 2 /Domain [0 1] /C0 [0.7] /C1 [0.7] /N 1 >>
  ]
>>";

    let dict = Object::from_bytes(data).ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Function::new(&dict);
    let function = made.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(function.eval(vec![0.2]), Ok(Some(vec![0.5])));
    assert_eq!(function.eval(vec![0.4]), Ok(Some(vec![0.5])));
    assert_eq!(function.eval(vec![0.9]), Ok(Some(vec![0.6])));

    Ok(())
  }

  #[test]
  fn a_stitched_function_that_cannot_be_read_spoils_the_whole() -> Result<(), crate::TestFailure>
  {
    let data = b"<<
  /FunctionType 3
  /Domain [0 1]
  /Bounds [0.5]
  /Encode [0 1 0 1]
  /Functions [
    << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>
    << /FunctionType 9 /Domain [0 1] >>
  ]
>>";

    let dict = Object::from_bytes(data).ok_or(crate::TestFailure::MissingValue)?;

    assert!(matches!(Function::new(&dict), Ok(None)));

    Ok(())
  }
}
