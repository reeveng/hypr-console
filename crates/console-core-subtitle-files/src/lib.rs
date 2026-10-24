//! A subtitle file, read into what it says and when.
//!
//! libass, inside ffmpeg, drew a film's subtitles into its frames, in the
//! fonts and places the file asked for. The frames come out of this desktop's
//! own decoders now, and the words go over them the way every other word on a
//! card does: in the card's own letters, at the foot of the picture. So what is
//! wanted out of a subtitle file is the lines and the moments between which
//! they show, and nothing of how they were styled -- a colour, a font, a place
//! on the screen and a karaoke sweep are all dropped, and every line keeps its
//! words and where it breaks.
//!
//! Three ways of writing them are what a film arrives with. SubRip is a
//! numbered block of two moments and some lines, WebVTT is the same block
//! with a header and its own markup, and ASS -- SSA before it -- is a table
//! whose `Format` line says which column is which, with the words last. A
//! Matroska film carries its tracks in the same three, one line to a block,
//! with the moments taken off and kept by the container, so [`said`] reads one
//! block's words without them.
//!
//! A file that is not UTF-8 is read as Latin-1, which is what a subtitle file
//! written before UTF-8 nearly always is; reading it as UTF-8 would turn every
//! accented letter into a question mark.

use std::path::Path;

use console_core_never::Never;
use console_core_number_conversion::index;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    SubRip,
    WebVtt,
    Ass,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub from: f64,
    pub until: f64,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inside {
    Yes,
    No,
}

const STANDARD_EVENTS: [&str; 10] = ["layer", "start", "end", "style", "name", "marginl", "marginr", "marginv", "effect", "text"];

const FIELDS_IN_A_BLOCK: u32 = 9;

impl Written {
    pub fn of(at: &Path) -> Result<Option<Written>, Never> {
        let ending = at.extension().map(|ending| ending.to_string_lossy().to_lowercase());

        Ok(match ending.as_deref() {
            Some("srt") => Some(Written::SubRip),
            Some("vtt") => Some(Written::WebVtt),
            Some("ass" | "ssa") => Some(Written::Ass),
            Some(_another) => None,
            None => None,
        })
    }
}

pub fn read(bytes: &[u8], written: Written) -> Result<Vec<Cue>, Never> {
    let Ok(text) = text_of(bytes);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    match written {
        Written::SubRip | Written::WebVtt => timed_blocks(&text, written),
        Written::Ass => events(&text),
    }
}

pub fn said(block: &str, written: Written) -> Result<Vec<String>, Never> {
    match written {
        Written::SubRip | Written::WebVtt => words(block, written),
        Written::Ass => {
            let Ok(fields) = index(FIELDS_IN_A_BLOCK);

            match block.splitn(fields, ',').last() {
                Some(text) => words(text, written),
                None => Ok(Vec::new()),
            }
        },
    }
}

pub fn showing(cues: &[Cue], at: f64) -> Result<Vec<String>, Never> {
    Ok(cues.iter().filter(|cue| cue.from <= at && at < cue.until).flat_map(|cue| cue.lines.iter().cloned()).collect())
}

fn text_of(bytes: &[u8]) -> Result<String, Never> {
    let bytes = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
        Some(after) => after,
        None => bytes,
    };

    Ok(match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_written_before_utf8) => bytes.iter().map(|byte| char::from(*byte)).collect(),
    })
}

fn timed_blocks(text: &str, written: Written) -> Result<Vec<Cue>, Never> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cues = Vec::new();

    for block in lines.split(|line| line.trim().is_empty()) {
        let Ok(cue) = timed_block(block, written);

        cues.extend(cue);
    }

    Ok(cues)
}

