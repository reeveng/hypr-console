//! TrueType bytecode decoder.

#[cfg(test)]
use super::OperandEncoding;
use super::{InlineOperands, Instruction, Opcode};

#[derive(Copy, Clone, Debug)]
pub struct DecodeError;

impl std::fmt::Display for DecodeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("unexpected end of bytecode")
  }
}

#[derive(Copy, Clone)]
pub struct Decoder<'a> {
  pub bytecode: &'a [u8],
  pub pc: u64,
}

impl<'a> Decoder<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Decoder::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn new(bytecode: &'a [u8], pc: u64) -> Self {
    Self { bytecode, pc }
  }

  #[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    expect(
      explicit048_no_unreal_state,
      reason = "the public decoder uses the same end-of-stream and per-instruction error contract as its Iterator implementation"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Decoder::decode` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn decode(&mut self) -> Option<Result<Instruction<'a>, DecodeError>> {
    let Ok(value_1) = console_core_number_conversion::index(self.pc);

    let bytecode = self.bytecode.get(value_1)?;
    let opcode = Opcode::from_byte(*bytecode);
    Some(self.decode_inner(opcode))
  }

  fn decode_inner(&mut self, opcode: Opcode) -> Result<Instruction<'a>, DecodeError> {
    let Ok(mut opcode_len) = opcode.len();
    let mut count_len = 0;

    match ::core::primitive::bool::then_some(opcode_len < 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_2) =
          console_core_number_conversion::index(::core::primitive::u64::wrapping_add(self.pc, 1));

        let inline_count =
          ::core::result::Result::<&_, _>::copied(self.bytecode.get(value_2).ok_or(DecodeError))?;
        opcode_len = ::core::primitive::i32::wrapping_add(
          ::core::primitive::i32::wrapping_mul(opcode_len.abs(), {
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(inline_count);
            migrated_value
          }),
          2,
        );
        count_len = 1;
      }
      ::core::option::Option::None => {}
    }

    let opcode_len_2 = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(opcode_len);
      migrated_value
    };
    let pc = self.pc;
    let next_pc = ::core::primitive::u64::wrapping_add(pc, opcode_len_2);

    let inline_start = ::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_add(pc, 1),
      count_len,
    );
    let inline_size = ::core::primitive::u64::wrapping_sub(next_pc, inline_start);
    let mut inline_operands = InlineOperands::default();

    match ::core::primitive::bool::then_some(inline_size > 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_3) = console_core_number_conversion::indices(
          inline_start..::core::primitive::u64::wrapping_add(inline_start, inline_size),
        );

        let bytecode = self.bytecode.get(value_3).ok_or(DecodeError)?;
        inline_operands.bytes = bytecode;
        inline_operands.encoding = opcode.operand_encoding();
      }
      ::core::option::Option::None => {}
    }

    match (opcode_len_2, &mut self.pc) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    Ok(Instruction {
      opcode,
      inline_operands,
      pc,
    })
  }
}

impl<'a> Iterator for Decoder<'a> {
  type Item = Result<Instruction<'a>, DecodeError>;

  fn next(&mut self) -> Option<Self::Item> {
    self.decode()
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the published `decode_all` function returns its iterator or trait value directly for existing callers to compose"
  )
)]
pub fn decode_all(
  bytecode: &[u8],
  pc: u64,
) -> impl Iterator<Item = Result<Instruction<'_>, DecodeError>> + '_ + Clone {
  Decoder::new(bytecode, pc)
}

#[cfg(test)]
mod tests {
  use super::{Opcode, OperandEncoding};

  #[test]
  fn mixed_ops() -> Result<(), crate::test_support::Failure> {
    let mut enc = Encoder::default();

    let cases: &[(Opcode, &[i16])] = &[
      (Opcode::PUSHB100, &[1, 2, 3, 255, 5]),
      (Opcode::PUSHW010, &[-1, 4508, -3]),
      (Opcode::IUP0, &[]),
      (Opcode::NPUSHB, &[55; 255]),
      (Opcode::MDRP00110, &[]),
      (Opcode::NPUSHW, &[i16::MIN; 32]),
      (Opcode::LOOPCALL, &[]),
      (Opcode::FLIPOFF, &[]),
      (
        Opcode::PUSHW011,
        &[
          i16::MIN,
          ::core::primitive::i16::wrapping_div(i16::MIN, 2),
          i16::MAX,
          ::core::primitive::i16::wrapping_div(i16::MAX, 2),
        ],
      ),
      (Opcode::GETVARIATION, &[]),
    ];
    for (opcode, values) in cases {
      match ::core::primitive::bool::then_some(!values.is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          enc.encode_push(values)?;
        }
        ::core::option::Option::None => {
          let Ok(()) = enc.encode(*opcode);
        }
      }
    }

    let instructions = super::decode_all(&enc.0, 0).collect::<Result<Vec<_>, _>>();
    let all_ins =
      crate::test_support::fixture_value(instructions, "encoded bytecode instructions")?;

    for (ins, (expected_opcode, expected_values)) in all_ins.iter().zip(cases) {
      assert_eq!(ins.opcode, *expected_opcode);
      let values = ins
        .inline_operands
        .values()
        .map(|v| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(v);
          migrated_value
        })
        .collect::<Vec<_>>();
      assert_eq!(&values, expected_values);
    }

    Ok(())
  }

  #[test]
  fn non_push_ops() -> Result<(), super::DecodeError> {
    let non_push_ops: Vec<_> = (0..=255)
      .filter(|b| {
        matches!(
          Opcode::from_byte(*b).operand_encoding(),
          OperandEncoding::NoInlineOperands
        )
      })
      .collect();
    let decoded: Vec<_> = super::decode_all(&non_push_ops, 0)
      .map(|instruction| {
        instruction.map(|ins| {
          let Ok(byte) = ins.opcode.to_byte();
          byte
        })
      })
      .collect::<Result<_, _>>()?;
    assert_eq!(non_push_ops, decoded);
    Ok(())
  }

  #[test]
  fn real_bytecode() -> Result<(), super::DecodeError> {
    let bytecode = [
      181, 5, 1, 9, 3, 1, 76, 75, 176, 45, 80, 88, 64, 35, 0, 3, 0, 9, 7, 3, 9, 105, 6, 4, 2, 1,
      1, 2, 97, 5, 1, 2, 2, 109, 77, 11, 8, 2, 7, 7, 0, 95, 10, 1, 0, 0, 107, 0, 78, 27, 64, 41,
      0, 7, 8, 0, 8, 7, 114, 0, 3, 0, 9, 8, 3, 9, 105, 6, 4, 2, 1, 1, 2, 97, 5, 1, 2, 2, 109, 77,
      11, 1, 8, 8, 0, 95, 10, 1, 0, 0, 107, 0, 78, 89, 64, 31, 37, 36, 1, 0, 40, 38, 36, 44, 37,
      44, 34, 32, 27, 25, 24, 23, 22, 20, 17, 16, 12, 10, 9, 8, 0, 35, 1, 35, 12, 13, 22, 43,
    ];

    let expected = [
      "PUSHB[5] 5 1 9 3 1 76",
      "MPPEM",
      "PUSHB[0] 45",
      "LT",
      "IF",
      "NPUSHB 0 3 0 9 7 3 9 105 6 4 2 1 1 2 97 5 1 2 2 109 77 11 8 2 7 7 0 95 10 1 0 0 107 0 78",
      "ELSE",
      "NPUSHB 0 7 8 0 8 7 114 0 3 0 9 8 3 9 105 6 4 2 1 1 2 97 5 1 2 2 109 77 11 1 8 8 0 95 10 1 0 0 107 0 78",
      "EIF",
      "NPUSHB 37 36 1 0 40 38 36 44 37 44 34 32 27 25 24 23 22 20 17 16 12 10 9 8 0 35 1 35 12 13 22",
      "CALL",
    ];
    let decoded_asm: Vec<_> = super::decode_all(&bytecode, 0)
      .map(|instruction| instruction.map(|ins| ins.to_string()))
      .collect::<Result<_, _>>()?;
    assert_eq!(decoded_asm, expected);
    Ok(())
  }

  #[derive(Default)]
  struct Encoder(Vec<u8>);

  impl Encoder {
    fn encode(&mut self, opcode: Opcode) -> Result<(), console_core_never::Never> {
      assert_eq!(
        opcode.operand_encoding(),
        OperandEncoding::NoInlineOperands,
        "use the encode_push method instead"
      );
      let Ok(raw) = opcode.to_byte();
      self.0.push(raw);
      Ok(())
    }

    fn encode_push(&mut self, values: &[i16]) -> Result<(), core::num::TryFromIntError> {
      let count = u8::try_from(values.len())?;

      match count {
        0 => return Ok(()),
        _nonempty => {}
      }

      #[derive(Clone, Copy)]
      enum PushWidth {
        Byte,
        Word,
      }

      let encoding = match crate::Presence::from(
        values
          .iter()
          .all(|value| (0..=i16::from(u8::MAX)).contains(value)),
      ) {
        crate::Presence::Present => PushWidth::Byte,
        crate::Presence::Absent => PushWidth::Word,
      };
      let opcode = match (encoding, count) {
        (PushWidth::Byte, 1..=8) => Opcode::PUSHB000,
        (PushWidth::Word, 1..=8) => Opcode::PUSHW000,
        (PushWidth::Byte, _) => Opcode::NPUSHB,
        (PushWidth::Word, _) => Opcode::NPUSHW,
      };
      let Ok(raw) = opcode.to_byte();

      match count {
        1..=8 => self.0.push(raw.wrapping_add(count).wrapping_sub(1)),
        _extended => self.0.extend([raw, count]),
      }

      match encoding {
        PushWidth::Byte => self.0.extend(values.iter().map(|&value| {
          let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(value);
          byte
        })),
        PushWidth::Word => self
          .0
          .extend(values.iter().flat_map(|value| value.to_be_bytes())),
      }

      Ok(())
    }
  }
}
