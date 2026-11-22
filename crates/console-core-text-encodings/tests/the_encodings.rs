use std::borrow::Cow;

use console_core_never::Never;
use console_core_text_encodings::{Decoded, Encoding, decode, decode_file};

fn read_as(text: &str, encoding: Encoding) -> Result<Decoded<'_>, Never> {
  Ok(Decoded {
    text: Cow::Borrowed(text),
    encoding,
  })
}

#[test]
fn text_that_is_utf8_is_handed_back_as_it_was_and_not_copied() {
  let title = "Inbox — Mail, déjà vu 🎮";
  let Ok(decoded) = decode(title.as_bytes());

  assert_eq!(decoded.text, title);
  assert_eq!(decoded.encoding, Encoding::Utf8);
  assert!(matches!(decoded.text, Cow::Borrowed(_)));
  assert_eq!(decode(b""), read_as("", Encoding::Utf8));
}

#[test]
fn a_title_written_in_latin_1_keeps_its_accents() {
  assert_eq!(
    decode(b"tradu\xe7\xe3ocoletiva.docx"),
    read_as("traduçãocoletiva.docx", Encoding::Windows1252)
  );
}

#[test]
fn a_unit_is_read_one_way_even_where_two_of_its_bytes_would_pass_for_utf8() {
  assert_eq!(
    decode(b"\x84Fu\xdf\x93"),
    read_as("„Fuß“", Encoding::Windows1252)
  );
}

#[test]
fn latin_1_that_happens_to_be_utf8_throughout_is_utf8() {
  assert_eq!(decode(b"CAF\xc9\x92S"), read_as("CAFɒS", Encoding::Utf8));
}

#[test]
fn a_program_on_windows_wrote_curly_quotes_and_a_euro_rather_than_control_characters() {
  assert_eq!(
    decode(b"don\x92t \x93quote\x94 \x80 5 \x96 \x85"),
    read_as("don’t “quote” € 5 – …", Encoding::Windows1252)
  );
}

#[test]
fn every_letter_windows_1252_has_above_ascii_is_the_one_the_standard_gives() {
  let given = [
    (0x80, '€'),
    (0x82, '‚'),
    (0x83, 'ƒ'),
    (0x84, '„'),
    (0x85, '…'),
    (0x86, '†'),
    (0x87, '‡'),
    (0x88, 'ˆ'),
    (0x89, '‰'),
    (0x8A, 'Š'),
    (0x8B, '‹'),
    (0x8C, 'Œ'),
    (0x8E, 'Ž'),
    (0x91, '‘'),
    (0x92, '’'),
    (0x93, '“'),
    (0x94, '”'),
    (0x95, '•'),
    (0x96, '–'),
    (0x97, '—'),
    (0x98, '˜'),
    (0x99, '™'),
    (0x9A, 'š'),
    (0x9B, '›'),
    (0x9C, 'œ'),
    (0x9E, 'ž'),
    (0x9F, 'Ÿ'),
    (0xA0, '\u{A0}'),
    (0xA9, '©'),
    (0xC0, 'À'),
    (0xDF, 'ß'),
    (0xE9, 'é'),
    (0xFF, 'ÿ'),
  ];

  for (byte, letter) in given {
    let alone = [byte];
    let Ok(decoded) = decode(&alone);

    assert_eq!(
      decoded.text.chars().collect::<Vec<char>>(),
      [letter],
      "{byte:#04x}"
    );
  }
}

#[test]
fn a_byte_windows_1252_has_no_letter_for_shows_that_something_was_there() {
  assert_eq!(
    decode(b"a\x81b\x8dc\x8fd\x90e\x9df"),
    read_as(
      "a\u{FFFD}b\u{FFFD}c\u{FFFD}d\u{FFFD}e\u{FFFD}f",
      Encoding::Windows1252
    )
  );
}

#[test]
fn utf8_cut_off_in_the_middle_of_its_last_letter_keeps_everything_before_the_cut() {
  assert_eq!(
    decode(b"Inbox \xe2\x80\x94 Mail\xe2\x80"),
    read_as("Inbox — Mail\u{FFFD}", Encoding::TruncatedUtf8)
  );
  assert_eq!(
    decode(b"d\xc3\xa9j\xc3"),
    read_as("déj\u{FFFD}", Encoding::TruncatedUtf8)
  );
  assert_eq!(
    decode(b"Play \xf0\x9f\x8e"),
    read_as("Play \u{FFFD}", Encoding::TruncatedUtf8)
  );
  assert_eq!(
    decode(b"Mail \xe2\x80"),
    read_as("Mail \u{FFFD}", Encoding::TruncatedUtf8)
  );
}

