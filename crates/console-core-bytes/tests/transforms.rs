use console_core_bytes::{Cursor, Truncated, endian, hex};

#[test]
fn checked_reads_leave_the_tail_available_for_retry() -> Result<(), Truncated> {
  let Ok(mut cursor) = Cursor::new(b"\x01\x02name\0");
  let opening = cursor.u32_be()?;
  assert_eq!(opening, 0x0102_6e61);
  assert_eq!(cursor.u32_be(), Err(Truncated));
  assert_eq!(cursor.take(u64::MAX), Err(Truncated));
  assert_eq!(cursor.take_until(b'!'), Err(Truncated));
  let name = cursor.take_until(0)?;
  assert_eq!(name, b"me");
  assert_eq!(cursor.byte(), Err(Truncated));

  Ok(())
}

#[test]
fn both_orders_sign_extend_24_bit_values() -> Result<(), console_core_never::Never> {
  let positive = [0x7f, 0xff, 0xff];
  let negative = [0x80, 0, 0];

  assert_eq!(endian::i24_be(positive), Ok(8_388_607));
  assert_eq!(endian::i24_be(negative), Ok(-8_388_608));
  assert_eq!(endian::i24_le([0xff, 0xff, 0x7f]), Ok(8_388_607));
  assert_eq!(endian::i24_le([0, 0, 0x80]), Ok(-8_388_608));
  assert_eq!(endian::u24_be(negative), Ok(8_388_608));
  assert_eq!(endian::u24_le([0, 0, 0x80]), Ok(8_388_608));

  Ok(())
}

#[test]
fn hexadecimal_pairs_require_two_ascii_digits() -> Result<(), hex::InvalidDigit> {
  let byte = hex::decode_pair(*b"fA")?;
  assert_eq!(byte, 0xfa);
  assert_eq!(hex::decode_pair(*b"+a"), Err(hex::InvalidDigit(b'+')));
  assert_eq!(hex::encode_byte(0x0a, hex::Case::Upper), Ok(*b"0A"));

  Ok(())
}

#[cfg(feature = "alloc")]
mod allocation {
  use console_core_bytes::{Bytes, base64, hex};

  #[test]
  fn padded_base64_handles_multiple_output_blocks() -> Result<(), base64::InvalidBase64> {
    let input: Vec<_> = (0u16..1025)
      .map(|value| {
        let [low, _high] = value.to_le_bytes();
        low
      })
      .collect();
    let Ok(encoded) = base64::display(&input);
    let spelling = encoded.to_string();
    let decoded = base64::decode(spelling.as_bytes())?;
    let with_spaces = base64::decode(b"Zg ==\r\n")?;
    let without_padding = base64::decode_unpadded(b"Zg")?;

    assert_eq!(decoded.as_ref(), input.as_slice());
    assert_eq!(with_spaces.as_ref(), b"f");
    assert_eq!(without_padding.as_ref(), b"f");
    assert!(matches!(base64::decode_unpadded(b"Zg="), Err(_has_padding)));
    assert!(matches!(base64::decode_unpadded(b"Z g"), Err(_has_space)));

    let Ok(short) = base64::display(&[0xfb, 0xff]);
    assert_eq!(short.to_string(), "+/8=");

    Ok(())
  }

  #[test]
  fn owned_payloads_transfer_storage_and_keep_invalid_utf8() -> Result<(), hex::DecodeError> {
    let original = vec![0, 0xff, 0xf0];
    let storage = original.as_ptr();
    let owned = Bytes::from(original);
    let Ok(encoded) = hex::encode(&owned, hex::Case::Lower);
    let decoded = hex::decode(encoded.as_bytes())?;

    assert_eq!(encoded, "00fff0");
    assert_eq!(decoded, owned);
    assert_eq!(hex::decode(b"abc"), Err(hex::DecodeError::OddLength));
    assert!(matches!(decoded.into_utf8(), Err(_invalid_text)));

    let Ok(raw) = owned.into_vec();
    assert_eq!(raw.as_ptr(), storage);

    Ok(())
  }
}
