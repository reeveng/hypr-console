//! Every file the desktop reads, in one place, pointing at each other.
//!
//! The copy is shared rather than made. A stage is every program `[build]`
//! names, and in a debug build that is gigabytes of unstripped binary; written
//! out with `std::fs::copy` it was gigabytes off the disk and back onto it for
//! every session, and a laptop with a thousand of them left on it had thirty-six
//! gigabytes of stages no one had looked at. `FICLONE` is the same file with the
//! same path and its own inode, sharing the extents it came from until something
//! writes, so a stage costs what its names cost and the writing that follows
//! cannot reach `target/`. A filesystem that cannot share extents says so and
//! the bytes are copied as before.
//!
//! Reading is bounded the same way. What decides a staged file's mode is its
//! first four bytes, and asking for them used to read the whole file into memory
//! first -- every binary in the stage, one at a time, to look at four bytes.

use console_manifest_engine::manifest::{Manifest, Section};
use console_manifest_migrations::USER;
use console_manifest_engine::modes;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_u32};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use console_core_directory_listing::Descend;
use std::path::{Path, PathBuf};

use crate::nested::Wallpaper;
use crate::{Unnested, nested, root, screen, session, stage};

pub const ROOM: f64 = 0.9;

pub fn walk(root: &Path) -> Result<Vec<PathBuf>, Never> {
    console_core_directory_listing::files(root)
}

const HEAD: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shared {
    Extents,
    Bytes,
}

#[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
        explicit040_no_torn_write,
        reason = "sharing extents is an ioctl between two descriptors and there is no rename over a name to do it with. What makes that safe is the stage rather than the write: a session directory is built whole and swept whole, and one that stopped halfway is named after a process that is gone, which is what `session::abandoned` removes before the next run stages anything"
    )
)]
fn clone_file(from: &Path, to: &Path) -> std::io::Result<Shared> {
    let source = std::fs::File::open(from)?;
    let target = std::fs::File::create(to)?;

    match rustix::fs::ioctl_ficlone(&target, &source) {
        Ok(()) => Ok(Shared::Extents),
        Err(_a_filesystem_that_shares_nothing) => {
            drop(target);
            std::fs::copy(from, to)?;

            Ok(Shared::Bytes)
        }
    }
}

fn head_of(at: &Path) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(at)?;
    let mut head = Vec::new();

    file.take(HEAD).read_to_end(&mut head)?;

    Ok(head)
}

fn copy_tree(root: &Path, into: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(into)?;

    let Ok(mut listing) = console_core_directory_listing::recursive(root, |_| Descend::Into);

    listing.try_for_each(|entry| {
        let source = entry.map_err(|unlisted| unlisted.fault)?;
        let target = match source.strip_prefix(root) {
            Ok(inside) => into.join(inside),
            Err(outside) => return Err(std::io::Error::other(outside)),
        };

        copied(&source, &target)
    })
}

fn copied(source: &Path, target: &Path) -> std::io::Result<()> {
    match (source.is_symlink(), source.is_dir()) {
        (true, _) => {
            let at = std::fs::read_link(source)?;
            let _ = std::fs::remove_file(target);

            std::os::unix::fs::symlink(at, target)
        }
        (false, true) => std::fs::create_dir_all(target),
        (false, false) => {
            let _shared = clone_file(source, target)?;

            Ok(())
        }
    }
}

