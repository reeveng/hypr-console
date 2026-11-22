//! The bytes the readers are handed: a linear congruential generator's words, little-endian, so
//! a transcript reads the same on every machine and a seed is all a test file has to name.

use console_core_never::Never;
use console_core_number_conversion::index;

pub fn seeded<const SEED: u32>(count: u32) -> Result<Vec<u8>, Never> {
  let Ok(wanted) = index(count);
  let states = (0..count.div_ceil(4)).scan(SEED, |state, _| {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);

    Some(*state)
  });

  Ok(states.flat_map(u32::to_le_bytes).take(wanted).collect())
}
