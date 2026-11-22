//! Compensating for the engine characteristics (rounding).
//!
//! Implements 4 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#compensating-for-the-engine-characteristics>

use super::{Engine, OpResult};

impl Engine<'_> {
  pub(super) fn op_round(&mut self) -> OpResult {
    let n1 = self.value_stack.pop_f26dot6()?;
    let n2 = self.graphics.round(n1)?;
    self.value_stack.push({
      let Ok(value) = n2.to_bits();
      value
    })
  }
}

#[cfg(test)]
mod tests {
  use super::super::{super::round::RoundMode, MockEngine};

  #[test]
  fn round() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    use RoundMode::*;

    let cases = [
      (Grid, &[(0, 0), (32, 64), (-32, -64), (64, 64), (50, 64)]),
      (
        HalfGrid,
        &[(0, 32), (32, 32), (-32, -32), (64, 96), (50, 32)],
      ),
      (
        DoubleGrid,
        &[(0, 0), (32, 32), (-32, -32), (64, 64), (50, 64)],
      ),
      (DownToGrid, &[(0, 0), (32, 0), (-32, 0), (64, 64), (50, 0)]),
      (
        UpToGrid,
        &[(0, 0), (32, 64), (-32, -64), (64, 64), (50, 64)],
      ),
      (Off, &[(0, 0), (32, 32), (-32, -32), (64, 64), (50, 50)]),
    ];
    for (mode, values) in cases {
      match mode {
        Grid => engine.op_rtg().required()?,
        HalfGrid => engine.op_rthg().required()?,
        DoubleGrid => engine.op_rtdg().required()?,
        DownToGrid => engine.op_rdtg().required()?,
        UpToGrid => engine.op_rutg().required()?,
        Off => engine.op_roff().required()?,
        Super | Super45 => {
          return Err(crate::TestFailure::Fault(alloc::boxed::Box::new(mode)));
        }
      }

      for (input, expected) in values {
        engine.value_stack.push(*input).required()?;
        engine.op_round().required()?;
        let result = engine.value_stack.pop().required()?;
        assert_eq!(*expected, result);
      }
    }

    Ok(())
  }
}
