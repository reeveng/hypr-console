//! Making what is already in a folder the one format this device keeps.
//!
//! The fetcher decides that for what it fetches -- opus for sound, mkv for a
//! film -- and this is the same decision applied to what arrived some other
//! way: transferred off a laptop, synced from a phone, copied off a stick. A
//! folder of nine extensions is a folder you need nine programs to be sure of,
//! and the point of one extension is that the thing you find that plays it
//! plays all of it.
//!
//! The two are not the same operation, and it matters which is which. A film
//! is remuxed: the streams are lifted out of one container and put in another,
//! nothing is decoded, nothing is lost, and a gigabyte takes a second. Sound is
//! re-encoded, which is a real loss -- an mp3 made opus has been through two
//! lossy encoders -- and it is done anyway because 128k opus off a 320k mp3 is
//! a thing nobody can hear the bottom of, and because the alternative is the
//! folder staying nine formats forever. What is replaced goes to the
//! wastebasket rather than being unlinked, so a conversion somebody regrets is
//! an hour's walk back rather than a loss.


use console_external_programs::Program;
use console_never::Never;
use console_number_conversion::fitted;
use std::path::{Path, PathBuf};

use crate::store::Kind;

pub const BITRATE: &str = "128k";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wants {
    Nothing,
    Leave,
    Ask,
    Made(Kind),
}

const SOUNDS: [&str; 7] = ["aac", "flac", "m4a", "mp3", "oga", "wav", "wma"];

const FILMS: [&str; 10] =
    ["avi", "flv", "m4v", "mov", "mp4", "mpeg", "mpg", "ts", "webm", "wmv"];

const EITHER: [&str; 2] = ["ogg", "webm"];

pub fn wants(name: &str) -> Result<Wants, Never> {
    let Some((_, end)) = name.rsplit_once('.') else { return Ok(Wants::Leave) };

    let end = end.to_lowercase();

    match end == "opus" || end == "mkv" {
        true => return Ok(Wants::Nothing),
        false => {},
    }

    match EITHER.contains(&end.as_str()) {
        true => return Ok(Wants::Ask),
        false => {},
    }

    match SOUNDS.contains(&end.as_str()) {
        true => return Ok(Wants::Made(Kind::Sound)),
        false => {},
    }

    Ok(match FILMS.contains(&end.as_str()) {
        true => Wants::Made(Kind::Film),
        false => Wants::Leave,
    })
}

pub fn beside(path: &Path, kind: Kind) -> Result<PathBuf, Never> {
    Ok(path.with_extension(match kind {
        Kind::Sound => crate::getting::SOUND,
        Kind::Film => crate::getting::FILM,
    }))
}

pub fn inside(said: &str) -> Result<Kind, Never> {
    let moving = said.lines().any(|line| {
        let mut said = line.trim().split(',');
        let kind = said.next().unwrap_or_default();
        kind == "video" && said.next().unwrap_or("0") != "1"
    });

    Ok(match moving {
        true => Kind::Film,
        false => Kind::Sound,
    })
}

pub fn about(path: &Path) -> Result<Vec<String>, Never> {
    let said = |word: &str| word.to_string();
    let Ok(ffprobe) = Program::Ffprobe.name();

    Ok(vec![
        said(ffprobe),
        said("-v"),
        said("error"),
        said("-select_streams"),
        said("v"),
        said("-show_entries"),
        said("stream=codec_type,disposition=attached_pic"),
        said("-of"),
        said("csv=p=0"),
        path.to_string_lossy().to_string(),
    ])
}

pub fn cover(from: &Path, to: &Path) -> Result<Vec<String>, Never> {
    let said = |word: &str| word.to_string();
    Ok(vec![
        said("ffmpeg"),
        said("-loglevel"),
        said("error"),
        said("-y"),
        said("-i"),
        from.to_string_lossy().to_string(),
        said("-map"),
        said("0:v"),
        said("-frames:v"),
        said("1"),
        to.to_string_lossy().to_string(),
    ])
}

pub fn sound(from: &Path, to: &Path, picture: Option<&str>) -> Result<Vec<String>, Never> {
    let said = |word: &str| word.to_string();
    let mut argv = vec![
        said("ffmpeg"),
        said("-loglevel"),
        said("error"),
        said("-y"),
        said("-i"),
        from.to_string_lossy().to_string(),
        said("-map"),
        said("0:a"),
        said("-c:a"),
        said("libopus"),
        said("-b:a"),
        said(BITRATE),
        said("-map_metadata"),
        said("0"),
    ];

    match picture {
        Some(picture) => {
            argv.push(said("-metadata"));
            argv.push(format!("METADATA_BLOCK_PICTURE={picture}"));
        }
        None => {},
    }

    argv.push(to.to_string_lossy().to_string());
    Ok(argv)
}

pub fn film(from: &Path, to: &Path) -> Result<Vec<String>, Never> {
    let said = |word: &str| word.to_string();
    Ok(vec![
        said("ffmpeg"),
        said("-loglevel"),
        said("error"),
        said("-y"),
        said("-i"),
        from.to_string_lossy().to_string(),
        said("-map"),
        said("0"),
        said("-c"),
        said("copy"),
        to.to_string_lossy().to_string(),
    ])
}

