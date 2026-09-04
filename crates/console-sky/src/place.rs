//! Where everything to do with the wallpaper lives.
//!
//! One file, so that the panel that offers a picture, the press that writes one
//! and the daemon that puts one on the screen cannot disagree about where it
//! is. They are three programs, and a path spelled out in three places is a
//! path that is spelled two ways.
//!
//! There are two sets of pictures and the difference between them matters. The
//! ones under `/usr/share` came with the machine and are replaced when it is
//! applied, so nothing of hers is kept there. The ones under her own directory
//! are the ones she added, and nothing but she takes them away.

use std::path::{Path, PathBuf};

/// The pictures the machine came with.
pub const CAME_WITH: &str = "/usr/share/backgrounds/console";

/// What the session says a name is, or nothing where it says nothing.
///
/// Unset is ordinary and every caller here has somewhere else to look. A name
/// set to something that is not text is not ordinary, and folded in with unset
/// it is a wallpaper kept somewhere nobody would think to go looking for it.
pub(crate) fn said(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(said) => Some(said),
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("console-sky: {name}: {fault}");
            None
        }
    }
}

/// Where the manifest keeps itself on the device.
///
/// The device holds this whole repository at this path: `console apply` builds
/// the crates out of it and `git -C /etc/console log` is how somebody sees what
/// changed. So the palette and the picture table do not have to be installed
/// anywhere, because the tree they live in is already on the machine.
pub const TREE: &str = "/etc/console";

/// The tree to read the palette and the table out of.
///
/// A tree above wherever somebody is standing, if they are standing in one, so
/// that working on the set means running against what has just been edited.
/// The device's own copy otherwise. This is the order `console-garden` reads its
/// palette in, and the reason is the same: a compiled program can be installed
/// anywhere, and the tree that matters is the one being worked in.
pub fn tree() -> PathBuf {
    // A working directory that cannot be read is one that was deleted under a
    // running program. The device's own tree is still the right answer; that it
    // was arrived at this way is worth a line, because every path in this file
    // is about to hang off it.
    let here = match std::env::current_dir() {
        Ok(here) => Some(here),
        Err(fault) => {
            eprintln!("console-sky: where this is running: {fault}");
            None
        }
    };

    here.and_then(|here| {
        here.ancestors()
            .find(|at| at.join("theme/palette.toml").is_file())
            .map(Path::to_path_buf)
    })
    .unwrap_or_else(|| PathBuf::from(TREE))
}

/// The table saying which picture answers what.
pub fn table() -> PathBuf {
    tree().join("theme/sky.toml")
}

/// Somebody's home, if the machine will say whose.
fn home() -> Option<PathBuf> {
    said("HOME").map(PathBuf::from)
}

/// The pictures she added, pressed.
pub fn hers() -> Option<PathBuf> {
    home().map(|at| at.join(".local/share/console/sky"))
}

/// Where a picture is dropped to be taken up.
///
/// A directory in the obvious place with the obvious name, because the settings
/// panel can offer to press what is in it but it cannot offer to find a file
/// somebody has put somewhere else. Anything a browser or a file manager can
/// save into, this can read.
pub fn dropped() -> Option<PathBuf> {
    home().map(|at| at.join("Pictures/Wallpapers"))
}

/// What she asked the wallpaper to do, written by the settings panel.
pub fn asked() -> Option<PathBuf> {
    home().map(|at| at.join(".config/console/sky.toml"))
}

