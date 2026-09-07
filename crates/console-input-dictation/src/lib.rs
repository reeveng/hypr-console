//! Speaking instead of typing.
//!
//! A handheld has no keyboard, and the one it draws on the screen is a thumb
//! hunting for letters over half the picture. So the bottom left paddle takes
//! what is said and writes it into whatever holds the focus, which is the same
//! thing a keyboard does and none of the walking.
//!
//! Three programs already know how to do the parts of this. `pw-record` takes
//! the microphone, `whisper-cli` turns a recording into words, and `wtype`
//! writes them where a keyboard would have. What is here is the deciding: the
//! shape of each of those calls, where the recording waits between the two
//! presses, and what to do with what comes back.
//!
//! Nothing in this file touches a device or starts a program, so what would be
//! run can be asked for and looked at without a microphone in the room.

pub mod comparing;
pub mod languages;

use std::path::{Path, PathBuf};

use console_core_external_programs::Program;
use console_core_never::Never;

pub const MODEL: &str = "ggml-large-v3-turbo-q5_0.bin";

pub const MODEL_FROM: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";

pub const RATE: &str = "16000";

pub const THREADS: &str = "12";

fn runtime() -> Result<PathBuf, Never> {
    let at = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(said) if !said.is_empty() => said,
        Ok(_) | Err(_) => "/tmp".to_string(),
    };

    Ok(Path::new(&at).join("console").join("voice"))
}

pub fn kept() -> Result<PathBuf, Never> {
    let home = match std::env::var("HOME") {
        Ok(home) => home,
        Err(_) => "/tmp".to_string(),
    };

    let share = match std::env::var("XDG_DATA_HOME") {
        Ok(said) if !said.is_empty() => said,
        Ok(_) | Err(_) => format!("{home}/.local/share"),
    };

    Ok(Path::new(&share).join("console").join("voice"))
}

pub fn said(press: u32) -> Result<PathBuf, Never> {
    let Ok(runtime) = runtime();

    Ok(runtime.join(format!("said-{press}.wav")))
}

pub fn taking() -> Result<PathBuf, Never> {
    let Ok(runtime) = runtime();

    Ok(runtime.join("taking.pid"))
}

pub fn taken(recorder: u32, press: u32) -> Result<String, Never> {
    Ok(format!("{recorder} {press}"))
}

pub fn told_by(note: &str) -> Result<Option<(i32, u32)>, Never> {
    let mut words = note.split_whitespace();

    let Some(first) = words.next() else { return Ok(None) };

    let Ok(recorder) = first.parse() else {
        return Ok(None);
    };

    let Some(second) = words.next() else { return Ok(None) };

    let Ok(press) = second.parse() else {
        return Ok(None);
    };

    Ok(Some((recorder, press)))
}

pub fn model() -> Result<PathBuf, Never> {
    let Ok(kept) = kept();

    Ok(kept.join(MODEL))
}

pub const WHISPER_FROM: &str = "https://github.com/ggml-org/whisper.cpp";

pub const WHISPER_AT: &str = "v1.9.1";

pub fn whisper() -> Result<PathBuf, Never> {
    let Ok(kept) = kept();
    let Ok(whisper_cli) = Program::WhisperCli.name();

    Ok(kept.join(whisper_cli))
}

pub fn making() -> Result<PathBuf, Never> {
    let Ok(runtime) = runtime();

    Ok(runtime.join("whisper.cpp"))
}

pub fn cloning(into: &Path) -> Result<Vec<String>, Never> {
    let Ok(git) = Program::Git.name();

    Ok([
        git,
        "clone",
        "--quiet",
        "--depth",
        "1",
        "--branch",
        WHISPER_AT,
        WHISPER_FROM,
        &into.to_string_lossy(),
    ]
    .map(str::to_string)
    .to_vec())
}

pub fn configuring(at: &Path) -> Result<Vec<String>, Never> {
    let Ok(cmake) = Program::Cmake.name();

    Ok([
        cmake,
        "-S",
        &at.to_string_lossy(),
        "-B",
        &at.join("build").to_string_lossy(),
        "-DGGML_VULKAN=ON",
        "-DBUILD_SHARED_LIBS=OFF",
        "-DCMAKE_BUILD_TYPE=Release",
        "-DWHISPER_BUILD_TESTS=OFF",
    ]
    .map(str::to_string)
    .to_vec())
}

