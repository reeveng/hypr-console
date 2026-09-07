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

use console_core_never::Never;

pub const CAME_WITH: &str = "/usr/share/backgrounds/console";

pub(crate) fn said(name: &str) -> Result<Option<String>, Never> {
    Ok(match std::env::var(name) {
        Ok(said) => Some(said),
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("console-sky: {name}: {fault}");
            None
        }
    })
}

pub const TREE: &str = "/etc/console";

pub fn tree() -> Result<PathBuf, Never> {
    let here = match std::env::current_dir() {
        Ok(here) => Some(here),
        Err(fault) => {
            eprintln!("console-sky: where this is running: {fault}");
            None
        }
    };

    Ok(here
        .and_then(|here| {
            here.ancestors()
                .find(|at| at.join("theme/palette.toml").is_file())
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| PathBuf::from(TREE)))
}

pub fn table() -> Result<PathBuf, Never> {
    let tree = tree()?;

    Ok(tree.join("theme/sky.toml"))
}

fn home() -> Result<Option<PathBuf>, Never> {
    let home = said("HOME")?;

    Ok(home.map(PathBuf::from))
}

pub fn hers() -> Result<Option<PathBuf>, Never> {
    let home = home()?;

    Ok(home.map(|at| at.join(".local/share/console/sky")))
}

pub fn dropped() -> Result<Option<PathBuf>, Never> {
    let home = home()?;

    Ok(home.map(|at| at.join("Pictures/Wallpapers")))
}

pub fn asked() -> Result<Option<PathBuf>, Never> {
    let home = home()?;

    Ok(home.map(|at| at.join(".config/console/sky.toml")))
}

fn kept() -> Result<Option<PathBuf>, Never> {
    let cache = said("XDG_CACHE_HOME")?;
    let home = home()?;

    Ok(cache
        .map(PathBuf::from)
        .or_else(|| home.map(|at| at.join(".cache")))
        .map(|at| at.join("awww")))
}

fn kept_as(picture: &Path) -> Result<Option<String>, Never> {
    let Some(said) = picture.to_str() else { return Ok(None) };

    Ok(Some(format!("{}__", said.replace('/', "_"))))
}

pub fn freshen(picture: &Path) -> Result<(), Never> {
    let kept = kept()?;

    match kept {
        Some(kept) => freshen_in(&kept, picture),
        None => Ok(()),
    }
}

fn freshen_in(kept: &Path, picture: &Path) -> Result<(), Never> {
    let Ok(full) = picture.canonicalize() else {
        return Ok(());
    };

    let Some(name) = kept_as(&full)? else { return Ok(()) };

    let Ok(pressed) = written(&full) else { return Ok(()) };

    let versions = listed(kept)?;

    for version in versions {
        let every = listed(&version)?;

        for frames in every {
            let named = frames
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let stale = written(&frames).is_ok_and(|kept| kept < pressed);

            match named.starts_with(&name) && stale {
                true => match std::fs::remove_file(&frames) {
                    Ok(()) => {},
                    Err(fault) => {
                        let at = frames.display();
                        eprintln!("{at} is the picture before this one, and stayed: {fault}");
                    }
                },
                false => {},
            }
        }
    }

    Ok(())
}

fn listed(at: &Path) -> Result<Vec<PathBuf>, Never> {
    Ok(std::fs::read_dir(at)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|found| found.path())
        .collect())
}

fn written(at: &Path) -> std::io::Result<std::time::SystemTime> {
    let about = at.metadata()?;

    about.modified()
}

pub fn picture(name: &str) -> Result<Option<(PathBuf, PathBuf)>, Never> {
    let hers = hers()?;
    let mine = hers.map(|at| at.join(name));
    let theirs = Path::new(CAME_WITH).join(name);

    Ok([mine.as_deref(), Some(&theirs)]
        .into_iter()
        .flatten()
        .map(|at| (at.with_extension("webp"), at.with_extension("still.webp")))
        .find(|(moving, _)| moving.is_file()))
}

