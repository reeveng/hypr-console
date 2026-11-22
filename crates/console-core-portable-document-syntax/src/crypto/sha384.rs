//! Shared SHA-384 adapter for PDF encryption.

pub(crate) fn calculate(
  data: &[u8],
) -> core::result::Result<[u8; 48], console_core_never::Never> {
  let Ok(digest) = console_core_checksums::sha384::of(data);

  Ok(digest)
}
