//! MD5 compatibility adapter for legacy PDF encryption.

pub(crate) fn calculate(
  data: &[u8],
) -> core::result::Result<[u8; 16], console_core_never::Never> {
  let Ok(digest) = console_core_checksums::md5::digest(data);

  Ok(digest)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn md5_test(data: &[u8]) -> Result<(), console_core_never::Never> {
    let Ok(our_result) = calculate(data);

    let external_result = md5::compute(data);

    assert_eq!(
      our_result,
      external_result.as_slice(),
      "MD5 calculation should match external md5 crate for input: {data:?}"
    );

    Ok(())
  }

  #[test]
  fn correctness() {
    let Ok(()) = md5_test(b"Hello, World!");
    let Ok(()) = md5_test(b"The quick brown fox jumps over the lazy dog");
    let Ok(()) = md5_test(b"abcdefghijklmnopqrstuvwxyz");
  }
}
