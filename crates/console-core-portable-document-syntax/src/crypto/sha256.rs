//! Shared SHA-256 adapter for PDF encryption.

pub(crate) fn calculate(
  data: &[u8],
) -> core::result::Result<[u8; 32], console_core_never::Never> {
  let Ok(digest) = console_core_checksums::sha256::of(data);

  Ok(digest)
}