pub fn compiling(at: &Path) -> Result<Vec<String>, Never> {
    let Ok(cmake) = Program::Cmake.name();
    let Ok(whisper_cli) = Program::WhisperCli.name();

    Ok([
        cmake,
        "--build",
        &at.join("build").to_string_lossy(),
        "--target",
        whisper_cli,
        "--parallel",
    ]
    .map(str::to_string)
    .to_vec())
}

pub fn made(at: &Path) -> Result<PathBuf, Never> {
    let Ok(whisper_cli) = Program::WhisperCli.name();

    Ok(at.join("build").join("bin").join(whisper_cli))
}

pub fn recording(into: &Path) -> Result<Vec<String>, Never> {
    let Ok(pw_record) = Program::PwRecord.name();

    Ok([
        pw_record,
        "--rate",
        RATE,
        "--channels",
        "1",
        "--format",
        "s16",
        &into.to_string_lossy(),
    ]
    .map(str::to_string)
    .to_vec())
}

pub fn hearing(
    whisper: &Path,
    model: &Path,
    said: &Path,
    language: &str,
) -> Result<Vec<String>, Never> {
    Ok([
        &whisper.to_string_lossy(),
        "--model",
        &model.to_string_lossy(),
        "--file",
        &said.to_string_lossy(),
        "--language",
        language,
        "--threads",
        THREADS,
        "--no-timestamps",
        "--no-prints",
    ]
    .map(str::to_string)
    .to_vec())
}

pub fn typing(words: &str) -> Result<Vec<String>, Never> {
    let Ok(wtype) = Program::Wtype.name();

    Ok([wtype, "--", words].map(str::to_string).to_vec())
}

pub fn fetching(into: &Path) -> Result<Vec<String>, Never> {
    let Ok(curl) = Program::Curl.name();

    Ok([
        curl,
        "--location",
        "--fail",
        "--silent",
        "--show-error",
        "--output",
        &into.to_string_lossy(),
        MODEL_FROM,
    ]
    .map(str::to_string)
    .to_vec())
}

const FRAME: u16 = 320;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level {
    pub middle: f32,
    pub loud: f32,
}

pub fn level(wav: &[u8]) -> Result<Level, Never> {
    let Ok(data) = data(wav);

    let Some(sound) = data else { return Ok(Level::default()) };

    let mut frames: Vec<f32> = sound
        .chunks_exact(2)
        .map(|pair| match pair {
            [low, high] => f32::from(i16::from_le_bytes([*low, *high])) / 32768.0,
            _ => 0.0,
        })
        .collect::<Vec<f32>>()
        .chunks_exact(usize::from(FRAME))
        .map(|frame| (frame.iter().map(|one| one * one).sum::<f32>() / f32::from(FRAME)).sqrt())
        .collect();

    match frames.len() < ENOUGH {
        true => return Ok(Level::default()),
        false => {},
    }

    frames.sort_by(f32::total_cmp);

    let middle = frames.get(frames.len().saturating_div(2)).copied();
    let loud = frames.get(frames.len().saturating_mul(9).saturating_div(10)).copied();

    Ok(match (middle, loud) {
        (Some(middle), Some(loud)) => Level { middle, loud },
        (Some(_), None) | (None, _) => Level::default(),
    })
}

fn data(wav: &[u8]) -> Result<Option<&[u8]>, Never> {
    match (wav.get(..4), wav.get(8..12)) {
        (Some(b"RIFF"), Some(b"WAVE")) => {},
        (Some(_) | None, _) => return Ok(None),
    }

    let mut at: usize = 12;

    while at.saturating_add(8) <= wav.len() {
        let Some(kind) = wav.get(at..at.saturating_add(4)) else { return Ok(None) };

        let Some([first, second, third, fourth]) = wav.get(at.saturating_add(4)..at.saturating_add(8))
        else {
            return Ok(None);
        };

        let Ok(long) =
            usize::try_from(u32::from_le_bytes([*first, *second, *third, *fourth]))
        else {
            return Ok(None);
        };

        let from = at.saturating_add(8);
        let to = from.saturating_add(long).min(wav.len());

        match kind == b"data" {
            true => return Ok(wav.get(from..to)),
            false => {},
        }

        at = from.saturating_add(long).saturating_add(long & 1);
    }

    Ok(None)
}

