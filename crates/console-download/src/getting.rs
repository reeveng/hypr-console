//! What fetches one thing, and where it lands.
//!
//! Nothing is asked. A person who typed a song's name has said what they want,
//! and a list of formats is a question about codecs asked of somebody holding a
//! handheld. So the file is chosen by a rule written once, here: the smallest
//! one that is still worth having on this screen.
//!
//! For sound that is the best the site has, because the best audio a site keeps
//! is already the small one -- four minutes of opus is four megabytes, and the
//! streams under it are the ones that sound like a telephone. For a film it is
//! the other way round: the largest is four times the size of the one this
//! screen can show, so the rule is the smallest file at the height worth
//! having.
//!
//! The picture goes inside the file either way. A song with no cover is a row
//! in the music panel with a grey square where the sleeve should be, and the
//! picture is on the page the thing was fetched from anyway.

use std::path::{Path, PathBuf};

use console_external_programs::Program;
use console_never::Never;
use gtk4::glib::{self, UserDirectory};

use crate::store::Kind;

pub const TALL: &str = "1080";

pub const SOUND: &str = "opus";

pub const FILM: &str = "mkv";

pub const NAMED: &str = "%(title)s [%(id)s].%(ext)s";

pub fn into(kind: Kind) -> Result<PathBuf, Never> {
    Ok(match kind {
        Kind::Sound => console_music::library::folder()?,
        Kind::Film => glib::user_special_dir(UserDirectory::Videos)
            .unwrap_or_else(|| glib::home_dir().join("Videos")),
    })
}

pub fn argv(kind: Kind, url: &str, into: &Path) -> Result<Vec<String>, Never> {
    let said = |word: &str| word.to_string();
    let Ok(yt_dlp) = Program::YtDlp.name();

    let mut argv = vec![
        said(yt_dlp),
        said("--no-playlist"),
        said("--embed-thumbnail"),
        said("--embed-metadata"),
        said("--convert-thumbnails"),
        said("jpg"),
        said("--paths"),
        into.to_string_lossy().to_string(),
        said("--output"),
        said(NAMED),
        said("--no-simulate"),
        said("--print"),
        said("after_move:filepath"),
    ];
    argv.extend(match kind {
        Kind::Sound => vec![
            said("--format"),
            said("bestaudio/best"),
            said("--extract-audio"),
            said("--audio-format"),
            said(SOUND),
        ],
        Kind::Film => vec![
            said("--format"),
            said("bestvideo*+bestaudio/best"),
            said("--format-sort"),
            format!("res:{TALL},+size"),
            said("--merge-output-format"),
            said(FILM),
        ],
    });
    argv.push(said("--"));
    argv.push(said(url));
    Ok(argv)
}

