//! Instruction decoding and dispatch.

use console_core_never::Never;
use read_fonts::tables::glyf::bytecode::Opcode;

use super::{super::program::Program, Engine, HintError, HintErrorKind, Instruction};

const MAX_RUN_INSTRUCTIONS: u32 = 1_000_000;

impl<'a> Engine<'a> {
  pub fn run_program(
    &mut self,
    program: Program,
    validation: crate::Validation,
  ) -> Result<(), HintError> {
    let Ok(()) = self.reset(program, validation);
    self.run()
  }

  pub fn reset(&mut self, program: Program, validation: crate::Validation) -> Result<(), Never> {
    let Ok(()) = self.program.reset(program);

    {
      let Ok(value) = self.graphics.reset();
      value
    };

    self.graphics.is_pedantic =
      crate::Pedantry::from(matches!(validation, crate::Validation::Pedantic));
    let Ok(()) = self.loop_budget.reset();

    match program {
      Program::Font => {
        let Ok(()) = self.definitions.functions.reset();
        let Ok(()) = self.definitions.instructions.reset();
      }
      Program::ControlValue => {
        self.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
      }
      Program::Glyph => {
        match ::core::primitive::bool::then_some(self.graphics.instruct_control & 2 != 0, ()) {
          ::core::option::Option::Some(()) => {
            {
              let Ok(value) = self.graphics.reset_retained();
              value
            };
          }
          ::core::option::Option::None => {}
        }

        let Ok(compatibility) = self.graphics.initial_backward_compatibility();

        self.graphics.backward_compatibility = compatibility;
      }
    }

    Ok(())
  }

