use super::{Opcode, OperandEncoding};
use crate::Emptiness;

#[derive(Copy, Clone, Debug)]
pub struct Instruction<'a> {
  pub opcode: Opcode,
  pub inline_operands: InlineOperands<'a>,
  pub pc: u64,
}

impl std::fmt::Display for Instruction<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{}", self.opcode.name())?;

    for value in self.inline_operands.values() {
      write!(f, " {value}")?;
    }

    Ok(())
  }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct InlineOperands<'a> {
  pub(super) bytes: &'a [u8],
  pub(super) encoding: OperandEncoding,
}

impl<'a> InlineOperands<'a> {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `InlineOperands::len` method returns `u64` directly in the existing parser interface")
  )]
  pub fn len(&self) -> u64 {
    match self.encoding {
      OperandEncoding::Words => {
        let Ok(value_1) =
          console_core_number_conversion::fitted::<_, u64>(self.bytes.len().wrapping_div(2));

        value_1
      }
      OperandEncoding::Bytes | OperandEncoding::NoInlineOperands => {
        let Ok(value_2) = console_core_number_conversion::fitted::<_, u64>(self.bytes.len());

        value_2
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `InlineOperands::is_empty` method returns `Emptiness` directly in the existing parser interface")
  )]
  pub fn is_empty(&self) -> Emptiness {
    match self.bytes {
      [] => Emptiness::Empty,
      [_, ..] => Emptiness::NonEmpty,
    }
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `InlineOperands::values` method returns its iterator or trait value directly for existing callers to compose")
  )]
  pub fn values(&self) -> impl Iterator<Item = i32> + 'a + Clone + use<'a> {
    let (bytes, words): (&[u8], &[u8]) = match self.encoding {
      OperandEncoding::Words => (&[], self.bytes),
      OperandEncoding::Bytes | OperandEncoding::NoInlineOperands => (self.bytes, &[]),
    };
    bytes
      .iter()
      .map(|byte| {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(*byte);
          migrated_value
        });
        migrated_value
      })
      .chain(words.as_chunks::<2>().0.iter().map(|pair| {
        let word = i16::from_be_bytes(*pair);

        i32::from(word)
      }))
  }
}

#[cfg(any(test, feature = "scaler_test"))]
pub struct MockInlineOperands {
  bytes: Vec<u8>,
  encoding: OperandEncoding,
}

#[cfg(any(test, feature = "scaler_test"))]
impl MockInlineOperands {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `MockInlineOperands::from_bytes` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn from_bytes(bytes: &[u8]) -> Self {
    Self {
      bytes: <::std::vec::Vec<::core::primitive::u8> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(bytes),
      encoding: OperandEncoding::Bytes,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `MockInlineOperands::from_words` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn from_words(words: &[i16]) -> Self {
    Self {
      bytes: words
        .iter()
        .map(|word| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(*word);
          migrated_value
        })
        .flat_map(|word| {
          vec![
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(
                ::core::primitive::u16::wrapping_shr(word, 8_u32),
              );
              migrated_value
            }),
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(word);
              migrated_value
            }),
          ]
        })
        .collect(),
      encoding: OperandEncoding::Words,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `MockInlineOperands::operands` method returns `InlineOperands` directly in the existing parser interface")
  )]
  pub fn operands(&self) -> InlineOperands<'_> {
    InlineOperands {
      bytes: &self.bytes,
      encoding: self.encoding,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::MockInlineOperands;

  #[test]
  fn byte_operands() {
    let values = [5, 2, 85, 92, 26, 42, u8::MIN, u8::MAX];
    let mock = MockInlineOperands::from_bytes(&values);
    let decoded = mock.operands().values().collect::<Vec<_>>();
    assert!(
      values
        .iter()
        .map(|x| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(*x);
          migrated_value
        })
        .eq(decoded.iter().copied())
    );
  }

  #[test]
  fn word_operands() {
    let values = [-5, 2, 2845, 92, -26, 42, i16::MIN, i16::MAX];
    let mock = MockInlineOperands::from_words(&values);
    let decoded = mock.operands().values().collect::<Vec<_>>();
    assert!(
      values
        .iter()
        .map(|x| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(*x);
          migrated_value
        })
        .eq(decoded.iter().copied())
    );
  }
}
