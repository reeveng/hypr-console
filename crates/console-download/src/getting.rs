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

use gtk4::glib::{self, UserDirectory};

use crate::store::Kind;

/// The tallest picture worth keeping, in lines.
///
/// The screen is 2560 by 1600 and this is well under it. A handheld held at
/// arm's length cannot show the difference between this and the one above it,
/// and the one above it is three times the file and three times the battery to
/// decode. It is the one number in this crate worth arguing about, which is why
/// it is a name rather than a number inside a string.
pub const TALL: &str = "1080";

/// What sound is kept as.
///
/// What the site already has it in, so the file is unwrapped rather than
/// re-encoded: a second encode of a lossy stream is quality thrown away for
/// nothing. It is also what the music player here plays and what its library
/// already lists.
pub const SOUND: &str = "opus";

/// What a film is kept as.
///
/// The one container that takes any pair of streams the site offers without
/// re-encoding either, and takes the picture as an attachment while it is at
/// it. mp4 wants a second program on the machine before it will hold a
/// thumbnail, and that is a package to install for a picture nothing but a file
/// manager ever draws.
pub const FILM: &str = "mkv";

/// How a fetched file is named.
///
/// yt-dlp's own default, said here rather than left implied, because the music
/// library reads it back: it takes the id in square brackets off the end to get
/// the title of a song. Two programs agreeing about a filename by accident is
/// two programs that disagree the day one of them changes.
pub const NAMED: &str = "%(title)s [%(id)s].%(ext)s";

/// Where each kind lands.
///
/// The music player's own folder for sound, asked of the player rather than
/// guessed at, so a song fetched here is one the Music panel lists a moment
/// later without anything being moved.
pub fn into(kind: Kind) -> PathBuf {
    match kind {
        Kind::Sound => console_music::library::folder(),
        Kind::Film => glib::user_special_dir(UserDirectory::Videos)
            .unwrap_or_else(|| glib::home_dir().join("Videos")),
    }
}

/// The words that fetch one thing.
pub fn argv(kind: Kind, url: &str, into: &Path) -> Vec<String> {
    let said = |word: &str| word.to_string();
    let mut argv = vec![
        said("yt-dlp"),
        // A link out of a browser is often a link into a playlist, and nobody
        // pressing A on one row asked for the two hundred things behind it.
        said("--no-playlist"),
        said("--embed-thumbnail"),
        said("--embed-metadata"),
        // The site's own picture is a webp, which ffmpeg will put inside a file
        // that half the players here then draw as nothing.
        said("--convert-thumbnails"),
        said("jpg"),
        said("--paths"),
        into.to_string_lossy().to_string(),
        said("--output"),
        said(NAMED),
        // What was written, so the program that ran this can say what arrived
        // by name. Asking for anything printed makes yt-dlp pretend to fetch,
        // which is what the second of these is undoing.
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
            // The height first and the size after it: of everything at that
            // height or under, the smallest file.
            said("--format-sort"),
            format!("res:{TALL},+size"),
            said("--merge-output-format"),
            said(FILM),
        ],
    });
    argv.push(said("--"));
    argv.push(said(url));
    argv
}

/// The site's name for a thing, out of a link to it.
///
/// The panel knows it already, but a link typed into the line is a link nobody
/// has looked up yet, and what is asked of the folder has to be the same
/// question either way. Everything YouTube writes is `watch?v=`, `youtu.be/` or
/// `shorts/`, and anything that is not one of those is a link this cannot
/// answer for rather than a wrong answer.
pub fn id_in(url: &str) -> Option<String> {
    let after = |mark: &str| url.split_once(mark).map(|(_, rest)| rest);
    let said = after("watch?v=")
        .or_else(|| after("youtu.be/"))
        .or_else(|| after("shorts/"))
        .or_else(|| after("/v/"))?;
    let end = |letter: char| letter == '&' || letter == '?' || letter == '/' || letter == '#';
    crate::store::named(said.split(end).next().unwrap_or_default())
}

/// Whether a name is something a fetch left behind rather than a thing to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Litter {
    /// Half of a fetch, under a name nobody chose.
    Yes,
    /// A file somebody meant to have.
    No,
}

/// Whether a folder already holds the thing that was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Have {
    /// It is there, under the id it was fetched by.
    It,
    /// It is not.
    Not,
}

/// What a fetch leaves behind when it fails partway.
///
/// yt-dlp tidies up after itself when a download fails and does not when a
/// postprocessor does: a conversion that will not run leaves the metadata it
/// had written and the half-made file beside the thing it was making. The
/// half-made one is the reason this matters rather than being untidy -- it is
/// called `.temp.opus`, which ends in an extension the music panel lists, so
/// the folder grows a second copy of the song under a name nobody chose.
pub fn leftover(name: &str) -> Litter {
    let ends = [".part", ".ytdl", ".meta"];
    let half = name.contains(".temp.") || ends.iter().any(|end| name.ends_with(end));

    match half {
        true => Litter::Yes,
        false => Litter::No,
    }
}