#[test]
fn one_accented_letter_at_the_end_of_plain_ascii_is_latin_1_rather_than_a_cut() {
  assert_eq!(decode(b"Caf\xe9"), read_as("Café", Encoding::Windows1252));
  assert_eq!(decode(b"\xe9"), read_as("é", Encoding::Windows1252));
}

#[test]
fn a_letter_cut_short_in_the_middle_is_not_a_cut_at_the_end() {
  assert_eq!(decode(b"\xe2\x82x"), read_as("â‚x", Encoding::Windows1252));
}

#[test]
fn an_overlong_or_a_surrogate_is_never_read_as_the_letter_it_spells() {
  assert_eq!(
    decode(b"..\xc0\xaf..\xc0\xaf"),
    read_as("..À¯..À¯", Encoding::Windows1252)
  );
  assert_eq!(decode(b"\xc0\x80"), read_as("À€", Encoding::Windows1252));
  assert_eq!(
    decode(b"\xe0\x80\xaf"),
    read_as("à€¯", Encoding::Windows1252)
  );
  assert_eq!(
    decode(b"\xed\xa0\xbd\xed\xb8\x80"),
    read_as("í\u{A0}½í¸€", Encoding::Windows1252)
  );
  assert_eq!(
    decode(b"\xf4\x90\x80\x80"),
    read_as("ô\u{FFFD}€€", Encoding::Windows1252)
  );
}

#[test]
fn a_unit_never_looks_for_a_byte_order_mark() {
  assert_eq!(
    decode(b"\xef\xbb\xbfhi"),
    read_as("\u{FEFF}hi", Encoding::Utf8)
  );
  assert_eq!(
    decode(b"\xff\xfeh\x00i\x00"),
    read_as("ÿþh\u{0}i\u{0}", Encoding::Windows1252)
  );
}

const SAID: &str = "1\r\n00:00:01,000 --> 00:00:02,000\r\nCafé, don’t 🎮\r\n";

#[test]
fn a_file_notepad_saved_as_unicode_is_read_as_utf_16() {
  let little: Vec<u8> = b"\xff\xfe"
    .iter()
    .copied()
    .chain(SAID.encode_utf16().flat_map(u16::to_le_bytes))
    .collect();
  let big: Vec<u8> = b"\xfe\xff"
    .iter()
    .copied()
    .chain(SAID.encode_utf16().flat_map(u16::to_be_bytes))
    .collect();

  assert_eq!(
    decode_file(&little),
    read_as(SAID, Encoding::Utf16LittleEndian)
  );
  assert_eq!(decode_file(&big), read_as(SAID, Encoding::Utf16BigEndian));
}

#[test]
fn half_a_surrogate_pair_and_a_last_byte_with_no_partner_are_one_replacement_each() {
  assert_eq!(
    decode_file(b"\xff\xfe\x3c\xd8a\x00b"),
    read_as("\u{FFFD}a\u{FFFD}", Encoding::Utf16LittleEndian)
  );
  assert_eq!(
    decode_file(b"\xfe\xff\xdf\xae\x00a"),
    read_as("\u{FFFD}a", Encoding::Utf16BigEndian)
  );
}

#[test]
fn a_file_that_stops_after_the_first_half_of_a_pair_and_a_byte_more_lost_one_letter() {
  assert_eq!(
    decode_file(b"\xff\xfea\x00\x3c\xd8\x8e"),
    read_as("a\u{FFFD}", Encoding::Utf16LittleEndian)
  );
  assert_eq!(
    decode_file(b"\xfe\xff\x00a\xd8\x3c\xdf"),
    read_as("a\u{FFFD}", Encoding::Utf16BigEndian)
  );
}

#[test]
fn a_file_marked_utf8_loses_the_mark_and_is_read_like_any_unit() {
  assert_eq!(
    decode_file(b"\xef\xbb\xbfCaf\xc3\xa9"),
    read_as("Café", Encoding::Utf8)
  );
  assert_eq!(
    decode_file(b"\xef\xbb\xbfCaf\xe9 cr\xe8me"),
    read_as("Café crème", Encoding::Windows1252)
  );
}

