use console_core_mq_decoding::{Context, Decoder};
use console_core_number_conversion::index;

use crate::error::DecodeError;

pub(crate) fn contexts(bits: u32) -> Result<Vec<Context>, DecodeError> {
  let count = 1u32.checked_shl(bits).ok_or(DecodeError::Overflow)?;
  let Ok(length) = index(count);

  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "the width is a fixed arithmetic template size or the rounded-up bit width of the symbol table already in memory"
    )
  )]
  let states = vec![Context::default(); length];

  Ok(states)
}

pub(crate) fn bit(
  decoder: &mut Decoder<'_>,
  contexts: &mut [Context],
  at: u32,
) -> Result<u32, DecodeError> {
  let Ok(at_2) = index(at);
  let context = contexts.get_mut(at_2).ok_or(DecodeError::Overflow)?;
  let Ok(bit) = decoder.read_bit(context);

  Ok(bit)
}
