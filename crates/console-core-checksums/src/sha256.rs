//! SHA-256, because PDF encryption at revision 6 derives its keys with it.
//! Incremental hashing retains at most one block of input.
//!
//! FIPS 180-4 defines every addition here modulo two to the thirty-second, so
//! `wrapping_add` is the algorithm's own policy rather than a size left
//! unbounded. The standard writes the message schedule as sixty-four words,
//! but a round only ever reads the sixteen before it, so those sixteen are
//! all that is kept: each round takes the oldest and appends the next.

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

const STANDING: [u32; 8] = [
  0x6a09_e667,
  0xbb67_ae85,
  0x3c6e_f372,
  0xa54f_f53a,
  0x510e_527f,
  0x9b05_688c,
  0x1f83_d9ab,
  0x5be0_cd19,
];

const ROUND: [u32; 64] = [
  0x428a_2f98,
  0x7137_4491,
  0xb5c0_fbcf,
  0xe9b5_dba5,
  0x3956_c25b,
  0x59f1_11f1,
  0x923f_82a4,
  0xab1c_5ed5,
  0xd807_aa98,
  0x1283_5b01,
  0x2431_85be,
  0x550c_7dc3,
  0x72be_5d74,
  0x80de_b1fe,
  0x9bdc_06a7,
  0xc19b_f174,
  0xe49b_69c1,
  0xefbe_4786,
  0x0fc1_9dc6,
  0x240c_a1cc,
  0x2de9_2c6f,
  0x4a74_84aa,
  0x5cb0_a9dc,
  0x76f9_88da,
  0x983e_5152,
  0xa831_c66d,
  0xb003_27c8,
  0xbf59_7fc7,
  0xc6e0_0bf3,
  0xd5a7_9147,
  0x06ca_6351,
  0x1429_2967,
  0x27b7_0a85,
  0x2e1b_2138,
  0x4d2c_6dfc,
  0x5338_0d13,
  0x650a_7354,
  0x766a_0abb,
  0x81c2_c92e,
  0x9272_2c85,
  0xa2bf_e8a1,
  0xa81a_664b,
  0xc24b_8b70,
  0xc76c_51a3,
  0xd192_e819,
  0xd699_0624,
  0xf40e_3585,
  0x106a_a070,
  0x19a4_c116,
  0x1e37_6c08,
  0x2748_774c,
  0x34b0_bcb5,
  0x391c_0cb3,
  0x4ed8_aa4a,
  0x5b9c_ca4f,
  0x682e_6ff3,
  0x748f_82ee,
  0x78a5_636f,
  0x84c8_7814,
  0x8cc7_0208,
  0x90be_fffa,
  0xa450_6ceb,
  0xbef9_a3f7,
  0xc671_78f2,
];

fn round(working: [u32; 8], scheduled: u32) -> Result<[u32; 8], Never> {
  let [a, b, c, d, e, f, g, h] = working;
  let chosen = (e & f) ^ (!e & g);
  let majority = (a & b) ^ (a & c) ^ (b & c);
  let upper = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
  let lower = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
  let first = h
    .wrapping_add(lower)
    .wrapping_add(chosen)
    .wrapping_add(scheduled);
  let second = upper.wrapping_add(majority);

  Ok([
    first.wrapping_add(second),
    a,
    b,
    c,
    d.wrapping_add(first),
    e,
    f,
    g,
  ])
}

fn compressed(standing: [u32; 8], block: &[u8; 64]) -> Result<[u32; 8], Never> {
  let (fours, _none_left) = block.as_chunks::<4>();
  let mut window = [0u32; 16];

  for (slot, four) in window.iter_mut().zip(fours) {
    *slot = u32::from_be_bytes(*four);
  }

  let mut working = standing;

  for constant in ROUND {
    let [
      w0,
      w1,
      w2,
      w3,
      w4,
      w5,
      w6,
      w7,
      w8,
      w9,
      w10,
      w11,
      w12,
      w13,
      w14,
      w15,
    ] = window;
    let near = w14.rotate_right(17) ^ w14.rotate_right(19) ^ w14.wrapping_shr(10);
    let far = w1.rotate_right(7) ^ w1.rotate_right(18) ^ w1.wrapping_shr(3);
    let next = near.wrapping_add(w9).wrapping_add(far).wrapping_add(w0);

    window = [
      w1, w2, w3, w4, w5, w6, w7, w8, w9, w10, w11, w12, w13, w14, w15, next,
    ];

    let Ok(turned) = round(working, constant.wrapping_add(w0));

    working = turned;
  }

  let mut summed = standing;

  for (held, turned) in summed.iter_mut().zip(working) {
    *held = held.wrapping_add(turned);
  }

  Ok(summed)
}

