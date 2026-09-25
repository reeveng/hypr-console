//! What applications this machine has, and how to start one.
//!
//! The menu read all of this inside its own program for as long as it was the
//! only thing that wanted it. The home screen wants the same answer: it draws
//! a few of these on the wallpaper, and the card that puts one there lists
//! every one of them. Two programs reading the desktop files two ways would be
//! two answers to one question, and the second of them would be wrong in some
//! way no one had thought about -- a Steam icon found here and not there, a
//! terminal program run without its terminal.
//!
//! So it is here, once, and the menu is one of the callers rather than the
//! owner.
//!
//! An application whose icon cannot be found is given the theme's picture of
//! a program rather than none, because a square with nothing in it reads as
//! a hole in the home screen rather than as something to press.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_external_programs::{Program, installed};
use console_core_never::Never;
use console_core_places::Base;

use crate::entry::Application;
use crate::icons::{FALLBACKS, UNPICTURED, steam_appid};
use crate::{counts, entry, icons, image, cache, words};

const HEAD: u64 = 65536;

const WHAT_THIS_CRATE_IS_CALLED: &str = "console-applications";

fn whoami() -> Result<String, Never> {
    let argv0 = std::env::args()
        .next()
        .and_then(|argv0| Path::new(&argv0).file_name().map(|name| name.to_string_lossy().to_string()));

    Ok(match argv0 {
        Some(called) => called,
        None => WHAT_THIS_CRATE_IS_CALLED.to_string(),
    })
}

pub fn counts_at() -> Result<Option<PathBuf>, Never> {
    let ours = Base::State.ours()?;

    Ok(ours.map(|ours| ours.join("menu-counts")))
}

fn index_at() -> Result<Option<PathBuf>, Never> {
    let ours = Base::Cache.ours()?;

    Ok(ours.map(|ours| ours.join("icon-index")))
}

fn kept_at() -> Result<Option<PathBuf>, Never> {
    let ours = Base::Cache.ours()?;

    Ok(ours.map(|ours| ours.join("menu-apps")))
}

fn icon_roots() -> Result<Vec<PathBuf>, Never> {
    let home = console_core_places::home()?;

    let share = Base::Share.hers()?;

    let mut roots = vec![PathBuf::from("/usr/share/icons"), PathBuf::from("/usr/share/pixmaps")];

    roots.extend(share.into_iter().map(|share| share.join("icons")));
    roots.extend(home.into_iter().map(|home| home.join(".icons")));

    Ok(roots)
}

fn steam_roots() -> Result<Vec<PathBuf>, Never> {
    let home = console_core_places::home()?;

    let share = Base::Share.hers()?;

    Ok(share
        .into_iter()
        .map(|share| share.join("Steam"))
        .chain(home.into_iter().map(|home| home.join(".steam/steam")))
        .collect())
}

fn icons_changed_at() -> Result<std::time::SystemTime, Never> {
    let mut newest = std::time::UNIX_EPOCH;

    let roots = icon_roots()?;

    for root in roots {
        let about = match root.metadata() {
            Ok(about) => about,
            Err(_fault) => continue,
        };

        newest = newest.max(match about.modified() {
            Ok(when) => when,
            Err(_no_modified_time) => std::time::UNIX_EPOCH,
        });

        let reading = match std::fs::read_dir(&root) {
            Ok(reading) => reading,
            Err(_fault) => continue,
        };

        for child in reading.filter_map(Result::ok) {
            match child.metadata() {
                Ok(about) => {
                    newest = newest.max(match about.modified() {
                        Ok(when) => when,
                        Err(_no_modified_time) => std::time::UNIX_EPOCH,
                    });
                }
                Err(_the_child_went_away) => {},
            }
        }
    }

    Ok(newest)
}

