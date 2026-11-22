//! FNV-1a fingerprints with explicit byte, word and UTF-16 input conventions.
//!
//! Word mixing is one XOR/multiply per word, not a byte serialization. It is
//! retained for existing golden fingerprints. None of these hashes is secure.
//! `of_utf16` mixes each UTF-16 code unit once, which is the historical
//! JavaScript hash. `legacy_av1` is the AV1 tests' historical fingerprint: two
//! 64-bit accumulators with different primes and seeds, not the standard
//! 128-bit FNV-1a. The state is a type of its own so that a state and a word,
//! one number each, cannot be handed over in each other's place.

use console_core_never::Never;

pub const OFFSET32: u32 = 0x811c_9dc5;
pub const OFFSET64: u64 = 0xcbf2_9ce4_8422_2325;

const PRIME32: u32 = 0x0100_0193;

const PRIME64: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State32(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State64(pub u64);

impl State32 {
  pub fn mixed(self, word: u32) -> Result<State32, Never> {
    let State32(state) = self;

    Ok(State32((state ^ word).wrapping_mul(PRIME32)))
  }
}

impl State64 {
  pub fn mixed(self, word: u64) -> Result<State64, Never> {
    let State64(state) = self;

    Ok(State64((state ^ word).wrapping_mul(PRIME64)))
  }
}

pub fn update32(state: u32, bytes: &[u8]) -> Result<u32, Never> {
  let State32(ended) = bytes.iter().fold(State32(state), |held, byte| {
    let Ok(next) = held.mixed(u32::from(*byte));

    next
  });

  Ok(ended)
}

pub fn update64(state: u64, bytes: &[u8]) -> Result<u64, Never> {
  let State64(ended) = bytes.iter().fold(State64(state), |held, byte| {
    let Ok(next) = held.mixed(u64::from(*byte));

    next
  });

  Ok(ended)
}

pub fn of32(bytes: &[u8]) -> Result<u32, Never> {
  update32(OFFSET32, bytes)
}

pub fn of64(bytes: &[u8]) -> Result<u64, Never> {
  update64(OFFSET64, bytes)
}

pub fn of_utf16(text: &str) -> Result<u32, Never> {
  let State32(ended) = text.encode_utf16().fold(State32(OFFSET32), |held, unit| {
    let Ok(next) = held.mixed(u32::from(unit));

    next
  });

  Ok(ended)
}

pub fn legacy_av1(bytes: &[u8]) -> Result<[u64; 2], Never> {
  Ok(
    bytes
      .iter()
      .fold([OFFSET64, 0x6c62_272e_07bb_0142], |[low, high], byte| {
        let Ok(State64(mixed)) = State64(high).mixed(u64::from(*byte));

        [
          (low ^ u64::from(*byte)).wrapping_mul(u64::from(PRIME32)),
          mixed,
        ]
      }),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn published_byte_vectors() {
    assert_eq!(of32(b""), Ok(OFFSET32));
    assert_eq!(of64(b""), Ok(OFFSET64));
    assert_eq!(of32(b"foobar"), Ok(0xbf9c_f968));
    assert_eq!(of64(b"foobar"), Ok(0x8594_4171_f739_67e8));
  }

  #[test]
  fn splitting_input_preserves_the_register() {
    let Ok(prefix64) = of64(b"foo");
    assert_eq!(update64(prefix64, b"bar"), of64(b"foobar"));
    let Ok(prefix32) = of32(b"foo");
    assert_eq!(update32(prefix32, b"bar"), of32(b"foobar"));
  }

  #[test]
  fn utf16_and_word_inputs_preserve_their_original_boundaries() {
    let Ok(first) = State32(OFFSET32).mixed(0x100);
    let Ok(second) = first.mixed(0xd83d);
    let Ok(State32(third)) = second.mixed(0xde00);
    assert_eq!(of_utf16("\u{100}\u{1f600}"), Ok(third));
    assert_ne!(of_utf16("\u{100}"), of32("\u{100}".as_bytes()));
    assert_eq!(
      State64(OFFSET64).mixed(u64::MAX),
      Ok(State64(0x509c_41b3_79fe_466e))
    );
  }

  #[test]
  fn historical_av1_fingerprint_keeps_both_original_accumulators() {
    assert_eq!(
      legacy_av1(b"123456789"),
      Ok([0x4994_6252_8f2a_bb7c, 0x7fae_ade2_afa8_96e1])
    );
  }
}