pub fn block(mime: &str, picture: &[u8]) -> Result<String, Never> {
    const FRONT_COVER: u32 = 3;
    let mut held = Vec::new();
    let mut four = |number: u32| held.extend_from_slice(&number.to_be_bytes());
    let Ok(wide) = fitted(mime.len());

    four(FRONT_COVER);
    four(wide);
    held.extend_from_slice(mime.as_bytes());

    for _ in 0..5 {
        held.extend_from_slice(&0u32.to_be_bytes());
    }

    let Ok(many) = fitted::<usize, u32>(picture.len());

    held.extend_from_slice(&many.to_be_bytes());
    held.extend_from_slice(picture);

    sixty_four(&held)
}

pub fn sixty_four(held: &[u8]) -> Result<String, Never> {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut said = String::with_capacity(held.len().div_ceil(3).saturating_mul(4));

    for lot in held.chunks(3) {
        let Ok(pad) = fitted::<usize, u32>(3usize.saturating_sub(lot.len()).saturating_mul(8));
        let held = lot.iter().fold(0u32, |held, byte| held.wrapping_shl(8) | u32::from(*byte));
        let held = held.wrapping_shl(pad);

        for at in 0..4 {
            match at <= lot.len() {
                true => {
                    let Ok(down) =
                        fitted::<usize, u32>(18usize.saturating_sub(at.saturating_mul(6)));
                    let Ok(which) = fitted::<u32, usize>(held.wrapping_shr(down) & 63);

                    match ALPHABET.get(which) {
                        Some(letter) => said.push(char::from(*letter)),
                        None => said.push('='),
                    }
                }
                false => said.push('='),
            }
        }
    }

    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_already_the_one_format_is_left_where_it_is() {
        assert_eq!(wants("Africa [x].opus"), Ok(Wants::Nothing));
        assert_eq!(wants("Africa [x].mkv"), Ok(Wants::Nothing));
    }

    #[test]
    fn a_song_becomes_opus_and_a_film_becomes_mkv() {
        assert_eq!(wants("Africa.mp3"), Ok(Wants::Made(Kind::Sound)));
        assert_eq!(wants("Africa.FLAC"), Ok(Wants::Made(Kind::Sound)));
        assert_eq!(wants("holiday.mp4"), Ok(Wants::Made(Kind::Film)));
        assert_eq!(wants("holiday.MOV"), Ok(Wants::Made(Kind::Film)));
    }

    #[test]
    fn what_is_not_ours_is_not_touched() {
        assert_eq!(wants("cover.jpg"), Ok(Wants::Leave));
        assert_eq!(wants("notes.txt"), Ok(Wants::Leave));
        assert_eq!(wants("a film.kdenlive"), Ok(Wants::Leave));
        assert_eq!(wants("no extension at all"), Ok(Wants::Leave));
    }

    #[test]
    fn what_the_name_cannot_say_is_asked_of_the_file() {
        assert_eq!(wants("something.webm"), Ok(Wants::Ask));
        assert_eq!(wants("something.ogg"), Ok(Wants::Ask));
    }

    #[test]
    fn a_cover_is_not_what_makes_a_file_a_film() {
        assert_eq!(inside("video,0"), Ok(Kind::Film));
        assert_eq!(inside("video,1"), Ok(Kind::Sound));
        assert_eq!(inside(""), Ok(Kind::Sound));
    }

    #[test]
    fn the_new_name_is_the_old_one_under_the_new_extension() {
        let at = Path::new("/home/ada/Music/Africa [x].mp3");
        let Ok(beside) = beside(at, Kind::Sound);

        assert_eq!(beside, Path::new("/home/ada/Music/Africa [x].opus"));
    }

    #[test]
    fn a_film_is_moved_rather_than_decoded() {
        let Ok(argv) = film(Path::new("/a/one.mp4"), Path::new("/a/one.mkv"));
        let at = argv.iter().position(|word| word == "-c").expect("how it is coded");

        assert_eq!(argv.get(at + 1).map(String::as_str), Some("copy"));
    }

    #[test]
    fn a_song_carries_its_words_and_its_cover_over() {
        let (from, to) = (Path::new("/a/one.mp3"), Path::new("/a/one.opus"));
        let Ok(with) = sound(from, to, Some("Zm9v"));
        let Ok(without) = sound(from, to, None);

        assert!(with.contains(&"METADATA_BLOCK_PICTURE=Zm9v".to_string()));
        assert!(with.contains(&"-map_metadata".to_string()));
        assert!(!without.iter().any(|word| word.contains("PICTURE")));
    }

    #[test]
    fn base_sixty_four_is_written_the_way_everybody_else_writes_it() {
        assert_eq!(sixty_four(b""), Ok(String::new()));
        assert_eq!(sixty_four(b"f"), Ok("Zg==".to_string()));
        assert_eq!(sixty_four(b"fo"), Ok("Zm8=".to_string()));
        assert_eq!(sixty_four(b"foo"), Ok("Zm9v".to_string()));
        assert_eq!(sixty_four(b"foobar"), Ok("Zm9vYmFy".to_string()));
        assert_eq!(sixty_four(&[0xff, 0xfe, 0xfd]), Ok("//79".to_string()));
    }

    #[test]
    fn the_picture_goes_in_as_the_block_every_writer_of_one_writes() {
        let Ok(said) = block("image/jpeg", b"\xff\xd8\xff");
        let held = said.as_bytes();
        assert!(!said.contains('\n'), "one line, because it is one argument");
        assert!(held.len() > 40);
    }
}
