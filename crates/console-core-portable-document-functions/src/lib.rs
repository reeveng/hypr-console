//! PDF functions.
//!
//! PDF has the concept of functions, representing objects that take a certain number of values
//! as input, do some processing on them and then return some output.

#![forbid(unsafe_code)]

mod type0;
mod type2;
mod type3;
mod type4;

use crate::type0::Type0;
use crate::type2::Type2;
use crate::type3::Type3;
use crate::type4::Type4;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::dict::keys::{DOMAIN, FUNCTION_TYPE, RANGE};
use console_core_portable_document_syntax::object::{Object, dict_or_stream};
use std::cmp::Ordering;
use std::sync::Arc;

pub type Values = Vec<f32>;
type TupleVec = Vec<(f32, f32)>;

#[derive(Debug)]
enum FunctionType {
  Type0(Type0),
  Type2(Type2),
  Type3(Type3),
  Type4(Type4),
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "graphics state snapshots share immutable compiled PDF functions. Copying a snapshot must not duplicate its sample table or calculator program, and dropping a function is thread-independent"
  )
)]
#[derive(Debug, Clone)]
pub struct Function(Arc<FunctionType>);

const MOST_BUILDING_STEPS: u32 = 65_536;
const DEEPEST_STITCHING: u32 = 4_096;

enum Work<'a> {
  Read(Object<'a>),
  Stitch(Type3, u32),
}

enum Built {
  Went,
  Failed,
  Finished(Function),
}

impl Function {
  pub fn new(obj: &Object<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let mut waiting = vec![Work::Read(obj.clone())];
    let mut built = Vec::new();

    for _ in 0..MOST_BUILDING_STEPS {
      let Ok(stepped) = build_step(&mut waiting, &mut built);

      match stepped {
        Built::Went => continue,
        Built::Failed => return Ok(None),
        Built::Finished(function) => return Ok(Some(function)),
      }
    }

    Ok(None)
  }

  pub fn eval(
    &self,
    mut input: Values,
  ) -> core::result::Result<Option<Values>, console_core_never::Never> {
    let mut current = self;
    let mut clampers = Vec::new();

    for _ in 0..DEEPEST_STITCHING {
      let stitching = match current.0.as_ref() {
        FunctionType::Type3(stitching) => stitching,
        leaf @ (FunctionType::Type0(_) | FunctionType::Type2(_) | FunctionType::Type4(_)) => {
          return evaluate(leaf, input, &clampers);
        }
      };
      let sample = match input.first() {
        Some(first) => PdfSample(*first),
        None => return Ok(None),
      };
      let Ok(selected) = stitching.select(sample);

      match selected {
        Some((inner, encoded)) => {
          current = inner;
          input.clear();
          input.push(encoded);
          clampers.push(&stitching.clamper);
        }
        None => return Ok(None),
      }
    }

    Ok(None)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "a compiled function is shared immutably across graphics state snapshots so a saved state does not allocate or copy its tables"
    )
  )]
  fn compiled(function: FunctionType) -> Result<Self, console_core_never::Never> {
    Ok(Self(Arc::new(function)))
  }
}

fn build_step<'a>(
  waiting: &mut Vec<Work<'a>>,
  built: &mut Vec<Function>,
) -> core::result::Result<Built, console_core_never::Never> {
  Ok(match waiting.pop() {
    None => match (built.pop(), built.as_slice()) {
      (Some(function), []) => Built::Finished(function),
      (Some(_), [_, ..]) | (None, _) => Built::Failed,
    },
    Some(Work::Read(object)) => {
      let Ok(read) = read(&object, waiting, built);

      match read {
        Some(()) => Built::Went,
        None => Built::Failed,
      }
    }
    Some(Work::Stitch(mut stitching, count)) => {
      let Ok(held) = console_core_number_conversion::fitted::<_, u32>(built.len());
      let start = match held.checked_sub(count) {
        Some(value) => value,
        None => return Ok(Built::Failed),
      };
      let Ok(first) = console_core_number_conversion::index(start);

      stitching.functions = built.split_off(first);
      let Ok(function) = Function::compiled(FunctionType::Type3(stitching));

      built.push(function);

      Built::Went
    }
  })
}