pub fn built() -> Result<Vec<(String, PathBuf)>, Never> {
    let beside = match std::env::current_exe() {
        Ok(at) => at.parent().map(Path::to_path_buf),
        Err(fault) => {
            eprintln!("console-desktop: where this program is: {fault}");

            None
        }
    };

    let beside = match beside {
        Some(beside) => beside,
        None => return Ok(Vec::new()),
    };

    let Ok(root) = root();

    let at = root.join(console_repository::MARK);
    let held = match std::fs::read_to_string(&at) {
        Ok(held) => held,
        Err(fault) => {
            eprintln!("console-desktop: {}: {fault}", at.display());

            String::new()
        }
    };

    let manifest = match Manifest::read(&held) {
        Ok(manifest) => manifest,
        Err(fault) => {
            eprintln!("console-desktop: {}: {fault}", at.display());

            Manifest::default()
        }
    };

    let Ok(named) = manifest.of(Section::Build);

    Ok(named
        .iter()
        .map(|name| (name.clone(), beside.join(name)))
        .filter(|(_, at)| at.is_file())
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Here<'a>(&'a str);

fn rewrite_paths(said: &str, here: Here<'_>) -> Result<String, Never> {
    let here = here.0;

    Ok(said
        .replace(&format!("/home/{USER}"), &format!("{here}/home"))
        .replace("/usr/local", &format!("{here}/usr/local"))
        .replace("/usr/share", &format!("{here}/usr/share")))
}

pub fn room_here(go: &console_screen::Screen) -> Result<Size<u32>, Never> {
    let monitors = match console_compositor::ask(console_compositor::Monitors) {
        Ok(monitors) => monitors,
        Err(_no_compositor_here) => {
            let Ok(pixels) = go.pixels();

            return Ok(pixels);
        }
    };

    let logical = |monitor: &console_compositor::Monitor| {
        let (wide, tall) = monitor.size?;
        let scale = monitor.scale?;
        let Ok(wide) = wide.float();
        let Ok(tall) = tall.float();

        Some((wide / scale, tall / scale))
    };

    let largest = monitors.iter().filter_map(logical).fold((f64::NAN, f64::NAN), |held, room| {
        (f64::max(held.0, room.0), f64::max(held.1, room.1))
    });

    let (wide, tall) = largest;

    Ok(match wide.is_finite() && tall.is_finite() {
        true => {
            let Ok(wide) = toward_zero_u32(wide * ROOM);
            let Ok(tall) = toward_zero_u32(tall * ROOM);

            Size { width: wide, height: tall }
        }
        false => {
            let Ok(pixels) = go.pixels();

            pixels
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    Aloud,
    Quietly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Headless,
    InAWindow,
}

pub fn stage_desktop(told: Verbosity, headless: Screen, wallpaper: Wallpaper) -> Result<PathBuf, Unnested> {
    let Ok(()) = session::swept();
    let Ok(here) = stage();

    let fault = |what: &'static str| move |error: std::io::Error| Unnested::Staging(what, error);
    let unwritten =
        |what: &'static str| move |error: console_core_atomic_writes::Unwritten| Unnested::Unwritten(what, error);
    let _ = std::fs::remove_dir_all(&here);
    std::fs::create_dir_all(&here).map_err(fault("the stage"))?;
    let Ok(root) = root();

    let files = root.join("files");
    copy_tree(&files.join("home/@user@"), &here.join("home")).map_err(fault("the home"))?;
    copy_tree(&files.join("usr"), &here.join("usr")).map_err(fault("the system"))?;

    let said_here = here.display().to_string();
    let Ok(staged) = walk(&here);

    for path in staged {
        match path.is_symlink() {
            true => continue,
            false => {},
        }

        let was = match std::fs::read_to_string(&path) {
            Ok(was) => was,
            Err(_unreadable) => continue,
        };

        let Ok(now) = rewrite_paths(&was, Here(&said_here));

        match now == was {
            true => {},
            false => console_core_atomic_writes::whole(&path, now.as_bytes())
                .map_err(unwritten("a staged file"))?,
        }
    }

    let Ok(programs) = built();

    for (name, at) in programs {
        let _ = clone_file(&at, &here.join("usr/local/bin").join(&name));
    }

    let unit = root.join(nested::UNIT);
    let held = std::fs::read_to_string(&unit).map_err(fault("the keyboard's unit"))?;
    let Ok(started_by) = nested::started_by(&held);

    let keyboard = started_by.ok_or_else(|| Unnested::NoExecStart(unit.clone()))?;
    let start = here.join("usr/local/bin/session-start");
    let Ok(start_said) = nested::session_start(&keyboard, wallpaper);
    let Ok(session) = rewrite_paths(&start_said, Here(&said_here));

    console_core_atomic_writes::whole(&start, session.as_bytes())
        .map_err(unwritten("session-start"))?;
    let Ok(staged) = walk(&here);

    for path in staged {
        match path.is_symlink() {
            true => continue,
            false => {},
        }

        let head = head_of(&path).map_err(fault("a staged file"))?;
        let live = match path.strip_prefix(&here) {
            Ok(under) => under.display().to_string(),
            Err(_outside_the_stage) => path.display().to_string(),
        };
        let Ok(mode) = modes::of(&format!("/{live}"), &head);
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));
    }

    let go = screen()?;
    let Ok(ours) = console_core_places::Base::Configuration.application_under(&here.join("home"));

    let device_configuration = ours.join("hypr/hyprland.lua");
    let at_scale = match headless {
        Screen::Headless => go.scale,
        Screen::InAWindow => {
            let Ok(room) = room_here(&go);
            let Ok(cut) = go.cut_to(room);

            match told == Verbosity::Aloud && (cut - go.scale).abs() > f64::EPSILON {
                true => {
                    let Ok(pixels) = go.pixels();
                    let (wide, tall) = (pixels.width, pixels.height);

                    eprintln!(
                        "this screen cannot hold {wide}x{tall}, so the window is at a scale of \
                         {cut:.2} rather than the device's {}",
                        go.scale
                    );
                }
                false => {},
            }

            cut
        }
    };
    let said = match headless {
        Screen::Headless => {
            let Ok(said) = nested::headless(&go);

            said
        }
        Screen::InAWindow => {
            let Ok(said) = nested::in_a_window(&go, at_scale);

            said
        }
    };
    let configuration = ours.join("hypr/nested.lua");
    let Ok(nested) = nested::configuration(
        nested::Names { screen: &said, device: &device_configuration.display().to_string() },
        wallpaper,
    );

    console_core_atomic_writes::whole(&configuration, nested.as_bytes())
        .map_err(unwritten("the nested config"))?;

    match told {
        Verbosity::Aloud => println!("staged in {}", here.display()),
        Verbosity::Quietly => {},
    }

    Ok(configuration)
}

pub fn environment() -> Result<Vec<(String, String)>, Never> {
    let Ok(here) = stage();

    let at = |what: &str| here.join(what).display().to_string();
    let Ok(said) = console_core_external_programs::path();

    let path = match said {
        Some(path) => path,
        None => String::new(),
    };

    Ok(vec![
        (String::from("HOME"), at("home")),
        (String::from("PATH"), format!("{}:{path}", at("usr/local/bin"))),
        (String::from("XDG_CACHE_HOME"), at("home/.cache")),
        (String::from("XDG_CONFIG_HOME"), at("home/.config")),
        (String::from("XDG_DATA_DIRS"), format!("{}:/usr/local/share:/usr/share", at("usr/share"))),
        (String::from("XDG_DATA_HOME"), at("home/.local/share")),
        (String::from("XDG_STATE_HOME"), at("home/.local/state")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    #[test]
    fn the_stage_starts_the_keyboard_the_unit_starts_and_from_inside_the_stage() -> Result<(), Failure> {
        let root = root()?;
        let held = std::fs::read_to_string(root.join(nested::UNIT))?;
        let unit = nested::started_by(&held)?;
        let started = unit.ok_or("the keyboard's unit starts nothing")?;
        let keyboard = started.split(' ').next().ok_or("the keyboard's unit starts an empty line")?;
        let start = nested::session_start(keyboard, Wallpaper::Started)?;
        let said = rewrite_paths(&start, Here("/s"))?;

        assert!(
            said.contains(r#"keyboard="/s/usr/local/bin/console-keyboard"#),
            "the staged session starts a keyboard the staged toggle cannot signal: {said}"
        );

        let Ok(wanted) = rewrite_paths(keyboard, Here("/s"));

        assert!(
            said.contains(&format!("keyboard=\"{wanted}\"")),
            "the staged session starts a command line the unit does not, so what this stage \
             types with is written down twice and only one of the two is the device's: {said}"
        );
        assert!(
            said.contains("/s/usr/local/lib/console/palette.sh"),
            "the staged session reads a palette that is not this tree's: {said}"
        );

        Ok(())
    }

    #[test]
    fn a_unit_that_starts_nothing_absolute_is_not_a_keyboard_the_stage_can_raise() {
        assert_eq!(nested::started_by("[Service]\nExecStart=console-keyboard\n"), Ok(None));
        assert_eq!(
            nested::started_by("[Service]\nExecStart=/usr/local/bin/console-keyboard -l x\n"),
            Ok(Some("/usr/local/bin/console-keyboard -l x".to_string()))
        );
    }

    #[test]
    fn every_absolute_path_points_back_into_the_stage() {
        let said =
            rewrite_paths("url(/usr/share/backgrounds/console.webp)\n/home/@user@/.cache", Here("/s"));

        assert_eq!(said, Ok("url(/s/usr/share/backgrounds/console.webp)\n/s/home/.cache".to_string()));
    }

    #[test]
    fn the_programs_the_device_builds_are_staged_too() -> Result<(), Failure> {
        let root = root()?;
        let held = std::fs::read_to_string(root.join("desktop.conf"))?;
        let manifest = Manifest::read(&held)?;
        let Ok(built) = manifest.of(Section::Build);

        assert!(built.contains(&"launcher".to_string()));

        Ok(())
    }

    #[test]
    #[cfg_attr(dylint_lib = "explicit040_no_torn_write", allow(explicit040_no_torn_write, reason = "a write in place is what is being asked about: a rename would give the staged copy a new file and prove nothing about the clone"))]
    fn a_staged_file_cannot_reach_the_one_it_came_from() -> Result<(), Failure> {
        let Ok(root) = root();

        let here = root.join(".stage").join(format!("cloning-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&here);
        std::fs::create_dir_all(&here)?;

        let from = here.join("built");
        let to = here.join("staged");
        std::fs::write(&from, b"the program as it was built")?;

        let shared = clone_file(&from, &to)?;
        let staged = std::fs::read(&to)?;

        assert_eq!(staged, b"the program as it was built".to_vec(), "the staged copy is not what it came from");

        std::fs::write(&to, b"and what the stage did to it")?;

        let built = std::fs::read(&from)?;

        assert_eq!(
            built,
            b"the program as it was built".to_vec(),
            "a write into the stage reached the build it was cloned from"
        );

        let _ = std::fs::remove_dir_all(&here);

        match shared {
            Shared::Extents => {},
            Shared::Bytes => eprintln!(
                "the tree is on a filesystem that cannot share extents, so every stage \
                 costs what it copies"
            ),
        }

        Ok(())
    }

    #[test]
    fn a_mode_is_decided_by_four_bytes_and_reads_four_bytes() -> Result<(), Failure> {
        let here = console_core_temporary_directories::fresh("head")?;
        let at = here.join("script");

        console_core_atomic_writes::whole(&at, b"#!/bin/sh\nand a great deal more after it\n")?;

        let head = head_of(&at)?;

        assert_eq!(head, b"#!/b".to_vec());

        let short = here.join("short");

        console_core_atomic_writes::whole(&short, b"ab")?;

        let head = head_of(&short)?;

        assert_eq!(head, b"ab".to_vec());

        let _ = std::fs::remove_dir_all(&here);

        Ok(())
    }

    #[test]
    fn a_session_is_told_the_stage_is_the_whole_system() -> Result<(), Failure> {
        let environment = environment()?;
        let named: Vec<String> = environment.into_iter().map(|(name, _)| name).collect();
        let mut ordered = named.clone();

        ordered.sort();

        assert_eq!(named, ordered, "the environment is not in order");

        Ok(())
    }
}
