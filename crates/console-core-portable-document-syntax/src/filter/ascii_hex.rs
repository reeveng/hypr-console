use alloc::vec::Vec;
use console_core_postscript_tokens::decoding;

pub(crate) fn decode(
  data: &[u8],
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  let Ok(into) = decode_into(data);

  Ok(into)
}

pub(crate) fn decode_into<B: Default + Extend<u8>>(
  data: &[u8],
) -> core::result::Result<Option<B>, console_core_never::Never> {
  let body = match data.split(|byte| *byte == b'>').next() {
    Some(body) => body,
    None => data,
  };
  let mut decoded = B::default();

  Ok(match decoding::hexadecimal(body, &mut decoded) {
    Ok(()) => Some(decoded),
    Err(_invalid_hexadecimal_data) => None,
  })
}

#[inline(always)]
pub(crate) fn decode_hex_digit(
  c: u8,
) -> core::result::Result<Option<u8>, console_core_never::Never> {
  Ok(match c {
    b'0'..=b'9' => Some(u8::wrapping_sub(c, b'0')),
    b'A'..=b'F' => Some(u8::wrapping_add(u8::wrapping_sub(c, b'A'), 10)),
    b'a'..=b'f' => Some(u8::wrapping_add(u8::wrapping_sub(c, b'a'), 10)),
    _ => None,
  })
}

#[cfg(test)]
mod tests {
  use crate::filter::ascii_hex::decode;

  #[test]
  fn decode_simple() {
    let input = b"AF3E2901>";
    assert_eq!(decode(input), Ok(Some(alloc::vec![0xaf, 0x3e, 0x29, 0x01])));
  }

  #[test]
  fn decode_whitespaces() {
    let input = b"AF3   E2   901>";
    assert_eq!(decode(input), Ok(Some(alloc::vec![0xaf, 0x3e, 0x29, 0x01])));
  }

  #[test]
  fn decode_without_gt() {
    let input = b"AF3E2901";
    assert_eq!(decode(input), Ok(Some(alloc::vec![0xaf, 0x3e, 0x29, 0x01])));
  }

  #[test]
  fn decode_with_padding() {
    let input = b"AF3E291>";
    assert_eq!(decode(input), Ok(Some(alloc::vec![0xaf, 0x3e, 0x29, 0x10])));
  }
}
