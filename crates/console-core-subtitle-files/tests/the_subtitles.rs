use std::path::Path;

use console_core_subtitle_files::{Cue, Written, read, said, showing};

macro_rules! cue {
    ($from:expr, $until:expr, [$($line:expr),*]) => {
        Cue { from: $from, until: $until, lines: vec![$(String::from($line)),*] }
    };
}

#[test]
fn a_subrip_file_is_its_blocks_with_the_markup_taken_off() {
    let file = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,500\r\n<i>Hello</i> there,\r\n{\\an8}General Kenobi\r\n\r\n2\r\n01:00:00,000 --> 01:00:01,000\r\nfish &amp; chips\r\n";

    assert_eq!(
        read(file.as_bytes(), Written::SubRip),
        Ok(vec![cue!(1.0, 2.5, ["Hello there,", "General Kenobi"]), cue!(3600.0, 3601.0, ["fish & chips"])])
    );
}

#[test]
fn a_webvtt_file_skips_its_header_and_notes_and_reads_short_moments() {
    let file = "WEBVTT\n\nNOTE written by hand\n\nintro\n00:01.000 --> 00:02.000 align:start\n<v Bob>Hi &lt;3</v>\n\n00:00:03.000 --> 00:00:04.000\n<c.yellow>Bye</c>\n";

    assert_eq!(read(file.as_bytes(), Written::WebVtt), Ok(vec![cue!(1.0, 2.0, ["Hi <3"]), cue!(3.0, 4.0, ["Bye"])]));
}

#[test]
fn an_ass_file_reads_its_columns_from_its_format_line_and_keeps_commas_in_the_words() {
    let file = "[Script Info]\nTitle: x\n\n[V4+ Styles]\nFormat: Name, Fontname\nStyle: Default,Arial\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.50,0:00:03.00,Default,,0,0,0,,{\\i1}Well,{\\i0} hello\\Nthere\\hfriend\nComment: 0,0:00:05.00,0:00:06.00,Default,,0,0,0,,not shown\n";

    assert_eq!(read(file.as_bytes(), Written::Ass), Ok(vec![cue!(1.5, 3.0, ["Well, hello", "there friend"])]));
}

#[test]
fn a_block_out_of_a_film_is_its_words_alone() {
    assert_eq!(said("0,0,Default,,0,0,0,,Hi, you\\Nthere", Written::Ass), Ok(vec![String::from("Hi, you"), String::from("there")]));
    assert_eq!(said("<b>Hi</b>\nthere", Written::SubRip), Ok(vec![String::from("Hi"), String::from("there")]));
}

#[test]
fn a_file_written_before_utf8_keeps_its_accents() {
    let file = b"1\n00:00:01,000 --> 00:00:02,000\nCaf\xe9\n";

    assert_eq!(read(file, Written::SubRip), Ok(vec![cue!(1.0, 2.0, ["Café"])]));
}

#[test]
fn what_shows_is_every_line_due_at_that_moment_and_nothing_after_its_end() {
    let cues = [cue!(1.0, 3.0, ["first"]), cue!(2.0, 4.0, ["second"])];

    assert_eq!(showing(&cues, 0.5), Ok(Vec::new()));
    assert_eq!(showing(&cues, 2.5), Ok(vec![String::from("first"), String::from("second")]));
    assert_eq!(showing(&cues, 3.0), Ok(vec![String::from("second")]));
    assert_eq!(showing(&cues, 4.0), Ok(Vec::new()));
}

#[test]
fn the_ending_says_how_it_is_written() {
    assert_eq!(Written::of(Path::new("/x/film.en.SRT")), Ok(Some(Written::SubRip)));
    assert_eq!(Written::of(Path::new("/x/film.vtt")), Ok(Some(Written::WebVtt)));
    assert_eq!(Written::of(Path::new("/x/film.ssa")), Ok(Some(Written::Ass)));
    assert_eq!(Written::of(Path::new("/x/film.mkv")), Ok(None));
}
