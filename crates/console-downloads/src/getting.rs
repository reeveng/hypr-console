//! What fetches one thing, and where it lands.
//!
//! Nothing is asked. A person who typed a song's name has said what they want,
//! and a list of formats is a question about codecs asked of someone holding a
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
//! in the music panel with a gray square where the sleeve should be, and the
//! picture is on the page the thing was fetched from anyway.
//!
//! A book is none of that. It is one file at one address on Project
//! Gutenberg, fetched with curl into the folder the library reads, and named by
//! its title and its number the way a song is named by its title and its id,
//! so the same question answers whether it is already there.

use std::path::{Path, PathBuf};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Folder;

use crate::store::Kind;


pub const TALL: &str = "1080";

pub const SOUND: &str = "opus";

pub const FILM: &str = "mkv";

pub const BOOK: &str = "epub";

pub const NAMED: &str = "%(title)s [%(id)s].%(ext)s";

pub const LONGEST_TITLE: u32 = 120;

pub fn into(kind: Kind) -> Result<PathBuf, Never> {
    let films = Folder::Videos.hers()?;
    let home = console_core_places::home()?;

    Ok(match kind {
        Kind::Sound => console_music_player::library::folder()?,
        Kind::Film => match films {
            Some(into) => into,

            None => {
                eprintln!("console-downloads: no HOME; a film is fetched into here");

                PathBuf::new()
            }
        },
        Kind::Book => match home {
            Some(home) => console_books::library::books_folder(&home)?,
            None => {
                eprintln!("console-downloads: no HOME; a book is fetched into here");

                PathBuf::new()
            },
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fetch<'a> {
    pub kind: Kind,
    pub url: &'a str,
    pub into: &'a Path,
    pub title: &'a str,
}

pub fn arguments(fetch: Fetch<'_>) -> Result<Vec<String>, Never> {
    let Fetch { kind, url, into, title } = fetch;

    match kind {
        Kind::Sound | Kind::Film => {},
        Kind::Book => return book(Fetch { kind, url, into, title }),
    }

    let said = |word: &str| word.to_string();
    let Ok(yt_dlp) = Program::YtDlp.name();

    let mut arguments = vec![
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
    arguments.extend(match kind {
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
        Kind::Book => Vec::new(),
    });
    arguments.push(said("--"));
    arguments.push(said(url));
    Ok(arguments)
}

fn book(fetch: Fetch<'_>) -> Result<Vec<String>, Never> {
    let Fetch { url, into, title, .. } = fetch;
    let Ok(curl) = Program::Curl.name();
    let Ok(id) = id_in(url);
    let Ok(file_name) = file_name(title);
    let Ok(standard) = crate::standard_ebooks::publication(url);

    let address = match standard {
        Some(address) => address,
        None => {
            let Ok(address) = crate::gutenberg::publication(url);

            address
        },
    };

    let named = match id {
        Some(id) => format!("{file_name} [{id}].{BOOK}"),
        None => format!("{file_name}.{BOOK}"),
    };

    Ok(vec![
        curl.to_string(),
        "--silent".to_string(),
        "--show-error".to_string(),
        "--fail".to_string(),
        "--location".to_string(),
        "--max-time".to_string(),
        "300".to_string(),
        "--remove-on-error".to_string(),
        "--output".to_string(),
        into.join(format!("{named}{UNFINISHED}")).to_string_lossy().to_string(),
        "--write-out".to_string(),
        "%{filename_effective}".to_string(),
        "--".to_string(),
        address,
    ])
}

pub fn file_name(title: &str) -> Result<String, Never> {
    let Ok(longest) = console_core_number_conversion::index(LONGEST_TITLE);

    let plain: String = title
        .chars()
        .map(|letter| match letter == '/' || letter.is_control() {
            true => ' ',
            false => letter,
        })
        .take(longest)
        .collect();

    let plain = plain.trim_start_matches(|letter: char| letter == '.' || letter.is_whitespace()).trim_end();

    Ok(match plain.is_empty() {
        true => "Book".to_string(),
        false => plain.to_string(),
    })
}

pub fn id_in(url: &str) -> Result<Option<String>, Never> {
    let Ok(standard) = crate::standard_ebooks::id_in(url);

    match standard {
        Some(id) => return Ok(Some(id)),
        None => {},
    }

    let after = |mark: &str| url.split_once(mark).map(|(_, rest)| rest);

    let said = match after("watch?v=")
        .or_else(|| after("youtu.be/"))
        .or_else(|| after("shorts/"))
        .or_else(|| after("/v/"))
        .or_else(|| after("/ebooks/"))
    {
        Some(said) => said,
        None => return Ok(None),
    };

    let end = |letter: char| letter == '&' || letter == '?' || letter == '/' || letter == '#' || letter == '.';

    let before = match said.split(end).next() {
        Some(before) => before,
        None => said,
    };

    crate::store::named(before)
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

pub const UNFINISHED: &str = ".part";

pub fn finished(part: &str) -> Result<Option<&str>, Never> {
    Ok(part.strip_suffix(UNFINISHED))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Book<'a> {
    pub id: &'a str,
    pub title: &'a str,
}

pub fn copies(names: impl IntoIterator<Item = String>, book: Book<'_>) -> Result<Vec<String>, Never> {
    let mark = format!("[{}]", book.id);
    let Ok(called) = file_name(book.title);
    let untagged = format!("{called}.{BOOK}");

    Ok(names
        .into_iter()
        .filter(|name| {
            let Ok(litter) = leftover(name);
            let stem = name.rsplit_once(" [").map(|(stem, _)| stem);
            let same = name.contains(&mark) || stem == Some(called.as_str()) || *name == untagged;

            litter == Litter::No && name.ends_with(&format!(".{BOOK}")) && same
        })
        .collect())
}

pub fn holds(folder: &Path, id: &str) -> Result<Have, Never> {
    let reading = match std::fs::read_dir(folder) {
        Ok(reading) => reading,
        Err(_unreadable) => return Ok(Have::Not),
    };

    let names = reading.flatten().map(|entry| entry.file_name().to_string_lossy().to_string());

    have_it(names, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(kind: Kind) -> Vec<String> {
        let Ok(arguments) = arguments(Fetch { kind, url: "https://youtu.be/abc", into: Path::new("/home/ada/Music"), title: "abc" });

        arguments
    }

    fn after(arguments: &[String], flag: &str) -> String {
        arguments.iter().skip_while(|word| *word != flag).nth(1).expect(flag).clone()
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
            console_music_player::library::named("Africa [FTQbiNvZqaY].opus"),
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
    fn a_book_is_its_gutenberg_number_named_the_way_a_song_is() {
        let Ok(arguments) = arguments(Fetch {
            kind: Kind::Book,
            url: "https://www.gutenberg.org/ebooks/84",
            into: Path::new("/home/ada/Books"),
            title: "Frankenstein; or/the modern prometheus",
        });

        assert_eq!(after(&arguments, "--output"), "/home/ada/Books/Frankenstein; or the modern prometheus [84].epub.part");
        assert_eq!(arguments.last().map(String::as_str), Some("https://www.gutenberg.org/ebooks/84.epub3.images"));
        assert_eq!(have_it(["Frankenstein [84].epub".to_string()], "84"), Ok(Have::It));
    }

    #[test]
    fn a_book_held_under_its_id_or_its_title_from_either_library_is_the_same_book() {
        let names = [
            "Meditations [2680].epub",
            "Meditations [marcus-aurelius_meditations_george-long].epub",
            "Meditations.epub",
            "Meditations [2680].epub.part",
            "Meditations and Other Essays [999].epub",
            "Frankenstein [84].epub",
        ]
        .map(str::to_string);

        assert_eq!(
            copies(names, Book { id: "2680", title: "Meditations" }),
            Ok(vec![
                "Meditations [2680].epub".to_string(),
                "Meditations [marcus-aurelius_meditations_george-long].epub".to_string(),
                "Meditations.epub".to_string(),
            ])
        );
        assert_eq!(finished("/home/ada/Books/Meditations [2680].epub.part"), Ok(Some("/home/ada/Books/Meditations [2680].epub")));
    }

    #[test]
    fn a_title_cannot_leave_the_folder_it_is_written_into() {
        assert_eq!(file_name("../../.bashrc"), Ok("bashrc".to_string()), "nor hide in it");
        assert_eq!(file_name("..."), Ok("Book".to_string()));
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
