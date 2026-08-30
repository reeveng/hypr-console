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

use std::path::{Path, PathBuf};

use crate::store::Kind;

/// How much of a bitrate a converted song is given.
///
/// Above what anybody can pick out of a lossy source on a handheld's speakers
/// or over bluetooth, and under what the original mp3 was spending, so a folder
/// made one format is a folder that got smaller.
pub const BITRATE: &str = "128k";

/// What a name says it is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wants {
    /// It is already what it should be.
    Nothing,
    /// Not ours to touch: a picture, a text file, a project somebody is working
    /// on.
    Leave,
    /// The extension does not say. `.webm` and `.ogg` are both a song and a
    /// film depending on what is inside, and the only honest way to tell is to
    /// look.
    Ask,
    Made(Kind),
}

/// Sound that is not opus yet.
const SOUNDS: [&str; 7] = ["aac", "flac", "m4a", "mp3", "oga", "wav", "wma"];

/// A film that is not mkv yet.
const FILMS: [&str; 10] =
    ["avi", "flv", "m4v", "mov", "mp4", "mpeg", "mpg", "ts", "webm", "wmv"];

/// The two that could be either.
const EITHER: [&str; 2] = ["ogg", "webm"];

/// What should become of a file, as far as its name can say.
pub fn wants(name: &str) -> Wants {
    let Some((_, end)) = name.rsplit_once('.') else { return Wants::Leave };
    let end = end.to_lowercase();
    if end == "opus" || end == "mkv" {
        return Wants::Nothing;
    }
    if EITHER.contains(&end.as_str()) {
        return Wants::Ask;
    }
    if SOUNDS.contains(&end.as_str()) {
        return Wants::Made(Kind::Sound);
    }
    match FILMS.contains(&end.as_str()) {
        true => Wants::Made(Kind::Film),
        false => Wants::Leave,
    }
}

/// The same thing under the extension it is going to have.
pub fn beside(path: &Path, kind: Kind) -> PathBuf {
    path.with_extension(match kind {
        Kind::Sound => crate::getting::SOUND,
        Kind::Film => crate::getting::FILM,
    })
}

/// Whether what is inside says film rather than song.
///
/// Asked of `.webm` and `.ogg`, which say nothing by their name. A stream that
/// is a picture of the singer is not a film: what makes it one is moving, and a
/// cover sits in the file as a single frame.
pub fn a_film(said: &str) -> bool {
    said.lines().any(|line| {
        let mut said = line.trim().split(',');
        let kind = said.next().unwrap_or_default();
        // A picture stream marked as an attached picture is the cover. Anything
        // else that is a picture is the film.
        kind == "video" && said.next().unwrap_or("0") != "1"
    })
}

/// What is asked about a file that does not say what it is.
pub fn about(path: &Path) -> Vec<String> {
    let said = |word: &str| word.to_string();
    vec![
        said("ffprobe"),
        said("-v"),
        said("error"),
        said("-select_streams"),
        said("v"),
        said("-show_entries"),
        // The kind of each picture stream, and whether it is one frame put
        // there as a cover or a film going past.
        said("stream=codec_type,disposition=attached_pic"),
        said("-of"),
        said("csv=p=0"),
        path.to_string_lossy().to_string(),
    ]
}

/// The cover of a song, taken out so it can be put back in.
pub fn cover(from: &Path, to: &Path) -> Vec<String> {
    let said = |word: &str| word.to_string();
    vec![
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
    ]
}

/// A song, made opus, with what was written on it carried over.
///
/// The picture cannot be carried the way the tags are: the opus muxer refuses a
/// picture stream outright, which is `Unsupported codec id in stream 1` and no
/// file at all. It goes in as the comment the format actually keeps a picture
/// in, which is what everything else that writes one writes.
pub fn sound(from: &Path, to: &Path, picture: Option<&str>) -> Vec<String> {
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
    if let Some(picture) = picture {
        argv.push(said("-metadata"));
        argv.push(format!("METADATA_BLOCK_PICTURE={picture}"));
    }
    argv.push(to.to_string_lossy().to_string());
    argv
}

/// A film, moved into an mkv without being decoded.
///
/// `-c copy` is the whole of it: the same streams, the same bytes, a different
/// container. Anything that will not go in whole is left alone rather than
/// re-encoded, because half an hour of a handheld's battery is not a thing to
/// spend on a file extension without being asked.
pub fn film(from: &Path, to: &Path) -> Vec<String> {
    let said = |word: &str| word.to_string();
    vec![
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
    ]
}

