//! Arithmetic and math instructions.
//!
//! Implements 10 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#arithmetic-and-math-instructions>

use super::{super::math, Engine, HintErrorKind, OpResult};

impl Engine<'_> {
  pub(super) fn op_add(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(a.wrapping_add(b)))
  }

  pub(super) fn op_sub(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(a.wrapping_sub(b)))
  }

  pub(super) fn op_div(&mut self) -> OpResult {
    self.value_stack.apply_binary(
      |a, b| match ::core::primitive::bool::then_some(b == 0, ()) {
        ::core::option::Option::Some(()) => Err(HintErrorKind::DivideByZero),
        ::core::option::Option::None => Ok({
          let Ok(math_result_1) = math::mul_div_no_round(math::ProductRatio {
            value: a,
            multiplier: 64,
            divisor: b,
          });
          math_result_1
        }),
      },
    )
  }

  pub(super) fn op_mul(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| {
      Ok({
        let Ok(math_result_2) = math::mul_div(math::ProductRatio {
          value: a,
          multiplier: b,
          divisor: 64,
        });
        math_result_2
      })
    })
  }

  pub(super) fn op_abs(&mut self) -> OpResult {
    self.value_stack.apply_unary(|n| Ok(n.wrapping_abs()))
  }

  pub(super) fn op_neg(&mut self) -> OpResult {
    self.value_stack.apply_unary(|n1| Ok(n1.wrapping_neg()))
  }

  pub(super) fn op_floor(&mut self) -> OpResult {
    self.value_stack.apply_unary(|n1| {
      Ok({
        let Ok(math_result_3) = math::floor(n1);
        math_result_3
      })
    })
  }

  pub(super) fn op_ceiling(&mut self) -> OpResult {
    self.value_stack.apply_unary(|n1| {
      Ok({
        let Ok(math_result_4) = math::ceil(n1);
        math_result_4
      })
    })
  }

  pub(super) fn op_max(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(a.max(b)))
  }

  pub(super) fn op_min(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(a.min(b)))
  }
}

#[cfg(test)]
mod tests {
  use super::{super::MockEngine, HintErrorKind, math};

  #[test]
  fn simple_binops() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    for a in -10..=10 {
      for b in -10..=10 {
        let input = &[a, b];
        engine.test_exec(input, i32::wrapping_add(a, b), |engine_2| {
          engine_2.op_add().required()?;
          Ok(())
        })?;
        engine.test_exec(input, i32::wrapping_sub(a, b), |engine_3| {
          engine_3.op_sub().required()?;
          Ok(())
        })?;
        engine.test_exec(input, a.max(b), |engine_4| {
          engine_4.op_max().required()?;
          Ok(())
        })?;
        engine.test_exec(input, a.min(b), |engine_5| {
          engine_5.op_min().required()?;
          Ok(())
        })?;
      }
    }

    Ok(())
  }

  #[test]
  fn simple_unops() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    for a in -10..=10 {
      let input = &[a];
      engine.test_exec(input, (a).wrapping_neg(), |engine_2| {
        engine_2.op_neg().required()?;
        Ok(())
      })?;
      engine.test_exec(input, a.abs(), |engine_3| {
        engine_3.op_abs().required()?;
        Ok(())
      })?;
    }

    Ok(())
  }

  #[test]
  fn f26dot6_binops() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    for a in -10..=10 {
      for b in -10..=10 {
        let a_2 = i32::wrapping_add(i32::wrapping_mul(a, 64), 30);
        let b_2 = i32::wrapping_sub(i32::wrapping_mul(b, 64), 30);
        let input = &[a_2, b_2];
        engine.test_exec(
          input,
          {
            let Ok(math_result_5) = math::mul_div(math::ProductRatio {
              value: a_2,
              multiplier: b_2,
              divisor: 64,
            });
            math_result_5
          },
          |interpreter| {
            interpreter.op_mul().required()?;
            Ok(())
          },
        )?;

        match ::core::primitive::bool::then_some(b_2 != 0, ()) {
          ::core::option::Option::Some(()) => {
            engine.test_exec(
              input,
              {
                let Ok(math_result_6) = math::mul_div_no_round(math::ProductRatio {
                  value: a_2,
                  multiplier: 64,
                  divisor: b_2,
                });
                math_result_6
              },
              |engine_2| {
                engine_2.op_div().required()?;
                Ok(())
              },
            )?;
          }
          ::core::option::Option::None => {
            engine.value_stack.push(a_2).required()?;
            engine.value_stack.push(b_2).required()?;
            assert!(matches!(engine.op_div(), Err(HintErrorKind::DivideByZero)));
          }
        }
      }
    }

    Ok(())
  }

  #[test]
  fn f26dot6_unops() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    for a in -10..=10 {
      for b in -10..=10 {
        let a_2 = i32::wrapping_add(i32::wrapping_mul(a, 64), b);
        let input = &[a_2];
        engine.test_exec(
          input,
          {
            let Ok(math_result_7) = math::floor(a_2);
            math_result_7
          },
          |engine_2| {
            engine_2.op_floor().required()?;
            Ok(())
          },
        )?;
        engine.test_exec(
          input,
          {
            let Ok(math_result_8) = math::ceil(a_2);
            math_result_8
          },
          |engine_3| {
            engine_3.op_ceiling().required()?;
            Ok(())
          },
        )?;
      }
    }

    Ok(())
  }
}
