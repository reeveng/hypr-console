use console_core_macintosh_roman::{decode, encode};

#[test]
fn every_byte_round_trips_through_unicode() {
  for code in 0u8..=u8::MAX {
    let Ok(unicode) = decode(code);
    let Ok(encoded) = encode(u32::from(unicode));

    assert_eq!(encoded, Some(code));
  }
}

#[test]
fn macintosh_symbols_and_extended_letters_keep_their_codes() {
  for (code, character) in [
    (0x11, '⌘'),
    (0x12, '⇧'),
    (0x13, '⌥'),
    (0x14, '⌃'),
    (0x80, 'Ä'),
    (0x8e, 'é'),
    (0xdb, '€'),
    (0xde, 'ﬁ'),
    (0xf0, '\u{f8ff}'),
  ] {
    let Ok(unicode) = decode(code);

    assert_eq!(u32::from(unicode), u32::from(character));
  }
}

#[test]
fn unrepresentable_codepoints_do_not_alias_their_low_sixteen_bits() {
  for unicode in [0x100c4, 0x1f600, 0x110000, u32::MAX] {
    let Ok(code) = encode(unicode);

    assert_eq!(code, None);
  }
}
