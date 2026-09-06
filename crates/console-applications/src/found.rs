//! What applications this machine has, and how to start one.
//!
//! The menu read all of this inside its own program for as long as it was the
//! only thing that wanted it. The home screen wants the same answer: it draws
//! a few of these on the wallpaper, and the card that puts one there lists
//! every one of them. Two programs reading the desktop files two ways would be
//! two answers to one question, and the second of them would be wrong in some
//! way nobody had thought about -- a Steam icon found here and not there, a
//! terminal program run without its terminal.
//!
//! So it is here, once, and the menu is one of the callers rather than the
//! owner.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_external_programs::Program;
use console_never::Never;

use crate::entry::{Application, Installed};
use crate::icons::{FALLBACKS, steam_appid};
use crate::{counts, entry, icons, image, kept, words};

fn whoami() -> Result<String, Never> {
    Ok(std::env::args()
        .next()
        .and_then(|argv0| Path::new(&argv0).file_name().map(|name| name.to_string_lossy().to_string()))
        .unwrap_or_else(|| "console-applications".to_string()))
}

pub fn said(name: &str) -> Result<Option<String>, Never> {
    match std::env::var(name) {
        Ok(said) => Ok(Some(said)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(fault) => {
            let who = whoami()?;

            eprintln!("{who}: {name}: {fault}");

            Ok(None)
        }
    }
}

pub fn home() -> Result<PathBuf, Never> {
    let said = said("HOME")?;

    Ok(PathBuf::from(said.unwrap_or_else(|| "/root".to_string())))
}

pub fn counts_at() -> Result<PathBuf, Never> {
    let home = home()?;

    Ok(home.join(".local/state/console/menu-counts"))
}

fn index_at() -> Result<PathBuf, Never> {
    let home = home()?;

    Ok(home.join(".cache/console/icon-index"))
}

fn kept_at() -> Result<PathBuf, Never> {
    let home = home()?;

    Ok(home.join(".cache/console/menu-apps"))
}

fn icon_roots() -> Result<Vec<PathBuf>, Never> {
    let home = home()?;

    Ok(vec![
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/share/pixmaps"),
        home.join(".local/share/icons"),
        home.join(".icons"),
    ])
}

fn steam_roots() -> Result<Vec<PathBuf>, Never> {
    let home = home()?;

    Ok(vec![home.join(".local/share/Steam"), home.join(".steam/steam")])
}

fn icons_changed_at() -> Result<std::time::SystemTime, Never> {
    let mut newest = std::time::UNIX_EPOCH;

    let roots = icon_roots()?;

    for root in roots {
        let Ok(about) = root.metadata() else { continue };

        newest = newest.max(match about.modified() {
            Ok(when) => when,
            Err(_no_modified_time) => std::time::UNIX_EPOCH,
        });

        let Ok(reading) = std::fs::read_dir(&root) else { continue };

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

    let kept = at
        .metadata()
        .and_then(|about| about.modified())
        .is_ok_and(|written| written >= changed);

    match kept {
        true => match std::fs::read_to_string(&at) {
            Ok(said) => {
                let held = icons::read(&said)?;

                return Ok(held);
            }
            Err(_the_index_is_unreadable) => {},
        },
        false => {},
    }

    let roots = icon_roots()?;

    let built = icons::built(&roots)?;

    match at.parent() {
        Some(parent) => {
            let _ = std::fs::create_dir_all(parent);
        }
        None => {},
    }

    let said = icons::written(&built)?;

    let _ = std::fs::write(&at, said);

    Ok(built)
}

fn steam_icon(appid: &str) -> Result<Option<String>, Never> {
    let mut fallbacks: BTreeMap<String, String> = BTreeMap::new();

    let roots = steam_roots()?;

    for root in roots {
        let cache = root.join("appcache/librarycache").join(appid);

        let Ok(reading) = std::fs::read_dir(&cache) else { continue };

        let mut paths: Vec<PathBuf> =
            reading.filter_map(Result::ok).map(|entry| entry.path()).collect();
        paths.sort();

        for path in paths {
            let suffix = path.extension().map(|kind| kind.to_string_lossy().to_lowercase());

            match matches!(suffix.as_deref(), Some("jpg" | "png")) {
                true => {},
                false => continue,
            }

            let Ok(head) = read_head(&path) else { continue };

            let Some((width, height)) = image::size(&head)? else { continue };

            match width == height {
                true => return Ok(Some(path.to_string_lossy().to_string())),
                false => {},
            }

            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            fallbacks.entry(name).or_insert_with(|| path.to_string_lossy().to_string());
        }
    }

    Ok(FALLBACKS.iter().find_map(|wanted| fallbacks.get(*wanted).cloned()))
}

fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut head = vec![0; 65536];
    let mut file = std::fs::File::open(path)?;
    let read = file.read(&mut head)?;
    head.truncate(read);
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

    let Some(appid) = steam_appid(name)? else { return Ok(None) };

    steam_icon(appid)
}

fn here(wanted: &str) -> Result<Installed, Never> {
    let path = said("PATH")?;

    let found = match wanted.starts_with('/') {
        true => Path::new(wanted).exists(),
        false => path.unwrap_or_default().split(':').any(|where_| {
            !where_.is_empty() && Path::new(where_).join(wanted).exists()
        }),
    };

    Ok(match found {
        true => Installed::Yes,
        false => Installed::No,
    })
}

pub struct Found {
    pub apps: BTreeMap<String, Application>,
    pub icon: BTreeMap<String, String>,
}

pub fn machine() -> Result<Found, Never> {
    let index = index()?;

    let said_dirs = said("XDG_DATA_DIRS")?;

    let data_dirs = said_dirs.unwrap_or_else(|| entry::DATA_DIRS.to_string());
    let mut apps: BTreeMap<String, Application> = BTreeMap::new();
    let mut icon: BTreeMap<String, String> = BTreeMap::new();

    let home = home()?;

    let files = entry::files(&home, &data_dirs)?;

    for path in files {
        let Ok(said) = std::fs::read_to_string(&path) else { continue };

        let Some(app) = entry::read(&said, here)? else { continue };

        match apps.contains_key(&app.name) {
            true => continue,
            false => {},
        }

        let found = icon_at(&app.icon, &index)?;

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
    let said_dirs = said("XDG_DATA_DIRS")?;

    let data_dirs = said_dirs.unwrap_or_else(|| entry::DATA_DIRS.to_string());
    let mut apps: BTreeMap<String, Application> = BTreeMap::new();

    let home = home()?;

    let files = entry::files(&home, &data_dirs)?;

    for path in files {
        let Ok(said) = std::fs::read_to_string(&path) else { continue };

        let Some(app) = entry::read(&said, here)? else { continue };

        apps.entry(app.name.clone()).or_insert(app);
    }

    Ok(Found { apps, icon: BTreeMap::new() })
}

pub fn remembered() -> Result<Found, Never> {
    let mut apps: BTreeMap<String, Application> = BTreeMap::new();
    let mut icon: BTreeMap<String, String> = BTreeMap::new();

    let at = kept_at()?;

    let remembered = match std::fs::read_to_string(&at) {
        Ok(held) => held,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(fault) => {
            let who = whoami()?;

            eprintln!("{who}: {}: {fault}", at.display());

            String::new()
        }
    };

    let held = kept::read(&remembered)?;

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
    let at = kept_at()?;

    let said = kept::written(apps, icon)?;

    match std::fs::read_to_string(&at).is_ok_and(|before| before == said) {
        true => return Ok(()),
        false => {},
    }

    match at.parent() {
        Some(parent) => {
            let _ = std::fs::create_dir_all(parent);
        }
        None => {},
    }

    let _ = std::fs::write(&at, said);

    Ok(())
}

pub fn counted() -> Result<BTreeMap<String, u64>, Never> {
    let at = counts_at()?;

    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(fault) => {
            let who = whoami()?;

            eprintln!("{who}: {}: {fault}", at.display());

            String::new()
        }
    };

    counts::read(&said)
}

pub fn run(app: &Application) -> Result<(), Never> {
    bump(&app.name)?;

    let words = words::split(&app.command)?;

    let Some(mut argv) = words else {
        eprintln!("{}: {:?} is not a command", app.name, app.command);

        return Ok(());
    };

    match app.terminal {
        true => {
            let Ok(alacritty) = Program::Alacritty.name();

            argv.insert(0, alacritty.to_string());
            argv.insert(1, "-e".to_string());
        }
        false => {},
    }

    let who = whoami()?;

    eprintln!("{who} chose {}: {}", app.name, argv.join(" "));
    let Ok(()) = console_panel::running::left_running(&argv);

    Ok(())
}

pub fn bump(name: &str) -> Result<(), Never> {
    let at = counts_at()?;

    match at.parent() {
        Some(parent) => {
            let _ = std::fs::create_dir_all(parent);
        }
        None => {},
    }

    let counted = counted()?;

    let bumped = counts::bumped(counted, name)?;

    let said = counts::written(&bumped)?;

    let _ = std::fs::write(&at, said);

    Ok(())
}
