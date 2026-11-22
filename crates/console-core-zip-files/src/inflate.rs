//! Compatibility entry point for the shared DEFLATE codec.

use crate::ZipError;

pub fn inflate(packed: &[u8], size: u32) -> Result<Vec<u8>, ZipError> {
  console_core_deflate::inflate(packed, size).map_err(ZipError::from)
}