/// Whether a folder already holds a thing, by the id in the name.
///
/// By the id rather than by the title, because the title is the site's and can
/// be anything, and because the extension is not known until it has been
/// fetched. It is the same square brackets the music library takes off a name
/// to read it.
pub fn have_it(names: impl IntoIterator<Item = String>, id: &str) -> Have {
    let mark = format!("[{id}]");
    let found = names
        .into_iter()
        .any(|name| leftover(&name) == Litter::No && name.contains(&mark));

    match found {
        true => Have::It,
        false => Have::Not,
    }
}

/// The same, asked of a folder on the disk.
pub fn holds(folder: &Path, id: &str) -> Have {
    let Ok(reading) = std::fs::read_dir(folder) else {
        return Have::Not;
    };

    let names = reading.flatten().map(|entry| entry.file_name().to_string_lossy().to_string());
    have_it(names, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(kind: Kind) -> Vec<String> {
        argv(kind, "https://youtu.be/abc", Path::new("/home/ada/Music"))
    }

    fn after(argv: &[String], flag: &str) -> String {
        let at = argv.iter().position(|word| word == flag).expect(flag);
        argv[at + 1].clone()
    }

    #[test]
    fn the_thing_that_fetches_is_yt_dlp_and_the_link_is_the_last_word() {
        let said = words(Kind::Sound);
        assert_eq!(said[0], "yt-dlp");
        assert_eq!(said[said.len() - 2], "--", "a link is a link and never a flag");
        assert_eq!(said[said.len() - 1], "https://youtu.be/abc");
    }

    /// The rule, which is the whole reason nothing is asked.
    #[test]
    fn a_film_is_the_smallest_file_at_the_height_this_screen_can_show() {
        let said = words(Kind::Film);
        assert_eq!(after(&said, "--format-sort"), format!("res:{TALL},+size"));
        assert_eq!(after(&said, "--merge-output-format"), FILM);
    }

    /// A second encode of a lossy stream is quality thrown away for nothing,
    /// and the format asked for is the one the site already has.
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

    /// The two of them have to agree about a filename, and the way they agree
    /// is that this one writes the name the other one reads.
    #[test]
    fn a_fetched_song_is_named_the_way_the_music_library_reads_a_name() {
        assert_eq!(after(&words(Kind::Sound), "--output"), NAMED);
        assert_eq!(console_music::library::named("Africa [FTQbiNvZqaY].opus"), "Africa");
    }

    #[test]
    fn a_folder_that_already_holds_it_is_known_by_the_id_in_the_name() {
        let names = ["Africa [FTQbiNvZqaY].opus".to_string(), "notes.txt".to_string()];
        assert_eq!(have_it(names.clone(), "FTQbiNvZqaY"), Have::It);
        assert_eq!(have_it(names, "qU9mHegkTc4"), Have::Not);
    }

    /// What a failed fetch left behind is not the song. Counted as one, the
    /// row would say "have it" about a file that will not play, and the fetch
    /// that would have mended it is the one thing that row then refuses.
    #[test]
    fn what_a_failed_fetch_left_behind_is_not_having_it() {
        let litter = ["Africa [FTQbiNvZqaY].temp.opus".to_string()];
        assert_eq!(have_it(litter, "FTQbiNvZqaY"), Have::Not);
    }

    #[test]
    fn the_sites_name_for_a_thing_is_read_out_of_a_link_to_it() {
        let id = Some("jNQXAC9IVRw".to_string());
        assert_eq!(id_in("https://www.youtube.com/watch?v=jNQXAC9IVRw"), id);
        assert_eq!(id_in("https://www.youtube.com/watch?v=jNQXAC9IVRw&t=42"), id);
        assert_eq!(id_in("https://youtu.be/jNQXAC9IVRw"), id);
        assert_eq!(id_in("https://www.youtube.com/shorts/jNQXAC9IVRw"), id);
        assert_eq!(id_in("https://example.com/a-film.mp4"), None);
    }

    /// The half-made file is called `.temp.opus`, which ends in an extension
    /// the music panel lists: left there, the folder has two of the song and
    /// one of them is broken.
    #[test]
    fn what_a_failed_fetch_leaves_behind_is_known_by_its_name() {
        assert_eq!(leftover("Africa [x].temp.opus"), Litter::Yes);
        assert_eq!(leftover("Africa [x].meta"), Litter::Yes);
        assert_eq!(leftover("Africa [x].opus.part"), Litter::Yes);
        assert_eq!(leftover("Africa [x].opus"), Litter::No);
        assert_eq!(leftover("Africa [x].mkv"), Litter::No);
    }
}
