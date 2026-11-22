//! Shared SHA-512 adapter for PDF encryption.

pub(crate) fn calculate(
  data: &[u8],
) -> core::result::Result<[u8; 64], console_core_never::Never> {
  let Ok(digest) = console_core_checksums::sha512::of(data);

  Ok(digest)
}
