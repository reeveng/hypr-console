//! Bytes that were meant as text, read as text when they are not UTF-8.
//!
//! The compositor hands back a window's title as the bytes the window set, and
//! nearly every window sets UTF-8. The ones that do not learned text before it
//! did -- a Java program writing in its platform's charset, an X client naming
//! itself in `WM_NAME`, whose `STRING` the X protocol says is Latin-1 -- and one
//! of them was enough to lose the whole answer: `serde_json` refuses a document
//! with a byte in it that is not UTF-8, so every window on the desktop went
//! missing for the sake of one title.
//!
//! **What is decided is a unit, never a byte.** A unit is what one writer
//! wrote -- a title, a line, a file -- and a writer writes in one encoding, so
//! a unit is read one way: as UTF-8 when it is UTF-8, and otherwise as
//! windows-1252, all of it. Both other sizes were tried, and both were wrong.
//! `hyprland-rs` guesses one encoding for the whole of an answer, and an
//! answer is every window's title at once: the guess that reads the one Latin-1
//! title right reads every other window's `Inbox — Mail` as `Inbox â€” Mail`.
//! This crate's first version went the other way and read every byte that was
//! not UTF-8 where it stood, which is wrong inside a Latin-1 title instead:
//! `„Fuß“` is `ß` and `“` side by side, and those two bytes happen to spell a
//! letter of UTF-8, so the title came back with `ߓ` in the middle of it. ftfy
//! decides per string, the IRC bouncers per line, encoding_rs per document;
//! the size of the unit is the caller's to know, and `console-compositor`
//! splits an answer into its strings before it asks.
//!
//! **A unit that stops in the middle of a letter is UTF-8 that was cut short,**
//! when there is anything else to say it was UTF-8. A line cut at a byte limit
//! loses the end of its last letter, which leaves a unit that is not UTF-8 by
//! one letter and would be read as windows-1252 all through -- the fault
//! Twisted met in IRC lines that were measured before they were encoded. So
//! what comes before the cut is kept, and the letter that was lost is one
//! `U+FFFD`, which is what the Unicode standard and the WHATWG decoder both
//! write for it. What says the unit was UTF-8 is a letter before the cut that
//! took more than one byte, or more than one byte of the letter that was cut.
//! A single byte left at the end of plain ASCII is as likely to be the last
//! letter of a Latin-1 `Café`, and it is read as that: wrong, it costs one
//! letter where a cut would have cost one `U+FFFD`.
//!
//! **windows-1252 rather than Latin-1.** The two agree on every letter, and
//! where Latin-1 has thirty-two control characters nobody types, windows-1252
//! has the curly quotes, the dashes and the euro a program on Windows wrote,
//! which is why the WHATWG standard and encoding_rs read a label saying Latin-1
//! as windows-1252 too. The five bytes windows-1252 gives no letter are
//! `U+FFFD`. The web gives them the control characters of the same number, so
//! that a page decoded and encoded again comes back byte for byte; nothing here
//! is encoded again, and a control character shows nothing where `U+FFFD`
//! shows that something was there.
//!
//! **No byte that is not ASCII is ever read as ASCII.** Decoders that took an
//! overlong `C0 AF` for `/` let a path through every filter that was looking
//! for one, and the same promise is what makes this safe to run inside a
//! format: no reading here makes a quote, an escape, a newline or a control
//! character that the bytes did not already have, so a JSON string decoded in
//! place is still the same JSON string, and a line is still one line. Nothing
//! is dropped either -- every byte that is not read as itself is read as a
//! letter or as a `U+FFFD`, because a decoder that quietly loses a byte is one
//! that can join `scr` and `ipt` back together.
//!
//! **A file can say what it is written in, and a unit cannot.** A byte-order
//! mark is a file's business: encoding_rs's `decode` turns into a UTF-16
//! decoder on the two bytes `FF FE`, and a Latin-1 title may begin `ÿþ`. So
//! [`decode`] never looks for one and [`decode_file`] always does. Notepad
//! saves a subtitle file as UTF-16 behind `FF FE`, which read as bytes is `ÿþ`
//! and a null after every letter; half of a surrogate pair, and a last byte
//! with nothing to pair it with, are one `U+FFFD` each, and a pair that stops
//! after its first half is one letter cut short however many of its bytes are
//! left, as the WHATWG decoder and encoding_rs write them. A UTF-8 mark is
//! taken off and what follows is read like any unit, with or without one: a
//! mark is what the last program to save the file claimed, and a program that
//! puts the mark on without converting what is under it is common enough that
//! the bytes are believed over it. What that costs is a UTF-8 file with
//! windows-1252 lines pasted into it, which reads as windows-1252 throughout; a
//! file that two writers wrote is two units, and only its reader can split it.
//!
//! **What is not here, on purpose.** There is no reading in pieces, because
//! the decision is about the whole of a unit and cannot be made before its end
//! -- `hyprland-rs` decoded each 4096 bytes of the event socket as it came, so
//! an event that straddled two reads was two broken ones. CESU-8, the surrogate
//! pairs MySQL and Java's serialization write, is one ftfy tries before
//! windows-1252; nothing that names a window writes it, so it reads as
//! windows-1252 here, which is a garbled title rather than a lost one. And
//! Latin-1 that happens to be UTF-8 throughout -- `Ã©` is the same two bytes as
//! `é` -- is UTF-8, because nothing short of a guess can say otherwise and
//! nearly everything is.
//!
//! What comes back says which way the unit was read, as encoding_rs's does, for
//! the caller that wants to mark a name nobody wrote in UTF-8. Valid UTF-8 comes
//! back borrowed, so the case that is nearly every case costs nothing.