/// The frames the wallpaper daemon keeps, if the machine will say where.
fn kept() -> Option<PathBuf> {
    said("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| home().map(|at| at.join(".cache")))
        .map(|at| at.join("awww"))
}

/// The start of the name the daemon keeps a picture's frames under.
///
/// The picture's whole path with every separator turned into an underscore,
/// and then the size it was fitted to and how. Only the path is matched here,
/// because a picture pressed again is stale at every size it was ever kept at.
fn kept_as(picture: &Path) -> Option<String> {
    Some(format!("{}__", picture.to_str()?.replace('/', "_")))
}

/// Throw away what the daemon kept of a picture that has been pressed since.
///
/// The daemon keeps every decoded frame of an animation in a file named after
/// the picture's path, its size and how it was fitted. Nothing in that name
/// comes from what is inside the file, so a picture redrawn at a path it has
/// been shown at before is served out of the old picture's frames: the still it
/// decodes is the new picture and the differences played over it are the old
/// one's, and the screen fills with blocks of the two mixed together. What the
/// name leaves out is the one thing that tells them apart, which is when the
/// picture was written, and that is what is compared here.
///
/// Asked of the picture that is about to go up rather than by emptying the
/// whole cache at every start, because those frames are what a picture costs.
/// With them a wallpaper goes up in the moment it is asked for. Without them
/// the client decodes and compresses the entire loop before any of it is drawn,
/// which was measured on the device at twenty-five seconds of a core, once for
/// every session and once more every time a window stopped covering the screen.
pub fn freshen(picture: &Path) {
    if let Some(kept) = kept() {
        freshen_in(&kept, picture);
    }
}

/// The same, against a named cache, so that a test has one of its own.
fn freshen_in(kept: &Path, picture: &Path) {
    let Ok(full) = picture.canonicalize() else {
        return;
    };

    let Some(name) = kept_as(&full) else { return };

    let Ok(pressed) = written(&full) else { return };

    // A level down, because the daemon keeps its frames under a directory
    // named for its own version and an upgrade leaves the old one behind.
    for version in listed(kept) {
        for frames in listed(&version) {
            let named = frames
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let stale = written(&frames).is_ok_and(|kept| kept < pressed);

            if named.starts_with(&name) && stale {
                // Said rather than swallowed. Frames that will not go are the
                // old picture played over the new one for as long as the file
                // is there, and the screen alone does not say which of the two
                // it is showing.
                if let Err(fault) = std::fs::remove_file(&frames) {
                    let at = frames.display();
                    eprintln!("{at} is the picture before this one, and stayed: {fault}");
                }
            }
        }
    }
}

/// What is in a directory, and nothing at all if there is no such directory.
fn listed(at: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(at)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|found| found.path())
        .collect()
}

/// When a file was last written.
fn written(at: &Path) -> std::io::Result<std::time::SystemTime> {
    at.metadata()?.modified()
}

/// The moving picture and the still one, for a picture by name.
///
/// Hers first, so that a picture she added under a name the machine also uses
/// is the one she gets. The machine's own set can be replaced by an update and
/// hers cannot, which makes hers the one to prefer when both answer.
pub fn picture(name: &str) -> Option<(PathBuf, PathBuf)> {
    let mine = hers().map(|at| at.join(name));
    let theirs = Path::new(CAME_WITH).join(name);
    [mine.as_deref(), Some(&theirs)]
        .into_iter()
        .flatten()
        .map(|at| (at.with_extension("webp"), at.with_extension("still.webp")))
        .find(|(moving, _)| moving.is_file())
}

/// Every picture on the machine, by name, hers and the ones it came with.
pub fn every() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();

    for at in [hers(), Some(PathBuf::from(CAME_WITH))].into_iter().flatten() {
        // No directory at all is ordinary: she has added nothing yet. A
        // directory that is there and will not be read is a fault, and told
        // apart from the first it is a person being shown an empty list and
        // told that is everything she has.
        let found = match std::fs::read_dir(&at) {
            Ok(found) => found,
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => continue,
            Err(fault) => {
                eprintln!("console-sky: {}: {fault}", at.display());

                continue;
            }
        };

        for entry in found {
            let path = match entry {
                Ok(entry) => entry.path(),
                Err(fault) => {
                    eprintln!("console-sky: {}: reading what is in it: {fault}", at.display());

                    continue;
                }
            };

            if !path.extension().is_some_and(|kind| kind == "webp") {
                continue;
            }

            let Some(name) = path.file_name().and_then(|name| name.to_str()) else { continue };

            // The still is not a picture in its own right, it is the resting
            // frame of one, and offering both would offer everything twice.
            let Some(name) = name.strip_suffix(".webp") else { continue };

            if !name.ends_with(".still") {
                names.push(name.to_string());
            }
        }
    }

    names.sort();
    names.dedup();

    names
}

