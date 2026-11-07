//! Codec-owned context initialization and position conversion for the shared CABAC engine.

use crate::cabac_tables::CABAC_INIT_MN;
use crate::error::{Error, Result};
use console_core_adaptive_arithmetic_decoding::{BytePosition, Decoder, Exhaustion};

pub const NUM_CTX: usize = 1024;

fn init_contexts(ctx: &mut [u8; NUM_CTX], slice_qp: i32, init: usize) {
    let qp = slice_qp.clamp(0, 51);
    let tab = &CABAC_INIT_MN[init];
    for (c, &(m, n)) in ctx.iter_mut().zip(tab.iter()) {
        let pre = (((m as i32) * qp) >> 4) + n as i32;
        let pre = pre.clamp(1, 126);
        *c = if pre <= 63 {
            ((63 - pre) << 1) as u8
        } else {
            (((pre - 64) << 1) | 1) as u8
        };
    }
}

pub struct Cabac<'a> {
    engine: Decoder<'a>,
    pub ctx: [u8; NUM_CTX],
}

impl<'a> Cabac<'a> {
    pub fn new(data: &'a [u8], pos: usize, slice_qp: i32, init: usize) -> Result<Self> {
        let mut ctx = [0; NUM_CTX];
        init_contexts(&mut ctx, slice_qp, init);
        let engine = Decoder::new(data, BytePosition((pos / 8) as u64))
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

    pub fn set_bit_pos(&mut self, pos: usize) {
        let Ok(()) = self
            .engine
            .set_byte_position(BytePosition((pos / 8) as u64));
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
            .decode_decision(&mut self.ctx[ctx_idx & (NUM_CTX - 1)]);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_init_formula() {
        let c = Cabac::new(&[0, 0, 0, 0], 0, 26, 3).unwrap();
        // ctxIdx 0: m=20, n=-15 at QP 26: ((20*26)>>4) - 15 = 32 - 15 = 17 -> pState 46, MPS 0
        assert_eq!(c.ctx[0], 46 << 1);
        // ctxIdx 60: m=0,n=41 -> pre 41 -> pState 22 MPS 0
        assert_eq!(c.ctx[60], 22 << 1);
        // ctxIdx 70 (I): m=0 n=11 -> pState 52 MPS 0
        assert_eq!(c.ctx[70], 52 << 1);
    }

    #[test]
    fn starts_use_bits_and_pcm_restarts_keep_contexts() -> Result<()> {
        let data = [0xFF, 0xFF, 0, 0, 0, 0, 0, 0];
        let mut c = Cabac::new(&data, 16, 26, 3)?;
        assert_eq!(c.bit_pos(), 25);
        assert_eq!(c.data(), data.as_slice());
        c.decode_decision(0);
        let contexts = c.ctx;
        c.set_bit_pos(32);
        c.init_engine()?;
        assert_eq!(c.bit_pos(), 41);
        assert_eq!(c.ctx, contexts);
        Ok(())
    }

    #[test]
    fn invalid_initial_offsets_keep_the_codec_error() {
        assert!(
            matches!(Cabac::new(&[0xFF, 0], 0, 26, 3), Err(Error::Invalid(why)) if why == "invalid CABAC offset at init")
        );
    }
}