const ENOUGH: usize = 10;

pub const SPEAKS: f32 = 2.5;

pub const LOUD: f32 = 0.20;

pub fn anything_said(wav: &[u8]) -> Result<Heard, Never> {
    let Ok(heard) = level(wav);
    let spoke =
        (heard.loud > 0.0 && heard.loud >= heard.middle * SPEAKS) || heard.middle >= LOUD;

    Ok(match spoke {
        true => Heard::Something,
        false => Heard::Nothing,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    Something,
    Nothing,
}

pub fn tidy(heard: &str) -> Result<String, Never> {
    let words: Vec<&str> = heard
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            let Ok(noise) = is_a_noise(line);

            noise == Line::Spoken
        })
        .collect();

    plainly(&words.join(" ").split_whitespace().collect::<Vec<&str>>().join(" "))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    TheRoom,
    Spoken,
}

fn is_a_noise(line: &str) -> Result<Line, Never> {
    let bracketed = |open: char, close: char| line.starts_with(open) && line.ends_with(close);

    Ok(match bracketed('[', ']') || bracketed('(', ')') || bracketed('*', '*') {
        true => Line::TheRoom,
        false => Line::Spoken,
    })
}

pub const SHORT: usize = 6;

pub fn plainly(said: &str) -> Result<String, Never> {
    let Ok(spoken) = spoken(said);

    match spoken > SHORT {
        true => return Ok(said.to_string()),
        false => {},
    }

    let letters: Vec<char> = said.chars().collect();
    let bare: String = letters
        .iter()
        .enumerate()
        .map(|(at, one)| {
            let Ok(letter) = is_a_word(*one, &letters, at);

            match letter {
                Letter::OfAWord => *one,
                Letter::AMark => ' ',
            }
        })
        .collect();

    Ok(bare.split_whitespace().collect::<Vec<&str>>().join(" "))
}

fn is_a_word(one: char, said: &[char], at: usize) -> Result<Letter, Never> {
    let Ok(upon) = is_upon_a_letter(one);

    match one.is_alphanumeric() || one.is_whitespace() || upon == Letter::OfAWord {
        true => return Ok(Letter::OfAWord),
        false => {},
    }

    let letter = |one: Option<&char>| one.is_some_and(|one| one.is_alphanumeric());
    let inside_a_word = at > 0
        && letter(said.get(at.saturating_sub(1)))
        && letter(said.get(at.saturating_add(1)));
    let Ok(starts) = starts_a_word(said, at);
    let kept =
        matches!(one, '\'' | '\u{2019}' | '-') && (inside_a_word || starts == Letter::OfAWord);

    Ok(match kept {
        true => Letter::OfAWord,
        false => Letter::AMark,
    })
}

fn starts_a_word(said: &[char], at: usize) -> Result<Letter, Never> {
    let boundary = |one: Option<&char>| one.is_none_or(|one| !one.is_alphanumeric());
    let opens = boundary(said.get(at.wrapping_sub(1)).filter(|_| at > 0))
        && said.get(at.saturating_add(1)).is_some_and(|one| one.is_alphabetic())
        && boundary(said.get(at.saturating_add(2)));

    Ok(match opens {
        true => Letter::OfAWord,
        false => Letter::AMark,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Letter {
    OfAWord,
    AMark,
}

fn is_upon_a_letter(one: char) -> Result<Letter, Never> {
    let upon = ('\u{0e31}'..='\u{0e3a}').contains(&one)
        || ('\u{0e47}'..='\u{0e4e}').contains(&one);

    Ok(match upon {
        true => Letter::OfAWord,
        false => Letter::AMark,
    })
}

fn spoken(said: &str) -> Result<usize, Never> {
    Ok(said
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|one| {
                    let Ok(script) = unspaced(one);

                    script == Script::Unspaced
                })
                .count()
                .max(1)
        })
        .sum())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Unspaced,
    Spaced,
}

