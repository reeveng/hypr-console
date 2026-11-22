//! Codec-owned context initialization and position conversion for the shared CABAC engine.

use console_core_adaptive_arithmetic_decoding::{BytePosition, Decoder, Exhaustion};
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

use crate::error::{Error, Result};
use crate::spec_tables::CABAC_INIT;

pub type Contexts = [u8; 256];

#[derive(Clone, Copy)]
pub enum ContextTable {
  Intra,
  Predictive,
  Bidirectional,
}

#[derive(Clone, Copy)]
pub struct Initialization {
  pub table: ContextTable,
  pub qp: i32,
}

fn init_contexts(
  contexts: &mut Contexts,
  initialization: Initialization,
) -> std::result::Result<(), Never> {
  let [intra, predicted, bidirectional] = &CABAC_INIT;
  let parameters = match initialization.table {
    ContextTable::Intra => intra,
    ContextTable::Predictive => predicted,
    ContextTable::Bidirectional => bidirectional,
  };
  let qp = initialization.qp.clamp(0, 51);

  for (state, &value) in contexts.iter_mut().zip(parameters) {
    let slope = i32::from(value.wrapping_shr(4))
      .wrapping_mul(5)
      .wrapping_sub(45);
    let offset = i32::from(value & 15).wrapping_shl(3).wrapping_sub(16);
    let pre = slope
      .wrapping_mul(qp)
      .wrapping_shr(4)
      .wrapping_add(offset)
      .clamp(1, 126);
    let bits = match pre {
      ..=63 => 63i32.wrapping_sub(pre).wrapping_shl(1),
      _ => pre.wrapping_sub(64).wrapping_shl(1) | 1,
    };
    let Ok(encoded) = wrapped::<_, u8>(bits);
    *state = encoded;
  }

  Ok(())
}

pub struct Cabac<'a> {
  engine: Decoder<'a>,
  ctx: Contexts,
}

impl<'a> Cabac<'a> {
  pub fn new(data: &'a [u8], pos: BytePosition, initialization: Initialization) -> Result<Self> {
    let mut ctx = [0; 256];
    let Ok(()) = init_contexts(&mut ctx, initialization);
    let engine = Decoder::new(data, pos).map_err(|why| Error::Invalid(why.to_string()))?;
    Ok(Self { engine, ctx })
  }

  pub fn init_engine(&mut self) -> Result<()> {
    self
      .engine
      .init_engine()
      .map_err(|why| Error::Invalid(why.to_string()))
  }

  pub fn initialize_contexts(
    &mut self,
    initialization: Initialization,
  ) -> std::result::Result<(), Never> {
    init_contexts(&mut self.ctx, initialization)
  }

  pub fn contexts(&self) -> std::result::Result<Contexts, Never> {
    Ok(self.ctx)
  }

  pub fn restore_contexts(&mut self, contexts: &Contexts) -> std::result::Result<(), Never> {
    self.ctx = *contexts;
    Ok(())
  }

  pub fn bit_pos(&self) -> std::result::Result<u64, Never> {
    let Ok(position) = self.engine.bit_position();
    Ok(position.0)
  }

  pub fn set_byte_pos(&mut self, position: BytePosition) -> std::result::Result<(), Never> {
    self.engine.set_byte_position(position)
  }

  pub fn data(&self) -> std::result::Result<&'a [u8], Never> {
    self.engine.data()
  }

  #[must_use]
  pub fn exhaustion(&self) -> std::result::Result<Exhaustion, Never> {
    self.engine.exhaustion()
  }

  #[inline(always)]
  pub fn decode_decision(&mut self, context: u64) -> std::result::Result<u32, Never> {
    let Ok(at) = index(context & 255);
    let mut unused = 0;
    let state = match self.ctx.get_mut(at) {
      Some(state) => state,
      None => &mut unused,
    };
    self.engine.decode_decision(state)
  }

  #[inline(always)]
  pub fn decode_bypass(&mut self) -> std::result::Result<u32, Never> {
    self.engine.decode_bypass()
  }

  pub fn decode_terminate(&mut self) -> std::result::Result<u32, Never> {
    self.engine.decode_terminate()
  }

  #[inline(always)]
  pub fn decode_bypass_bits(&mut self, count: u8) -> std::result::Result<u32, Never> {
    self.engine.decode_bypass_bits(count)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::places::read;

  const INITIAL: Initialization = Initialization {
    table: ContextTable::Intra,
    qp: 30,
  };

  #[test]
  fn context_init_formula() {
    let mut ctx: Contexts = [0u8; 256];
    let Ok(()) = init_contexts(&mut ctx, INITIAL);
    let Ok(bypass) = read(&ctx, crate::spec_tables::CU_TRANSQUANT_BYPASS);
    let Ok(quadtree) = read(&ctx, crate::spec_tables::SPLIT_CU);
    assert_eq!(bypass, 1);
    assert_eq!(quadtree, 2);
  }

  #[test]
  fn starts_use_bytes_and_substream_restarts_keep_contexts() -> Result<()> {
    let data = [0xFF, 0xFF, 0, 0, 0, 0, 0, 0];
    let mut coder = Cabac::new(&data, BytePosition(2), INITIAL)?;
    let Ok(position) = coder.bit_pos();
    let Ok(contents) = coder.data();
    assert_eq!(position, 25);
    assert_eq!(contents, data.as_slice());
    let Ok(_decision) = coder.decode_decision(0);
    let Ok(contexts) = coder.contexts();
    let Ok(()) = coder.set_byte_pos(BytePosition(4));
    coder.init_engine()?;
    let Ok(restarted) = coder.bit_pos();
    let Ok(retained) = coder.contexts();
    assert_eq!(restarted, 41);
    assert_eq!(retained, contexts);
    Ok(())
  }

  #[test]
  fn invalid_initial_offsets_keep_the_codec_error() {
    assert!(
      matches!(Cabac::new(&[0xFF, 0], BytePosition(0), INITIAL), Err(Error::Invalid(why)) if why == "invalid CABAC offset at init")
    );
  }
}
