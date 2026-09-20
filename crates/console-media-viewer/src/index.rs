//! Everything on this device that can be shown, in one list under its letter.
//!
//! The Media page was the folder the picture came out of, which is the right
//! answer to "what else is beside this" and the wrong one to "what have I
//! got". A film fetched by the download panel lands in Videos, a photograph
//! lands in Pictures, and the viewer pressed on the home screen opens Pictures
//! -- so the films were on the device, in a folder with a name, and there was
//! no road to them from the panel that shows films. What a person asks a
//! viewer for is their pictures and their films, not the directory tree they
//! happen to be filed in.
//!
//! So this walks the folders a person keeps media in and puts what it finds
//! under a heading for the letter it starts with, the way anybody's shelf of
//! anything is arranged. The reel is untouched: next and previous still walk
//! the folder the thing being watched is in, because that is what next means
//! while you are looking at one of a set.
//!
//! ## Which folders, and why not the whole home
//!
//! Pictures, Videos and Downloads, and the folder the panel was opened on when
//! it is not already inside one of them. The whole home directory was the
//! first answer and it is the wrong one on this device in particular: a game
//! installs thousands of textures and title cards under it, every one of them
//! a `.png` this panel would claim it could show, and a list of a person's
//! photographs with four thousand sprite sheets in it is not a list of their
//! photographs. The three folders are where everything that arrives here
//! actually arrives -- `console_downloads::getting` writes films into Videos
//! and the browser writes into Downloads -- so what is left out is what
//! somebody filed by hand somewhere else, and the folder they opened is the
//! way back to that.
//!
//! The heading itself is `console_panel::page`: what letter a row stands under
//! is a question about a list on a screen rather than about media, and the
//! music panel's folder asks it too.
//!
//! ## Nothing here reads a disk
//!
//! The walk is handed a reader, the same way `console_music::looking`
//! is, so the shape of a library can be asked about without one on the machine
//! asking. The caps are that crate's and for its reason: a walk that would go
//! on for minutes is a page that never draws, and stopping is better than
//! either.

use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::kinds::{self, Kind};

const ENOUGH: usize = 4000;

const FAR: usize = 400;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Read {
    pub name: String,
    pub path: PathBuf,
    pub folder: bool,
    pub mime: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Found {
    pub name: String,
    pub path: PathBuf,
    pub kind: Kind,
}

pub fn kept(folders: &[PathBuf]) -> Result<Vec<PathBuf>, Never> {
    let every: BTreeSet<&Path> = folders.iter().map(PathBuf::as_path).collect();
    let mut kept: Vec<PathBuf> = Vec::new();
    let mut already: BTreeSet<&Path> = BTreeSet::new();

    for folder in folders {
        let inside = folder.ancestors().skip(1).any(|above| every.contains(above));

        match inside || !already.insert(folder.as_path()) {
            true => {},
            false => kept.push(folder.clone()),
        }
    }

    Ok(kept)
}

pub fn under(
    folders: &[PathBuf],
    read: &dyn Fn(&Path) -> Result<Vec<Read>, Never>,
) -> Result<Vec<Found>, Never> {
    let mut found: Vec<Found> = Vec::new();
    let mut waiting: VecDeque<PathBuf> = folders.iter().cloned().collect();
    let mut read_so_far: usize = 0;

    while let Some(at) = waiting.pop_front() {
        match found.len() >= ENOUGH || read_so_far >= FAR {
            true => break,
            false => {},
        }

        read_so_far = read_so_far.saturating_add(1);

        let here = read(&at)?;

        'over_things: for thing in here {
            match thing.name.starts_with('.') {
                true => continue 'over_things,
                false => {},
            }

            match thing.folder {
                true => waiting.push_back(thing.path),
                false => {
                    let Ok(kind) = kinds::of(&thing.mime);

                    match kind {
                        Some(kind) => {
                            found.push(Found { name: thing.name, path: thing.path, kind })
                        },
                        None => {},
                    }
                },
            }
        }
    }

    sorted(found)
}

pub fn sorted(mut found: Vec<Found>) -> Result<Vec<Found>, Never> {
    found.sort_by(|one, other| {
        let Ok(first) = console_panel::page::standing(&one.name);
        let Ok(second) = console_panel::page::standing(&other.name);

        first
            .cmp(&second)
            .then_with(|| one.name.to_lowercase().cmp(&other.name.to_lowercase()))
            .then_with(|| one.path.cmp(&other.path))
    });
    found.dedup_by(|one, other| one.path == other.path);

    Ok(found)
}