fn index() -> Result<BTreeMap<String, String>, Never> {
    let at = index_at()?;

    let changed = icons_changed_at()?;

    let kept = match at.as_ref() {
        Some(at) => {
            at.metadata().and_then(|about| about.modified()).is_ok_and(|written| written >= changed)
        }
        None => false,
    };

    match kept {
        true => match at.as_ref().map(std::fs::read_to_string) {
            Some(Ok(said)) => {
                let held = icons::read(&said)?;

                return Ok(held);
            }
            Some(Err(_the_index_is_unreadable)) => {},
            None => {},
        },
        false => {},
    }

    let roots = icon_roots()?;

    let built = icons::built(&roots)?;

    match at {
        Some(at) => {
            match at.parent() {
                Some(parent) => {
                    let _ = std::fs::create_dir_all(parent);
                }
                None => {},
            }

            let said = icons::written(&built)?;

            let _ = console_core_atomic_writes::whole(&at, said.as_bytes());
        }
        None => {},
    }

    Ok(built)
}

fn steam_icon(appid: &str) -> Result<Option<String>, Never> {
    let mut fallbacks: BTreeMap<String, String> = BTreeMap::new();

    let roots = steam_roots()?;

    for root in roots {
        let cache = root.join("appcache/librarycache").join(appid);

        let reading = match std::fs::read_dir(&cache) {
            Ok(reading) => reading,
            Err(_fault) => continue,
        };

        let mut paths: Vec<PathBuf> =
            reading.filter_map(Result::ok).map(|entry| entry.path()).collect();
        paths.sort();

        'over_pictures: for path in paths {
            let suffix = path.extension().map(|kind| kind.to_string_lossy().to_lowercase());

            match matches!(suffix.as_deref(), Some("jpg" | "png")) {
                true => {},
                false => continue 'over_pictures,
            }

            let head = match read_head(&path) {
                Ok(head) => head,
                Err(_fault) => continue 'over_pictures,
            };

            let size = image::size(&head)?;

            let (width, height) = match size {
                Some((width, height)) => (width, height),
                None => continue 'over_pictures,
            };

            match width == height {
                true => return Ok(Some(path.to_string_lossy().to_string())),
                false => {},
            }

            let name = match path.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => continue 'over_pictures,
            };

            fallbacks.entry(name).or_insert_with(|| path.to_string_lossy().to_string());
        }
    }

    Ok(FALLBACKS.iter().find_map(|wanted| fallbacks.get(*wanted).cloned()))
}

fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut head = Vec::new();

    file.take(HEAD).read_to_end(&mut head)?;

    Ok(head)
}

fn icon_at(name: &str, index: &BTreeMap<String, String>) -> Result<Option<String>, Never> {
    match name.is_empty() {
        true => return Ok(None),
        false => {},
    }

    match name.starts_with('/') {
        true => return Ok(Path::new(name).exists().then(|| name.to_string())),
        false => {},
    }

    match index.get(name) {
        Some(found) => return Ok(Some(found.clone())),
        None => {},
    }

    let appid = steam_appid(name)?;

    let appid = match appid {
        Some(appid) => appid,
        None => return Ok(None),
    };

    steam_icon(appid)
}

fn pictured(found: Option<String>, index: &BTreeMap<String, String>) -> Result<Option<String>, Never> {
    Ok(found.or_else(|| index.get(UNPICTURED).cloned()))
}

pub struct Found {
    pub apps: BTreeMap<String, Application>,
    pub icon: BTreeMap<String, String>,
}

pub fn machine() -> Result<Found, Never> {
    let index = index()?;

    let mut apps: BTreeMap<String, Application> = BTreeMap::new();
    let mut icon: BTreeMap<String, String> = BTreeMap::new();

    let roots = console_core_places::applications()?;

    let files = entry::files(&roots)?;

    for path in files {
        let said = match std::fs::read_to_string(&path) {
            Ok(said) => said,
            Err(_fault) => continue,
        };

        let app = entry::read(&said, installed)?;

        let app = match app {
            Some(app) => app,
            None => continue,
        };

        match apps.contains_key(&app.name) {
            true => continue,
            false => {},
        }

        let found = icon_at(&app.icon, &index)?;
        let found = pictured(found, &index)?;

        match found {
            Some(found) => {
                icon.insert(app.name.clone(), found);
            }
            None => {},
        }

        apps.insert(app.name.clone(), app);
    }

    keep(&apps, &icon)?;

    Ok(Found { apps, icon })
}

