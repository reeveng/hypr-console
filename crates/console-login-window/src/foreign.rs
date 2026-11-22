//! C string results are borrowed from their caller-owned output buffers.

use std::ffi::{CStr, c_char};
use std::io;

use console_core_number_conversion::{fitted, index};

pub(crate) fn text_at(buffer: &[u8], returned: *const c_char) -> io::Result<&CStr> {
  let Ok(address) = fitted::<_, u64>(returned.addr());
  let Ok(start) = fitted::<_, u64>(buffer.as_ptr().addr());
  let offset = address
    .checked_sub(start)
    .ok_or(io::ErrorKind::InvalidData)?;
  let Ok(at) = index(offset);
  let remaining = buffer.get(at..).ok_or(io::ErrorKind::InvalidData)?;

  CStr::from_bytes_until_nul(remaining)
    .map_err(|_unterminated| io::Error::from(io::ErrorKind::InvalidData))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_output_string_has_to_end_within_the_owned_buffer() {
    let output = b"value";
    let start = output.as_ptr().cast();

    assert!(matches!(text_at(output, start), Err(_unterminated)));
    assert!(matches!(
      text_at(b"value\0", std::ptr::null()),
      Err(_outside_buffer)
    ));
  }
}