fn read<'a>(
  object: &Object<'a>,
  waiting: &mut Vec<Work<'a>>,
  built: &mut Vec<Function>,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  let Ok(explicit_value) = dict_or_stream(object);
  let (dict, stream) = match explicit_value {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(explicit_value_2) = dict.get::<u8>(FUNCTION_TYPE);
  let function_type = match explicit_value_2 {
    Some(value) => value,
    None => return Ok(None),
  };

  let leaf = match function_type {
    0 => {
      let Ok(made) = Type0::new(match stream {
        Some(value) => value,
        None => return Ok(None),
      });

      FunctionType::Type0(match made {
        Some(value) => value,
        None => return Ok(None),
      })
    }
    2 => {
      let Ok(made) = Type2::new(dict);

      FunctionType::Type2(match made {
        Some(value) => value,
        None => return Ok(None),
      })
    }
    3 => {
      let Ok(made) = Type3::read(dict);
      let (stitching, inner) = match made {
        Some(value) => value,
        None => return Ok(None),
      };
      let Ok(count) = console_core_number_conversion::fitted::<_, u32>(inner.len());

      waiting.push(Work::Stitch(stitching, count));
      waiting.extend(inner.into_iter().rev().map(Work::Read));

      return Ok(Some(()));
    }
    4 => {
      let Ok(made) = Type4::new(match stream {
        Some(value) => value,
        None => return Ok(None),
      });

      FunctionType::Type4(match made {
        Some(value) => value,
        None => return Ok(None),
      })
    }
    _ => return Ok(None),
  };

  let Ok(function) = Function::compiled(leaf);

  built.push(function);

  Ok(Some(()))
}

fn evaluate(
  function: &FunctionType,
  input: Values,
  clampers: &[&Clamper],
) -> core::result::Result<Option<Values>, console_core_never::Never> {
  let evaluated = match function {
    FunctionType::Type0(t0) => {
      let Ok(evaluated) = t0.eval(input);

      evaluated
    }
    FunctionType::Type2(t2) => {
      let first = match input.first() {
        Some(value) => PdfSample(*value),
        None => return Ok(None),
      };
      let Ok(evaluated) = t2.eval(first);

      Some(evaluated)
    }
    FunctionType::Type3(_t3) => None,
    FunctionType::Type4(t4) => {
      let Ok(evaluated) = t4.eval(input);

      evaluated
    }
  };
  let mut output = match evaluated {
    Some(output) => output,
    None => return Ok(None),
  };

  for clamper in clampers.iter().rev() {
    let Ok(()) = clamper.clamp_output(&mut output);
  }

  Ok(Some(output))
}

#[derive(Debug, Clone)]
struct Clamper {
  domain: TupleVec,
  range: Option<TupleVec>,
}

impl Clamper {
  fn new(dict: &Dict<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(explicit_value) = dict.get::<TupleVec>(DOMAIN);
    let domain = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(range) = dict.get::<TupleVec>(RANGE);

    Ok(Some(Self { domain, range }))
  }

  fn clamp_input(
    &self,
    input: &mut [f32],
  ) -> core::result::Result<(), console_core_never::Never> {
    match input.len().cmp(&self.domain.len()) {
      Ordering::Less | Ordering::Greater => {
        #[cfg(feature = "logging")]
        console_core_logging::warn!(
          "the domain of the function didn't match the input arguments"
        );
      }
      Ordering::Equal => {}
    }

    for ((min, max), val) in self.domain.iter().zip(input.iter_mut()) {
      *val = val.min(*max).max(*min);
    }

    Ok(())
  }

  fn clamp_output(
    &self,
    output: &mut [f32],
  ) -> core::result::Result<(), console_core_never::Never> {
    match &self.range {
      Some(range) => {
        match range.len().cmp(&output.len()) {
          Ordering::Less | Ordering::Greater => {
            #[cfg(feature = "logging")]
            console_core_logging::warn!(
              "the range of the function didn't match the output arguments"
            );
          }
          Ordering::Equal => {}
        }

        for ((min, max), val) in range.iter().zip(output.iter_mut()) {
          *val = val.min(*max).max(*min);
        }
      }
      None => {}
    }

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "PDF function samples are real numbers whose physical meaning is assigned by the caller"
  )
)]
#[derive(Debug, Clone, Copy)]
pub struct PdfSample(pub f32);

pub struct Interpolation {
  pub domain: [f32; 2],
  pub range: [f32; 2],
}

pub fn interpolate(
  PdfSample(sample): PdfSample,
  Interpolation { domain, range }: Interpolation,
) -> core::result::Result<f32, console_core_never::Never> {
  let [from_low, from_high] = domain;
  let [to_low, to_high] = range;

  Ok(to_low + (sample - from_low) * (to_high - to_low) / (from_high - from_low))
}

#[cfg(test)]
#[derive(Debug)]
enum TestFailure {
  MissingValue,
}

mod answers;
pub use answers::*;