pub fn quickly() -> Result<Found, Never> {
    let mut apps: BTreeMap<String, Application> = BTreeMap::new();

    let roots = console_core_places::applications()?;

    let files = entry::files(&roots)?;

    for path in files {
        let said = match std::fs::read_to_string(&path) {
            Ok(said) => said,
            Err(_fault) => continue,
        };

        let app = entry::read(&said, installed)?;

        let app = match app {
            Some(app) => app,
            None => continue,
        };

        apps.entry(app.name.clone()).or_insert(app);
    }

    Ok(Found { apps, icon: BTreeMap::new() })
}

fn said_at(at: Option<PathBuf>) -> Result<String, Never> {
    let at = match at {
        Some(at) => at,
        None => return Ok(String::new()),
    };

    match console_core_atomic_writes::text_or_empty(&at) {
        Ok(said) => Ok(said),
        Err(fault) => {
            let who = whoami()?;

            eprintln!("{who}: {fault}");

            Ok(String::new())
        }
    }
}

pub fn remembered() -> Result<Found, Never> {
    let mut apps: BTreeMap<String, Application> = BTreeMap::new();
    let mut icon: BTreeMap<String, String> = BTreeMap::new();

    let at = kept_at()?;

    let remembered = said_at(at)?;


    let held = cache::read(&remembered)?;

    for held in held {
        match held.picture.is_empty() {
            true => {},
            false => {
                icon.insert(held.app.name.clone(), held.picture);
            }
        }

        apps.insert(held.app.name.clone(), held.app);
    }

    Ok(Found { apps, icon })
}

fn keep(
    apps: &BTreeMap<String, Application>,
    icon: &BTreeMap<String, String>,
) -> Result<(), Never> {
    let held = kept_at()?;

    let at = match held {
        Some(at) => at,
        None => return Ok(()),
    };

    let said = cache::written(apps, icon)?;

    match std::fs::read_to_string(&at).is_ok_and(|before| before == said) {
        true => return Ok(()),
        false => {},
    }

    let _ = console_core_atomic_writes::whole_with_folders(&at, said.as_bytes());

    Ok(())
}

pub fn counted() -> Result<BTreeMap<String, u64>, Never> {
    let at = counts_at()?;

    let said = said_at(at)?;


    counts::read(&said)
}

pub fn command(app: &Application) -> Result<Option<Vec<String>>, Never> {
    bump(&app.name)?;

    let words = words::split(&app.command)?;

    let mut arguments = match words {
        Some(arguments) => arguments,
        None => {
            eprintln!("{}: {:?} is not a command", app.name, app.command);

            return Ok(None);
        }
    };

    match app.terminal {
        true => {
            let Ok(alacritty) = Program::Alacritty.name();

            arguments.insert(0, alacritty.to_string());
            arguments.insert(1, "-e".to_string());
        }
        false => {},
    }

    let who = whoami()?;

    eprintln!("{who} chose {}: {}", app.name, arguments.join(" "));

    Ok(Some(arguments))
}

pub fn bump(name: &str) -> Result<(), Never> {
    let held = counts_at()?;

    let at = match held {
        Some(at) => at,
        None => return Ok(()),
    };

    match at.parent() {
        Some(parent) => {
            let _ = std::fs::create_dir_all(parent);
        }
        None => {},
    }

    let counted = counted()?;

    let bumped = counts::bumped(counted, name)?;

    let said = counts::written(&bumped)?;

    let _ = console_core_atomic_writes::whole(&at, said.as_bytes());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("firefox".to_string(), "/icons/firefox.png".to_string()),
            (UNPICTURED.to_string(), "/icons/program.png".to_string()),
        ])
    }

    #[test]
    fn an_application_with_no_icon_file_is_given_the_picture_of_a_program() {
        assert_eq!(pictured(None, &index()), Ok(Some("/icons/program.png".to_string())));
    }

    #[test]
    fn an_application_whose_icon_was_found_keeps_it() {
        let found = Some("/icons/firefox.png".to_string());

        assert_eq!(pictured(found.clone(), &index()), Ok(found));
    }
}
