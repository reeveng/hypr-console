//! Compatibility entry point for the shared DEFLATE codec.

use crate::ZipError;

pub fn deflate(bytes: &[u8]) -> Result<Vec<u8>, ZipError> {
  console_core_deflate::deflate(bytes).map_err(ZipError::from)
}
