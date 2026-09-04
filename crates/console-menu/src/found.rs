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

use crate::entry::{Application, Installed};
use crate::icons::{FALLBACKS, steam_appid};
use crate::{counts, entry, icons, image, kept, words};

/// What this program is called, for the lines it writes when something is
/// wrong.
///
/// Taken from the program rather than written down, so the menu says "launcher"
/// and the home screen says its own name out of one function.
fn whoami() -> String {
    std::env::args()
        .next()
        .and_then(|argv0| Path::new(&argv0).file_name().map(|name| name.to_string_lossy().to_string()))
        .unwrap_or_else(|| "console-menu".to_string())
}

/// What the session says a name is, or nothing where it says nothing.
///
/// Unset is ordinary and each caller below has its own answer for it. A name
/// set to something that is not text is somebody's session being wrong rather
/// than quiet, and it used to reach the same fallback without a word.
pub fn said(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(said) => Some(said),
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("{}: {name}: {fault}", whoami());

            None
        }
    }
}

pub fn home() -> PathBuf {
    PathBuf::from(said("HOME").unwrap_or_else(|| "/root".to_string()))
}

pub fn counts_at() -> PathBuf {
    home().join(".local/state/console/menu-counts")
}

fn index_at() -> PathBuf {
    home().join(".cache/console/icon-index")
}

fn kept_at() -> PathBuf {
    home().join(".cache/console/menu-apps")
}

fn icon_roots() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/share/pixmaps"),
        home().join(".local/share/icons"),
        home().join(".icons"),
    ]
}

fn steam_roots() -> Vec<PathBuf> {
    vec![home().join(".local/share/Steam"), home().join(".steam/steam")]
}

/// The newest of the icon directories, which a package install touches.
fn icons_changed_at() -> std::time::SystemTime {
    let mut newest = std::time::UNIX_EPOCH;

    for root in icon_roots() {
        let Ok(about) = root.metadata() else { continue };

        // A filesystem that keeps no modified time answers with the epoch,
        // which is older than anything and so never makes the index look fresh.
        newest = newest.max(match about.modified() {
            Ok(when) => when,
            Err(_no_modified_time) => std::time::UNIX_EPOCH,
        });

        let Ok(reading) = std::fs::read_dir(&root) else { continue };

        for child in reading.filter_map(Result::ok) {
            if let Ok(about) = child.metadata() {
                newest = newest.max(match about.modified() {
                    Ok(when) => when,
                    Err(_no_modified_time) => std::time::UNIX_EPOCH,
                });
            }
        }
    }

    newest
}

/// The kept index, rebuilt when something has been installed since.
fn index() -> BTreeMap<String, String> {
    let at = index_at();
    let kept = at
        .metadata()
        .and_then(|about| about.modified())
        .is_ok_and(|written| written >= icons_changed_at());

    if kept && let Ok(said) = std::fs::read_to_string(&at) {
        return icons::read(&said);
    }

    let built = icons::built(&icon_roots());

    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let _ = std::fs::write(&at, icons::written(&built));
    built
}

/// A game's icon out of Steam's own cache, found by shape.
fn steam_icon(appid: &str) -> Option<String> {
    let mut fallbacks: BTreeMap<String, String> = BTreeMap::new();

    for root in steam_roots() {
        let cache = root.join("appcache/librarycache").join(appid);

        let Ok(reading) = std::fs::read_dir(&cache) else { continue };

        let mut paths: Vec<PathBuf> =
            reading.filter_map(Result::ok).map(|entry| entry.path()).collect();
        paths.sort();

        for path in paths {
            let suffix = path.extension().map(|kind| kind.to_string_lossy().to_lowercase());

            if !matches!(suffix.as_deref(), Some("jpg" | "png")) {
                continue;
            }

            let Ok(head) = read_head(&path) else { continue };

            let Some((width, height)) = image::size(&head) else { continue };

            if width == height {
                return Some(path.to_string_lossy().to_string());
            }

            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            fallbacks.entry(name).or_insert_with(|| path.to_string_lossy().to_string());
        }
    }

    FALLBACKS.iter().find_map(|wanted| fallbacks.get(*wanted).cloned())
}

/// Enough of a file to hold any header this reads.
fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut head = vec![0; 65536];
    let mut file = std::fs::File::open(path)?;
    let read = file.read(&mut head)?;
    head.truncate(read);
    Ok(head)
}

/// A file for an icon name, or nothing. Names may already be a path.
fn icon_at(name: &str, index: &BTreeMap<String, String>) -> Option<String> {
    if name.is_empty() {
        return None;
    }

    if name.starts_with('/') {
        return Path::new(name).exists().then(|| name.to_string());
    }

    if let Some(found) = index.get(name) {
        return Some(found.clone());
    }

    steam_appid(name).and_then(steam_icon)
}

/// Whether a program named in TryExec is on this machine.
fn here(wanted: &str) -> Installed {
    let found = match wanted.starts_with('/') {
        true => Path::new(wanted).exists(),
        false => said("PATH").unwrap_or_default().split(':').any(|where_| {
            !where_.is_empty() && Path::new(where_).join(wanted).exists()
        }),
    };

    match found {
        true => Installed::Yes,
        false => Installed::No,
    }
}