/// A picture, as the comment a vorbis file keeps one in.
///
/// The block FLAC specified and everything since has used: what kind of picture
/// it is, what it is called, how big it is, and then the picture. The sizes are
/// allowed to be nothing, and are, because reading them out of the file is work
/// done to write down something every decoder works out for itself.
pub fn block(mime: &str, picture: &[u8]) -> String {
    const FRONT_COVER: u32 = 3;
    let mut held = Vec::new();
    let mut four = |number: u32| held.extend_from_slice(&number.to_be_bytes());
    four(FRONT_COVER);
    four(mime.len() as u32);
    held.extend_from_slice(mime.as_bytes());
    // No description, and no width, height, depth or colours.
    for _ in 0..5 {
        held.extend_from_slice(&0u32.to_be_bytes());
    }
    held.extend_from_slice(&(picture.len() as u32).to_be_bytes());
    held.extend_from_slice(picture);
    sixty_four(&held)
}

/// Base sixty-four, which is what that comment is written in.
pub fn sixty_four(held: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut said = String::with_capacity(held.len().div_ceil(3) * 4);
    for lot in held.chunks(3) {
        let held = lot.iter().fold(0u32, |held, byte| (held << 8) | u32::from(*byte));
        let held = held << (8 * (3 - lot.len()));
        for at in 0..4 {
            match at <= lot.len() {
                true => said.push(ALPHABET[(held >> (18 - 6 * at)) as usize & 63] as char),
                false => said.push('='),
            }
        }
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_already_the_one_format_is_left_where_it_is() {
        assert_eq!(wants("Africa [x].opus"), Wants::Nothing);
        assert_eq!(wants("Africa [x].mkv"), Wants::Nothing);
    }

    #[test]
    fn a_song_becomes_opus_and_a_film_becomes_mkv() {
        assert_eq!(wants("Africa.mp3"), Wants::Made(Kind::Sound));
        assert_eq!(wants("Africa.FLAC"), Wants::Made(Kind::Sound));
        assert_eq!(wants("holiday.mp4"), Wants::Made(Kind::Film));
        assert_eq!(wants("holiday.MOV"), Wants::Made(Kind::Film));
    }

    /// A folder holds more than what plays: the pictures a download left, a
    /// text file, somebody's project.
    #[test]
    fn what_is_not_ours_is_not_touched() {
        assert_eq!(wants("cover.jpg"), Wants::Leave);
        assert_eq!(wants("notes.txt"), Wants::Leave);
        assert_eq!(wants("a film.kdenlive"), Wants::Leave);
        assert_eq!(wants("no extension at all"), Wants::Leave);
    }

    /// Both of these are a song in one file and a film in the next, and the
    /// name says nothing about which.
    #[test]
    fn what_the_name_cannot_say_is_asked_of_the_file() {
        assert_eq!(wants("something.webm"), Wants::Ask);
        assert_eq!(wants("something.ogg"), Wants::Ask);
    }

    /// A cover is a picture stream too, and a song with one on it is still a
    /// song.
    #[test]
    fn a_cover_is_not_what_makes_a_file_a_film() {
        assert!(a_film("video,0"));
        assert!(!a_film("video,1"));
        assert!(!a_film(""));
    }

    #[test]
    fn the_new_name_is_the_old_one_under_the_new_extension() {
        let at = Path::new("/home/ada/Music/Africa [x].mp3");
        assert_eq!(beside(at, Kind::Sound), Path::new("/home/ada/Music/Africa [x].opus"));
    }

    #[test]
    fn a_film_is_moved_rather_than_decoded() {
        let argv = film(Path::new("/a/one.mp4"), Path::new("/a/one.mkv"));
        let at = argv.iter().position(|word| word == "-c").expect("how it is coded");
        assert_eq!(argv[at + 1], "copy");
    }

    /// The muxer refuses a picture stream outright, so the picture goes in as
    /// the comment the format keeps one in, and only when there is one.
    #[test]
    fn a_song_carries_its_words_and_its_cover_over() {
        let (from, to) = (Path::new("/a/one.mp3"), Path::new("/a/one.opus"));
        let with = sound(from, to, Some("Zm9v"));
        assert!(with.contains(&"METADATA_BLOCK_PICTURE=Zm9v".to_string()));
        assert!(with.contains(&"-map_metadata".to_string()));
        assert!(!sound(from, to, None).iter().any(|word| word.contains("PICTURE")));
    }

    #[test]
    fn base_sixty_four_is_written_the_way_everybody_else_writes_it() {
        assert_eq!(sixty_four(b""), "");
        assert_eq!(sixty_four(b"f"), "Zg==");
        assert_eq!(sixty_four(b"fo"), "Zm8=");
        assert_eq!(sixty_four(b"foo"), "Zm9v");
        assert_eq!(sixty_four(b"foobar"), "Zm9vYmFy");
        assert_eq!(sixty_four(&[0xff, 0xfe, 0xfd]), "//79");
    }

    /// The block the comment holds: what kind of picture, what it is called,
    /// and then the picture itself.
    #[test]
    fn the_picture_goes_in_as_the_block_every_writer_of_one_writes() {
        let said = block("image/jpeg", b"\xff\xd8\xff");
        let held = said.as_bytes();
        assert!(!said.contains('\n'), "one line, because it is one argument");
        assert!(held.len() > 40);
    }
}
