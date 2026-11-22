//! Incremental MD5 with fixed-size buffering and a shared compression kernel.
//!
//! Adapted from Symphonia, copyright 2019-2022 The Project Symphonia
//! Developers, under the Mozilla Public License 2.0 kept beside this crate.

use core::num::NonZeroU32;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::Checksum;

const BLOCK: u32 = 64;
const MASK: u64 = 63;
const STANDING: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
const WORDS: NonZeroU32 = match NonZeroU32::new(16) {
  Some(count) => count,
  None => NonZeroU32::MIN,
};

const TURN: [u32; 64] = [
  7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
  14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
  21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const SINE: [u32; 64] = [
  0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
  0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
  0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
  0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
  0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
  0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
  0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
  0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

fn mixed(round: u32, held: [u32; 4]) -> Result<(u32, u32), Never> {
  let [_a, b, c, d] = held;

  Ok(match round {
    0..=15 => ((b & c) | (!b & d), round),
    16..=31 => (
      (d & b) | (!d & c),
      round.wrapping_mul(5).wrapping_add(1) % WORDS,
    ),
    32..=47 => (b ^ c ^ d, round.wrapping_mul(3).wrapping_add(5) % WORDS),
    _last_sixteen => (c ^ (b | !d), round.wrapping_mul(7) % WORDS),
  })
}

fn transform(state: &mut [u32; 4], block: &[u8; 64]) -> Result<(), Never> {
  let mut words = [0u32; 16];

  for (word, bytes) in words.iter_mut().zip(block.as_chunks::<4>().0) {
    *word = u32::from_le_bytes(*bytes);
  }

  let mut turning = *state;

  for (round, (turn, sine)) in (0u32..64).zip(TURN.iter().zip(SINE.iter())) {
    let Ok((mixed_bits, position)) = mixed(round, turning);
    let Ok(at) = index(position);

    let word = match words.get(at) {
      Some(word) => *word,
      None => 0,
    };

    let [a, b, c, d] = turning;
    let sum = mixed_bits
      .wrapping_add(a)
      .wrapping_add(*sine)
      .wrapping_add(word);

    turning = [d, b.wrapping_add(sum.rotate_left(*turn)), b, c];
  }

  for (before, after) in state.iter_mut().zip(turning) {
    *before = before.wrapping_add(after);
  }

  Ok(())
}

pub struct Md5 {
  state: [u32; 4],
  block: [u8; 64],
  len: u64,
}

impl Default for Md5 {
  fn default() -> Self {
    Md5 {
      state: STANDING,
      block: [0; 64],
      len: 0,
    }
  }
}

impl Md5 {
  pub fn md5(&self) -> Result<[u8; 16], Never> {
    let mut block = self.block;
    let mut state = self.state;
    let Ok(buffered) = index(self.len & MASK);

    for byte in block.iter_mut().skip(buffered) {
      *byte = 0;
    }

    match block.get_mut(buffered) {
      Some(terminator) => *terminator = 0x80,
      None => {}
    }

    match buffered {
      56.. => {
        let Ok(()) = transform(&mut state, &block);

        block = [0; 64];
      }
      0..=55 => {}
    }

    for (slot, byte) in block
      .iter_mut()
      .skip(56)
      .zip(self.len.wrapping_shl(3).to_le_bytes())
    {
      *slot = byte;
    }

    let Ok(()) = transform(&mut state, &block);
    let mut hash = [0u8; 16];

    for (number, output) in state.iter().zip(hash.as_chunks_mut::<4>().0) {
      *output = number.to_le_bytes();
    }

    Ok(hash)
  }

  fn buffered<'a>(&mut self, bytes: &'a [u8]) -> Result<&'a [u8], Never> {
    let Ok(held) = fitted::<u64, u32>(self.len & MASK);

    match held {
      0 => return Ok(bytes),
      _partial => {}
    }

    let Ok(wanted) = index(BLOCK.saturating_sub(held));
    let (taken, rest) = match bytes.split_at_checked(wanted) {
      Some(parts) => parts,
      None => (bytes, b"".as_slice()),
    };
    let Ok(offset) = index(held);

    for (slot, byte) in self.block.iter_mut().skip(offset).zip(taken) {
      *slot = *byte;
    }

    let Ok(read) = fitted::<_, u64>(taken.len());

    self.len = self.len.wrapping_add(read);

    match self.len & MASK {
      0 => {
        let Ok(()) = transform(&mut self.state, &self.block);
      }
      _partial => {}
    }

    Ok(rest)
  }
}

