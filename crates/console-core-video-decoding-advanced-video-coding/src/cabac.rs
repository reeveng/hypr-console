//! Codec-owned context initialization and position conversion for the shared CABAC engine.

use crate::cabac_tables::CABAC_INIT_MN;
use crate::error::{Error, Result, invalid};
use console_core_adaptive_arithmetic_decoding::{BitPosition, BytePosition, Decoder, Exhaustion};
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

pub(crate) struct Cabac<'a> {
  engine: Decoder<'a>,
  ctx: [u8; 1024],
}

impl<'a> Cabac<'a> {
  pub(crate) fn new(
    data: &'a [u8],
    position: BytePosition,
    slice_qp: i32,
    init: u32,
  ) -> Result<Self> {
    let Ok(init_index) = index(init);
    let table = match CABAC_INIT_MN.get(init_index) {
      Some(table) => table,
      None => return invalid("invalid CABAC context initialization index"),
    };
    let mut ctx = [0_u8; 1024];
    let qp = slice_qp.clamp(0, 51);

    for (context, &(slope, intercept)) in ctx.iter_mut().zip(table) {
      let pre = i32::from(slope)
        .wrapping_mul(qp)
        .wrapping_shr(4)
        .wrapping_add(i32::from(intercept))
        .clamp(1, 126);
      let state = match pre {
        1..=63 => 63_i32.wrapping_sub(pre).wrapping_shl(1),
        _ => pre.wrapping_sub(64).wrapping_shl(1) | 1,
      };
      let Ok(value) = wrapped::<_, u8>(state);
      *context = value;
    }

    let engine = Decoder::new(data, position).map_err(|why| Error::Invalid(why.to_string()))?;
    Ok(Self { engine, ctx })
  }

  pub(crate) fn init_engine(&mut self) -> Result<()> {
    self
      .engine
      .init_engine()
      .map_err(|why| Error::Invalid(why.to_string()))
  }

  pub(crate) fn bit_pos(&self) -> core::result::Result<BitPosition, Never> {
    self.engine.bit_position()
  }

  pub(crate) fn set_byte_pos(
    &mut self,
    position: BytePosition,
  ) -> core::result::Result<(), Never> {
    self.engine.set_byte_position(position)
  }

  pub(crate) fn data(&self) -> core::result::Result<&'a [u8], Never> {
    self.engine.data()
  }

  pub(crate) fn exhaustion(&self) -> core::result::Result<Exhaustion, Never> {
    self.engine.exhaustion()
  }

  #[inline(always)]
  pub(crate) fn decode_context(&mut self, context: u16) -> core::result::Result<u32, Never> {
    let Ok(position) = index(context & 1023);

    match self.ctx.get_mut(position) {
      Some(context_2) => self.engine.decode_decision(context_2),
      None => Ok(0),
    }
  }

  #[inline(always)]
  pub(crate) fn decode_bypass(&mut self) -> core::result::Result<u32, Never> {
    self.engine.decode_bypass()
  }

  pub(crate) fn decode_terminate(&mut self) -> core::result::Result<u32, Never> {
    self.engine.decode_terminate()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn context_init_formula() -> ::core::result::Result<(), crate::error::Error> {
    let c = Cabac::new(&[0, 0, 0, 0], BytePosition(0), 26, 3)?;

    assert_eq!(
      *match &c.ctx {
        [element, ..] => element,
      },
      u8::wrapping_shl(46, 1_u32)
    );

    assert_eq!(
      *match <[_]>::get(&c.ctx, 60) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None =>
          return ::core::result::Result::Err(crate::error::Error::Invalid(
            ::core::convert::From::from("index or range outside the buffer")
          )),
      },
      u8::wrapping_shl(22, 1_u32)
    );

    assert_eq!(
      *match <[_]>::get(&c.ctx, 70) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None =>
          return ::core::result::Result::Err(crate::error::Error::Invalid(
            ::core::convert::From::from("index or range outside the buffer")
          )),
      },
      u8::wrapping_shl(52, 1_u32)
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn starts_use_bits_and_pcm_restarts_keep_contexts() -> Result<()> {
    let data = [0xFF, 0xFF, 0, 0, 0, 0, 0, 0];
    let mut c = Cabac::new(&data, BytePosition(2), 26, 3)?;
    let Ok(bit_pos) = c.bit_pos();
    assert_eq!(bit_pos, BitPosition(25));
    let Ok(c_data) = c.data();
    assert_eq!(c_data, data.as_slice());
    let Ok(_) = c.decode_context(0);
    let contexts = c.ctx;
    let Ok(()) = c.set_byte_pos(BytePosition(4));
    c.init_engine()?;
    let Ok(c_bit_pos) = c.bit_pos();
    assert_eq!(c_bit_pos, BitPosition(41));
    assert_eq!(c.ctx, contexts);
    Ok(())
  }

  #[test]
  fn invalid_initial_offsets_keep_the_codec_error() {
    assert!(
      matches!(Cabac::new(&[0xFF, 0], BytePosition(0), 26, 3), Err(Error::Invalid(why)) if why == "invalid CABAC offset at init")
    );
  }
}