pub fn every() -> Result<Vec<String>, Never> {
    let hers = hers()?;
    let mut names: Vec<String> = Vec::new();

    for at in [hers, Some(PathBuf::from(CAME_WITH))].into_iter().flatten() {
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

            match path.extension().is_some_and(|kind| kind == "webp") {
                true => {},
                false => continue,
            }

            let Some(name) = path.file_name().and_then(|name| name.to_str()) else { continue };

            let Some(name) = name.strip_suffix(".webp") else { continue };

            match name.ends_with(".still") {
                true => {},
                false => names.push(name.to_string()),
            }
        }
    }

    names.sort();
    names.dedup();

    Ok(names)
}

pub fn showing(query: &str) -> Result<String, Never> {
    Ok(query
        .rsplit_once("image: ")
        .map(|(_, path)| path.trim())
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".webp"))
        .map(|name| name.strip_suffix(".still").unwrap_or(name))
        .unwrap_or_default()
        .to_string())
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

    #[test]
    fn the_picture_on_the_screen_is_read_out_of_what_the_daemon_says() {
        let said = "skytest: eDP-1: 1920x1200, scale: 1, currently displaying: image: \
                    /usr/share/backgrounds/console/star-ride.webp";
        assert_eq!(showing(said), Ok("star-ride".to_string()));
    }

    #[test]
    fn the_still_of_a_picture_is_that_picture() {
        let said =
            "eDP-1: currently displaying: image: /usr/share/backgrounds/console/campfire.still.webp";
        assert_eq!(showing(said), Ok("campfire".to_string()));
    }

    #[test]
    fn a_path_holding_a_colon_is_still_a_path() {
        let said = "eDP-1: currently displaying: image: /home/ada/Pictures/a: b/one.webp";
        assert_eq!(showing(said), Ok("one".to_string()));
    }

    #[test]
    fn a_daemon_showing_no_picture_names_none() {
        assert_eq!(showing(""), Ok(String::new()));
        assert_eq!(showing("eDP-1: currently displaying: color: #110b12"), Ok(String::new()));
        assert_eq!(showing("no daemon is running"), Ok(String::new()));
    }

    #[test]
    fn the_frames_of_a_picture_are_kept_under_its_path_with_the_slashes_flattened() {
        let at = Path::new("/usr/share/backgrounds/console/lazy-river.webp");
        let Ok(name) = kept_as(at);

        assert_eq!(name.unwrap(), "_usr_share_backgrounds_console_lazy-river.webp__");
    }

    #[test]
    fn frames_older_than_the_picture_go_and_frames_newer_than_it_stay() {
        let here = std::env::temp_dir().join(format!("console-sky-kept-{}", std::process::id()));
        let kept = here.join("awww/0.12.1");
        std::fs::create_dir_all(&kept).expect("somewhere to keep frames");
        let picture = here.join("river.webp");
        std::fs::write(&picture, b"a picture").expect("a picture");

        let Ok(name) = kept_as(&picture.canonicalize().unwrap());

        let name = name.unwrap();
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

        let Ok(()) = freshen_in(&here.join("awww"), &picture);

        assert!(
            !stale.exists(),
            "the frames of the picture before it were kept"
        );
        assert!(fresh.exists(), "frames of this picture were thrown away");
        assert!(other.exists(), "another picture's frames were thrown away");
        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    fn a_cache_that_is_not_there_is_nothing_to_throw_away() {
        let here = std::env::temp_dir().join(format!("console-sky-none-{}", std::process::id()));
        std::fs::create_dir_all(&here).expect("somewhere");
        let picture = here.join("river.webp");
        std::fs::write(&picture, b"a picture").expect("a picture");
        let Ok(()) = freshen_in(&here.join("nothing-is-kept-here"), &picture);
        let _ = std::fs::remove_dir_all(&here);
    }

    fn touch(at: &Path, when: std::time::SystemTime) {
        let file = std::fs::File::options()
            .write(true)
            .open(at)
            .expect("the file");
        file.set_times(std::fs::FileTimes::new().set_modified(when))
            .expect("its time");
    }

    #[test]
    fn hers_is_looked_in_before_the_set_the_machine_came_with() {
        let Ok(found) = picture("nothing-is-called-this");

        assert!(found.is_none());
    }
}