#[test]
fn a_mark_and_nothing_after_it_is_an_empty_file() {
  assert_eq!(decode_file(b"\xef\xbb\xbf"), read_as("", Encoding::Utf8));
  assert_eq!(
    decode_file(b"\xff\xfe"),
    read_as("", Encoding::Utf16LittleEndian)
  );
  assert_eq!(
    decode_file(b"\xfe\xff"),
    read_as("", Encoding::Utf16BigEndian)
  );
  assert_eq!(decode_file(b""), read_as("", Encoding::Utf8));
}

#[test]
fn a_file_with_no_mark_is_read_the_way_a_unit_is() {
  assert_eq!(
    decode_file(b"Caf\xe9"),
    read_as("Café", Encoding::Windows1252)
  );
  assert_eq!(
    decode_file(b"\xfeCaf\xe9"),
    read_as("þCafé", Encoding::Windows1252)
  );
}

const ALPHABET: [u8; 18] = [
  b'A', b'"', b'\\', b'/', b'\n', 0x00, 0x80, 0x93, 0xA9, 0xBF, 0xC0, 0xC3, 0xDF, 0xE2, 0xED,
  0xF0, 0xF4, 0xFF,
];

fn spelled_in_four_or_fewer() -> Result<Vec<Vec<u8>>, Never> {
  let (every, _longest) = [(); 4].iter().fold(
    (vec![Vec::new()], vec![Vec::new()]),
    |(mut every, longest), _round| {
      let longer: Vec<Vec<u8>> = longest
        .iter()
        .flat_map(|word| {
          ALPHABET
            .iter()
            .map(move |letter| [word.as_slice(), &[*letter]].concat())
        })
        .collect();

      every.extend_from_slice(&longer);

      (every, longer)
    },
  );

  Ok(every)
}

fn every_pair() -> Result<Vec<Vec<u8>>, Never> {
  Ok(
    (0..=u8::MAX)
      .flat_map(|first| (0..=u8::MAX).map(move |second| vec![first, second]))
      .collect(),
  )
}

fn holds(unit: &[u8]) -> Result<(), Never> {
  let Ok(decoded) = decode(unit);
  let ascii_in: String = unit
    .iter()
    .filter(|byte| byte.is_ascii())
    .map(|byte| char::from(*byte))
    .collect();
  let ascii_out: String = decoded.text.chars().filter(char::is_ascii).collect();

  assert_eq!(
    ascii_out, ascii_in,
    "{unit:x?} made or lost ASCII as {:?}",
    decoded.text
  );

  let agrees = match (decoded.encoding, std::str::from_utf8(unit)) {
    (Encoding::Utf8, Ok(text)) => decoded.text == text,
    (Encoding::TruncatedUtf8, Err(fault)) => {
      let kept = match unit.get(..fault.valid_up_to()).map(std::str::from_utf8) {
        Some(Ok(before)) => format!("{before}\u{FFFD}"),
        Some(Err(_not_utf8_before_the_cut)) => String::new(),
        None => String::new(),
      };

      fault.error_len().is_none() && kept == decoded.text
    }
    (Encoding::Windows1252, Err(_not_utf8)) => decoded.text.chars().count() == unit.len(),
    (Encoding::Utf8, Err(_not_utf8)) => false,
    (Encoding::TruncatedUtf8 | Encoding::Windows1252, Ok(_utf8)) => false,
    (Encoding::Utf16LittleEndian | Encoding::Utf16BigEndian, Ok(_utf8)) => false,
    (Encoding::Utf16LittleEndian | Encoding::Utf16BigEndian, Err(_not_utf8)) => false,
  };

  assert!(
    agrees,
    "{unit:x?} was read as {:?}, {:?}",
    decoded.encoding, decoded.text
  );

  let Ok(again) = decode(decoded.text.as_bytes());

  assert_eq!(again.text, decoded.text, "{unit:x?}");
  assert_eq!(again.encoding, Encoding::Utf8, "{unit:x?}");

  let marked = [b"\xef\xbb\xbf".as_slice(), unit].concat();

  assert_eq!(
    decode_file(&marked),
    Ok(decoded),
    "a UTF-8 mark changed how {unit:x?} reads"
  );

  Ok(())
}

#[test]
fn no_unit_made_of_the_difficult_bytes_loses_a_byte_or_gains_an_ascii_one() -> Result<(), Never> {
  let Ok(units) = spelled_in_four_or_fewer();

  for unit in &units {
    let Ok(()) = holds(unit);
  }

  Ok(())
}

#[test]
fn no_pair_of_bytes_loses_a_byte_or_gains_an_ascii_one() -> Result<(), Never> {
  let Ok(pairs) = every_pair();

  for pair in &pairs {
    let Ok(()) = holds(pair);
  }

  Ok(())
}