use std::borrow::Cow;
use std::str::Utf8Error;

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
  Utf8,
  TruncatedUtf8,
  Windows1252,
  Utf16LittleEndian,
  Utf16BigEndian,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded<'a> {
  pub text: Cow<'a, str>,
  pub encoding: Encoding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Evidence {
  Utf8,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteOrder {
  LittleEndian,
  BigEndian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeftOver {
  None,
  TheRestOfTheLastLetter,
  AByteOfItsOwn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
  Utf8,
  Utf16(ByteOrder),
}

const MARKS: [(&[u8], Mark); 3] = [
  (b"\xEF\xBB\xBF", Mark::Utf8),
  (b"\xFF\xFE", Mark::Utf16(ByteOrder::LittleEndian)),
  (b"\xFE\xFF", Mark::Utf16(ByteOrder::BigEndian)),
];

pub fn decode_file(file: &[u8]) -> Result<Decoded<'_>, Never> {
  let marked = MARKS
    .iter()
    .find_map(|(mark, says)| file.strip_prefix(*mark).map(|after| (*says, after)));

  match marked {
    Some((Mark::Utf8, after)) => decode(after),
    Some((Mark::Utf16(order), after)) => utf_16(after, order),
    None => decode(file),
  }
}

fn utf_16(after: &[u8], order: ByteOrder) -> Result<Decoded<'_>, Never> {
  let (pairs, left_over) = after.as_chunks::<2>();
  let mut text = String::with_capacity(after.len());

  let unit_of = |pair: &[u8; 2]| match order {
    ByteOrder::LittleEndian => u16::from_le_bytes(*pair),
    ByteOrder::BigEndian => u16::from_be_bytes(*pair),
  };

  for unit in char::decode_utf16(pairs.iter().map(unit_of)) {
    text.push(match unit {
      Ok(letter) => letter,
      Err(_half_a_pair) => char::REPLACEMENT_CHARACTER,
    });
  }

  let left = match (pairs.last().map(unit_of), left_over) {
    (Some(_) | None, []) => LeftOver::None,
    (Some(0xD800..=0xDBFF), [_unpaired, ..]) => LeftOver::TheRestOfTheLastLetter,
    (Some(_) | None, [_unpaired, ..]) => LeftOver::AByteOfItsOwn,
  };

  match left {
    LeftOver::None | LeftOver::TheRestOfTheLastLetter => {}
    LeftOver::AByteOfItsOwn => text.push(char::REPLACEMENT_CHARACTER),
  }

  let encoding = match order {
    ByteOrder::LittleEndian => Encoding::Utf16LittleEndian,
    ByteOrder::BigEndian => Encoding::Utf16BigEndian,
  };

  Ok(Decoded {
    text: Cow::Owned(text),
    encoding,
  })
}

