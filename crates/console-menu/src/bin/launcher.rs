//! The menu.
//!
//! Applications come out in the order you actually use them: the ones you open
//! most, most often, and everything else alphabetically after them.
//!
//! It is drawn as a panel, like the settings, the guide and the files. It was
//! wofi for a long time, and wofi cost four separate things: it listed itself
//! under its own name, so the bar could not say whether the menu was up; it
//! could not be told to shrink when the on-screen keyboard took the bottom of
//! the screen; it wanted one press to highlight a row and another to take it;
//! and the icon that opened it could not close it. None of the four is a fault
//! in wofi. They are one fact: the menu was the only surface on this machine
//! that was not ours.
//!
//! What is typed is a name to the machine and a question to the browser, and
//! it does not stop being the second because it was the first. The last row of
//! the list offers to ask it, under everything the machine answered with.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use console_menu::icons::{FALLBACKS, steam_appid};
use console_menu::{counts, entry, icons, image, narrow, words};
use console_defaults::engines;
use console_panel::page::{Does, Page, Picture, Row, Rows};
use console_panel::{chooser, panel};

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()))
}

fn counts_at() -> PathBuf {
    home().join(".local/state/console/menu-counts")
}

fn index_at() -> PathBuf {
    home().join(".cache/console/icon-index")
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
        newest = newest.max(about.modified().unwrap_or(std::time::UNIX_EPOCH));
        let Ok(reading) = std::fs::read_dir(&root) else { continue };
        for child in reading.filter_map(Result::ok) {
            if let Ok(about) = child.metadata() {
                newest = newest.max(about.modified().unwrap_or(std::time::UNIX_EPOCH));
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
fn here(wanted: &str) -> bool {
    if wanted.starts_with('/') {
        return Path::new(wanted).exists();
    }
    std::env::var("PATH").unwrap_or_default().split(':').any(|where_| {
        !where_.is_empty() && Path::new(where_).join(wanted).exists()
    })
}

/// Everything the menu knows, worked out once when it opens.
///
/// A menu is up for as long as it takes to choose something, and reading the
/// desktop files again for every letter typed would be the whole of
/// `/usr/share/applications` read on every thumb press.
struct Everything {
    apps: BTreeMap<String, entry::Application>,
    icon: BTreeMap<String, String>,
    counted: BTreeMap<String, u64>,
    /// The names in the order they are used in.
    order: Vec<String>,
}

/// What has been typed so far.
type Typed = Arc<Mutex<String>>;

/// What the empty line says it is for.
const ABOUT: &str = "Type to narrow the list";

/// Read the machine: every application it has, and a picture for each.
fn everything() -> Everything {
    let index = index();
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| entry::DATA_DIRS.to_string());
    let mut apps: BTreeMap<String, entry::Application> = BTreeMap::new();
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
    let counted = counts::read(&std::fs::read_to_string(counts_at()).unwrap_or_default());
    let names: Vec<String> = apps.keys().cloned().collect();
    let order = counts::order(&names, &counted);
    Everything { apps, icon, counted, order }
}

/// One application, with its picture.
///
/// The panel keeps room at the front of every row whether or not there is a
/// picture to put in it, so an application the icon theme has nothing for
/// still has its name where the others have theirs.
fn app_row(all: &Arc<Everything>, name: &str) -> Row {
    let picture =
        all.icon.get(name).map_or(Picture::Space, |at| Picture::At(PathBuf::from(at)));
    let app = all.apps.get(name).cloned();
    let counted = all.counted.clone();
    let named = name.to_string();
    Row::new(
        name,
        "",
        Does::call(move |_| {
            start(app.as_ref(), counted.clone(), &named);
            true
        }),
    )
    .picturing(picture)
}

/// The row that asks the browser instead, under everything the machine has.
///
/// wofi handed back whatever was typed and the browser was asked it without
/// anybody being told that was about to happen. Said out loud it is a row like
/// any other: it can be read before it is taken, and stepped past.
///
/// It stood there only while the list had been narrowed to nothing, which made
/// the browser the answer to a word the machine did not know and no answer at
/// all to a word it half knew. "map", on a machine with a map editor installed,
/// is somebody who has to leave the menu and start again somewhere else. So the
/// row is the last one on the list rather than the only one: what the machine
/// answers with comes first, and under it, one press further down than the last
/// application, is the rest of the world.
fn looking_up_row(said: &str) -> Row {
    let word = said.to_string();
    Row::new(
        &format!("Look up {said:?}"),
        "",
        Does::call(move |_| {
            looked_up(&word);
            true
        }),
    )
    .picturing(Picture::Space)
}

/// The list as the typed word leaves it, with the offer to look that word up.
///
/// Nothing typed is not a question, so the menu opens on the applications and
/// nothing else: an empty line handed to an engine is that engine's front page,
/// which is not what anybody standing on the bottom row meant to ask for.
fn rows(typed: &Typed, all: &Arc<Everything>) -> Vec<Row> {
    let word = typed.lock().map(|held| held.clone()).unwrap_or_default();
    let standing = narrow::matching(&all.order, &word);
    let mut rows: Vec<Row> = standing.iter().map(|name| app_row(all, name)).collect();
    let said = word.trim();
    if !said.is_empty() {
        rows.push(looking_up_row(said));
    }
    rows
}

/// The one tab, and the line that narrows it.
fn pages(typed: &Typed, all: &Arc<Everything>) -> Vec<Page> {
    let listing = Arc::clone(typed);
    let listed = Arc::clone(all);
    let typing = Arc::clone(typed);
    vec![Page::new("Menu", Rows::asked(move || rows(&listing, &listed))).searching(
        ABOUT,
        move |showing, word| {
            let changed = match typing.lock() {
                Ok(mut held) if *held != word => {
                    *held = word.to_string();
                    true
                }
                _ => false,
            };
            // Back to the top, because the row that was being stood on is not
            // the row standing there now.
            if changed {
                showing.replace(0);
            }
        },
    )]
}

fn main() {
    // The menu is on a button, on a paddle, on a key and on the bar. Pressed
    // again while it is already up, each of those used to draw a second menu
    // over the first, in the same place, and backing out of one left the other
    // looking like a menu that ignores you. Now the second press closes it.
    let asked: Vec<String> = std::env::args().skip(1).collect();
    // The daemon says --keep because the paddles it reads only open. The bar
    // does not, because a finger has no other way to put the menu away.
    let again = match asked.iter().any(|word| word == "--keep") {
        true => chooser::Again::Keeps,
        false => chooser::Again::Closes,
    };
    if !chooser::alone("menu", again) {
        return;
    }

    let all = Arc::new(everything());
    let typed: Typed = Arc::new(Mutex::new(String::new()));
    panel::show(Arc::new(move || pages(&typed, &all)), 0, None);
}

/// Run what was chosen, and say what that was.
///
/// A press that chose something and a press that chose nothing look the same
/// from the outside, which is how "it only works sometimes" gets reported
/// about a button that worked every time and a program that never started.
fn start(app: Option<&entry::Application>, counted: BTreeMap<String, u64>, chosen: &str) {
    let Some(app) = app else {
        looked_up(chosen);
        return;
    };
    bump(&app.name, counted);
    let Some(mut argv) = words::split(&app.command) else {
        eprintln!("{}: {:?} is not a command", app.name, app.command);
        return;
    };
    if app.terminal {
        argv.insert(0, "alacritty".to_string());
        argv.insert(1, "-e".to_string());
    }
    eprintln!("the menu chose {}: {}", app.name, argv.join(" "));
    console_panel::running::left_running(&argv);
}

/// A line that matched no application, handed to the browser.
///
/// Somebody who wanted something this machine does not have, or has under a
/// name they did not type. A menu that closed and did nothing was the old
/// answer to the first of those and had nothing at all to say to the second.
///
/// Which engine is asked, and which browser opens it, are both the settings
/// panel's to say. Neither is named here.
fn looked_up(said: &str) {
    let Some(engine) = engines::one(&engines::chosen()) else { return };
    let Some(address) = engines::address(said, engine) else { return };
    eprintln!("the menu was asked {said:?}: {address}");
    console_panel::running::left_running(&opening(&address));
}

/// What opens an address, whoever this desktop says its browser is.
fn opening(address: &str) -> Vec<String> {
    vec!["xdg-open".to_string(), address.to_string()]
}

fn bump(name: &str, counted: BTreeMap<String, u64>) {
    let at = counts_at();
    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&at, counts::written(&counts::bumped(counted, name)));
}
