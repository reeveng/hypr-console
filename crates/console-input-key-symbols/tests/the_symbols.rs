use console_input_key_symbols::{Keysym, Region, syms};

#[test]
fn names_and_aliases_resolve_without_a_keymap() {
  assert_eq!(Keysym::from_str("Return"), Ok(Some(Keysym::Return)));
  assert_eq!(Keysym::from_str("quoteright"), Ok(Some(Keysym::apostrophe)));
  assert_eq!(
    Keysym::from_str("XF86_AudioMute"),
    Ok(Some(Keysym::XF86AudioMute))
  );
  assert_eq!(syms::Return, Keysym::Return);
  assert_eq!(Keysym::Return.char(), Ok(Some('\r')));
}

#[test]
fn characters_and_casing_cover_legacy_and_unicode_symbols() {
  assert_eq!(Keysym::from_char('a'), Ok(Keysym::a));
  assert_eq!(Keysym::a.to_uppercase(), Ok(Keysym::A));
  assert_eq!(Keysym::from_char('α'), Ok(Keysym::Greek_alpha));
  assert_eq!(Keysym::Greek_alpha.to_uppercase(), Ok(Keysym::Greek_ALPHA));
  let Ok(emoji) = Keysym::from_char('😀');
  assert_eq!(Keysym::from_str("U1f600"), Ok(Some(emoji)));
  assert_eq!(emoji.char(), Ok(Some('😀')));
  assert_eq!(Keysym::KP_1.region(), Ok(Region::Keypad));
  assert_eq!(Keysym::_1.region(), Ok(Region::Main));
}

#[test]
fn unknown_names_are_rejected_after_the_hash_candidate_is_checked() {
  for name in [
    "",
    "return",
    "a-name-with-no-symbol",
    "Return ",
    "0x100000000",
    "U110000",
  ] {
    assert_eq!(Keysym::from_str(name), Ok(None), "{name}");
  }

  assert_eq!(
    Keysym::from_str("0xdeadbeef"),
    Ok(Some(Keysym(0xdead_beef)))
  );
}