pub fn decode(unit: &[u8]) -> Result<Decoded<'_>, Never> {
  let fault = match std::str::from_utf8(unit) {
    Ok(text) => {
      return Ok(Decoded {
        text: Cow::Borrowed(text),
        encoding: Encoding::Utf8,
      });
    }
    Err(fault) => fault,
  };

  match fault.error_len() {
    Some(_not_utf8_before_the_end) => windows_1252_throughout(unit),
    None => cut_short(unit, fault),
  }
}

fn cut_short(unit: &[u8], fault: Utf8Error) -> Result<Decoded<'_>, Never> {
  let (whole, cut) = match unit.split_at_checked(fault.valid_up_to()) {
    Some(halves) => halves,
    None => return windows_1252_throughout(unit),
  };

  let before = match std::str::from_utf8(whole) {
    Ok(before) => before,
    Err(_not_utf8_after_all) => return windows_1252_throughout(unit),
  };

  let evidence = match (before.is_ascii(), cut) {
    (true, [] | [_]) => Evidence::None,
    (true, [_, _, ..]) | (false, _) => Evidence::Utf8,
  };

  match evidence {
    Evidence::Utf8 => {
      let mut text = String::with_capacity(unit.len());

      text.push_str(before);
      text.push(char::REPLACEMENT_CHARACTER);

      Ok(Decoded {
        text: Cow::Owned(text),
        encoding: Encoding::TruncatedUtf8,
      })
    }
    Evidence::None => windows_1252_throughout(unit),
  }
}

fn windows_1252_throughout(unit: &[u8]) -> Result<Decoded<'_>, Never> {
  let mut text = String::with_capacity(unit.len());

  for byte in unit {
    let Ok(letter) = windows_1252(*byte);

    text.push(letter);
  }

  Ok(Decoded {
    text: Cow::Owned(text),
    encoding: Encoding::Windows1252,
  })
}

fn windows_1252(byte: u8) -> Result<char, Never> {
  Ok(match byte {
    0x80 => '\u{20AC}',
    0x82 => '\u{201A}',
    0x83 => '\u{0192}',
    0x84 => '\u{201E}',
    0x85 => '\u{2026}',
    0x86 => '\u{2020}',
    0x87 => '\u{2021}',
    0x88 => '\u{02C6}',
    0x89 => '\u{2030}',
    0x8A => '\u{0160}',
    0x8B => '\u{2039}',
    0x8C => '\u{0152}',
    0x8E => '\u{017D}',
    0x91 => '\u{2018}',
    0x92 => '\u{2019}',
    0x93 => '\u{201C}',
    0x94 => '\u{201D}',
    0x95 => '\u{2022}',
    0x96 => '\u{2013}',
    0x97 => '\u{2014}',
    0x98 => '\u{02DC}',
    0x99 => '\u{2122}',
    0x9A => '\u{0161}',
    0x9B => '\u{203A}',
    0x9C => '\u{0153}',
    0x9E => '\u{017E}',
    0x9F => '\u{0178}',
    0x81 | 0x8D | 0x8F | 0x90 | 0x9D => char::REPLACEMENT_CHARACTER,
    latin_1 => char::from(latin_1),
  })
}
