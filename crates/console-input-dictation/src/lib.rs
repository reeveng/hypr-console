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

use console_core_arguments::{Command, Flag, Operands, Takes};
use console_core_external_programs::Program;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub const FETCH: Flag = Flag {
    spelling: "--fetch",
    takes: Takes::None,
    about: "download the language and build the hearing, and listen to nothing",
};

pub const BUILD: Flag = Flag { spelling: "--build", takes: Takes::None, about: "build the hearing, and listen to nothing" };

pub const COMMAND: Command = Command {
    name: "console-dictate",
    about: "start listening, or write down what was heard when it already is",
    flags: &[FETCH, BUILD],
    operands: Operands::None,
};

pub const MODEL: &str = "ggml-large-v3-turbo-q5_0.bin";

pub const MODEL_FROM: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";

pub const RATE: &str = "16000";

pub const THREADS: &str = "12";

fn runtime() -> Result<PathBuf, Never> {
    let ours = console_core_places::application_runtime()?;

    Ok(match ours {
        Some(ours) => ours.join("voice"),
        None => Path::new("/tmp").join(console_core_places::APPLICATION).join("voice"),
    })
}

pub fn models_path() -> Result<Option<PathBuf>, Never> {
    let ours = console_core_places::Base::Share.ours()?;

    Ok(ours.map(|ours| ours.join("voice")))
}

pub fn recording_path(press: u32) -> Result<PathBuf, Never> {
    let Ok(runtime) = runtime();

    Ok(runtime.join(format!("said-{press}.wav")))
}

