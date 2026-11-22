use console_core_font_encodings::{mac_os_roman, mac_roman, standard, symbol, win_ansi};
use console_core_glyph_names::{get, get_reverse};
use console_core_never::Never;

fn text(named: Result<Option<&'static str>, Never>) -> Result<Option<&'static str>, Never> {
  let Ok(found) = named;

  match found {
    Some(name) => get(name),
    None => Ok(None),
  }
}

fn code_for(name: &str) -> Result<Option<u8>, Never> {
  let Ok(supplemental) = mac_os_roman::get_inverse(name);

  match supplemental {
    Some(code) => Ok(Some(code)),
    None => mac_roman::get_inverse(name),
  }
}

fn name_for(code: u8) -> Result<Option<&'static str>, Never> {
  let Ok(roman) = mac_roman::get(code);

  match roman {
    Some(name) => Ok(Some(name)),
    None => mac_os_roman::get(code),
  }
}

fn round_trip(character: char) -> Result<Option<&'static str>, Never> {
  let Ok(named) = get_reverse(character);

  match named {
    Some(name) => {
      let Ok(coded) = code_for(name);

      match coded {
        Some(code) => text(name_for(code)),
        None => Ok(None),
      }
    }
    None => Ok(None),
  }
}

#[test]
fn encoded_characters_can_be_read_without_a_document_interpreter() {
  assert_eq!(text(standard::get(b'A')), Ok(Some("A")));
  assert_eq!(text(win_ansi::get(128)), Ok(Some("€")));
  assert_eq!(text(symbol::get(65)), Ok(Some("Α")));
}

#[test]
fn mac_roman_names_can_be_encoded_again() {
  assert_eq!(round_trip('é'), Ok(Some("é")));
  assert_eq!(round_trip('≠'), Ok(Some("≠")));
  assert_eq!(mac_os_roman::get_inverse("eacute"), Ok(None));
}

#[test]
fn undefined_codes_keep_each_encodings_fallback() {
  assert_eq!(standard::get(0), Ok(None));
  assert_eq!(win_ansi::get(0), Ok(None));
  assert_eq!(win_ansi::get(129), Ok(Some("bullet")));
  assert_eq!(get("a-name-that-is-not-a-glyph"), Ok(None));
  assert_eq!(get_reverse('\u{10ffff}'), Ok(None));
}
