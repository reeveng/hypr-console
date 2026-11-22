//! Managing the flow of control.
//!
//! Implements 6 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#managing-the-flow-of-control>

use read_fonts::tables::glyf::bytecode::Opcode;

use super::{Engine, HintErrorKind, OpResult};

impl Engine<'_> {
  pub(super) fn op_if(&mut self) -> OpResult {
    match ::core::primitive::bool::then_some({
      let answer = self.value_stack.pop()?;
      answer
    } == 0, ())
    {
      ::core::option::Option::Some(()) => {
        let mut nest_depth = 1;
        let mut out = false;

        'if_body: for instruction in self.program.decoder.by_ref() {
          let instruction_2 = instruction?;
          let opcode = instruction_2.opcode;

          match opcode {
            Opcode::IF => nest_depth = i32::wrapping_add(nest_depth, 1),
            Opcode::ELSE => out = nest_depth == 1,
            Opcode::EIF => {
              nest_depth = i32::wrapping_sub(nest_depth, 1);
              out = nest_depth == 0;
            }
            Opcode::SVTCA0
            | Opcode::SVTCA1
            | Opcode::SPVTCA0
            | Opcode::SPVTCA1
            | Opcode::SFVTCA0
            | Opcode::SFVTCA1
            | Opcode::SPVTL0
            | Opcode::SPVTL1
            | Opcode::SFVTL0
            | Opcode::SFVTL1
            | Opcode::SPVFS
            | Opcode::SFVFS
            | Opcode::GPV
            | Opcode::GFV
            | Opcode::SFVTPV
            | Opcode::ISECT
            | Opcode::SRP0
            | Opcode::SRP1
            | Opcode::SRP2
            | Opcode::SZP0
            | Opcode::SZP1
            | Opcode::SZP2
            | Opcode::SZPS
            | Opcode::SLOOP
            | Opcode::RTG
            | Opcode::RTHG
            | Opcode::SMD
            | Opcode::JMPR
            | Opcode::SCVTCI
            | Opcode::SSWCI
            | Opcode::SSW
            | Opcode::DUP
            | Opcode::POP
            | Opcode::CLEAR
            | Opcode::SWAP
            | Opcode::DEPTH
            | Opcode::CINDEX
            | Opcode::MINDEX
            | Opcode::ALIGNPTS
            | Opcode::INS28
            | Opcode::UTP
            | Opcode::LOOPCALL
            | Opcode::CALL
            | Opcode::FDEF
            | Opcode::ENDF
            | Opcode::MDAP0
            | Opcode::MDAP1
            | Opcode::IUP0
            | Opcode::IUP1
            | Opcode::SHP0
            | Opcode::SHP1
            | Opcode::SHC0
            | Opcode::SHC1
            | Opcode::SHZ0
            | Opcode::SHZ1
            | Opcode::SHPIX
            | Opcode::IP
            | Opcode::MSIRP0
            | Opcode::MSIRP1
            | Opcode::ALIGNRP
            | Opcode::RTDG
            | Opcode::MIAP0
            | Opcode::MIAP1
            | Opcode::NPUSHB
            | Opcode::NPUSHW
            | Opcode::WS
            | Opcode::RS
            | Opcode::WCVTP
            | Opcode::RCVT
            | Opcode::GC0
            | Opcode::GC1
            | Opcode::SCFS
            | Opcode::MD0
            | Opcode::MD1
            | Opcode::MPPEM
            | Opcode::MPS
            | Opcode::FLIPON
            | Opcode::FLIPOFF
            | Opcode::DEBUG
            | Opcode::LT
            | Opcode::LTEQ
            | Opcode::GT
            | Opcode::GTEQ
            | Opcode::EQ
            | Opcode::NEQ
            | Opcode::ODD
            | Opcode::EVEN
            | Opcode::AND
            | Opcode::OR
            | Opcode::NOT
            | Opcode::DELTAP1
            | Opcode::SDB
            | Opcode::SDS
            | Opcode::ADD
            | Opcode::SUB
            | Opcode::DIV
            | Opcode::MUL
            | Opcode::ABS
            | Opcode::NEG
            | Opcode::FLOOR
            | Opcode::CEILING
            | Opcode::ROUND00
            | Opcode::ROUND01
            | Opcode::ROUND10
            | Opcode::ROUND11
            | Opcode::NROUND00
            | Opcode::NROUND01
            | Opcode::NROUND10
            | Opcode::NROUND11
            | Opcode::WCVTF
            | Opcode::DELTAP2
            | Opcode::DELTAP3
            | Opcode::DELTAC1
            | Opcode::DELTAC2
            | Opcode::DELTAC3
            | Opcode::SROUND
            | Opcode::S45ROUND
            | Opcode::JROT
            | Opcode::JROF
            | Opcode::ROFF
            | Opcode::INS7B
            | Opcode::RUTG
            | Opcode::RDTG
            | Opcode::SANGW
            | Opcode::AA
            | Opcode::FLIPPT
            | Opcode::FLIPRGON
            | Opcode::FLIPRGOFF
            | Opcode::INS83
            | Opcode::INS84
            | Opcode::SCANCTRL
            | Opcode::SDPVTL0
            | Opcode::SDPVTL1
            | Opcode::GETINFO
            | Opcode::IDEF
            | Opcode::ROLL
            | Opcode::MAX
            | Opcode::MIN
            | Opcode::SCANTYPE
            | Opcode::INSTCTRL
            | Opcode::INS8F
            | Opcode::INS90
            | Opcode::GETVARIATION
            | Opcode::GETDATA
            | Opcode::INS93
            | Opcode::INS94
            | Opcode::INS95
            | Opcode::INS96
            | Opcode::INS97
            | Opcode::INS98
            | Opcode::INS99
            | Opcode::INS9A
            | Opcode::INS9B
            | Opcode::INS9C
            | Opcode::INS9D
            | Opcode::INS9E
            | Opcode::INS9F
            | Opcode::INSA0
            | Opcode::INSA1
            | Opcode::INSA2
            | Opcode::INSA3
            | Opcode::INSA4
            | Opcode::INSA5
            | Opcode::INSA6
            | Opcode::INSA7
            | Opcode::INSA8
            | Opcode::INSA9
            | Opcode::INSAA
            | Opcode::INSAB
            | Opcode::INSAC
            | Opcode::INSAD
            | Opcode::INSAE
            | Opcode::INSAF
            | Opcode::PUSHB000
            | Opcode::PUSHB001
            | Opcode::PUSHB010
            | Opcode::PUSHB011
            | Opcode::PUSHB100
            | Opcode::PUSHB101
            | Opcode::PUSHB110
            | Opcode::PUSHB111
            | Opcode::PUSHW000
            | Opcode::PUSHW001
            | Opcode::PUSHW010
            | Opcode::PUSHW011
            | Opcode::PUSHW100
            | Opcode::PUSHW101
            | Opcode::PUSHW110
            | Opcode::PUSHW111
            | Opcode::MDRP00000
            | Opcode::MDRP00001
            | Opcode::MDRP00010
            | Opcode::MDRP00011
            | Opcode::MDRP00100
            | Opcode::MDRP00101
            | Opcode::MDRP00110
            | Opcode::MDRP00111
            | Opcode::MDRP01000
            | Opcode::MDRP01001
            | Opcode::MDRP01010
            | Opcode::MDRP01011
            | Opcode::MDRP01100
            | Opcode::MDRP01101
            | Opcode::MDRP01110
            | Opcode::MDRP01111
            | Opcode::MDRP10000
            | Opcode::MDRP10001
            | Opcode::MDRP10010
            | Opcode::MDRP10011
            | Opcode::MDRP10100
            | Opcode::MDRP10101
            | Opcode::MDRP10110
            | Opcode::MDRP10111
            | Opcode::MDRP11000
            | Opcode::MDRP11001
            | Opcode::MDRP11010
            | Opcode::MDRP11011
            | Opcode::MDRP11100
            | Opcode::MDRP11101
            | Opcode::MDRP11110
            | Opcode::MDRP11111
            | Opcode::MIRP00000
            | Opcode::MIRP00001
            | Opcode::MIRP00010
            | Opcode::MIRP00011
            | Opcode::MIRP00100
            | Opcode::MIRP00101
            | Opcode::MIRP00110
            | Opcode::MIRP00111
            | Opcode::MIRP01000
            | Opcode::MIRP01001
            | Opcode::MIRP01010
            | Opcode::MIRP01011
            | Opcode::MIRP01100
            | Opcode::MIRP01101
            | Opcode::MIRP01110
            | Opcode::MIRP01111
            | Opcode::MIRP10000
            | Opcode::MIRP10001
            | Opcode::MIRP10010
            | Opcode::MIRP10011
            | Opcode::MIRP10100
            | Opcode::MIRP10101
            | Opcode::MIRP10110
            | Opcode::MIRP10111
            | Opcode::MIRP11000
            | Opcode::MIRP11001
            | Opcode::MIRP11010
            | Opcode::MIRP11011
            | Opcode::MIRP11100
            | Opcode::MIRP11101
            | Opcode::MIRP11110
            | Opcode::MIRP11111 => {}
          }

          match ::core::primitive::bool::then_some(out, ()) {
            ::core::option::Option::Some(()) => break 'if_body,
            ::core::option::Option::None => {}
          }
        }

        match ::core::primitive::bool::then_some(out, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => return Err(HintErrorKind::UnexpectedEndOfBytecode),
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub(super) fn op_else(&mut self) -> OpResult {
    let mut nest_depth = 1;

    'else_body: for instruction in self.program.decoder.by_ref() {
      let instruction_2 = instruction?;
      let opcode = instruction_2.opcode;

      match opcode {
        Opcode::IF => nest_depth = i32::wrapping_add(nest_depth, 1),
        Opcode::EIF => nest_depth = i32::wrapping_sub(nest_depth, 1),
        Opcode::SVTCA0
        | Opcode::SVTCA1
        | Opcode::SPVTCA0
        | Opcode::SPVTCA1
        | Opcode::SFVTCA0
        | Opcode::SFVTCA1
        | Opcode::SPVTL0
        | Opcode::SPVTL1
        | Opcode::SFVTL0
        | Opcode::SFVTL1
        | Opcode::SPVFS
        | Opcode::SFVFS
        | Opcode::GPV
        | Opcode::GFV
        | Opcode::SFVTPV
        | Opcode::ISECT
        | Opcode::SRP0
        | Opcode::SRP1
        | Opcode::SRP2
        | Opcode::SZP0
        | Opcode::SZP1
        | Opcode::SZP2
        | Opcode::SZPS
        | Opcode::SLOOP
        | Opcode::RTG
        | Opcode::RTHG
        | Opcode::SMD
        | Opcode::ELSE
        | Opcode::JMPR
        | Opcode::SCVTCI
        | Opcode::SSWCI
        | Opcode::SSW
        | Opcode::DUP
        | Opcode::POP
        | Opcode::CLEAR
        | Opcode::SWAP
        | Opcode::DEPTH
        | Opcode::CINDEX
        | Opcode::MINDEX
        | Opcode::ALIGNPTS
        | Opcode::INS28
        | Opcode::UTP
        | Opcode::LOOPCALL
        | Opcode::CALL
        | Opcode::FDEF
        | Opcode::ENDF
        | Opcode::MDAP0
        | Opcode::MDAP1
        | Opcode::IUP0
        | Opcode::IUP1
        | Opcode::SHP0
        | Opcode::SHP1
        | Opcode::SHC0
        | Opcode::SHC1
        | Opcode::SHZ0
        | Opcode::SHZ1
        | Opcode::SHPIX
        | Opcode::IP
        | Opcode::MSIRP0
        | Opcode::MSIRP1
        | Opcode::ALIGNRP
        | Opcode::RTDG
        | Opcode::MIAP0
        | Opcode::MIAP1
        | Opcode::NPUSHB
        | Opcode::NPUSHW
        | Opcode::WS
        | Opcode::RS
        | Opcode::WCVTP
        | Opcode::RCVT
        | Opcode::GC0
        | Opcode::GC1
        | Opcode::SCFS
        | Opcode::MD0
        | Opcode::MD1
        | Opcode::MPPEM
        | Opcode::MPS
        | Opcode::FLIPON
        | Opcode::FLIPOFF
        | Opcode::DEBUG
        | Opcode::LT
        | Opcode::LTEQ
        | Opcode::GT
        | Opcode::GTEQ
        | Opcode::EQ
        | Opcode::NEQ
        | Opcode::ODD
        | Opcode::EVEN
        | Opcode::AND
        | Opcode::OR
        | Opcode::NOT
        | Opcode::DELTAP1
        | Opcode::SDB
        | Opcode::SDS
        | Opcode::ADD
        | Opcode::SUB
        | Opcode::DIV
        | Opcode::MUL
        | Opcode::ABS
        | Opcode::NEG
        | Opcode::FLOOR
        | Opcode::CEILING
        | Opcode::ROUND00
        | Opcode::ROUND01
        | Opcode::ROUND10
        | Opcode::ROUND11
        | Opcode::NROUND00
        | Opcode::NROUND01
        | Opcode::NROUND10
        | Opcode::NROUND11
        | Opcode::WCVTF
        | Opcode::DELTAP2
        | Opcode::DELTAP3
        | Opcode::DELTAC1
        | Opcode::DELTAC2
        | Opcode::DELTAC3
        | Opcode::SROUND
        | Opcode::S45ROUND
        | Opcode::JROT
        | Opcode::JROF
        | Opcode::ROFF
        | Opcode::INS7B
        | Opcode::RUTG
        | Opcode::RDTG
        | Opcode::SANGW
        | Opcode::AA
        | Opcode::FLIPPT
        | Opcode::FLIPRGON
        | Opcode::FLIPRGOFF
        | Opcode::INS83
        | Opcode::INS84
        | Opcode::SCANCTRL
        | Opcode::SDPVTL0
        | Opcode::SDPVTL1
        | Opcode::GETINFO
        | Opcode::IDEF
        | Opcode::ROLL
        | Opcode::MAX
        | Opcode::MIN
        | Opcode::SCANTYPE
        | Opcode::INSTCTRL
        | Opcode::INS8F
        | Opcode::INS90
        | Opcode::GETVARIATION
        | Opcode::GETDATA
        | Opcode::INS93
        | Opcode::INS94
        | Opcode::INS95
        | Opcode::INS96
        | Opcode::INS97
        | Opcode::INS98
        | Opcode::INS99
        | Opcode::INS9A
        | Opcode::INS9B
        | Opcode::INS9C
        | Opcode::INS9D
        | Opcode::INS9E
        | Opcode::INS9F
        | Opcode::INSA0
        | Opcode::INSA1
        | Opcode::INSA2
        | Opcode::INSA3
        | Opcode::INSA4
        | Opcode::INSA5
        | Opcode::INSA6
        | Opcode::INSA7
        | Opcode::INSA8
        | Opcode::INSA9
        | Opcode::INSAA
        | Opcode::INSAB
        | Opcode::INSAC
        | Opcode::INSAD
        | Opcode::INSAE
        | Opcode::INSAF
        | Opcode::PUSHB000
        | Opcode::PUSHB001
        | Opcode::PUSHB010
        | Opcode::PUSHB011
        | Opcode::PUSHB100
        | Opcode::PUSHB101
        | Opcode::PUSHB110
        | Opcode::PUSHB111
        | Opcode::PUSHW000
        | Opcode::PUSHW001
        | Opcode::PUSHW010
        | Opcode::PUSHW011
        | Opcode::PUSHW100
        | Opcode::PUSHW101
        | Opcode::PUSHW110
        | Opcode::PUSHW111
        | Opcode::MDRP00000
        | Opcode::MDRP00001
        | Opcode::MDRP00010
        | Opcode::MDRP00011
        | Opcode::MDRP00100
        | Opcode::MDRP00101
        | Opcode::MDRP00110
        | Opcode::MDRP00111
        | Opcode::MDRP01000
        | Opcode::MDRP01001
        | Opcode::MDRP01010
        | Opcode::MDRP01011
        | Opcode::MDRP01100
        | Opcode::MDRP01101
        | Opcode::MDRP01110
        | Opcode::MDRP01111
        | Opcode::MDRP10000
        | Opcode::MDRP10001
        | Opcode::MDRP10010
        | Opcode::MDRP10011
        | Opcode::MDRP10100
        | Opcode::MDRP10101
        | Opcode::MDRP10110
        | Opcode::MDRP10111
        | Opcode::MDRP11000
        | Opcode::MDRP11001
        | Opcode::MDRP11010
        | Opcode::MDRP11011
        | Opcode::MDRP11100
        | Opcode::MDRP11101
        | Opcode::MDRP11110
        | Opcode::MDRP11111
        | Opcode::MIRP00000
        | Opcode::MIRP00001
        | Opcode::MIRP00010
        | Opcode::MIRP00011
        | Opcode::MIRP00100
        | Opcode::MIRP00101
        | Opcode::MIRP00110
        | Opcode::MIRP00111
        | Opcode::MIRP01000
        | Opcode::MIRP01001
        | Opcode::MIRP01010
        | Opcode::MIRP01011
        | Opcode::MIRP01100
        | Opcode::MIRP01101
        | Opcode::MIRP01110
        | Opcode::MIRP01111
        | Opcode::MIRP10000
        | Opcode::MIRP10001
        | Opcode::MIRP10010
        | Opcode::MIRP10011
        | Opcode::MIRP10100
        | Opcode::MIRP10101
        | Opcode::MIRP10110
        | Opcode::MIRP10111
        | Opcode::MIRP11000
        | Opcode::MIRP11001
        | Opcode::MIRP11010
        | Opcode::MIRP11011
        | Opcode::MIRP11100
        | Opcode::MIRP11101
        | Opcode::MIRP11110
        | Opcode::MIRP11111 => {}
      }

      match ::core::primitive::bool::then_some(nest_depth == 0, ()) {
        ::core::option::Option::Some(()) => break 'else_body,
        ::core::option::Option::None => {}
      }
    }

    match ::core::primitive::bool::then_some(nest_depth == 0, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(HintErrorKind::UnexpectedEndOfBytecode),
    }

    Ok(())
  }

  pub(super) fn op_eif(&mut self) -> OpResult {
    Ok(())
  }

  pub(super) fn op_jrot(&mut self) -> OpResult {
    let e = self.value_stack.pop()?;
    self.do_jump(match ::core::primitive::bool::then_some(e != 0, ()) {
      ::core::option::Option::Some(()) => Jump::Taken,
      ::core::option::Option::None => Jump::Skipped,
    })
  }

  pub(super) fn op_jmpr(&mut self) -> OpResult {
    self.do_jump(match ::core::primitive::bool::then_some(true, ()) {
      ::core::option::Option::Some(()) => Jump::Taken,
      ::core::option::Option::None => Jump::Skipped,
    })
  }

  pub(super) fn op_jrof(&mut self) -> OpResult {
    let e = self.value_stack.pop()?;
    self.do_jump(match ::core::primitive::bool::then_some(e == 0, ()) {
      ::core::option::Option::Some(()) => Jump::Taken,
      ::core::option::Option::None => Jump::Skipped,
    })
  }

  fn do_jump(&mut self, jump: Jump) -> OpResult {
    let offset = self.value_stack.pop()?;
    let jump_offset = offset.wrapping_sub(1);

    match jump {
      Jump::Skipped => return Ok(()),
      Jump::Taken => {}
    }

    match jump_offset {
      -1 => return Err(HintErrorKind::InvalidJump),
      ..=-2 => self.loop_budget.doing_backward_jump()?,
      0.. => {}
    }

    self.program.decoder.pc = self
      .program
      .decoder
      .pc
      .wrapping_add_signed(i64::from(jump_offset));

    Ok(())
  }
}

#[derive(Clone, Copy)]
enum Jump {
  Taken,
  Skipped,
}

#[cfg(test)]
mod tests {
  use super::{super::MockEngine, HintErrorKind, Opcode};

  #[test]
  fn if_else() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    use Opcode::*;
    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    #[rustfmt::skip]
        let ops = [
            IF,
                ADD,
                SUB,
                IF,
                    MUL,
                    DIV,
                ELSE,
                    IUP0,
                    IUP1,
                EIF,
            ELSE,
                RUTG,
                IF,
                EIF,
            EIF
        ];
    let bytecode = ops.map(|op| {
      let Ok(byte) = op.to_byte();
      byte
    });
    engine.program.decoder.bytecode = bytecode.as_slice();

    {
      engine.program.decoder.pc = 1;
      engine.value_stack.push(1).required()?;
      engine.op_if().required()?;
      assert_eq!(engine.program.decoder.pc, 1);

      engine.program.decoder.pc = 1;
      engine.value_stack.push(0).required()?;
      engine.op_if().required()?;
      assert_eq!(engine.program.decoder.pc, 11);
    }

    {
      engine.program.decoder.pc = 4;
      engine.value_stack.push(1).required()?;
      engine.op_if().required()?;
      assert_eq!(engine.program.decoder.pc, 4);

      engine.program.decoder.pc = 4;
      engine.value_stack.push(0).required()?;
      engine.op_if().required()?;
      assert_eq!(engine.program.decoder.pc, 7);
    }

    {
      engine.program.decoder.pc = 10;
      engine.op_else().required()?;
      assert_eq!(engine.program.decoder.pc, 15);
      engine.program.decoder.pc = 8;
      engine.op_else().required()?;
      assert_eq!(engine.program.decoder.pc, 10);
    }

    Ok(())
  }

  #[test]
  fn jumps() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    {
      engine.program.decoder.pc = 1000;
      engine.value_stack.push(100).required()?;
      engine.op_jmpr().required()?;
      assert_eq!(engine.program.decoder.pc, 1099);
    }

    {
      engine.program.decoder.pc = 1000;

      engine.value_stack.push(100).required()?;
      engine.value_stack.push(0).required()?;
      engine.op_jrot().required()?;
      assert_eq!(engine.program.decoder.pc, 1000);

      engine.value_stack.push(100).required()?;
      engine.value_stack.push(1).required()?;
      engine.op_jrot().required()?;
      assert_eq!(engine.program.decoder.pc, 1099);
    }

    {
      engine.program.decoder.pc = 1000;

      engine.value_stack.push(-100).required()?;
      engine.value_stack.push(1).required()?;
      engine.op_jrof().required()?;
      assert_eq!(engine.program.decoder.pc, 1000);

      engine.value_stack.push(-100).required()?;
      engine.value_stack.push(0).required()?;
      engine.op_jrof().required()?;
      assert_eq!(engine.program.decoder.pc, 899);
    }

    {
      engine.loop_budget.limit = 40;

      for i in 0..45 {
        engine.value_stack.push(-5).required()?;
        let result = engine.op_jmpr();

        match ::core::primitive::bool::then_some(i < 39, ()) {
          ::core::option::Option::Some(()) => {
            result.required()?;
          }
          ::core::option::Option::None => {
            assert!(matches!(
              result,
              Err(HintErrorKind::ExceededExecutionBudget)
            ));
          }
        }
      }
    }

    Ok(())
  }
}