fn unspaced(one: &char) -> Result<Script, Never> {
    Ok(match ('\u{0e00}'..='\u{0e7f}').contains(one) {
        true => Script::Unspaced,
        false => Script::Spaced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording(into: &Path) -> Vec<String> {
        let Ok(argv) = super::recording(into);

        argv
    }

    fn hearing(whisper: &Path, model: &Path, said: &Path, language: &str) -> Vec<String> {
        let Ok(argv) = super::hearing(whisper, model, said, language);

        argv
    }

    fn typing(words: &str) -> Vec<String> {
        let Ok(argv) = super::typing(words);

        argv
    }

    fn cloning(into: &Path) -> Vec<String> {
        let Ok(argv) = super::cloning(into);

        argv
    }

    fn configuring(at: &Path) -> Vec<String> {
        let Ok(argv) = super::configuring(at);

        argv
    }

    fn tidy(heard: &str) -> String {
        let Ok(tidy) = super::tidy(heard);

        tidy
    }

    fn level(wav: &[u8]) -> Level {
        let Ok(level) = super::level(wav);

        level
    }

    fn anything_said(wav: &[u8]) -> Heard {
        let Ok(heard) = super::anything_said(wav);

        heard
    }

    fn making() -> PathBuf {
        let Ok(making) = super::making();

        making
    }

    fn whisper() -> PathBuf {
        let Ok(whisper) = super::whisper();

        whisper
    }

    fn model() -> PathBuf {
        let Ok(model) = super::model();

        model
    }

    fn said(press: u32) -> PathBuf {
        let Ok(said) = super::said(press);

        said
    }

    fn taking() -> PathBuf {
        let Ok(taking) = super::taking();

        taking
    }

    fn taken(recorder: u32, press: u32) -> String {
        let Ok(taken) = super::taken(recorder, press);

        taken
    }

    fn told_by(note: &str) -> Option<(i32, u32)> {
        let Ok(told) = super::told_by(note);

        told
    }

    #[test]
    fn a_recording_is_one_channel_at_the_rate_the_hearing_wants() {
        let argv = recording(Path::new("/run/said.wav"));
        let Ok(pw_record) = Program::PwRecord.name();

        assert_eq!(argv.first().map(String::as_str), Some(pw_record));
        assert!(argv.windows(2).any(|pair| pair == ["--rate", RATE]));
        assert!(argv.windows(2).any(|pair| pair == ["--channels", "1"]));
        assert_eq!(argv.last().map(String::as_str), Some("/run/said.wav"));
    }

    #[test]
    fn the_hearing_is_told_the_model_the_file_and_to_say_nothing_else() {
        let (whisper, model) = (Path::new("/keep/whisper-cli"), Path::new("/keep/model.bin"));
        let argv = hearing(whisper, model, Path::new("/run/said.wav"), "auto");
        assert_eq!(argv.first().map(String::as_str), Some("/keep/whisper-cli"));
        assert!(argv.windows(2).any(|pair| pair == ["--model", "/keep/model.bin"]));
        assert!(argv.windows(2).any(|pair| pair == ["--file", "/run/said.wav"]));
        assert!(argv.iter().any(|word| word == "--no-timestamps"));
        assert!(argv.iter().any(|word| word == "--no-prints"));
    }

    #[test]
    fn the_hearing_listens_for_the_language_that_was_chosen() {
        for language in &languages::EVERY {
            let argv = hearing(Path::new("w"), Path::new("m"), Path::new("s"), language.key);
            assert!(
                argv.windows(2).any(|pair| pair == ["--language", language.key]),
                "{} was not asked for",
                language.says
            );
        }
    }

    #[test]
    fn what_is_typed_is_handed_over_as_words_and_not_as_options() {
        let Ok(wtype) = Program::Wtype.name();

        assert_eq!(typing("-n is a flag"), [wtype, "--", "-n is a flag"]);
    }

    #[test]
    fn what_was_heard_comes_back_as_one_line() {
        let said = "  Hello there, it has been a while.\n\n  How have you been keeping?  \n";
        assert_eq!(tidy(said), "Hello there, it has been a while. How have you been keeping?");
    }

    #[test]
    fn a_short_thing_comes_back_with_no_marks_on_it() {
        assert_eq!(tidy("Settings."), "Settings");
        assert_eq!(tidy("Blåhaj, please?"), "Blåhaj please");
        assert_eq!(tidy("\"Where is it?\""), "Where is it");
    }

    #[test]
    fn a_sentence_keeps_the_marks_it_came_with() {
        let said = "I'll be there at six, but I might be late, so don't wait for me.";
        assert_eq!(tidy(said), said);
    }

    #[test]
    fn the_marks_inside_words_are_part_of_the_words() {
        assert_eq!(tidy("don't -- well-known, isn't it?"), "don't well-known isn't it");
    }

    #[test]
    fn a_language_with_no_spaces_is_counted_by_its_characters() {
        let said = "ฉันไปตลาดเมื่อเช้านี้ และซื้อผลไม้มาด้วย ทั้งหมดสดมาก";
        assert_eq!(tidy(said), said);
    }

    #[test]
    fn the_marks_a_thai_word_is_written_with_are_part_of_the_word() {
        assert_eq!(tidy("สวัสดีค่ะ"), "สวัสดีค่ะ");
        assert_eq!(tidy("ขอบคุณ"), "ขอบคุณ");
        assert_eq!(tidy("ไปไหน?"), "ไปไหน");
    }

    #[test]
    fn a_dutch_plural_keeps_the_apostrophe_that_makes_it_one() {
        assert_eq!(tidy("Ik heb twee auto's."), "Ik heb twee auto's");
        assert_eq!(tidy("Waar zijn de foto's?"), "Waar zijn de foto's");
    }

    #[test]
    fn a_dutch_word_that_begins_with_an_apostrophe_keeps_it() {
        assert_eq!(tidy("'s Ochtends drink ik koffie"), "'s Ochtends drink ik koffie");
        assert_eq!(tidy("'t Is koud vandaag"), "'t Is koud vandaag");
        assert_eq!(tidy("Geef me 'n moment."), "Geef me 'n moment");
    }

    #[test]
    fn a_quotation_is_not_a_word_that_begins_with_an_apostrophe() {
        assert_eq!(tidy("'hello' she said"), "hello she said");
    }

    #[test]
    fn the_line_between_a_name_and_a_sentence_falls_where_it_says_it_does() {
        let asked = "Waar heb ik de sleutels gelaten?";
        assert_eq!(tidy(asked), "Waar heb ik de sleutels gelaten", "six words is a name");
        let said = "Waar heb ik de blauwe sleutels gelaten?";
        assert_eq!(tidy(said), said, "seven words is a sentence");
    }

    #[test]
    fn the_thai_marks_that_are_not_letters_survive_a_short_thing() {
        assert_eq!(tidy("จันทร์"), "จันทร์", "the silent-letter mark");
        assert_eq!(tidy("สัตว์"), "สัตว์", "the same, after a vowel mark");
        assert_eq!(tidy("น้ำ"), "น้ำ", "a tone mark and a sara am");
    }

    #[test]
    fn a_thai_word_still_loses_the_punctuation_beside_it() {
        assert_eq!(tidy("น้ำ?"), "น้ำ");
    }

    #[test]
    fn a_thai_line_is_counted_by_characters_across_the_gaps() {
        assert_eq!(tidy("หมา ม้า มา"), "หมา ม้า มา");
    }

    #[test]
    fn the_marks_for_what_is_not_speech_are_not_words() {
        assert_eq!(tidy("[BLANK_AUDIO]"), "");
        assert_eq!(tidy("(wind)\nHello\n*laughs*"), "Hello");
    }

    fn a_wav(samples: &[i16]) -> Vec<u8> {
        let sound: Vec<u8> = samples.iter().flat_map(|one| one.to_le_bytes()).collect();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + sound.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&32000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(sound.len() as u32).to_le_bytes());
        wav.extend_from_slice(&sound);
        wav
    }

    fn a_room(level: i16) -> Vec<i16> {
        (0..16000).map(|at| if at % 2 == 0 { level } else { -level }).collect()
    }

    fn a_sentence(room: i16, voice: i16) -> Vec<i16> {
        let mut said = a_room(room);
        for (at, one) in said[4000..9000].iter_mut().enumerate() {
            *one = if at % 2 == 0 { voice } else { -voice };
        }
        said
    }

    #[test]
    fn the_quiet_and_the_loud_of_a_recording_are_measured_in_frames() {
        let heard = level(&a_wav(&a_room(3277)));
        assert!((heard.middle - 0.1).abs() < 0.001, "{heard:?}");
        assert!((heard.loud - 0.1).abs() < 0.001, "{heard:?}");
    }

    #[test]
    fn the_samples_are_found_past_whatever_else_the_writer_wrote() {
        let mut wav = a_wav(&a_sentence(300, 9000));
        let extra = b"LIST\x04\x00\x00\x00abcd";
        wav.splice(12..12, extra.iter().copied());
        assert_eq!(anything_said(&wav), Heard::Something);
    }

    #[test]
    fn what_is_not_a_recording_is_not_a_sentence() {
        assert_eq!(level(b""), Level::default());
        assert_eq!(level(b"this is not a wav at all"), Level::default());
        assert_eq!(anything_said(b""), Heard::Nothing);
    }

    #[test]
    fn a_room_with_nobody_in_it_is_not_asked_about() {
        assert_eq!(anything_said(&a_wav(&a_room(0))), Heard::Nothing);
        assert_eq!(anything_said(&a_wav(&a_room(300))), Heard::Nothing);
        assert_eq!(anything_said(&a_wav(&a_sentence(300, 400))), Heard::Nothing);
    }

    #[test]
    fn the_gain_knob_moves_the_room_and_not_the_answer() {
        for level in [118, 455, 1541, 4260] {
            let voice = (i32::from(level) * 8).min(30_000) as i16;
            assert_eq!(
                anything_said(&a_wav(&a_room(level))),
                Heard::Nothing,
                "{level} is a room"
            );
            assert_eq!(
                anything_said(&a_wav(&a_sentence(level, voice))),
                Heard::Something,
                "{level} is spoken"
            );
        }
    }

    #[test]
    fn talking_all_the_way_through_is_talking() {
        assert_eq!(anything_said(&a_wav(&a_room(9000))), Heard::Something);
    }

    #[test]
    fn a_recording_too_short_to_have_a_middle_is_nothing_said() {
        assert_eq!(anything_said(&a_wav(&a_room(9000)[..1000])), Heard::Nothing);
    }

    #[test]
    fn the_hearing_is_built_for_the_card_this_machine_has() {
        let argv = configuring(Path::new("/run/whisper.cpp"));
        assert!(argv.iter().any(|word| word == "-DGGML_VULKAN=ON"));
        assert!(argv.iter().any(|word| word == "-DBUILD_SHARED_LIBS=OFF"));
    }

    #[test]
    fn the_source_of_the_hearing_is_pinned() {
        let argv = cloning(Path::new("/run/whisper.cpp"));
        assert!(argv.windows(2).any(|pair| pair == ["--branch", WHISPER_AT]));
        assert!(argv.iter().any(|word| word == WHISPER_FROM));
        assert!(WHISPER_AT.starts_with('v'), "a tag, not a branch");
    }

    #[test]
    fn the_build_tree_does_not_outlive_the_build() {
        // SAFETY: one thread, and both variables are put back before it ends.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        unsafe { std::env::set_var("XDG_DATA_HOME", "/home/somebody/.local/share") };
        assert!(making().starts_with("/run/user/1000"));
        assert!(whisper().starts_with("/home/somebody/.local/share"));
        assert_eq!(whisper().parent(), model().parent(), "beside the model");
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
    }

    #[test]
    fn the_recording_waits_where_a_session_ending_clears_it() {
        // SAFETY: one thread, and both variables are put back before it ends.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        unsafe { std::env::set_var("XDG_DATA_HOME", "/home/somebody/.local/share") };
        assert_eq!(said(41), Path::new("/run/user/1000/console/voice/said-41.wav"));
        assert_eq!(taking(), Path::new("/run/user/1000/console/voice/taking.pid"));
        assert_eq!(model(), Path::new("/home/somebody/.local/share/console/voice").join(MODEL));
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
    }

    #[test]
    fn two_presses_do_not_share_a_recording() {
        assert_ne!(said(41), said(42));
    }

    #[test]
    fn the_note_says_who_to_stop_and_what_to_read() {
        assert_eq!(taken(1234, 99), "1234 99");
        assert_eq!(told_by("1234 99"), Some((1234, 99)));
        assert_eq!(told_by(" 1234  99 \n"), Some((1234, 99)));
    }

    #[test]
    fn a_note_that_names_no_recording_is_refused() {
        assert_eq!(told_by("1234"), None);
        assert_eq!(told_by(""), None);
        assert_eq!(told_by("not a number at all"), None);
    }
}
