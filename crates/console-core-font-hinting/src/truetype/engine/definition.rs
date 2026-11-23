//! Defining and using functions and instructions.
//!
//! Implements 5 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#defining-and-using-functions-and-instructions>

use read_fonts::tables::glyf::bytecode::Opcode;

use super::{
  super::{definition::Definition, program::Program},
  Engine, HintErrorKind, OpResult,
};

const MAX_DEFINITION_SIZE: u64 = 65_535;

impl Engine<'_> {
  pub(super) fn op_fdef(&mut self) -> OpResult {
    let f = self.value_stack.pop()?;
    self.do_def(DefKind::Function, f)
  }

  pub(super) fn op_endf(&mut self) -> OpResult {
    self.program.leave()
  }

  pub(super) fn op_call(&mut self) -> OpResult {
    let f = self.value_stack.pop()?;
    self.do_call(DefKind::Function, 1, f)
  }

  pub(super) fn op_loopcall(&mut self) -> OpResult {
    let f = self.value_stack.pop()?;
    let count = self.value_stack.pop()?;

    match ::core::primitive::bool::then_some(count > 0, ()) {
      ::core::option::Option::Some(()) => {
        let loop_count =
          u32::try_from(count).map_err(|_negative| HintErrorKind::NegativeLoopCounter)?;
        self.loop_budget.doing_loop_call(loop_count)?;
        self.do_call(DefKind::Function, loop_count, f)
      }
      ::core::option::Option::None => Ok(()),
    }
  }

  pub(super) fn op_idef(&mut self) -> OpResult {
    let opcode = self.value_stack.pop()?;
    self.do_def(DefKind::Instruction, opcode)
  }

  pub(super) fn op_unknown(&mut self, opcode: u8) -> OpResult {
    self.do_call(DefKind::Instruction, 1, i32::from(opcode))
  }

  fn do_def(&mut self, kind: DefKind, key: i32) -> OpResult {
    match ::core::primitive::bool::then_some(self.program.initial == Program::Glyph, ()) {
      ::core::option::Option::Some(()) => {
        return Err(HintErrorKind::DefinitionInGlyphProgram);
      }
      ::core::option::Option::None => {}
    }

    let defs = match kind {
      DefKind::Function => &mut self.definitions.functions,
      DefKind::Instruction => &mut self.definitions.instructions,
    };
    let def = defs.allocate(key)?;
    let start = self.program.decoder.pc;

    let remaining_bytes = {
      let Ok(length) =
        console_core_number_conversion::fitted::<_, u64>(self.program.decoder.bytecode.len());
      length
    }
    .saturating_sub(start);

    for _ in 0..remaining_bytes {
      let decoded = self
        .program
        .decoder
        .decode()
        .ok_or(HintErrorKind::UnexpectedEndOfBytecode)?;
      let ins = decoded?;

      match ::core::primitive::bool::then_some(
        ins.opcode == Opcode::FDEF || ins.opcode == Opcode::IDEF,
        (),
      ) {
        ::core::option::Option::Some(()) => return Err(HintErrorKind::NestedDefinition),
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(ins.opcode == Opcode::ENDF, ()) {
        ::core::option::Option::Some(()) => {
          let end = ins
            .pc
            .checked_add(1)
            .ok_or(HintErrorKind::DefinitionTooLarge)?;
          let range = start..end;

          match ::core::primitive::bool::then_some(
            matches!(self.graphics.is_pedantic, crate::Pedantry::Strict)
              && range.end.saturating_sub(range.start) > MAX_DEFINITION_SIZE,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              *def = Default::default();
              return Err(HintErrorKind::DefinitionTooLarge);
            }
            ::core::option::Option::None => {}
          }

          let start_2 =
            u32::try_from(range.start).map_err(|_too_wide| HintErrorKind::DefinitionTooLarge)?;
          let end_2 =
            u32::try_from(range.end).map_err(|_too_wide| HintErrorKind::DefinitionTooLarge)?;
          let Ok(definition) = Definition::new(self.program.current, start_2..end_2, key);
          *def = definition;
          return Ok(());
        }
        ::core::option::Option::None => {}
      }
    }

    Err(HintErrorKind::UnexpectedEndOfBytecode)
  }

  fn do_call(&mut self, kind: DefKind, count: u32, key: i32) -> OpResult {
    match ::core::primitive::bool::then_some(count == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let def = match kind {
      DefKind::Function => self.definitions.functions.get(key),
      DefKind::Instruction => match self.definitions.instructions.get(key) {
        Err(HintErrorKind::InvalidDefinition(opcode)) => {
          let opcode_byte = u8::try_from(opcode)
            .map_err(|_not_an_opcode| HintErrorKind::InvalidStackValue(opcode))?;
          Err(HintErrorKind::UnhandledOpcode(Opcode::from_byte(
            opcode_byte,
          )))
        }
        Ok(definition) => Ok(definition),
        Err(
          error @ (HintErrorKind::UnexpectedEndOfBytecode
          | HintErrorKind::UnhandledOpcode(_)
          | HintErrorKind::DefinitionInGlyphProgram
          | HintErrorKind::NestedDefinition
          | HintErrorKind::DefinitionTooLarge
          | HintErrorKind::TooManyDefinitions
          | HintErrorKind::ValueStackOverflow
          | HintErrorKind::ValueStackUnderflow
          | HintErrorKind::CallStackOverflow
          | HintErrorKind::CallStackUnderflow
          | HintErrorKind::InvalidStackValue(_)
          | HintErrorKind::InvalidPointIndex(_)
          | HintErrorKind::InvalidPointRange(_, _)
          | HintErrorKind::InvalidContourIndex(_)
          | HintErrorKind::InvalidCvtIndex(_)
          | HintErrorKind::InvalidStorageIndex(_)
          | HintErrorKind::InvalidOutlineBufferLength
          | HintErrorKind::DivideByZero
          | HintErrorKind::InvalidZoneIndex(_)
          | HintErrorKind::NegativeLoopCounter
          | HintErrorKind::InvalidJump
          | HintErrorKind::ExceededExecutionBudget),
        ) => Err(error),
      },
    };
    let definition = def?;
    self.program.enter(*definition, count)
  }
}