/// Which picture the wallpaper daemon says it is showing, by name.
///
/// Read out of `awww query`, which answers a line for each screen ending in the
/// path of what is on it. Split on the label rather than on the last colon,
/// because a path may hold a colon and a label may not.
///
/// The still and the moving picture are the same picture under two names, so
/// the still's suffix comes off: a wallpaper covered by a window is still the
/// wallpaper that is up.
pub fn showing(query: &str) -> String {
    query
        .rsplit_once("image: ")
        .map(|(_, path)| path.trim())
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".webp"))
        .map(|name| name.strip_suffix(".still").unwrap_or(name))
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_a_moving_file_and_a_still_one_beside_it() {
        let at = Path::new(CAME_WITH).join("star-ride");
        assert_eq!(
            at.with_extension("webp").file_name().unwrap(),
            "star-ride.webp"
        );
        assert_eq!(
            at.with_extension("still.webp").file_name().unwrap(),
            "star-ride.still.webp"
        );
    }

    /// The line awww actually answers with, taken off a running daemon.
    #[test]
    fn the_picture_on_the_screen_is_read_out_of_what_the_daemon_says() {
        let said = "skytest: eDP-1: 1920x1200, scale: 1, currently displaying: image: \
                    /usr/share/backgrounds/console/star-ride.webp";
        assert_eq!(showing(said), "star-ride");
    }

    /// A covered wallpaper is showing its still, and that is the same picture.
    #[test]
    fn the_still_of_a_picture_is_that_picture() {
        let said =
            "eDP-1: currently displaying: image: /usr/share/backgrounds/console/campfire.still.webp";
        assert_eq!(showing(said), "campfire");
    }

    /// Split on the last colon this would take half a path.
    #[test]
    fn a_path_holding_a_colon_is_still_a_path() {
        let said = "eDP-1: currently displaying: image: /home/ada/Pictures/a: b/one.webp";
        assert_eq!(showing(said), "one");
    }

    /// Everything the daemon can answer that is not a picture.
    #[test]
    fn a_daemon_showing_no_picture_names_none() {
        assert_eq!(showing(""), "");
        assert_eq!(showing("eDP-1: currently displaying: color: #110b12"), "");
        assert_eq!(showing("no daemon is running"), "");
    }

    /// The name awww actually writes, taken off the daemon's own cache.
    #[test]
    fn the_frames_of_a_picture_are_kept_under_its_path_with_the_slashes_flattened() {
        let at = Path::new("/usr/share/backgrounds/console/lazy-river.webp");
        assert_eq!(
            kept_as(at).unwrap(),
            "_usr_share_backgrounds_console_lazy-river.webp__"
        );
    }

    /// A picture pressed since its frames were kept is a picture whose frames
    /// are the picture before it, and those are the frames thrown away.
    #[test]
    fn frames_older_than_the_picture_go_and_frames_newer_than_it_stay() {
        let here = std::env::temp_dir().join(format!("console-sky-kept-{}", std::process::id()));
        let kept = here.join("awww/0.12.1");
        std::fs::create_dir_all(&kept).expect("somewhere to keep frames");
        let picture = here.join("river.webp");
        std::fs::write(&picture, b"a picture").expect("a picture");

        let name = kept_as(&picture.canonicalize().unwrap()).unwrap();
        let stale = kept.join(format!("{name}2560x1600_crop_argb"));
        let fresh = kept.join(format!("{name}1920x1200_crop_argb"));
        let other = kept.join("_somewhere_else_snow.webp__2560x1600_crop_argb");
        for at in [&stale, &fresh, &other] {
            std::fs::write(at, b"frames").expect("frames");
        }
        let written = picture.metadata().unwrap().modified().unwrap();
        touch(&stale, written - std::time::Duration::from_secs(60));
        touch(&fresh, written + std::time::Duration::from_secs(60));
        touch(&other, written - std::time::Duration::from_secs(60));

        freshen_in(&here.join("awww"), &picture);

        assert!(
            !stale.exists(),
            "the frames of the picture before it were kept"
        );
        assert!(fresh.exists(), "frames of this picture were thrown away");
        assert!(other.exists(), "another picture's frames were thrown away");
        let _ = std::fs::remove_dir_all(&here);
    }

    /// A cache that is not there is not an error: it is a machine that has not
    /// shown a wallpaper yet.
    #[test]
    fn a_cache_that_is_not_there_is_nothing_to_throw_away() {
        let here = std::env::temp_dir().join(format!("console-sky-none-{}", std::process::id()));
        std::fs::create_dir_all(&here).expect("somewhere");
        let picture = here.join("river.webp");
        std::fs::write(&picture, b"a picture").expect("a picture");
        freshen_in(&here.join("nothing-is-kept-here"), &picture);
        let _ = std::fs::remove_dir_all(&here);
    }

    /// Set a file's time, so that the test does not have to wait for one.
    fn touch(at: &Path, when: std::time::SystemTime) {
        let file = std::fs::File::options()
            .write(true)
            .open(at)
            .expect("the file");
        file.set_times(std::fs::FileTimes::new().set_modified(when))
            .expect("its time");
    }

    /// The one thing this file exists to get right, said as a test because the
    /// two directories are otherwise the same shape and the order is invisible.
    #[test]
    fn hers_is_looked_in_before_the_set_the_machine_came_with() {
        // Nothing on this machine is at either path, so what is asserted is the
        // shape of the search rather than its answer.
        assert!(picture("nothing-is-called-this").is_none());
    }
}