fn timed_block(block: &[&str], written: Written) -> Result<Option<Cue>, Never> {
    let mut after = block.iter().skip_while(|line| !line.contains("-->"));

    let timing = match after.next() {
        Some(timing) => timing,
        None => return Ok(None),
    };

    let (from, until) = match timing.split_once("-->") {
        Some((from, rest)) => (from, rest.split_whitespace().next()),
        None => return Ok(None),
    };

    let Ok(from) = moment(from);
    let Ok(until) = until.map_or(Ok(None), moment);
    let text: Vec<&str> = after.copied().collect();
    let Ok(lines) = words(&text.join("\n"), written);

    Ok(match (from, until) {
        (Some(from), Some(until)) => Some(Cue { from, until, lines }),
        (None, _) | (_, None) => None,
    })
}

fn events(text: &str) -> Result<Vec<Cue>, Never> {
    let listed = match text.split_once("[Events]") {
        Some((_, after)) => after,
        None => return Ok(Vec::new()),
    };

    let listed = match listed.split_once("\n[") {
        Some((listed, _the_next_section)) => listed,
        None => listed,
    };

    let named = listed.lines().find_map(|line| line.trim().strip_prefix("Format:"));

    let names: Vec<String> = match named {
        Some(named) => named.split(',').map(|name| name.trim().to_lowercase()).collect(),
        None => STANDARD_EVENTS.iter().map(|name| (*name).to_string()).collect(),
    };

    let mut cues = Vec::new();

    for line in listed.lines() {
        let Ok(cue) = dialogue(line.trim(), &names);

        cues.extend(cue);
    }

    Ok(cues)
}

fn dialogue(line: &str, names: &[String]) -> Result<Option<Cue>, Never> {
    let fields = match line.strip_prefix("Dialogue:") {
        Some(fields) => fields,
        None => return Ok(None),
    };

    let (mut from, mut until, mut text) = (None, None, None);

    for (name, field) in names.iter().zip(fields.splitn(names.len(), ',')) {
        match name.as_str() {
            "start" => from = Some(field),
            "end" => until = Some(field),
            "text" => text = Some(field),
            _another_column => {},
        }
    }

    let Ok(from) = from.map_or(Ok(None), moment);
    let Ok(until) = until.map_or(Ok(None), moment);

    Ok(match (from, until, text) {
        (Some(from), Some(until), Some(text)) => {
            let Ok(lines) = words(text, Written::Ass);

            Some(Cue { from, until, lines })
        },
        (None, _, _) | (_, None, _) | (_, _, None) => None,
    })
}

fn moment(said: &str) -> Result<Option<f64>, Never> {
    let said = said.trim().replace(',', ".");
    let mut parts = said.rsplit(':').map(str::parse::<f64>);

    Ok(match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(Ok(seconds)), Some(Ok(minutes)), None, None) => Some(minutes * 60.0 + seconds),
        (Some(Ok(seconds)), Some(Ok(minutes)), Some(Ok(hours)), None) => Some(hours * 3600.0 + minutes * 60.0 + seconds),
        (_, _, _, _) => None,
    })
}

fn words(text: &str, written: Written) -> Result<Vec<String>, Never> {
    let plain = match written {
        Written::SubRip | Written::WebVtt => {
            let Ok(untagged) = without(text, ('<', '>'));
            let Ok(unstyled) = without(&untagged, ('{', '}'));

            unstyled.replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ").replace("&lrm;", "").replace("&rlm;", "").replace("&amp;", "&")
        },
        Written::Ass => {
            let Ok(unstyled) = without(text, ('{', '}'));

            unstyled.replace("\\N", "\n").replace("\\n", " ").replace("\\h", " ")
        },
    };

    Ok(plain.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect())
}

fn without(text: &str, marks: (char, char)) -> Result<String, Never> {
    let (open, close) = marks;

    let (kept, _) = text.chars().fold((String::new(), Inside::No), |(mut kept, inside), letter| {
        let inside = match (inside, letter == open, letter == close) {
            (Inside::No, true, _) => Inside::Yes,
            (Inside::No, false, _) => {
                kept.push(letter);

                Inside::No
            },
            (Inside::Yes, _, true) => Inside::No,
            (Inside::Yes, _, false) => Inside::Yes,
        };

        (kept, inside)
    });

    Ok(kept)
}