  pub fn run(&mut self) -> Result<(), HintError> {
    for dispatched in 0..=MAX_RUN_INSTRUCTIONS {
      let decoded = self.decode()?;
      let ins = match decoded {
        Some(ins) => ins,
        None => return Ok(()),
      };
      self.dispatch(&ins)?;

      match ::core::primitive::bool::then_some(dispatched == MAX_RUN_INSTRUCTIONS, ()) {
        ::core::option::Option::Some(()) => {
          return Err(HintError {
            program: self.program.current,
            glyph_id: None,
            pc: ins.pc,
            opcode: Some(ins.opcode),
            kind: HintErrorKind::ExceededExecutionBudget,
          });
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }

  pub fn decode(&mut self) -> Result<Option<Instruction<'a>>, HintError> {
    let decoded = match self.program.decoder.decode() {
      Some(decoded) => decoded,
      None => return Ok(None),
    };
    let ins = decoded.map_err(|_decode_error| HintError {
      program: self.program.current,
      glyph_id: None,
      pc: self.program.decoder.pc,
      opcode: None,
      kind: HintErrorKind::UnexpectedEndOfBytecode,
    })?;
    Ok(Some(ins))
  }

  pub fn dispatch(&mut self, ins: &Instruction) -> Result<(), HintError> {
    let current_program = self.program.current;
    self.dispatch_inner(ins).map_err(|kind| HintError {
      program: current_program,
      glyph_id: None,
      pc: ins.pc,
      opcode: Some(ins.opcode),
      kind,
    })
  }

  fn dispatch_inner(&mut self, ins: &Instruction) -> Result<(), HintErrorKind> {
    use Opcode::*;
    let opcode = ins.opcode;
    let Ok(raw_opcode) = (opcode).to_byte();

    match ins.opcode {
      SVTCA0 | SVTCA1 | SPVTCA0 | SPVTCA1 | SFVTCA0 | SFVTCA1 => self.op_svtca(raw_opcode)?,
      SPVTL0 | SPVTL1 | SFVTL0 | SFVTL1 => self.op_svtl(raw_opcode)?,
      SPVFS => self.op_spvfs()?,
      SFVFS => self.op_sfvfs()?,
      GPV => self.op_gpv()?,
      GFV => self.op_gfv()?,
      SFVTPV => self.op_sfvtpv()?,
      ISECT => self.op_isect()?,
      SRP0 => self.op_srp0()?,
      SRP1 => self.op_srp1()?,
      SRP2 => self.op_srp2()?,
      SZP0 => self.op_szp0()?,
      SZP1 => self.op_szp1()?,
      SZP2 => self.op_szp2()?,
      SZPS => self.op_szps()?,
      SLOOP => self.op_sloop()?,
      RTG => self.op_rtg()?,
      RTHG => self.op_rthg()?,
      SMD => self.op_smd()?,
      ELSE => self.op_else()?,
      JMPR => self.op_jmpr()?,
      SCVTCI => self.op_scvtci()?,
      SSWCI => self.op_sswci()?,
      SSW => self.op_ssw()?,
      DUP => self.op_dup()?,
      POP => self.op_pop()?,
      CLEAR => self.op_clear()?,
      SWAP => self.op_swap()?,
      DEPTH => self.op_depth()?,
      CINDEX => self.op_cindex()?,
      MINDEX => self.op_mindex()?,
      ALIGNPTS => self.op_alignpts()?,

      UTP => self.op_utp()?,
      LOOPCALL => self.op_loopcall()?,
      CALL => self.op_call()?,
      FDEF => self.op_fdef()?,
      ENDF => self.op_endf()?,
      MDAP0 | MDAP1 => self.op_mdap(raw_opcode)?,
      IUP0 | IUP1 => self.op_iup(raw_opcode)?,
      SHP0 | SHP1 => self.op_shp(raw_opcode)?,
      SHC0 | SHC1 => self.op_shc(raw_opcode)?,
      SHZ0 | SHZ1 => self.op_shz(raw_opcode)?,
      SHPIX => self.op_shpix()?,
      IP => self.op_ip()?,
      MSIRP0 | MSIRP1 => self.op_msirp(raw_opcode)?,
      ALIGNRP => self.op_alignrp()?,
      RTDG => self.op_rtdg()?,
      MIAP0 | MIAP1 => self.op_miap(raw_opcode)?,
      NPUSHB | NPUSHW => self.op_push(&ins.inline_operands)?,
      WS => self.op_ws()?,
      RS => self.op_rs()?,
      WCVTP => self.op_wcvtp()?,
      RCVT => self.op_rcvt()?,
      GC0 | GC1 => self.op_gc(raw_opcode)?,
      SCFS => self.op_scfs()?,
      MD0 | MD1 => self.op_md(raw_opcode)?,
      MPPEM => self.op_mppem()?,
      MPS => self.op_mps()?,
      FLIPON => self.op_flipon()?,
      FLIPOFF => self.op_flipoff()?,

      DEBUG => {
        self.value_stack.pop()?;
      }
      LT => self.op_lt()?,
      LTEQ => self.op_lteq()?,
      GT => self.op_gt()?,
      GTEQ => self.op_gteq()?,
      EQ => self.op_eq()?,
      NEQ => self.op_neq()?,
      ODD => self.op_odd()?,
      EVEN => self.op_even()?,
      IF => self.op_if()?,
      EIF => self.op_eif()?,
      AND => self.op_and()?,
      OR => self.op_or()?,
      NOT => self.op_not()?,
      DELTAP1 => self.op_deltap(opcode)?,
      SDB => self.op_sdb()?,
      SDS => self.op_sds()?,
      ADD => self.op_add()?,
      SUB => self.op_sub()?,
      DIV => self.op_div()?,
      MUL => self.op_mul()?,
      ABS => self.op_abs()?,
      NEG => self.op_neg()?,
      FLOOR => self.op_floor()?,
      CEILING => self.op_ceiling()?,
      ROUND00 | ROUND01 | ROUND10 | ROUND11 => self.op_round()?,

      NROUND00 | NROUND01 | NROUND10 | NROUND11 => {}
      WCVTF => self.op_wcvtf()?,
      DELTAP2 | DELTAP3 => self.op_deltap(opcode)?,
      DELTAC1 | DELTAC2 | DELTAC3 => self.op_deltac(opcode)?,
      SROUND => self.op_sround()?,
      S45ROUND => self.op_s45round()?,
      JROT => self.op_jrot()?,
      JROF => self.op_jrof()?,
      ROFF => self.op_roff()?,

      RUTG => self.op_rutg()?,
      RDTG => self.op_rdtg()?,
      SANGW => self.op_sangw()?,

      AA => {}
      FLIPPT => self.op_flippt()?,
      FLIPRGON => self.op_fliprgon()?,
      FLIPRGOFF => self.op_fliprgoff()?,

      SCANCTRL => self.op_scanctrl()?,
      SDPVTL0 | SDPVTL1 => self.op_sdpvtl(raw_opcode)?,
      GETINFO => self.op_getinfo()?,
      IDEF => self.op_idef()?,
      ROLL => self.op_roll()?,
      MAX => self.op_max()?,
      MIN => self.op_min()?,
      SCANTYPE => self.op_scantype()?,
      INSTCTRL => self.op_instctrl()?,

      GETVARIATION => self.op_getvariation()?,
      GETDATA => self.op_getdata()?,
      INS28 | INS7B | INS83 | INS84 | INS8F | INS90 | INS93 | INS94 | INS95 | INS96 | INS97
      | INS98 | INS99 | INS9A | INS9B | INS9C | INS9D | INS9E | INS9F | INSA0 | INSA1 | INSA2
      | INSA3 | INSA4 | INSA5 | INSA6 | INSA7 | INSA8 | INSA9 | INSAA | INSAB | INSAC | INSAD
      | INSAE | INSAF | PUSHB000 | PUSHB001 | PUSHB010 | PUSHB011 | PUSHB100 | PUSHB101
      | PUSHB110 | PUSHB111 | PUSHW000 | PUSHW001 | PUSHW010 | PUSHW011 | PUSHW100 | PUSHW101
      | PUSHW110 | PUSHW111 | MDRP00000 | MDRP00001 | MDRP00010 | MDRP00011 | MDRP00100
      | MDRP00101 | MDRP00110 | MDRP00111 | MDRP01000 | MDRP01001 | MDRP01010 | MDRP01011
      | MDRP01100 | MDRP01101 | MDRP01110 | MDRP01111 | MDRP10000 | MDRP10001 | MDRP10010
      | MDRP10011 | MDRP10100 | MDRP10101 | MDRP10110 | MDRP10111 | MDRP11000 | MDRP11001
      | MDRP11010 | MDRP11011 | MDRP11100 | MDRP11101 | MDRP11110 | MDRP11111 | MIRP00000
      | MIRP00001 | MIRP00010 | MIRP00011 | MIRP00100 | MIRP00101 | MIRP00110 | MIRP00111
      | MIRP01000 | MIRP01001 | MIRP01010 | MIRP01011 | MIRP01100 | MIRP01101 | MIRP01110
      | MIRP01111 | MIRP10000 | MIRP10001 | MIRP10010 | MIRP10011 | MIRP10100 | MIRP10101
      | MIRP10110 | MIRP10111 | MIRP11000 | MIRP11001 | MIRP11010 | MIRP11011 | MIRP11100
      | MIRP11101 | MIRP11110 | MIRP11111 => {
        match (opcode >= MIRP00000, opcode >= MDRP00000, opcode >= PUSHB000) {
          (true, _, _) => self.op_mirp(raw_opcode)?,
          (false, true, _) => self.op_mdrp(raw_opcode)?,
          (false, false, true) => self.op_push(&ins.inline_operands)?,
          (false, false, false) => return self.op_unknown(raw_opcode),
        }
      }
    }

    Ok(())
  }
}
