//! Codec-owned context initialization and position conversion for the shared CABAC engine.

use crate::error::{Error, Result};
use crate::spec_tables::{CABAC_INIT, NUM_CTX};
use console_core_adaptive_arithmetic_decoding::{BytePosition, Decoder, Exhaustion};

/// Context states, padded to a power of two so that the per-bin index needs no bounds check.
pub type Contexts = [u8; CTX_SLOTS];
const CTX_SLOTS: usize = NUM_CTX.next_power_of_two();

/// Initialise all context variables for `slice_qp` and `init_type` (9.3.2.2). Each context is stored as
/// `pStateIdx << 1 | valMps`.
pub fn init_contexts(ctx: &mut Contexts, slice_qp: i32, init_type: usize) {
    let qp = slice_qp.clamp(0, 51);
    for (c, &v) in ctx.iter_mut().zip(CABAC_INIT[init_type].iter()) {
        let slope = (v >> 4) as i32;
        let offset = (v & 15) as i32;
        let m = slope * 5 - 45;
        let n = (offset << 3) - 16;
        let pre = (((m * qp) >> 4) + n).clamp(1, 126);
        *c = if pre <= 63 {
            ((63 - pre) << 1) as u8
        } else {
            (((pre - 64) << 1) | 1) as u8
        };
    }
}

pub struct Cabac<'a> {
    engine: Decoder<'a>,
    pub ctx: Contexts,
}

impl<'a> Cabac<'a> {
    pub fn new(data: &'a [u8], pos: usize, slice_qp: i32, init: usize) -> Result<Self> {
        let mut ctx = [0; CTX_SLOTS];
        init_contexts(&mut ctx, slice_qp, init);
        let engine = Decoder::new(data, BytePosition(pos as u64))
            .map_err(|why| Error::Invalid(why.to_string()))?;
        Ok(Self { engine, ctx })
    }

    pub fn init_engine(&mut self) -> Result<()> {
        self.engine
            .init_engine()
            .map_err(|why| Error::Invalid(why.to_string()))
    }

    pub fn bit_pos(&self) -> usize {
        let Ok(position) = self.engine.bit_position();
        position.0 as usize
    }

    pub fn set_byte_pos(&mut self, pos: usize) {
        let Ok(()) = self.engine.set_byte_position(BytePosition(pos as u64));
    }

    pub fn data(&self) -> &'a [u8] {
        let Ok(data) = self.engine.data();
        data
    }

    pub fn overrun(&self) -> bool {
        let Ok(exhaustion) = self.engine.exhaustion();
        exhaustion == Exhaustion::Overrun
    }

    #[inline(always)]
    pub fn decode_decision(&mut self, ctx_idx: usize) -> u32 {
        let Ok(bin) = self
            .engine
            .decode_decision(&mut self.ctx[ctx_idx & (CTX_SLOTS - 1)]);
        bin
    }

    #[inline(always)]
    pub fn decode_bypass(&mut self) -> u32 {
        let Ok(bin) = self.engine.decode_bypass();
        bin
    }

    pub fn decode_terminate(&mut self) -> u32 {
        let Ok(bin) = self.engine.decode_terminate();
        bin
    }

    #[inline(always)]
    pub fn decode_bypass_bits(&mut self, count: u8) -> u32 {
        let Ok(bins) = self.engine.decode_bypass_bits(count);
        bins
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_init_formula() {
        let mut ctx: Contexts = [0u8; CTX_SLOTS];
        // initValue 154 is the "equiprobable" value: m = 0, n = 64 -> pStateIdx 0, valMps 1
        init_contexts(&mut ctx, 30, 0);
        assert_eq!(ctx[crate::spec_tables::CU_TRANSQUANT_BYPASS], 1);
        // split_cu_flag ctx 0 (initValue 139): slope 8, offset 11 -> m = -5, n = 72; QP 30: (-150 >> 4) + 72 = 62
        assert_eq!(ctx[crate::spec_tables::SPLIT_CU], (63 - 62) << 1);
    }

    #[test]
    fn starts_use_bytes_and_substream_restarts_keep_contexts() -> Result<()> {
        let data = [0xFF, 0xFF, 0, 0, 0, 0, 0, 0];
        let mut c = Cabac::new(&data, 2, 30, 0)?;
        assert_eq!(c.bit_pos(), 25);
        assert_eq!(c.data(), data.as_slice());
        c.decode_decision(0);
        let contexts = c.ctx;
        c.set_byte_pos(4);
        c.init_engine()?;
        assert_eq!(c.bit_pos(), 41);
        assert_eq!(c.ctx, contexts);
        Ok(())
    }

    #[test]
    fn invalid_initial_offsets_keep_the_codec_error() {
        assert!(
            matches!(Cabac::new(&[0xFF, 0], 0, 30, 0), Err(Error::Invalid(why)) if why == "invalid CABAC offset at init")
        );
    }
}
