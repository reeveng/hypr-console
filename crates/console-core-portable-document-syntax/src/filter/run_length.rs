use crate::reader::Reader;
use alloc::vec::Vec;
use console_core_number_conversion::index;
use core::iter::repeat_n;

pub(crate) fn decode(
  data: &[u8],
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  let Ok(mut reader) = Reader::new(data);
  let mut decoded = Vec::new();

  for _ in data {
    let Ok(control) = reader.read_byte();

    match control {
      Some(128) => return Ok(Some(decoded)),
      Some(length @ 0..=127) => {
        let Ok(count) = index(u16::from(length).wrapping_add(1));
        let Ok(literal) = reader.read_bytes(count);

        match literal {
          Some(bytes) => decoded.extend_from_slice(bytes),
          None => return Ok(Some(decoded)),
        }
      }
      Some(length @ 129..=255) => {
        let Ok(byte) = reader.read_byte();
        let value = match byte {
          Some(byte_2) => byte_2,
          None => return Ok(None),
        };
        let Ok(count) = index(257_u16.wrapping_sub(u16::from(length)));
        decoded.extend(repeat_n(value, count));
      }
      None => return Ok(None),
    }
  }

  Ok(None)
}

#[cfg(test)]
mod tests {
  use crate::filter::run_length::decode;

  #[test]
  fn run_length() {
    let input = alloc::vec![4, 10, 11, 12, 13, 14, 253, 3, 128];
    assert_eq!(
      decode(&input),
      Ok(Some(alloc::vec![10, 11, 12, 13, 14, 3, 3, 3, 3]))
    );
  }
}
