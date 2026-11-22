//! Incremental checksums agree across byte and block boundaries.

use console_core_checksums::{
  Checksum,
  crc8::Crc8Ccitt,
  crc16::{Crc16Ansi, Crc16AnsiLe},
  md5::Md5,
  ogg::Crc32,
};

#[test]
fn crc_polynomials_have_their_own_check_values() {
  let Ok(mut crc8) = Crc8Ccitt::new(0);
  let Ok(mut crc16) = Crc16Ansi::new(0);
  let Ok(mut reflected) = Crc16AnsiLe::new(0);
  let Ok(()) = crc8.process_buf_bytes(b"123456789");
  let Ok(()) = crc16.process_buf_bytes(b"123456789");
  let Ok(()) = reflected.process_buf_bytes(b"123456789");
  assert_eq!(crc8.crc(), Ok(0xf4));
  assert_eq!(crc16.crc(), Ok(0xfee8));
  assert_eq!(reflected.crc(), Ok(0xbb3d));
}

#[test]
fn ogg_updates_keep_the_initial_state_across_partition_boundaries() {
  let bytes: Vec<u8> = (0..1029)
    .map(|n: u32| {
      let [byte, ..] = n.wrapping_mul(29).to_le_bytes();

      byte
    })
    .collect();

  for initial in [0, 0xffff_ffff, 0x1234_5678] {
    let Ok(mut expected) = Crc32::new(initial);
    let Ok(()) = expected.process_buf_bytes(&bytes);

    for width in [1u32, 2, 3, 7, 8, 9, 15, 16, 17, 64, 255] {
      let Ok(size) = console_core_number_conversion::index(width);
      let Ok(mut actual) = Crc32::new(initial);

      for chunk in bytes.chunks(size.max(1)) {
        let Ok(()) = actual.process_buf_bytes(chunk);
      }

      assert_eq!(actual.crc(), expected.crc());
    }
  }
}

#[test]
fn md5_padding_is_stable_at_block_boundaries_and_finalization_preserves_future_updates() {
  for length in [0, 1, 55, 56, 63, 64, 65, 119, 120, 127, 128, 129, 4097] {
    let bytes: Vec<u8> = (0..length)
      .map(|n: u32| {
        let [byte, ..] = n.wrapping_mul(29).to_le_bytes();

        byte
      })
      .collect();
    let mut expected = Md5::default();
    let Ok(()) = expected.process_buf_bytes(&bytes);

    for width in [1u32, 3, 7, 16, 63, 64, 65, 127] {
      let Ok(size) = console_core_number_conversion::index(width);
      let mut actual = Md5::default();

      for chunk in bytes.chunks(size.max(1)) {
        let Ok(()) = actual.process_buf_bytes(chunk);
      }

      assert_eq!(actual.md5(), expected.md5());
      let Ok(()) = actual.process_buf_bytes(b"suffix");
      let mut extended = Md5::default();
      let Ok(()) = extended.process_buf_bytes(&bytes);
      let Ok(()) = extended.process_buf_bytes(b"suffix");
      assert_eq!(actual.md5(), extended.md5());
    }
  }
}

#[test]
fn reflected_crc32_updates_preserve_registers_across_partitions() {
  let bytes: Vec<u8> = (0..1029u32)
    .map(|n| {
      let [byte, ..] = n.wrapping_mul(29).to_le_bytes();

      byte
    })
    .collect();

  for initial in [0, 0xffff_ffff, 0x1234_5678] {
    let reference = bytes.iter().fold(initial, |crc, byte| {
      (0..8).fold(crc ^ u32::from(*byte), |bits, _bit| match bits & 1 {
        0 => bits.wrapping_shr(1),
        _odd => bits.wrapping_shr(1) ^ 0xedb8_8320,
      })
    });

    for width in [1u32, 2, 3, 7, 8, 9, 15, 16, 17, 64, 255] {
      let Ok(size) = console_core_number_conversion::index(width);
      let actual = bytes.chunks(size.max(1)).fold(initial, |crc, chunk| {
        let Ok(updated) = console_core_checksums::crc32::update(crc, chunk);

        updated
      });

      assert_eq!(actual, reference);
    }
  }
}
