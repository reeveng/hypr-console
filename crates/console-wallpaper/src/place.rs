//! Where everything to do with the wallpaper lives.
//!
//! One file, so that the panel that offers a picture, the render that writes one
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
use console_core_places::Base;
use console_repository::NotFound;

pub const CAME_WITH: &str = "/usr/share/backgrounds/console";

pub const TREE: &str = console_repository::DEVICE_ROOT;

pub fn tree() -> Result<PathBuf, Never> {
    Ok(match console_repository::root() {
        Ok(root) => root,
        Err(NotFound::Outside(_this_is_not_a_checkout)) => PathBuf::from(TREE),
        Err(fault @ NotFound::Nowhere(_)) => {
            eprintln!("console-wallpaper: {fault}");

            PathBuf::from(TREE)
        }
    })
}

pub fn table() -> Result<PathBuf, Never> {
    let tree = tree()?;

    Ok(tree.join("theme/sky.toml"))
}

pub fn user() -> Result<Option<PathBuf>, Never> {
    let ours = Base::Share.ours()?;

    Ok(ours.map(|at| at.join("sky")))
}

pub fn dropped() -> Result<Option<PathBuf>, Never> {
    let home = console_core_places::home()?;

    Ok(home.map(|at| at.join("Pictures/Wallpapers")))
}

pub fn config_path() -> Result<Option<PathBuf>, Never> {
    let ours = Base::Configuration.ours()?;

    Ok(ours.map(|at| at.join("sky.toml")))
}

fn awww_cache() -> Result<Option<PathBuf>, Never> {
    let cache = Base::Cache.user()?;

    Ok(cache.map(|at| at.join("awww")))
}

fn cached_name(picture: &Path) -> Result<Option<String>, Never> {
    let said = match picture.to_str() {
        Some(said) => said,
        None => return Ok(None),
    };

    Ok(Some(format!("{}__", said.replace('/', "_"))))
}

pub fn refresh(picture: &Path) -> Result<(), Never> {
    let kept = awww_cache()?;

    match kept {
        Some(kept) => refresh_in(&kept, picture),
        None => Ok(()),
    }
}

