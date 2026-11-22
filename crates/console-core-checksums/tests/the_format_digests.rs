use console_core_checksums::{md5, sha256, sha384, sha512};
use console_core_number_conversion::index;
use sha2::{Digest, Sha256, Sha384, Sha512};

#[test]
fn sha_padding_matches_an_independent_implementation_at_each_block_boundary() {
  for length in [
    0, 1, 55, 56, 63, 64, 65, 111, 112, 127, 128, 129, 255, 256, 1024,
  ] {
    let bytes: Vec<u8> = (0u32..length)
      .map(|offset| {
        let [low, ..] = offset.to_le_bytes();

        low
      })
      .collect();
    let Ok(actual256) = sha256::of(&bytes);
    assert_eq!(
      actual256.as_slice(),
      Sha256::digest(&bytes).as_slice(),
      "SHA-256 length {length}"
    );
    let Ok(actual384) = sha384::of(&bytes);
    assert_eq!(
      actual384.as_slice(),
      Sha384::digest(&bytes).as_slice(),
      "SHA-384 length {length}"
    );
    let Ok(actual512) = sha512::of(&bytes);
    assert_eq!(
      actual512.as_slice(),
      Sha512::digest(&bytes).as_slice(),
      "SHA-512 length {length}"
    );
  }
}

#[test]
fn incremental_sha256_matches_the_reference_across_partial_blocks() {
  for length in [0, 55, 56, 63, 64, 65, 119, 120, 127, 128, 129, 4097] {
    let bytes: Vec<u8> = (0u32..length)
      .map(|position| {
        let [low, ..] = position.to_le_bytes();
        low
      })
      .collect();
    let expected = Sha256::digest(&bytes);

    for width in [1u16, 7, 55, 63, 64, 65, 127, 128, 129] {
      let mut digest = sha256::Sha256::default();
      let Ok(chunk_width) = index(width);

      for chunk in bytes.chunks(chunk_width.max(1)) {
        let Ok(()) = digest.update(chunk);
        let Ok(()) = digest.update(b"");
      }

      let Ok(actual) = digest.finalize();
      assert_eq!(
        actual.as_slice(),
        expected.as_slice(),
        "length {length}, chunk {width}"
      );
    }
  }
}

#[test]
fn raw_md5_accepts_all_byte_values_for_pdf_encryption() {
  let bytes: Vec<u8> = (0..=255).collect();
  assert_eq!(
    md5::digest(&bytes),
    Ok([
      0xe2, 0xc8, 0x65, 0xdb, 0x41, 0x62, 0xbe, 0xd9, 0x63, 0xbf, 0xaa, 0x9e, 0xf6, 0xac, 0x18,
      0xf0,
    ])
  );
}