/// The applications this machine has, and a picture for each.
pub struct Found {
    pub apps: BTreeMap<String, Application>,
    pub icon: BTreeMap<String, String>,
}

/// Read the machine: every application it has, and a picture for each.
///
/// Written down as it is found, so the next reading can open on it. What it
/// costs is one file, and only when the answer has changed since the last one.
pub fn machine() -> Found {
    let index = index();
    let data_dirs = said("XDG_DATA_DIRS").unwrap_or_else(|| entry::DATA_DIRS.to_string());
    let mut apps: BTreeMap<String, Application> = BTreeMap::new();
    let mut icon: BTreeMap<String, String> = BTreeMap::new();

    for path in entry::files(&home(), &data_dirs) {
        let Ok(said) = std::fs::read_to_string(&path) else { continue };

        let Some(app) = entry::read(&said, here) else { continue };

        if apps.contains_key(&app.name) {
            continue;
        }

        if let Some(found) = icon_at(&app.icon, &index) {
            icon.insert(app.name.clone(), found);
        }

        apps.insert(app.name.clone(), app);
    }

    keep(&apps, &icon);
    Found { apps, icon }
}

/// The applications, without looking for a picture for any of them.
///
/// Reading the desktop files is a few hundred small files and is quick.
/// Finding a picture for each one is a walk of every icon directory on the
/// machine, which is seconds on a cold cache -- and a cold cache is exactly
/// the first boot, where a surface that waits for it is a surface that is not
/// there when somebody first looks at the screen.
///
/// So this is the half that is quick, for whoever can draw a name before it
/// has a picture. Nothing is written down: what this found is a smaller answer
/// than [`machine`] gives, and remembering it would be remembering that this
/// machine has no icons.
pub fn quickly() -> Found {
    let data_dirs = said("XDG_DATA_DIRS").unwrap_or_else(|| entry::DATA_DIRS.to_string());
    let mut apps: BTreeMap<String, Application> = BTreeMap::new();

    for path in entry::files(&home(), &data_dirs) {
        let Ok(said) = std::fs::read_to_string(&path) else { continue };

        let Some(app) = entry::read(&said, here) else { continue };

        apps.entry(app.name.clone()).or_insert(app);
    }

    Found { apps, icon: BTreeMap::new() }
}

/// What was found last time, off one file.
///
/// The list a surface opens on, so the applications are on the screen in the
/// moment it is, rather than a moment after. What is on this machine is very
/// nearly what was on it, and the reading behind this replaces the whole list
/// either way.
pub fn remembered() -> Found {
    let mut apps: BTreeMap<String, Application> = BTreeMap::new();
    let mut icon: BTreeMap<String, String> = BTreeMap::new();

    let at = kept_at();

    // No file is the first run and the list is built from scratch. A file that
    // is there and will not be read gives the same empty list and means
    // something else: the surface is about to look as though it has never been
    // opened on a machine where it has.
    let remembered = match std::fs::read_to_string(&at) {
        Ok(held) => held,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(fault) => {
            eprintln!("{}: {}: {fault}", whoami(), at.display());

            String::new()
        }
    };

    for held in kept::read(&remembered) {
        if !held.picture.is_empty() {
            icon.insert(held.app.name.clone(), held.picture);
        }

        apps.insert(held.app.name.clone(), held.app);
    }

    Found { apps, icon }
}

/// Write down what was found, if it is not already what is written down.
///
/// Nothing installed and nothing removed is a menu opened and closed all day
/// that writes once, on the first opening after something changed.
fn keep(apps: &BTreeMap<String, Application>, icon: &BTreeMap<String, String>) {
    let at = kept_at();
    let said = kept::written(apps, icon);

    if std::fs::read_to_string(&at).is_ok_and(|before| before == said) {
        return;
    }

    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let _ = std::fs::write(&at, said);
}

/// How often each application has been opened from this desktop.
pub fn counted() -> BTreeMap<String, u64> {
    let at = counts_at();

    // Nobody has opened anything yet, or the counts are there and will not be
    // read. Both give an unordered list and only one of them is ordinary.
    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(fault) => {
            eprintln!("{}: {}: {fault}", whoami(), at.display());

            String::new()
        }
    };

    counts::read(&said)
}

/// Start an application, and count that it was started.
///
/// The counting is what puts the ones somebody actually uses at the top of the
/// menu, and it is the order the home screen fills itself in on the first boot.
/// Doing it here means a home screen that starts something is a home screen
/// that says so, without the two programs having to agree about a file.
pub fn run(app: &Application) {
    bump(&app.name);

    let Some(mut argv) = words::split(&app.command) else {
        eprintln!("{}: {:?} is not a command", app.name, app.command);
        return;
    };

    if app.terminal {
        argv.insert(0, "alacritty".to_string());
        argv.insert(1, "-e".to_string());
    }

    eprintln!("{} chose {}: {}", whoami(), app.name, argv.join(" "));
    console_panel::running::left_running(&argv);
}

/// One more opening of this, written down.
pub fn bump(name: &str) {
    let at = counts_at();

    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let _ = std::fs::write(&at, counts::written(&counts::bumped(counted(), name)));
}