fn refresh_in(kept: &Path, picture: &Path) -> Result<(), Never> {
    let full = match picture.canonicalize() {
        Ok(full) => full,
        Err(_not_resolved) => return Ok(()),
    };

    let name = cached_name(&full)?;

    let name = match name {
        Some(name) => name,
        None => return Ok(()),
    };

    let rendered = match modified(&full) {
        Ok(rendered) => rendered,
        Err(_unwritten) => return Ok(()),
    };

    let versions = listed(kept)?;

    for version in versions {
        let every = listed(&version)?;

        'over_frames: for frames in every {
            let named = match frames.file_name() {
                Some(named) => named.to_string_lossy().to_string(),
                None => continue 'over_frames,
            };
            let stale = modified(&frames).is_ok_and(|kept| kept < rendered);

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

fn modified(at: &Path) -> std::io::Result<std::time::SystemTime> {
    let about = at.metadata()?;

    about.modified()
}

pub fn picture(name: &str) -> Result<Option<(PathBuf, PathBuf)>, Never> {
    let hers = user()?;
    let mine = hers.map(|at| at.join(name));
    let theirs = Path::new(CAME_WITH).join(name);

    Ok([mine.as_deref(), Some(&theirs)]
        .into_iter()
        .flatten()
        .map(|at| (at.with_extension("webp"), at.with_extension("still.webp")))
        .find(|(moving, _)| moving.is_file()))
}

pub fn every() -> Result<Vec<String>, Never> {
    let hers = user()?;
    let mut names: Vec<String> = Vec::new();

    for at in [hers, Some(PathBuf::from(CAME_WITH))].into_iter().flatten() {
        let found = match std::fs::read_dir(&at) {
            Ok(found) => found,
            Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
                true => continue,
                false => {
                    eprintln!("console-wallpaper: {}: {fault}", at.display());

                    continue;
                }
            },
        };

        'over_entries: for entry in found {
            let path = match entry {
                Ok(entry) => entry.path(),
                Err(fault) => {
                    eprintln!("console-wallpaper: {}: reading what is in it: {fault}", at.display());

                    continue 'over_entries;
                }
            };

            match path.extension().is_some_and(|kind| kind == "webp") {
                true => {},
                false => continue 'over_entries,
            }

            let name = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name,
                None => continue 'over_entries,
            };

            let name = match name.strip_suffix(".webp") {
                Some(name) => name,
                None => continue 'over_entries,
            };

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

pub fn current_picture(query: &str) -> Result<String, Never> {
    let named = query
        .rsplit_once("image: ")
        .map(|(_, path)| path.trim())
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".webp"))
        .map(|name| match name.strip_suffix(".still") {
            Some(moving) => moving,
            None => name,
        });

    Ok(match named {
        Some(named) => named.to_string(),
        None => String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn a_picture_is_a_moving_file_and_a_still_one_beside_it() {
        let at = Path::new(CAME_WITH).join("star-ride");

        assert_eq!(at.with_extension("webp").file_name(), Some("star-ride.webp".as_ref()));
        assert_eq!(at.with_extension("still.webp").file_name(), Some("star-ride.still.webp".as_ref()));
    }

    #[test]
    fn the_picture_on_the_screen_is_read_out_of_what_the_daemon_says() {
        let said = "skytest: eDP-1: 1920x1200, scale: 1, currently displaying: image: \
                    /usr/share/backgrounds/console/star-ride.webp";
        assert_eq!(current_picture(said), Ok("star-ride".to_string()));
    }

    #[test]
    fn the_still_of_a_picture_is_that_picture() {
        let said =
            "eDP-1: currently displaying: image: /usr/share/backgrounds/console/campfire.still.webp";
        assert_eq!(current_picture(said), Ok("campfire".to_string()));
    }

    #[test]
    fn a_path_holding_a_colon_is_still_a_path() {
        let said = "eDP-1: currently displaying: image: /home/ada/Pictures/a: b/one.webp";
        assert_eq!(current_picture(said), Ok("one".to_string()));
    }

    #[test]
    fn a_daemon_showing_no_picture_names_none() {
        assert_eq!(current_picture(""), Ok(String::new()));
        assert_eq!(current_picture("eDP-1: currently displaying: color: #110b12"), Ok(String::new()));
        assert_eq!(current_picture("no daemon is running"), Ok(String::new()));
    }

    #[test]
    fn the_frames_of_a_picture_are_kept_under_its_path_with_the_slashes_flattened() {
        let at = Path::new("/usr/share/backgrounds/console/lazy-river.webp");
        let Ok(name) = cached_name(at);

        assert_eq!(name.as_deref(), Some("_usr_share_backgrounds_console_lazy-river.webp__"));
    }

    #[test]
    fn frames_older_than_the_picture_go_and_frames_newer_than_it_stay() -> Result<(), Box<dyn Error>> {
        let here = console_core_temporary_directories::fresh("wallpaper-kept")?;
        let kept = here.join("awww/0.12.1");

        std::fs::create_dir_all(&kept)?;

        let picture = here.join("river.webp");

        console_core_atomic_writes::whole(&picture, b"a picture")?;

        let canonical = picture.canonicalize()?;
        let Ok(name) = cached_name(&canonical);
        let name = name.ok_or("a picture with a name is kept under one")?;
        let stale = kept.join(format!("{name}2560x1600_crop_argb"));
        let fresh = kept.join(format!("{name}1920x1200_crop_argb"));
        let other = kept.join("_somewhere_else_snow.webp__2560x1600_crop_argb");

        for at in [&stale, &fresh, &other] {
            console_core_atomic_writes::whole(at, b"frames")?;
        }

        let metadata = picture.metadata()?;
        let written = metadata.modified()?;
        let minute = std::time::Duration::from_secs(60);
        let before = written.checked_sub(minute).ok_or("a minute before the picture")?;
        let after = written.checked_add(minute).ok_or("a minute after the picture")?;

        touch(&stale, before)?;
        touch(&fresh, after)?;
        touch(&other, before)?;

        let Ok(()) = refresh_in(&here.join("awww"), &picture);

        assert!(
            !stale.exists(),
            "the frames of the picture before it were kept"
        );
        assert!(fresh.exists(), "frames of this picture were thrown away");
        assert!(other.exists(), "another picture's frames were thrown away");

        std::fs::remove_dir_all(&here)?;

        Ok(())
    }

    #[test]
    fn a_cache_that_is_not_there_is_nothing_to_throw_away() -> Result<(), Box<dyn Error>> {
        let here = console_core_temporary_directories::fresh("wallpaper-none")?;
        let picture = here.join("river.webp");

        console_core_atomic_writes::whole(&picture, b"a picture")?;

        let Ok(()) = refresh_in(&here.join("nothing-is-kept-here"), &picture);

        std::fs::remove_dir_all(&here)?;

        Ok(())
    }

    fn touch(at: &Path, when: std::time::SystemTime) -> Result<(), std::io::Error> {
        let file = std::fs::File::open(at)?;

        file.set_times(std::fs::FileTimes::new().set_modified(when))
    }

    #[test]
    fn hers_is_looked_in_before_the_set_the_machine_came_with() {
        let Ok(found) = picture("nothing-is-called-this");

        assert!(found.is_none());
    }
}
