//! TrueType program management.

use super::{
  call_stack::{CallRecord, CallStack},
  definition::Definition,
  error::HintErrorKind,
};
use console_core_never::Never;
use raw::tables::glyf::bytecode::Decoder;

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "Program exposes stable public numeric discriminants and byte representation"
  )
)]
#[repr(u8)]
pub enum Program {
  #[default]
  Font = 0,
  ControlValue = 1,
  Glyph = 2,
}

#[derive(Clone, Copy)]
pub struct ProgramBytes<'a> {
  pub font: &'a [u8],
  pub control_value: &'a [u8],
  pub glyph: &'a [u8],
}

impl<'a> ProgramBytes<'a> {
  fn get(&self, program: Program) -> Result<&'a [u8], Never> {
    Ok(match program {
      Program::Font => self.font,
      Program::ControlValue => self.control_value,
      Program::Glyph => self.glyph,
    })
  }

  #[cfg(test)]
  fn replace_font(&mut self, bytes: &'a [u8]) -> Result<(), Never> {
    self.font = bytes;
    Ok(())
  }
}

pub struct ProgramState<'a> {
  pub bytecode: ProgramBytes<'a>,
  pub initial: Program,
  pub current: Program,
  pub decoder: Decoder<'a>,
  pub call_stack: CallStack,
}

impl<'a> ProgramState<'a> {
  pub fn new(bytecode: ProgramBytes<'a>, initial: Program) -> Result<Self, Never> {
    let Ok(bytes) = bytecode.get(initial);
    Ok(Self {
      bytecode,
      initial,
      current: initial,
      decoder: Decoder::new(bytes, 0),
      call_stack: CallStack::default(),
    })
  }

  pub fn reset(&mut self, program: Program) -> Result<(), Never> {
    self.initial = program;
    self.current = program;
    let Ok(bytes) = self.bytecode.get(program);
    self.decoder = Decoder::new(bytes, 0);
    let Ok(()) = self.call_stack.clear();
    Ok(())
  }

  pub fn enter(&mut self, definition: Definition, count: u32) -> Result<(), HintErrorKind> {
    let Ok(program) = definition.program();
    let Ok(range) = definition.code_range();
    let pc = u64::from(range.start);
    let Ok(bytes) = self.bytecode.get(program);
    self.call_stack.push(CallRecord {
      caller_program: self.current,
      return_pc: self.decoder.pc,
      current_count: count,
      definition,
    })?;
    self.current = program;
    self.decoder = Decoder::new(bytes, pc);
    Ok(())
  }

  pub fn leave(&mut self) -> Result<(), HintErrorKind> {
    let mut record = self.call_stack.pop()?;

    match ::core::primitive::bool::then_some(record.current_count > 1, ()) {
      ::core::option::Option::Some(()) => {
        record.current_count = record.current_count.saturating_sub(1);
        let Ok(range) = record.definition.code_range();
        let pc = u64::from(range.start);
        self.decoder.pc = pc;
        self.call_stack.push(record)?;
      }
      ::core::option::Option::None => {
        self.current = record.caller_program;
        let Ok(bytes) = self.bytecode.get(record.caller_program);
        self.decoder.bytecode = bytes;
        self.decoder.pc = record.return_pc;
      }
    }

    Ok(())
  }

  #[cfg(test)]
  pub fn set_font_code(&mut self, code: &'a [u8]) -> Result<(), Never> {
    let Ok(()) = self.bytecode.replace_font(code);
    self.decoder.bytecode = code;
    self.current = Program::Font;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accounting() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let font_code: &[u8] = &[0];
    let cv_code: &[u8] = &[1];
    let glyph_code: &[u8] = &[2];
    let Ok(mut state) = ProgramState::new(
      ProgramBytes {
        font: font_code,
        control_value: cv_code,
        glyph: glyph_code,
      },
      Program::Glyph,
    );

    assert_eq!(state.active_state(), Ok((Program::Glyph, glyph_code, 0)));
    let Ok(font_def) = Definition::new(Program::Font, 10..20, 0);
    let Ok(cv_def) = Definition::new(Program::ControlValue, 33..111, 1);

    state.enter(cv_def, 1).required()?;
    assert_eq!(
      state.active_state(),
      Ok((Program::ControlValue, cv_code, 33))
    );

    state.decoder.pc = state.decoder.pc.saturating_add(20);

    state.enter(font_def, 1).required()?;
    assert_eq!(state.active_state(), Ok((Program::Font, font_code, 10)));

    state.leave().required()?;
    assert_eq!(
      state.active_state(),
      Ok((Program::ControlValue, cv_code, 53))
    );

    state.leave().required()?;
    assert_eq!(state.active_state(), Ok((Program::Glyph, glyph_code, 0)));

    Ok(())
  }

  #[test]
  fn loop_call() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let font_code: &[u8] = &[0];
    let cv_code: &[u8] = &[1];
    let glyph_code: &[u8] = &[2];
    let Ok(mut state) = ProgramState::new(
      ProgramBytes {
        font: font_code,
        control_value: cv_code,
        glyph: glyph_code,
      },
      Program::Glyph,
    );
    let Ok(font_def) = Definition::new(Program::Font, 10..20, 0);

    state.enter(font_def, 3).required()?;

    for _ in 0..3 {
      assert_eq!(state.active_state(), Ok((Program::Font, font_code, 10)));

      state.decoder.pc = state.decoder.pc.saturating_add(22);
      state.leave().required()?;
    }

    assert_eq!(state.active_state(), Ok((Program::Glyph, glyph_code, 0)));

    Ok(())
  }

  impl<'a> ProgramState<'a> {
    fn active_state(&self) -> Result<(Program, &'a [u8], u64), Never> {
      Ok((
        self.current,
        self.decoder.bytecode,
        ({
          let Ok(value) = console_core_number_conversion::index(self.decoder.pc);

          {
            let Ok(value_2) = console_core_number_conversion::fitted::<_, u64>(value);
            value_2
          }
        }),
      ))
    }
  }
}