#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn at(name: &str, mime: &str) -> Read {
        Read {
            name: name.to_string(),
            path: PathBuf::from("/home/somebody/Pictures").join(name),
            folder: false,
            mime: mime.to_string(),
        }
    }

    fn found(name: &str, kind: Kind) -> Found {
        Found {
            name: name.to_string(),
            path: PathBuf::from("/home/somebody/Pictures").join(name),
            kind,
        }
    }

    fn reader(tree: BTreeMap<PathBuf, Vec<Read>>) -> impl Fn(&Path) -> Result<Vec<Read>, Never> {
        move |at: &Path| Ok(tree.get(at).cloned().unwrap_or_default())
    }

    fn one_folder(things: Vec<Read>) -> BTreeMap<PathBuf, Vec<Read>> {
        BTreeMap::from([(PathBuf::from("/home/somebody/Pictures"), things)])
    }

    fn pictures() -> Vec<PathBuf> {
        vec![PathBuf::from("/home/somebody/Pictures")]
    }

    fn under(tree: BTreeMap<PathBuf, Vec<Read>>) -> Vec<Found> {
        let Ok(under) = super::under(&pictures(), &reader(tree));

        under
    }

    #[test]
    fn a_picture_and_a_film_are_both_in_the_list_and_a_document_is_not() {
        let walked = under(one_folder(vec![
            at("beach.jpg", "image/jpeg"),
            at("notes.txt", "text/plain"),
            at("holiday.mkv", "video/matroska"),
        ]));

        assert_eq!(walked, vec![found("beach.jpg", Kind::Picture), found("holiday.mkv", Kind::Film)]);
    }

    #[test]
    fn the_walk_goes_into_the_folders_it_finds() {
        let tree = BTreeMap::from([
            (
                PathBuf::from("/home/somebody/Pictures"),
                vec![Read {
                    name: "2019".to_string(),
                    path: PathBuf::from("/home/somebody/Pictures/2019"),
                    folder: true,
                    mime: "inode/directory".to_string(),
                }],
            ),
            (
                PathBuf::from("/home/somebody/Pictures/2019"),
                vec![Read {
                    name: "boat.png".to_string(),
                    path: PathBuf::from("/home/somebody/Pictures/2019/boat.png"),
                    folder: false,
                    mime: "image/png".to_string(),
                }],
            ),
        ]);

        let walked = under(tree);

        assert_eq!(walked.len(), 1);
        assert_eq!(walked.first().map(|found| found.name.as_str()), Some("boat.png"));
    }

    #[test]
    fn a_hidden_thing_is_not_somebodys_media_and_neither_is_what_is_under_it() {
        let tree = BTreeMap::from([
            (
                PathBuf::from("/home/somebody/Pictures"),
                vec![
                    at(".thumbnail.jpg", "image/jpeg"),
                    Read {
                        name: ".cache".to_string(),
                        path: PathBuf::from("/home/somebody/Pictures/.cache"),
                        folder: true,
                        mime: "inode/directory".to_string(),
                    },
                ],
            ),
            (
                PathBuf::from("/home/somebody/Pictures/.cache"),
                vec![at("kept.jpg", "image/jpeg")],
            ),
        ]);

        assert_eq!(under(tree), Vec::new());
    }

    #[test]
    fn the_list_is_in_the_order_somebody_reads_it_and_case_is_not_a_sort() {
        let walked = under(one_folder(vec![
            at("zebra.jpg", "image/jpeg"),
            at("Apple.jpg", "image/jpeg"),
            at("apricot.jpg", "image/jpeg"),
        ]));
        let names: Vec<&str> = walked.iter().map(|found| found.name.as_str()).collect();

        assert_eq!(names, ["Apple.jpg", "apricot.jpg", "zebra.jpg"]);
    }

    #[test]
    fn the_numbers_come_first_and_what_begins_with_neither_comes_last() {
        let walked = under(one_folder(vec![
            at("_draft.png", "image/png"),
            at("boat.png", "image/png"),
            at("2019.png", "image/png"),
        ]));
        let names: Vec<&str> = walked.iter().map(|found| found.name.as_str()).collect();

        assert_eq!(names, ["2019.png", "boat.png", "_draft.png"], "Other is a heading, not a letter");
    }

    #[test]
    fn a_folder_inside_another_is_walked_once_rather_than_twice() {
        let Ok(kept) = kept(&[
            PathBuf::from("/home/somebody/Pictures"),
            PathBuf::from("/home/somebody/Pictures/2019"),
            PathBuf::from("/home/somebody/Videos"),
            PathBuf::from("/home/somebody/Videos"),
        ]);

        assert_eq!(
            kept,
            vec![PathBuf::from("/home/somebody/Pictures"), PathBuf::from("/home/somebody/Videos")]
        );
    }

    #[test]
    fn a_walk_down_a_tree_with_no_bottom_stops_rather_than_never_drawing() {
        let deep = FAR.saturating_add(50);
        let mut tree: BTreeMap<PathBuf, Vec<Read>> = BTreeMap::new();

        for step in 0..deep {
            let here = PathBuf::from("/home/somebody/Pictures").join("down".repeat(step));
            let below = PathBuf::from("/home/somebody/Pictures").join("down".repeat(step + 1));

            tree.insert(here, vec![
                Read {
                    name: "down".to_string(),
                    path: below,
                    folder: true,
                    mime: "inode/directory".to_string(),
                },
                at(&format!("{step:05}.jpg"), "image/jpeg"),
            ]);
        }

        assert_eq!(under(tree).len(), FAR, "the walk read {FAR} folders and stopped");
    }
}