#[derive(Clone, Debug)]
pub struct Sha256 {
  state: [u32; 8],
  block: [u8; 64],
  len: u64,
}

impl Default for Sha256 {
  fn default() -> Self {
    Self {
      state: STANDING,
      block: [0; 64],
      len: 0,
    }
  }
}

impl Sha256 {
  pub fn update(&mut self, bytes: &[u8]) -> Result<(), Never> {
    let occupied = self.len & 63;
    let Ok(held) = index(occupied);
    let Ok(length) = fixed_index(bytes.len());

    self.len = self.len.wrapping_add(length);

    let pending = match held {
      0 => bytes,
      _partial => {
        let needed = 64_u64.saturating_sub(occupied);
        let Ok(split) = index(needed);
        let (head, rest) = match bytes.split_at_checked(split) {
          Some(parts) => parts,
          None => (bytes, b"".as_slice()),
        };

        for (slot, byte) in self.block.iter_mut().skip(held).zip(head) {
          *slot = *byte;
        }

        match length.cmp(&needed) {
          core::cmp::Ordering::Less => return Ok(()),
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
            let Ok(state) = compressed(self.state, &self.block);

            self.state = state;
          }
        }

        rest
      }
    };

    let (blocks, tail) = pending.as_chunks::<64>();

    for block in blocks {
      let Ok(state) = compressed(self.state, block);

      self.state = state;
    }

    for (slot, byte) in self.block.iter_mut().zip(tail) {
      *slot = *byte;
    }

    Ok(())
  }

  pub fn finalize(self) -> Result<[u8; 32], Never> {
    let Ok(held) = index(self.len & 63);
    let mut block = self.block;
    let mut state = self.state;

    let padding = [0x80].into_iter().chain(core::iter::repeat_n(0, 63));

    for (slot, byte) in block.iter_mut().skip(held).zip(padding) {
      *slot = byte;
    }

    match held {
      0..=55 => {}
      _needs_another_block => {
        let Ok(next) = compressed(state, &block);

        state = next;
        block = [0; 64];
      }
    }

    for (slot, byte) in block
      .iter_mut()
      .skip(56)
      .zip(self.len.wrapping_mul(8).to_be_bytes())
    {
      *slot = byte;
    }

    let Ok(summed) = compressed(state, &block);
    let mut digest = [0u8; 32];

    for (four, word) in digest.as_chunks_mut::<4>().0.iter_mut().zip(summed) {
      *four = word.to_be_bytes();
    }

    Ok(digest)
  }
}

pub fn of(data: &[u8]) -> Result<[u8; 32], Never> {
  let mut digest = Sha256::default();
  let Ok(()) = digest.update(data);

  digest.finalize()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_examples_fips_180_4_works_through() {
    assert_eq!(
      of(b"abc"),
      Ok([
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
      ])
    );
    assert_eq!(
      of(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
      Ok([
        0x24, 0x8d, 0x6a, 0x61, 0xd2, 0x06, 0x38, 0xb8, 0xe5, 0xc0, 0x26, 0x93, 0x0c, 0x3e, 0x60,
        0x39, 0xa3, 0x3c, 0xe4, 0x59, 0x64, 0xff, 0x21, 0x67, 0xf6, 0xec, 0xed, 0xd4, 0x19, 0xdb,
        0x06, 0xc1,
      ])
    );
  }

  #[test]
  fn nothing_hashes_to_the_published_empty_digest() {
    assert_eq!(
      of(b""),
      Ok([
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
      ])
    );
  }
}