pub fn taking() -> Result<PathBuf, Never> {
    let Ok(runtime) = runtime();

    Ok(runtime.join("taking.pid"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note {
    pub recorder: u32,
    pub press: u32,
}

pub fn serialize_note(note: Note) -> Result<String, Never> {
    let Note { recorder, press } = note;

    Ok(format!("{recorder} {press}"))
}

pub fn told_by(note: &str) -> Result<Option<(i32, u32)>, Never> {
    let mut words = note.split_whitespace();

    let first = match words.next() {
        Some(first) => first,
        None => return Ok(None),
    };

    let recorder = match first.parse() {
        Ok(recorder) => recorder,
        Err(_not_a_number) => return Ok(None),
    };

    let second = match words.next() {
        Some(second) => second,
        None => return Ok(None),
    };

    let press = match second.parse() {
        Ok(press) => press,
        Err(_not_a_number) => return Ok(None),
    };

    Ok(Some((recorder, press)))
}

pub fn model() -> Result<Option<PathBuf>, Never> {
    let kept = models_path()?;

    Ok(kept.map(|kept| kept.join(MODEL)))
}

pub const WHISPER_FROM: &str = "https://github.com/ggml-org/whisper.cpp";

pub const WHISPER_AT: &str = "v1.9.1";

pub fn whisper() -> Result<Option<PathBuf>, Never> {
    let kept = models_path()?;

    let Ok(whisper_cli) = Program::WhisperCli.name();

    Ok(kept.map(|kept| kept.join(whisper_cli)))
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

pub fn whisper_binary(at: &Path) -> Result<PathBuf, Never> {
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

pub fn whisper_arguments(
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

    let sound = match data {
        Some(sound) => sound,
        None => return Ok(Level::default()),
    };

    let Ok(frame) = index(FRAME);
    let mut frames: Vec<f32> = sound
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0)
        .collect::<Vec<f32>>()
        .chunks_exact(frame)
        .map(|frame| (frame.iter().map(|one| one * one).sum::<f32>() / f32::from(FRAME)).sqrt())
        .collect();

    let Ok(counted) = fitted::<_, u32>(frames.len());

    match counted < ENOUGH {
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

    let Ok(header) = index(RIFF_HEADER);

    let rest = match wav.get(header..) {
        Some(rest) => rest,
        None => return Ok(None),
    };

    let found = console_core_iteration::iterate(rest, |rest| {
        let (kind, size, body) = match rest
            .split_first_chunk::<4>()
            .and_then(|(kind, rest)| rest.split_first_chunk::<4>().map(|(size, body)| (kind, u32::from_le_bytes(*size), body)))
        {
            Some(chunk) => chunk,
            None => return Ok(Step::Halt(None)),
        };

        let Ok(long) = index(size);

        Ok(match kind == b"data" {
            true => Step::Halt(Some(match body.get(..long) {
                Some(chunk) => chunk,
                None => body,
            })),
            false => {
                let Ok(padded) = index(size.saturating_add(size & 1));

                match body.get(padded..) {
                    Some(rest) => Step::Again(rest),
                    None => Step::Halt(None),
                }
            }
        })
    });

    Ok(match found {
        Ok(found) => found,
        Err(_endless) => None,
    })
}

const RIFF_HEADER: u32 = 12;

const ENOUGH: u32 = 10;

pub const SPEAKS: f32 = 2.5;

pub const LOUD: f32 = 0.20;

pub fn detect_speech(wav: &[u8]) -> Result<VoiceActivity, Never> {
    let Ok(heard) = level(wav);
    let spoke =
        (heard.loud > 0.0 && heard.loud >= heard.middle * SPEAKS) || heard.middle >= LOUD;

    Ok(match spoke {
        true => VoiceActivity::Speech,
        false => VoiceActivity::Silence,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceActivity {
    Speech,
    Silence,
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

pub const SHORT: u32 = 6;

pub fn plainly(said: &str) -> Result<String, Never> {
    let Ok(spoken) = count_words(said);

    match spoken > SHORT {
        true => return Ok(said.to_string()),
        false => {},
    }

    let letters: Vec<char> = said.chars().collect();
    let bare: String = letters
        .iter()
        .enumerate()
        .map(|(at, one)| {
            let Ok(at) = fitted(at);
            let Ok(letter) = is_a_word(*one, &letters, at);

            match letter {
                Letter::OfAWord => *one,
                Letter::AMark => ' ',
            }
        })
        .collect();

    Ok(bare.split_whitespace().collect::<Vec<&str>>().join(" "))
}

fn is_a_word(one: char, said: &[char], at: u32) -> Result<Letter, Never> {
    let Ok(upon) = is_upon_a_letter(one);

    match one.is_alphanumeric() || one.is_whitespace() || upon == Letter::OfAWord {
        true => return Ok(Letter::OfAWord),
        false => {},
    }

    let letter = |one: Option<&char>| one.is_some_and(|one| one.is_alphanumeric());
    let Ok(before) = index(at.saturating_sub(1));
    let Ok(after) = index(at.saturating_add(1));
    let inside_a_word = at > 0 && letter(said.get(before)) && letter(said.get(after));
    let Ok(starts) = starts_a_word(said, at);
    let kept =
        matches!(one, '\'' | '\u{2019}' | '-') && (inside_a_word || starts == Letter::OfAWord);

    Ok(match kept {
        true => Letter::OfAWord,
        false => Letter::AMark,
    })
}

fn starts_a_word(said: &[char], at: u32) -> Result<Letter, Never> {
    let boundary = |one: Option<&char>| one.is_none_or(|one| !one.is_alphanumeric());
    let Ok(before) = index(at.saturating_sub(1));
    let Ok(after) = index(at.saturating_add(1));
    let Ok(beyond) = index(at.saturating_add(2));
    let opens = boundary(said.get(before).filter(|_| at > 0))
        && said.get(after).is_some_and(|one| one.is_alphabetic())
        && boundary(said.get(beyond));

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

fn count_words(said: &str) -> Result<u32, Never> {
    Ok(said
        .split_whitespace()
        .map(|word| {
            let Ok(counted) = fitted::<_, u32>(word
                .chars()
                .filter(|one| {
                    let Ok(script) = unspaced(one);

                    script == Script::Unspaced
                })
                .count());

            counted.max(1)
        })
        .fold(0, u32::saturating_add))
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

    #[test]
    fn a_recording_is_one_channel_at_the_rate_the_hearing_wants() {
        let Ok(arguments) = recording(Path::new("/run/said.wav"));
        let Ok(pw_record) = Program::PwRecord.name();

        assert_eq!(arguments.first().map(String::as_str), Some(pw_record));
        assert!(arguments.windows(2).any(|pair| pair == ["--rate", RATE]));
        assert!(arguments.windows(2).any(|pair| pair == ["--channels", "1"]));
        assert_eq!(arguments.last().map(String::as_str), Some("/run/said.wav"));
    }

    #[test]
    fn the_hearing_is_told_the_model_the_file_and_to_say_nothing_else() {
        let (whisper, model) = (Path::new("/keep/whisper-cli"), Path::new("/keep/model.bin"));
        let Ok(arguments) = whisper_arguments(whisper, model, Path::new("/run/said.wav"), "auto");
        assert_eq!(arguments.first().map(String::as_str), Some("/keep/whisper-cli"));
        assert!(arguments.windows(2).any(|pair| pair == ["--model", "/keep/model.bin"]));
        assert!(arguments.windows(2).any(|pair| pair == ["--file", "/run/said.wav"]));
        assert!(arguments.iter().any(|word| word == "--no-timestamps"));
        assert!(arguments.iter().any(|word| word == "--no-prints"));
    }

    #[test]
    fn the_hearing_listens_for_the_language_that_was_chosen() {
        for language in &languages::EVERY {
            let Ok(arguments) = whisper_arguments(Path::new("w"), Path::new("m"), Path::new("s"), language.key);
            assert!(
                arguments.windows(2).any(|pair| pair == ["--language", language.key]),
                "{} was not asked for",
                language.says
            );
        }
    }

    #[test]
    fn what_is_typed_is_handed_over_as_words_and_not_as_options() {
        let Ok(wtype) = Program::Wtype.name();

        assert_eq!(typing("-n is a flag"), Ok(vec![wtype.to_string(), "--".to_string(), "-n is a flag".to_string()]));
    }

    #[test]
    fn what_was_heard_comes_back_as_one_line() {
        let said = "  Hello there, it has been a while.\n\n  How have you been keeping?  \n";
        assert_eq!(tidy(said), Ok("Hello there, it has been a while. How have you been keeping?".to_string()));
    }

    #[test]
    fn a_short_thing_comes_back_with_no_marks_on_it() {
        assert_eq!(tidy("Settings."), Ok("Settings".to_string()));
        assert_eq!(tidy("Blåhaj, please?"), Ok("Blåhaj please".to_string()));
        assert_eq!(tidy("\"Where is it?\""), Ok("Where is it".to_string()));
    }

    #[test]
    fn a_sentence_keeps_the_marks_it_came_with() {
        let said = "I'll be there at six, but I might be late, so don't wait for me.";
        assert_eq!(tidy(said), Ok(said.to_string()));
    }

    #[test]
    fn the_marks_inside_words_are_part_of_the_words() {
        assert_eq!(tidy("don't -- well-known, isn't it?"), Ok("don't well-known isn't it".to_string()));
    }

    #[test]
    fn a_language_with_no_spaces_is_counted_by_its_characters() {
        let said = "ฉันไปตลาดเมื่อเช้านี้ และซื้อผลไม้มาด้วย ทั้งหมดสดมาก";
        assert_eq!(tidy(said), Ok(said.to_string()));
    }

    #[test]
    fn the_marks_a_thai_word_is_written_with_are_part_of_the_word() {
        assert_eq!(tidy("สวัสดีค่ะ"), Ok("สวัสดีค่ะ".to_string()));
        assert_eq!(tidy("ขอบคุณ"), Ok("ขอบคุณ".to_string()));
        assert_eq!(tidy("ไปไหน?"), Ok("ไปไหน".to_string()));
    }

    #[test]
    fn a_dutch_plural_keeps_the_apostrophe_that_makes_it_one() {
        assert_eq!(tidy("Ik heb twee auto's."), Ok("Ik heb twee auto's".to_string()));
        assert_eq!(tidy("Waar zijn de foto's?"), Ok("Waar zijn de foto's".to_string()));
    }

    #[test]
    fn a_dutch_word_that_begins_with_an_apostrophe_keeps_it() {
        assert_eq!(tidy("'s Ochtends drink ik koffie"), Ok("'s Ochtends drink ik koffie".to_string()));
        assert_eq!(tidy("'t Is koud vandaag"), Ok("'t Is koud vandaag".to_string()));
        assert_eq!(tidy("Geef me 'n moment."), Ok("Geef me 'n moment".to_string()));
    }

    #[test]
    fn a_quotation_is_not_a_word_that_begins_with_an_apostrophe() {
        assert_eq!(tidy("'hello' she said"), Ok("hello she said".to_string()));
    }

    #[test]
    fn the_line_between_a_name_and_a_sentence_falls_where_it_says_it_does() {
        let asked = "Waar heb ik de sleutels gelaten?";
        assert_eq!(tidy(asked), Ok("Waar heb ik de sleutels gelaten".to_string()), "six words is a name");
        let said = "Waar heb ik de blauwe sleutels gelaten?";
        assert_eq!(tidy(said), Ok(said.to_string()), "seven words is a sentence");
    }

    #[test]
    fn the_thai_marks_that_are_not_letters_survive_a_short_thing() {
        assert_eq!(tidy("จันทร์"), Ok("จันทร์".to_string()), "the silent-letter mark");
        assert_eq!(tidy("สัตว์"), Ok("สัตว์".to_string()), "the same, after a vowel mark");
        assert_eq!(tidy("น้ำ"), Ok("น้ำ".to_string()), "a tone mark and a sara am");
    }

    #[test]
    fn a_thai_word_still_loses_the_punctuation_beside_it() {
        assert_eq!(tidy("น้ำ?"), Ok("น้ำ".to_string()));
    }

    #[test]
    fn a_thai_line_is_counted_by_characters_across_the_gaps() {
        assert_eq!(tidy("หมา ม้า มา"), Ok("หมา ม้า มา".to_string()));
    }

    #[test]
    fn the_marks_for_what_is_not_speech_are_not_words() {
        assert_eq!(tidy("[BLANK_AUDIO]"), Ok("".to_string()));
        assert_eq!(tidy("(wind)\nHello\n*laughs*"), Ok("Hello".to_string()));
    }

    fn a_wav(samples: &[i16]) -> Result<Vec<u8>, Never> {
        let sound: Vec<u8> = samples.iter().flat_map(|one| one.to_le_bytes()).collect();
        let Ok(length) = console_core_number_conversion::fitted::<_, u32>(sound.len());
        let mut wav = Vec::new();

        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&length.saturating_add(36).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&32000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&length.to_le_bytes());
        wav.extend_from_slice(&sound);

        Ok(wav)
    }

    fn a_room(level: i16) -> Result<Vec<i16>, Never> {
        Ok([level, level.saturating_neg()].into_iter().cycle().take(16_000).collect())
    }

    fn a_sentence((room, voice): (i16, i16)) -> Result<Vec<i16>, Never> {
        let Ok(mut said) = a_room(room);
        let Ok(spoken) = a_room(voice);

        for (one, spoken) in said.iter_mut().skip(4000).take(5000).zip(spoken) {
            *one = spoken;
        }

        Ok(said)
    }

    fn heard(samples: &[i16]) -> Result<VoiceActivity, Never> {
        let Ok(wav) = a_wav(samples);

        detect_speech(&wav)
    }

    #[test]
    fn the_quiet_and_the_loud_of_a_recording_are_measured_in_frames() {
        let Ok(room) = a_room(3277);
        let Ok(wav) = a_wav(&room);
        let Ok(heard) = level(&wav);

        assert!((heard.middle - 0.1).abs() < 0.001, "{heard:?}");
        assert!((heard.loud - 0.1).abs() < 0.001, "{heard:?}");
    }

    #[test]
    fn the_samples_are_found_past_whatever_else_the_writer_wrote() {
        let Ok(sentence) = a_sentence((300, 9000));
        let Ok(mut wav) = a_wav(&sentence);
        let extra = b"LIST\x04\x00\x00\x00abcd";

        wav.splice(12..12, extra.iter().copied());

        assert_eq!(detect_speech(&wav), Ok(VoiceActivity::Speech));
    }

    #[test]
    fn what_is_not_a_recording_is_not_a_sentence() {
        assert_eq!(level(b""), Ok(Level::default()));
        assert_eq!(level(b"this is not a wav at all"), Ok(Level::default()));
        assert_eq!(detect_speech(b""), Ok(VoiceActivity::Silence));
    }

    #[test]
    fn a_room_with_no_one_in_it_is_not_asked_about() {
        let Ok(silent) = a_room(0);
        let Ok(humming) = a_room(300);
        let Ok(mumbled) = a_sentence((300, 400));

        assert_eq!(heard(&silent), Ok(VoiceActivity::Silence));
        assert_eq!(heard(&humming), Ok(VoiceActivity::Silence));
        assert_eq!(heard(&mumbled), Ok(VoiceActivity::Silence));
    }

    #[test]
    fn the_gain_knob_moves_the_room_and_not_the_answer() {
        for level in [118_i16, 455, 1541, 4260] {
            let voice = level.saturating_mul(8).min(30_000);
            let Ok(room) = a_room(level);
            let Ok(sentence) = a_sentence((level, voice));

            assert_eq!(heard(&room), Ok(VoiceActivity::Silence), "{level} is a room");
            assert_eq!(heard(&sentence), Ok(VoiceActivity::Speech), "{level} is spoken");
        }
    }

    #[test]
    fn talking_all_the_way_through_is_talking() {
        let Ok(talking) = a_room(9000);

        assert_eq!(heard(&talking), Ok(VoiceActivity::Speech));
    }

    #[test]
    fn a_recording_too_short_to_have_a_middle_is_nothing_said() {
        let Ok(talking) = a_room(9000);
        let short: Vec<i16> = talking.into_iter().take(1000).collect();

        assert_eq!(heard(&short), Ok(VoiceActivity::Silence));
    }

    #[test]
    fn the_hearing_is_built_for_the_card_this_machine_has() {
        let Ok(arguments) = configuring(Path::new("/run/whisper.cpp"));
        assert!(arguments.iter().any(|word| word == "-DGGML_VULKAN=ON"));
        assert!(arguments.iter().any(|word| word == "-DBUILD_SHARED_LIBS=OFF"));
    }

    #[test]
    fn the_source_of_the_hearing_is_pinned() {
        let Ok(arguments) = cloning(Path::new("/run/whisper.cpp"));
        assert!(arguments.windows(2).any(|pair| pair == ["--branch", WHISPER_AT]));
        assert!(arguments.iter().any(|word| word == WHISPER_FROM));
        assert!(WHISPER_AT.starts_with('v'), "a tag, not a branch");
    }

    #[test]
    fn the_build_tree_does_not_outlive_the_build() {
        let Ok(making) = making();
        let Ok(runtime) = runtime();
        let Ok(kept) = models_path();
        let Ok(whisper) = whisper();
        let Ok(model) = model();
        let whisper_in = whisper.as_deref().and_then(Path::parent);

        assert!(making.starts_with(&runtime));
        assert_eq!(whisper_in, kept.as_deref());
        assert_eq!(whisper_in, model.as_deref().and_then(Path::parent), "beside the model");
    }

    #[test]
    fn the_recording_waits_where_a_session_ending_clears_it() {
        let Ok(runtime) = runtime();
        let Ok(session) = console_core_places::application_runtime();
        let Ok(kept) = models_path();

        match session {
            Some(session) => assert!(runtime.starts_with(session), "{}", runtime.display()),
            None => assert!(runtime.starts_with("/tmp"), "{}", runtime.display()),
        }

        assert_eq!(recording_path(41), Ok(runtime.join("said-41.wav")));
        assert_eq!(taking(), Ok(runtime.join("taking.pid")));
        assert_eq!(model(), Ok(kept.map(|kept| kept.join(MODEL))));
    }

    #[test]
    fn two_presses_do_not_share_a_recording() {
        let Ok(later) = recording_path(42);

        assert_ne!(recording_path(41), Ok(later));
    }

    #[test]
    fn the_note_says_who_to_stop_and_what_to_read() {
        assert_eq!(serialize_note(Note { recorder: 1234, press: 99 }), Ok("1234 99".to_string()));
        assert_eq!(told_by("1234 99"), Ok(Some((1234, 99))));
        assert_eq!(told_by(" 1234  99 \n"), Ok(Some((1234, 99))));
    }

    #[test]
    fn a_note_that_names_no_recording_is_refused() {
        assert_eq!(told_by("1234"), Ok(None));
        assert_eq!(told_by(""), Ok(None));
        assert_eq!(told_by("not a number at all"), Ok(None));
    }
}