enum DefKind {
  Function,
  Instruction,
}

#[cfg(test)]
mod tests {
  use super::{
    super::{
      super::program::{Program, ProgramBytes, ProgramState},
      MockEngine,
    },
    HintErrorKind, MAX_DEFINITION_SIZE, Opcode,
  };

  #[test]
  fn define_function_call_loopcall() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(PUSHB001); migrated_value }), 1, 0,

            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 2,
                ({ let Ok(migrated_value) = op(ADD); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),


            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 0,
                ({ let Ok(migrated_value) = op(CALL); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB001); migrated_value }), 5, 0,
                ({ let Ok(migrated_value) = op(LOOPCALL); migrated_value }),
                ({ let Ok(migrated_value) = op(NEG); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];

    let Ok(()) = engine.program.set_font_code(&font_code);
    engine.run().required()?;

    engine.value_stack.push(10).required()?;
    engine.value_stack.push(1).required()?;
    engine.op_call().required()?;
    engine.run().required()?;
    assert_eq!(engine.value_stack.pop(), Ok(-22));

    Ok(())
  }

  #[test]
  fn override_function() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(PUSHB001); migrated_value }), 0, 0,

            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 2,
                ({ let Ok(migrated_value) = op(ADD); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),

            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 2,
                ({ let Ok(migrated_value) = op(SUB); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];

    let Ok(()) = engine.program.set_font_code(&font_code);
    engine.run().required()?;

    engine.value_stack.push(10).required()?;
    engine.value_stack.push(0).required()?;
    engine.op_call().required()?;
    engine.run().required()?;
    assert_eq!(engine.value_stack.pop(), Ok(8));

    Ok(())
  }

  #[test]
  fn call_different_program() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 0,

            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 2,
                ({ let Ok(migrated_value) = op(ADD); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];
    #[rustfmt::skip]
        let cv_code = [

            ({ let Ok(migrated_value) = op(PUSHB001); migrated_value }), 40, 0,
            ({ let Ok(migrated_value) = op(CALL); migrated_value }),
            ({ let Ok(migrated_value) = op(NEG); migrated_value })
        ];
    let glyph_code = &[];

    let Ok(font_program_state) = ProgramState::new(
      ProgramBytes {
        font: &font_code,
        control_value: &cv_code,
        glyph: glyph_code,
      },
      Program::Font,
    );
    engine.program = font_program_state;
    engine.run().required()?;

    let Ok(control_program_state) = ProgramState::new(
      ProgramBytes {
        font: &font_code,
        control_value: &cv_code,
        glyph: glyph_code,
      },
      Program::ControlValue,
    );
    engine.program = control_program_state;
    engine.run().required()?;

    assert_eq!(engine.value_stack.pop(), Ok(-42));

    Ok(())
  }

  #[test]
  fn loopcall_budget() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    let limit = engine.loop_budget.limit;
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(PUSHB001); migrated_value }), 1, 0,

            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),


            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB001); migrated_value }), ({ let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(limit); migrated_value }), 0,
                ({ let Ok(migrated_value) = op(LOOPCALL); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB001); migrated_value }), 1, 0,
                ({ let Ok(migrated_value) = op(LOOPCALL); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];

    let Ok(()) = engine.program.set_font_code(&font_code);
    engine.run().required()?;

    engine.value_stack.push(10).required()?;
    engine.value_stack.push(1).required()?;
    engine.op_call().required()?;
    let err = engine.run().unwrap_err();
    assert!(matches!(err.kind, HintErrorKind::ExceededExecutionBudget));
    assert_eq!(err.pc, 13);

    Ok(())
  }

  #[test]
  fn define_instruction_and_use() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [

            ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), ({ let Ok(migrated_value) = op(INS93); migrated_value }),
            ({ let Ok(migrated_value) = op(IDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 2,
                ({ let Ok(migrated_value) = op(ADD); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),

            ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 0,
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(INS93); migrated_value }),
                ({ let Ok(migrated_value) = op(NEG); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];

    let Ok(()) = engine.program.set_font_code(&font_code);
    engine.run().required()?;

    engine.value_stack.push(10).required()?;
    engine.value_stack.push(0).required()?;
    engine.op_call().required()?;
    engine.run().required()?;
    assert_eq!(engine.value_stack.pop(), Ok(-12));

    Ok(())
  }

  #[test]
  fn nested_definition() {
    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(PUSHB001); migrated_value }), 1, 0,
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];

    let Ok(()) = engine.program.set_font_code(&font_code);
    let err = engine.run().unwrap_err();
    assert!(matches!(err.kind, HintErrorKind::NestedDefinition));
    assert_eq!(err.pc, 3);
  }

  #[test]
  fn definition_in_glyph_program() {
    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 0,
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];
    let Ok(()) = engine.program.set_font_code(&font_code);
    engine.program.initial = Program::Glyph;
    let err = engine.run().unwrap_err();
    assert!(matches!(err.kind, HintErrorKind::DefinitionInGlyphProgram));
    assert_eq!(err.pc, 2);
  }

  #[test]
  fn undefined_function() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.value_stack.push(111).required()?;
    assert!(matches!(
      engine.op_call(),
      Err(HintErrorKind::InvalidDefinition(111))
    ));

    Ok(())
  }

  #[test]
  fn infinite_recursion() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [

            ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 0,
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }),
                ({ let Ok(migrated_value) = op(PUSHB000); migrated_value }), 0,
                ({ let Ok(migrated_value) = op(CALL); migrated_value }),
            ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];
    let Ok(()) = engine.program.set_font_code(&font_code);
    engine.run().required()?;

    engine.value_stack.push(0).required()?;
    engine.op_call().required()?;
    let err = engine.run().unwrap_err();
    assert!(matches!(err.kind, HintErrorKind::CallStackOverflow));
    assert_eq!(err.pc, 5);

    Ok(())
  }

  #[test]
  fn call_stack_underflow() {
    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(ENDF); migrated_value })
        ];
    let Ok(()) = engine.program.set_font_code(&font_code);
    let err = engine.run().unwrap_err();
    assert!(matches!(err.kind, HintErrorKind::CallStackUnderflow));
    assert_eq!(err.pc, 0);
  }

  #[test]
  fn unhandled_opcode() {
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(Opcode::INS28); migrated_value }),
        ];
    let Ok(()) = engine.program.set_font_code(&font_code);
    let err = engine.run().unwrap_err();
    assert!(matches!(
      err.kind,
      HintErrorKind::UnhandledOpcode(Opcode::INS28)
    ));
    assert_eq!(err.pc, 0);
  }

  #[test]
  fn too_many_definitions() {
    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    #[rustfmt::skip]
        let font_code = [
            ({ let Ok(migrated_value) = op(PUSHB101); migrated_value }), 0, 1, 2, 3, 4, 5,
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }), ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }), ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }), ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }), ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }), ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
            ({ let Ok(migrated_value) = op(FDEF); migrated_value }), ({ let Ok(migrated_value) = op(ENDF); migrated_value }),
        ];
    let Ok(()) = engine.program.set_font_code(&font_code);
    let err = engine.run().unwrap_err();
    assert!(matches!(err.kind, HintErrorKind::TooManyDefinitions));
    assert_eq!(err.pc, 17);
  }

  #[test]
  fn big_definition() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    let mut font_code = vec![];
    font_code.extend_from_slice(&[
      ({
        let Ok(migrated_value) = op(PUSHB000);
        migrated_value
      }),
      0,
      ({
        let Ok(migrated_value) = op(FDEF);
        migrated_value
      }),
    ]);
    font_code.extend(std::iter::repeat_n(
      {
        let Ok(migrated_value) = op(NEG);
        migrated_value
      },
      {
        let Ok(value) =
          console_core_number_conversion::index(MAX_DEFINITION_SIZE.saturating_add(1));
        value
      },
    ));
    font_code.push({
      let Ok(migrated_value) = op(ENDF);
      migrated_value
    });
    let Ok(()) = engine.program.set_font_code(&font_code);
    engine.graphics.is_pedantic = crate::Pedantry::Strict;
    engine.value_stack.push(1).required()?;
    let err = engine.run().unwrap_err();
    assert!(matches!(err.kind, HintErrorKind::DefinitionTooLarge));
    assert_eq!(err.pc, 2);

    Ok(())
  }

  fn op(opcode: Opcode) -> core::result::Result<u8, console_core_never::Never> {
    opcode.to_byte()
  }
}
