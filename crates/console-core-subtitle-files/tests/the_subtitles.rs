use std::path::Path;

use console_core_subtitle_files::{Cue, Seconds, Written, read, said, showing};

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the fixture keeps its cue times and expected lines together while the macro constructs owned text and typed timestamps"
  )
)]
macro_rules! cue {
    ($from:expr, $until:expr, [$($line:expr),*]) => {
        Cue { from: Seconds($from), until: Seconds($until), lines: vec![$(String::from($line)),*] }
    };
}

#[test]
fn a_subrip_file_is_its_blocks_with_the_markup_taken_off() {
  let file = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,500\r\n<i>Hello</i> there,\r\n{\\an8}General Kenobi\r\n\r\n2\r\n01:00:00,000 --> 01:00:01,000\r\nfish &amp; chips\r\n";

  assert_eq!(
    read(file.as_bytes(), Written::SubRip),
    Ok(vec![
      cue!(1.0, 2.5, ["Hello there,", "General Kenobi"]),
      cue!(3600.0, 3601.0, ["fish & chips"])
    ])
  );
}

#[test]
fn a_webvtt_file_skips_its_header_and_notes_and_reads_short_moments() {
  let file = "WEBVTT\n\nNOTE written by hand\n\nintro\n00:01.000 --> 00:02.000 align:start\n<v Bob>Hi &lt;3</v>\n\n00:00:03.000 --> 00:00:04.000\n<c.yellow>Bye</c>\n";

  assert_eq!(
    read(file.as_bytes(), Written::WebVtt),
    Ok(vec![cue!(1.0, 2.0, ["Hi <3"]), cue!(3.0, 4.0, ["Bye"])])
  );
}

#[test]
fn an_ass_file_reads_its_columns_from_its_format_line_and_keeps_commas_in_the_words() {
  let file = "[Script Info]\nTitle: x\n\n[V4+ Styles]\nFormat: Name, Fontname\nStyle: Default,Arial\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.50,0:00:03.00,Default,,0,0,0,,{\\i1}Well,{\\i0} hello\\Nthere\\hfriend\nComment: 0,0:00:05.00,0:00:06.00,Default,,0,0,0,,not shown\n";

  assert_eq!(
    read(file.as_bytes(), Written::Ass),
    Ok(vec![cue!(1.5, 3.0, ["Well, hello", "there friend"])])
  );
}

#[test]
fn a_block_out_of_a_film_is_its_words_alone() {
  assert_eq!(
    said(b"0,0,Default,,0,0,0,,Hi, you\\Nthere", Written::Ass),
    Ok(vec![String::from("Hi, you"), String::from("there")])
  );
  assert_eq!(
    said(b"<b>Hi</b>\nthere", Written::SubRip),
    Ok(vec![String::from("Hi"), String::from("there")])
  );
}

#[test]
fn a_block_out_of_a_film_written_before_utf8_keeps_its_accents() {
  assert_eq!(
    said(b"0,0,Default,,0,0,0,,Caf\xe9, cr\xe8me", Written::Ass),
    Ok(vec![String::from("Café, crème")])
  );
  assert_eq!(
    said(b"<i>d\xe9j\xe0 vu</i>", Written::SubRip),
    Ok(vec![String::from("déjà vu")])
  );
}

#[test]
fn a_file_written_before_utf8_keeps_its_accents() {
  let file = b"1\n00:00:01,000 --> 00:00:02,000\nCaf\xe9\n";

  assert_eq!(
    read(file, Written::SubRip),
    Ok(vec![cue!(1.0, 2.0, ["Café"])])
  );
}

#[test]
fn a_file_written_on_windows_keeps_its_curly_quotes_and_dashes() {
  let file = b"1\n00:00:01,000 --> 00:00:02,000\nI don\x92t know \x96 \x93maybe\x94\x85\n";

  assert_eq!(
    read(file, Written::SubRip),
    Ok(vec![cue!(1.0, 2.0, ["I don’t know – “maybe”…"])])
  );
}

#[test]
fn a_file_notepad_saved_as_unicode_is_read_into_its_cues() {
  let said = "1\r\n00:00:01,000 --> 00:00:02,500\r\nCafé, don’t\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\n🎮 \u{4F60}\u{597D}\r\n";
  let little: Vec<u8> = b"\xff\xfe"
    .iter()
    .copied()
    .chain(said.encode_utf16().flat_map(u16::to_le_bytes))
    .collect();
  let big: Vec<u8> = b"\xfe\xff"
    .iter()
    .copied()
    .chain(said.encode_utf16().flat_map(u16::to_be_bytes))
    .collect();
  let cues = vec![
    cue!(1.0, 2.5, ["Café, don’t"]),
    cue!(3.0, 4.0, ["🎮 \u{4F60}\u{597D}"]),
  ];

  assert_eq!(read(&little, Written::SubRip), Ok(cues.clone()));
  assert_eq!(read(&big, Written::SubRip), Ok(cues));
}

#[test]
fn a_file_marked_utf8_is_read_without_its_mark() {
  let file = b"\xef\xbb\xbf1\n00:00:01,000 --> 00:00:02,000\nd\xc3\xa9j\xc3\xa0 vu\n";

  assert_eq!(
    read(file, Written::SubRip),
    Ok(vec![cue!(1.0, 2.0, ["déjà vu"])])
  );
}

#[test]
fn what_shows_is_every_line_due_at_that_moment_and_nothing_after_its_end() {
  let cues = [cue!(1.0, 3.0, ["first"]), cue!(2.0, 4.0, ["second"])];

  assert_eq!(showing(&cues, Seconds(0.5)), Ok(Vec::new()));
  assert_eq!(
    showing(&cues, Seconds(2.5)),
    Ok(vec![String::from("first"), String::from("second")])
  );
  assert_eq!(
    showing(&cues, Seconds(3.0)),
    Ok(vec![String::from("second")])
  );
  assert_eq!(showing(&cues, Seconds(4.0)), Ok(Vec::new()));
}

#[test]
fn the_ending_says_how_it_is_written() {
  assert_eq!(
    Written::of(Path::new("/x/film.en.SRT")),
    Ok(Some(Written::SubRip))
  );
  assert_eq!(
    Written::of(Path::new("/x/film.vtt")),
    Ok(Some(Written::WebVtt))
  );
  assert_eq!(
    Written::of(Path::new("/x/film.ssa")),
    Ok(Some(Written::Ass))
  );
  assert_eq!(Written::of(Path::new("/x/film.mkv")), Ok(None));
}