impl Checksum for Md5 {
  #[inline(always)]
  fn process_byte(&mut self, byte: u8) -> Result<(), Never> {
    let Ok(at) = index(self.len & MASK);

    match self.block.get_mut(at) {
      Some(slot) => *slot = byte,
      None => {}
    }

    self.len = self.len.wrapping_add(1);

    match self.len & MASK {
      0 => transform(&mut self.state, &self.block),
      _partial => Ok(()),
    }
  }

  fn process_buf_bytes(&mut self, buf: &[u8]) -> Result<(), Never> {
    let Ok(pending) = self.buffered(buf);
    let (blocks, rest) = pending.as_chunks::<64>();

    for block in blocks {
      let Ok(()) = transform(&mut self.state, block);
    }

    for (slot, byte) in self.block.iter_mut().zip(rest) {
      *slot = *byte;
    }

    let Ok(read) = fitted::<_, u64>(pending.len());

    self.len = self.len.wrapping_add(read);

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::Md5;
  use crate::Checksum;

  #[test]
  fn verify_md5() {
    const STRINGS: [&[u8]; 8] = [
      b"",
      b"a",
      b"abc",
      b"The quick brown fox jumps over the lazy dog",
      b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
      b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!",
      b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!?",
      b".s)cyIl?XKs}wDnLEUeZj'72=A/0!w;B[e*QUh)0{&XcGvf'xMx5Chhx_'ahg{GP|_R(0=Xe`lXQN_@MK9::",
    ];

    #[rustfmt::skip]
        const HASHES: [[u8; 16]; 8] = [
            [
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04,
                0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8, 0x42, 0x7e,
            ],
            [
                0x0c, 0xc1, 0x75, 0xb9, 0xc0, 0xf1, 0xb6, 0xa8,
                0x31, 0xc3, 0x99, 0xe2, 0x69, 0x77, 0x26, 0x61,
            ],
            [
                0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0,
                0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1, 0x7f, 0x72,
            ],
            [
                0x9e, 0x10, 0x7d, 0x9d, 0x37, 0x2b, 0xb6, 0x82,
                0x6b, 0xd8, 0x1d, 0x35, 0x42, 0xa4, 0x19, 0xd6,
            ],
            [
                0xd1, 0x74, 0xab, 0x98, 0xd2, 0x77, 0xd9, 0xf5,
                0xa5, 0x61, 0x1c, 0x2c, 0x9f, 0x41, 0x9d, 0x9f,
            ],
            [
                0x64, 0x1b, 0xa6, 0x02, 0x88, 0xc1, 0x7a, 0x2d,
                0xa5, 0x09, 0x00, 0x77, 0xeb, 0x89, 0x58, 0xad,
            ],
            [
                0x0a, 0x71, 0xdb, 0x4d, 0xf3, 0x50, 0x92, 0x73,
                0x62, 0x42, 0x3a, 0x42, 0xdc, 0xf8, 0x14, 0x57,
            ],
            [
                0x0b, 0x76, 0x74, 0x7e, 0xfd, 0xcd, 0xb9, 0x33,
                0x67, 0xfe, 0x2d, 0xa3, 0x21, 0x1b, 0x5d, 0x41,
            ],
        ];

    for (string, hash) in STRINGS.iter().zip(&HASHES) {
      let mut md5: Md5 = Default::default();

      let Ok(()) = md5.process_buf_bytes(string);

      assert_eq!(md5.md5(), Ok(*hash));
    }

    for (string, hash) in STRINGS.iter().zip(&HASHES) {
      let mut md5: Md5 = Default::default();

      for bytes in string.chunks(21) {
        let Ok(()) = md5.process_buf_bytes(bytes);
      }

      assert_eq!(md5.md5(), Ok(*hash));
    }

    for (string, hash) in STRINGS.iter().zip(&HASHES) {
      let mut md5: Md5 = Default::default();

      for byte in string.iter() {
        let Ok(()) = md5.process_byte(*byte);
      }

      assert_eq!(md5.md5(), Ok(*hash));
    }
  }
}
