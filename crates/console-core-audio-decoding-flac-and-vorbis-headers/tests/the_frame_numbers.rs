use console_core_audio_decoding_flac_and_vorbis_headers::flac::read_coded_number;
use console_core_byte_streams::{BufReader, ReadBytes};

#[test]
fn extended_flac_numbers_are_not_limited_to_unicode() -> std::io::Result<()> {
  let Ok(mut stream) = BufReader::new(&[
    0xf8, 0x88, 0x80, 0x80, 0x80, 0xfc, 0x84, 0x80, 0x80, 0x80, 0x80, 0xfe, 0xbf, 0xbf, 0xbf,
    0xbf, 0xbf, 0xbf, 0x42,
  ]);

  let first = read_coded_number(&mut stream)?;
  let second = read_coded_number(&mut stream)?;
  let third = read_coded_number(&mut stream)?;
  let after = stream.read_u8()?;

  assert_eq!(first, Some(0x20_0000));
  assert_eq!(second, Some(0x400_0000));
  assert_eq!(third, Some(0xf_ffff_ffff));
  assert_eq!(after, 0x42);

  Ok(())
}

#[test]
fn a_truncated_number_is_distinct_from_an_invalid_leading_byte() -> std::io::Result<()> {
  let Ok(mut invalid) = BufReader::new(&[0xff, 0x80, 0xbf]);

  for _ in 0..3 {
    let read = read_coded_number(&mut invalid)?;

    assert_eq!(read, None);
  }

  let Ok(mut truncated) = BufReader::new(&[0xe2, 0x82]);
  let kind = match read_coded_number(&mut truncated) {
    Ok(read) => {
      return Err(std::io::Error::other(format!(
        "read {read:?} from a truncated number"
      )));
    }
    Err(fault) => fault.kind(),
  };

  assert_eq!(kind, std::io::ErrorKind::UnexpectedEof);

  Ok(())
}

#[test]
fn ordinary_numbers_and_stray_bytes_keep_their_meanings() -> std::io::Result<()> {
  let Ok(mut stream) = BufReader::new(&[
    0x24, 0xc2, 0xa2, 0xe0, 0xa4, 0xb9, 0xe2, 0x82, 0xac, 0xf0, 0x90, 0x8d, 0x88, 0xff, 0x80,
    0xbf,
  ]);

  for expected in [
    Some(36),
    Some(162),
    Some(2361),
    Some(8364),
    Some(66376),
    None,
    None,
    None,
  ] {
    let read = read_coded_number(&mut stream)?;

    assert_eq!(read, expected);
  }

  Ok(())
}
