//! SHA-512, because PDF encryption at revision 6 derives its keys with it.
//!
//! FIPS 180-4 defines every addition here modulo two to the sixty-fourth, so
//! `wrapping_add` is the algorithm's own policy rather than a size left
//! unbounded. As in SHA-256, only the sixteen schedule words a round can still
//! read are kept. SHA-384 is these rounds from other starting words, cut
//! short, so `digest` is told the words to start from and how many bytes of
//! what they end on to answer.

use console_core_never::Never;

use crate::padding::padded;

const STANDING: [u64; 8] = [
  0x6a09_e667_f3bc_c908,
  0xbb67_ae85_84ca_a73b,
  0x3c6e_f372_fe94_f82b,
  0xa54f_f53a_5f1d_36f1,
  0x510e_527f_ade6_82d1,
  0x9b05_688c_2b3e_6c1f,
  0x1f83_d9ab_fb41_bd6b,
  0x5be0_cd19_137e_2179,
];

const ROUND: [u64; 80] = [
  0x428a_2f98_d728_ae22,
  0x7137_4491_23ef_65cd,
  0xb5c0_fbcf_ec4d_3b2f,
  0xe9b5_dba5_8189_dbbc,
  0x3956_c25b_f348_b538,
  0x59f1_11f1_b605_d019,
  0x923f_82a4_af19_4f9b,
  0xab1c_5ed5_da6d_8118,
  0xd807_aa98_a303_0242,
  0x1283_5b01_4570_6fbe,
  0x2431_85be_4ee4_b28c,
  0x550c_7dc3_d5ff_b4e2,
  0x72be_5d74_f27b_896f,
  0x80de_b1fe_3b16_96b1,
  0x9bdc_06a7_25c7_1235,
  0xc19b_f174_cf69_2694,
  0xe49b_69c1_9ef1_4ad2,
  0xefbe_4786_384f_25e3,
  0x0fc1_9dc6_8b8c_d5b5,
  0x240c_a1cc_77ac_9c65,
  0x2de9_2c6f_592b_0275,
  0x4a74_84aa_6ea6_e483,
  0x5cb0_a9dc_bd41_fbd4,
  0x76f9_88da_8311_53b5,
  0x983e_5152_ee66_dfab,
  0xa831_c66d_2db4_3210,
  0xb003_27c8_98fb_213f,
  0xbf59_7fc7_beef_0ee4,
  0xc6e0_0bf3_3da8_8fc2,
  0xd5a7_9147_930a_a725,
  0x06ca_6351_e003_826f,
  0x1429_2967_0a0e_6e70,
  0x27b7_0a85_46d2_2ffc,
  0x2e1b_2138_5c26_c926,
  0x4d2c_6dfc_5ac4_2aed,
  0x5338_0d13_9d95_b3df,
  0x650a_7354_8baf_63de,
  0x766a_0abb_3c77_b2a8,
  0x81c2_c92e_47ed_aee6,
  0x9272_2c85_1482_353b,
  0xa2bf_e8a1_4cf1_0364,
  0xa81a_664b_bc42_3001,
  0xc24b_8b70_d0f8_9791,
  0xc76c_51a3_0654_be30,
  0xd192_e819_d6ef_5218,
  0xd699_0624_5565_a910,
  0xf40e_3585_5771_202a,
  0x106a_a070_32bb_d1b8,
  0x19a4_c116_b8d2_d0c8,
  0x1e37_6c08_5141_ab53,
  0x2748_774c_df8e_eb99,
  0x34b0_bcb5_e19b_48a8,
  0x391c_0cb3_c5c9_5a63,
  0x4ed8_aa4a_e341_8acb,
  0x5b9c_ca4f_7763_e373,
  0x682e_6ff3_d6b2_b8a3,
  0x748f_82ee_5def_b2fc,
  0x78a5_636f_4317_2f60,
  0x84c8_7814_a1f0_ab72,
  0x8cc7_0208_1a64_39ec,
  0x90be_fffa_2363_1e28,
  0xa450_6ceb_de82_bde9,
  0xbef9_a3f7_b2c6_7915,
  0xc671_78f2_e372_532b,
  0xca27_3ece_ea26_619c,
  0xd186_b8c7_21c0_c207,
  0xeada_7dd6_cde0_eb1e,
  0xf57d_4f7f_ee6e_d178,
  0x06f0_67aa_7217_6fba,
  0x0a63_7dc5_a2c8_98a6,
  0x113f_9804_bef9_0dae,
  0x1b71_0b35_131c_471b,
  0x28db_77f5_2304_7d84,
  0x32ca_ab7b_40c7_2493,
  0x3c9e_be0a_15c9_bebc,
  0x431d_67c4_9c10_0d4c,
  0x4cc5_d4be_cb3e_42b6,
  0x597f_299c_fc65_7e2a,
  0x5fcb_6fab_3ad6_faec,
  0x6c44_198c_4a47_5817,
];

fn round(working: [u64; 8], scheduled: u64) -> Result<[u64; 8], Never> {
  let [a, b, c, d, e, f, g, h] = working;
  let chosen = (e & f) ^ (!e & g);
  let majority = (a & b) ^ (a & c) ^ (b & c);
  let upper = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
  let lower = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
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

fn compressed(standing: [u64; 8], block: &[u8; 128]) -> Result<[u64; 8], Never> {
  let (eights, _none_left) = block.as_chunks::<8>();
  let mut window = [0u64; 16];

  for (slot, eight) in window.iter_mut().zip(eights) {
    *slot = u64::from_be_bytes(*eight);
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
    let near = w14.rotate_right(19) ^ w14.rotate_right(61) ^ w14.wrapping_shr(6);
    let far = w1.rotate_right(1) ^ w1.rotate_right(8) ^ w1.wrapping_shr(7);
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

pub(crate) fn digest<const BYTES: usize>(
  data: &[u8],
  standing: [u64; 8],
) -> Result<[u8; BYTES], Never> {
  let Ok(held) = padded::<128, 16>(data);
  let (blocks, _none_left) = held.as_chunks::<128>();

  let words = blocks.iter().fold(standing, |working, block| {
    let Ok(next) = compressed(working, block);

    next
  });

  let mut written = [0u8; BYTES];
  let (eights, _none_left) = written.as_chunks_mut::<8>();

  for (eight, word) in eights.iter_mut().zip(words) {
    *eight = word.to_be_bytes();
  }

  Ok(written)
}

pub fn of(data: &[u8]) -> Result<[u8; 64], Never> {
  digest(data, STANDING)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_examples_fips_180_4_works_through() {
    assert_eq!(
      of(b"abc"),
      Ok([
        0xdd, 0xaf, 0x35, 0xa1, 0x93, 0x61, 0x7a, 0xba, 0xcc, 0x41, 0x73, 0x49, 0xae, 0x20, 0x41,
        0x31, 0x12, 0xe6, 0xfa, 0x4e, 0x89, 0xa9, 0x7e, 0xa2, 0x0a, 0x9e, 0xee, 0xe6, 0x4b, 0x55,
        0xd3, 0x9a, 0x21, 0x92, 0x99, 0x2a, 0x27, 0x4f, 0xc1, 0xa8, 0x36, 0xba, 0x3c, 0x23, 0xa3,
        0xfe, 0xeb, 0xbd, 0x45, 0x4d, 0x44, 0x23, 0x64, 0x3c, 0xe8, 0x0e, 0x2a, 0x9a, 0xc9, 0x4f,
        0xa5, 0x4c, 0xa4, 0x9f,
      ])
    );
  }
}
