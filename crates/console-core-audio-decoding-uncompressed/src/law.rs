//! G.711 companding, computed from the segment and the quantisation step.
//!
//! The two functions are the arithmetic of Sun Microsystems' `g711.c` (public
//! domain, "provided for unrestricted use"), and `tests/the_pcm.rs` presses
//! every one of the two hundred and fifty-six codes through each of them.

use console_core_never::Never;

const QUANTISATION: u8 = 0x0f;
const SEGMENT: u8 = 0x70;
const SEGMENT_SHIFT: u32 = 4;
const SIGN: u8 = 0x80;
const MU_BIAS: i16 = 0x84;
const A_BIAS: i16 = 0x108;
const A_FIRST_SEGMENT: i16 = 0x8;

pub fn alaw_to_linear(coded: u8) -> Result<i16, Never> {
  let code = coded ^ 0x55;
  let step = i16::from(code & QUANTISATION).wrapping_shl(4);
  let segment = u32::from((code & SEGMENT).wrapping_shr(SEGMENT_SHIFT));
  let magnitude = match segment {
    0 => step.wrapping_add(A_FIRST_SEGMENT),
    1 => step.wrapping_add(A_BIAS),
    higher => step
      .wrapping_add(A_BIAS)
      .wrapping_shl(higher.wrapping_sub(1)),
  };

  Ok(match code & SIGN {
    0 => magnitude.wrapping_neg(),
    _ => magnitude,
  })
}

pub fn mulaw_to_linear(coded: u8) -> Result<i16, Never> {
  let code = !coded;
  let segment = u32::from((code & SEGMENT).wrapping_shr(SEGMENT_SHIFT));
  let biased = i16::from(code & QUANTISATION)
    .wrapping_shl(3)
    .wrapping_add(MU_BIAS)
    .wrapping_shl(segment);

  Ok(match code & SIGN {
    0 => biased.wrapping_sub(MU_BIAS),
    _ => MU_BIAS.wrapping_sub(biased),
  })
}