pub fn id_in(url: &str) -> Result<Option<String>, Never> {
    let after = |mark: &str| url.split_once(mark).map(|(_, rest)| rest);

    let Some(said) = after("watch?v=")
        .or_else(|| after("youtu.be/"))
        .or_else(|| after("shorts/"))
        .or_else(|| after("/v/"))
    else {
        return Ok(None);
    };

    let end = |letter: char| letter == '&' || letter == '?' || letter == '/' || letter == '#';

    crate::store::named(said.split(end).next().unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Litter {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Have {
    It,
    Not,
}

pub fn leftover(name: &str) -> Result<Litter, Never> {
    let ends = [".part", ".ytdl", ".meta"];
    let half = name.contains(".temp.") || ends.iter().any(|end| name.ends_with(end));

    Ok(match half {
        true => Litter::Yes,
        false => Litter::No,
    })
}

pub fn have_it(names: impl IntoIterator<Item = String>, id: &str) -> Result<Have, Never> {
    let mark = format!("[{id}]");
    let found = names
        .into_iter()
        .any(|name| {
            let Ok(litter) = leftover(&name);

            litter == Litter::No && name.contains(&mark)
        });

    Ok(match found {
        true => Have::It,
        false => Have::Not,
    })
}

pub fn holds(folder: &Path, id: &str) -> Result<Have, Never> {
    let Ok(reading) = std::fs::read_dir(folder) else {
        return Ok(Have::Not);
    };

    let names = reading.flatten().map(|entry| entry.file_name().to_string_lossy().to_string());

    have_it(names, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(kind: Kind) -> Vec<String> {
        let Ok(argv) = argv(kind, "https://youtu.be/abc", Path::new("/home/ada/Music"));

        argv
    }

    fn after(argv: &[String], flag: &str) -> String {
        let at = argv.iter().position(|word| word == flag).expect(flag);
        argv[at + 1].clone()
    }

    #[test]
    fn the_thing_that_fetches_is_yt_dlp_and_the_link_is_the_last_word() {
        let said = words(Kind::Sound);
        let Ok(yt_dlp) = Program::YtDlp.name();

        assert_eq!(said[0], yt_dlp);
        assert_eq!(said[said.len() - 2], "--", "a link is a link and never a flag");
        assert_eq!(said[said.len() - 1], "https://youtu.be/abc");
    }

    #[test]
    fn a_film_is_the_smallest_file_at_the_height_this_screen_can_show() {
        let said = words(Kind::Film);
        assert_eq!(after(&said, "--format-sort"), format!("res:{TALL},+size"));
        assert_eq!(after(&said, "--merge-output-format"), FILM);
    }

    #[test]
    fn sound_is_the_best_the_site_has_and_is_unwrapped_rather_than_encoded() {
        let said = words(Kind::Sound);
        assert_eq!(after(&said, "--format"), "bestaudio/best");
        assert_eq!(after(&said, "--audio-format"), SOUND);
    }

    #[test]
    fn the_picture_goes_inside_the_file_whichever_kind_it_is() {
        for kind in Kind::BOTH {
            assert!(words(kind).contains(&"--embed-thumbnail".to_string()));
        }
    }

    #[test]
    fn a_fetched_song_is_named_the_way_the_music_library_reads_a_name() {
        assert_eq!(after(&words(Kind::Sound), "--output"), NAMED);
        assert_eq!(
            console_music::library::named("Africa [FTQbiNvZqaY].opus"),
            Ok("Africa".to_string()),
        );
    }

    #[test]
    fn a_folder_that_already_holds_it_is_known_by_the_id_in_the_name() {
        let names = ["Africa [FTQbiNvZqaY].opus".to_string(), "notes.txt".to_string()];
        assert_eq!(have_it(names.clone(), "FTQbiNvZqaY"), Ok(Have::It));
        assert_eq!(have_it(names, "qU9mHegkTc4"), Ok(Have::Not));
    }

    #[test]
    fn what_a_failed_fetch_left_behind_is_not_having_it() {
        let litter = ["Africa [FTQbiNvZqaY].temp.opus".to_string()];
        assert_eq!(have_it(litter, "FTQbiNvZqaY"), Ok(Have::Not));
    }

    #[test]
    fn the_sites_name_for_a_thing_is_read_out_of_a_link_to_it() {
        let id = Some("jNQXAC9IVRw".to_string());
        assert_eq!(id_in("https://www.youtube.com/watch?v=jNQXAC9IVRw"), Ok(id.clone()));
        assert_eq!(id_in("https://www.youtube.com/watch?v=jNQXAC9IVRw&t=42"), Ok(id.clone()));
        assert_eq!(id_in("https://youtu.be/jNQXAC9IVRw"), Ok(id.clone()));
        assert_eq!(id_in("https://www.youtube.com/shorts/jNQXAC9IVRw"), Ok(id));
        assert_eq!(id_in("https://example.com/a-film.mp4"), Ok(None));
    }

    #[test]
    fn what_a_failed_fetch_leaves_behind_is_known_by_its_name() {
        assert_eq!(leftover("Africa [x].temp.opus"), Ok(Litter::Yes));
        assert_eq!(leftover("Africa [x].meta"), Ok(Litter::Yes));
        assert_eq!(leftover("Africa [x].opus.part"), Ok(Litter::Yes));
        assert_eq!(leftover("Africa [x].opus"), Ok(Litter::No));
        assert_eq!(leftover("Africa [x].mkv"), Ok(Litter::No));
    }
}
