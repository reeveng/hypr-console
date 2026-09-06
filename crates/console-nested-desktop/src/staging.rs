//! Every file the desktop reads, in one place, pointing at each other.


use console_external_programs::Program;
use console_never::Never;
use console_number_conversion::toward_zero_u32;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::{HOME, nested, root, screen, stage};

pub const ROOM: f64 = 0.9;

pub fn mode_of(live: &str, head: &[u8]) -> Result<u32, Never> {
    Ok(match live {
        path if path.contains("/bin/") || path.contains("/sbin/") => 0o755,
        _ => match head {
            [b'#', b'!', ..] | [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    })
}

pub fn walk(at: &Path) -> Result<Vec<PathBuf>, Never> {
    let mut found = Vec::new();

    let Ok(entries) = std::fs::read_dir(at) else { return Ok(found) };

    for path in entries.flatten().map(|entry| entry.path()) {
        match path.is_dir() && !path.is_symlink() {
            true => {
                let Ok(under) = walk(&path);

                found.extend(under);
            }
            false => found.push(path),
        }
    }

    found.sort();

    Ok(found)
}

fn copied(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;

    let listed = std::fs::read_dir(from)?;

    for entry in listed.flatten() {
        let (source, target) = (entry.path(), to.join(entry.file_name()));

        match (source.is_symlink(), source.is_dir()) {
            (true, _) => {
                let at = std::fs::read_link(&source)?;
                let _ = std::fs::remove_file(&target);
                std::os::unix::fs::symlink(at, &target)?;
            }
            (_, true) => copied(&source, &target)?,
            _ => {
                std::fs::copy(&source, &target)?;
            }
        }
    }

    Ok(())
}

pub fn built() -> Result<Vec<(String, PathBuf)>, Never> {
    let beside = match std::env::current_exe() {
        Ok(at) => at.parent().map(Path::to_path_buf),
        Err(fault) => {
            eprintln!("console-desktop: where this program is: {fault}");

            None
        }
    };

    let Some(beside) = beside else { return Ok(Vec::new()) };

    let Ok(root) = root();

    let at = root.join("desktop.conf");
    let held = match std::fs::read_to_string(&at) {
        Ok(held) => held,
        Err(fault) => {
            eprintln!("console-desktop: {}: {fault}", at.display());

            String::new()
        }
    };

    let Ok(named) = section(&held, "build");

    Ok(named
        .into_iter()
        .map(|name| (name.clone(), beside.join(name)))
        .filter(|(_, at)| at.is_file())
        .collect())
}

fn section(held: &str, wanted: &str) -> Result<Vec<String>, Never> {
    Ok(held
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .fold((Vec::new(), String::new()), |(mut out, at), line| {
            match line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                Some(name) => (out, name.to_string()),
                None => {
                    match at == wanted {
                        true => out.push(line.to_string()),
                        false => {},
                    }

                    (out, at)
                }
            }
        })
        .0)
}

fn rewritten(said: &str, here: &str) -> Result<String, Never> {
    Ok(said
        .replace(HOME, &format!("{here}/home"))
        .replace("/usr/local", &format!("{here}/usr/local"))
        .replace("/usr/share", &format!("{here}/usr/share")))
}

pub fn room_here(go: &console_screen::Screen) -> Result<(u32, u32), Never> {
    let Ok(mut asking) = Program::Hyprctl.command();

    let said = match asking.args(["monitors", "-j"]).output() {
        Ok(done) => String::from_utf8_lossy(&done.stdout).into_owned(),
        Err(_no_compositor_here) => String::new(),
    };

    let Ok(monitors) = serde_json::from_str::<serde_json::Value>(&said) else {
        let Ok(pixels) = go.pixels();

        return Ok(pixels);
    };

    let largest = |what: &str| {
        let every = monitors.as_array()?;

        every
            .iter()
            .filter_map(|monitor| {
                let asked = monitor.get(what)?;
                let size = asked.as_f64()?;
                let said = monitor.get("scale")?;
                let scale = said.as_f64()?;

                Some(size / scale)
            })
            .fold(f64::NAN, f64::max)
            .into()
    };

    Ok(match (largest("width"), largest("height")) {
        (Some(wide), Some(tall)) if wide.is_finite() && tall.is_finite() => {
            let Ok(wide) = toward_zero_u32(wide * ROOM);
            let Ok(tall) = toward_zero_u32(tall * ROOM);

            (wide, tall)
        }
        _ => {
            let Ok(pixels) = go.pixels();

            pixels
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Told {
    Aloud,
    Quietly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Headless,
    InAWindow,
}

pub fn staged(told: Told, headless: Screen) -> Result<PathBuf, String> {
    let Ok(here) = stage();

    let fault = |what: &'static str| move |e: std::io::Error| format!("{what}: {e}");
    let _ = std::fs::remove_dir_all(&here);
    std::fs::create_dir_all(&here).map_err(fault("the stage"))?;
    let Ok(root) = root();

    let files = root.join("files");
    copied(&files.join("home/@user@"), &here.join("home")).map_err(fault("the home"))?;
    copied(&files.join("usr"), &here.join("usr")).map_err(fault("the system"))?;

    let said_here = here.display().to_string();
    let Ok(staged) = walk(&here);

    for path in staged {
        match path.is_symlink() {
            true => continue,
            false => {},
        }

        let Ok(was) = std::fs::read_to_string(&path) else { continue };

        let Ok(now) = rewritten(&was, &said_here);

        match now == was {
            true => {},
            false => std::fs::write(&path, now).map_err(fault("a staged file"))?,
        }
    }

    let Ok(programs) = built();

    for (name, at) in programs {
        let _ = std::fs::copy(&at, here.join("usr/local/bin").join(&name));
    }

    let unit = root.join(nested::UNIT);
    let held = std::fs::read_to_string(&unit).map_err(fault("the keyboard's unit"))?;
    let Ok(started_by) = nested::started_by(&held);

    let keyboard = started_by.ok_or_else(|| {
        format!(
            "{} names no absolute ExecStart, so the stage would start a keyboard nothing \
             could raise",
            unit.display()
        )
    })?;
    let start = here.join("usr/local/bin/session-start");
    let Ok(start_said) = nested::session_start(&keyboard);
    let Ok(session) = rewritten(&start_said, &said_here);

    std::fs::write(&start, session).map_err(fault("session-start"))?;
    let Ok(staged) = walk(&here);

    for path in staged {
        match path.is_symlink() {
            true => continue,
            false => {},
        }

        let head: Vec<u8> = std::fs::read(&path).unwrap_or_default().into_iter().take(4).collect();
        let live = path.strip_prefix(&here).unwrap_or(&path).display().to_string();
        let Ok(mode) = mode_of(&format!("/{live}"), &head);
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));
    }

    let go = screen()?;
    let device_config = here.join("home/.config/hypr/hyprland.lua");
    let said = match headless {
        Screen::Headless => {
            let Ok(said) = nested::headless(&go);

            said
        }
        Screen::InAWindow => {
            let Ok(room) = room_here(&go);
            let Ok(scale) = go.cut_to(room);

            match told == Told::Aloud && (scale - go.scale).abs() > f64::EPSILON {
                true => {
                    let Ok((wide, tall)) = go.pixels();

                    eprintln!(
                        "this screen cannot hold {wide}x{tall}, so the window is at a scale of \
                         {scale:.2} rather than the device's {}",
                        go.scale
                    );
                }
                false => {},
            }

            let Ok(said) = nested::in_a_window(&go, scale);

            said
        }
    };
    let bar = here.join("home/.config/console/bar.css");

    match bar.parent() {
        Some(holding) => {
            let _ = std::fs::create_dir_all(holding);
        }
        None => {},
    }

    let Ok(css) = console_screen::bar_css(&go, go.scale);

    std::fs::write(&bar, css).map_err(fault("the bar's width"))?;

    let config = here.join("home/.config/hypr/nested.lua");
    let Ok(nested) = nested::config(&said, &device_config.display().to_string());

    std::fs::write(&config, nested).map_err(fault("the nested config"))?;

    match told {
        Told::Aloud => println!("staged in {}", here.display()),
        Told::Quietly => {},
    }

    Ok(config)
}

pub fn environment() -> Result<Vec<(String, String)>, Never> {
    let Ok(here) = stage();

    let at = |what: &str| here.join(what).display().to_string();
    let Ok(said) = crate::said("PATH");
    let path = said.unwrap_or_default();

    Ok(vec![
        ("HOME".into(), at("home")),
        ("PATH".into(), format!("{}:{path}", at("usr/local/bin"))),
        ("XDG_CACHE_HOME".into(), at("home/.cache")),
        ("XDG_CONFIG_HOME".into(), at("home/.config")),
        ("XDG_DATA_DIRS".into(), format!("{}:/usr/local/share:/usr/share", at("usr/share"))),
        ("XDG_DATA_HOME".into(), at("home/.local/share")),
        ("XDG_STATE_HOME".into(), at("home/.local/state")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anything_under_bin_is_staged_able_to_run() {
        assert_eq!(mode_of("/usr/local/bin/launcher", b"#!/b"), Ok(0o755));
        assert_eq!(mode_of("/usr/local/lib/console/palette.sh", b"#!/b"), Ok(0o755));
        assert_eq!(mode_of("/home/@user@/.config/hypr/hyprland.lua", b"-- a"), Ok(0o644));
    }

    #[test]
    fn the_stage_starts_the_keyboard_the_unit_starts_and_from_inside_the_stage() {
        let root = root().expect("the tree");
        let held = std::fs::read_to_string(root.join(nested::UNIT)).expect("the keyboard's unit");
        let keyboard = nested::started_by(&held).expect("a unit").expect("an ExecStart");
        let start = nested::session_start(&keyboard).expect("the session's start");
        let said = rewritten(&start, "/s").expect("the rewriting");
        assert!(
            said.contains(r#"keyboard="/s/usr/local/bin/virtual-keyboard"#),
            "the staged session starts a keyboard the staged toggle cannot signal: {said}"
        );
        assert!(
            said.contains("--landscape-layers"),
            "the staged keyboard is not given the alphabets the device's is: {said}"
        );
        assert!(
            said.contains("/s/usr/local/lib/console/palette.sh"),
            "the staged session reads a palette that is not this tree's: {said}"
        );
    }

    #[test]
    fn a_unit_that_starts_nothing_absolute_is_not_a_keyboard_the_stage_can_raise() {
        assert_eq!(nested::started_by("[Service]\nExecStart=virtual-keyboard\n"), Ok(None));
        assert_eq!(
            nested::started_by("[Service]\nExecStart=/usr/local/bin/virtual-keyboard -l x\n"),
            Ok(Some("/usr/local/bin/virtual-keyboard -l x".to_string()))
        );
    }

    #[test]
    fn every_absolute_path_points_back_into_the_stage() {
        let said = rewritten("url(/usr/share/backgrounds/console.webp)\n/home/@user@/.cache", "/s");
        assert_eq!(said, Ok("url(/s/usr/share/backgrounds/console.webp)\n/s/home/.cache".to_string()));
    }

    #[test]
    fn the_programs_the_device_builds_are_staged_too() {
        let root = root().expect("the tree");
        let held = std::fs::read_to_string(root.join("desktop.conf")).expect("desktop.conf");
        let built = section(&held, "build").expect("the build section");
        assert!(built.contains(&"launcher".to_string()));
    }

    #[test]
    fn a_session_is_told_the_stage_is_the_whole_system() {
        let environment = environment().expect("the environment");
        let named: Vec<String> = environment.into_iter().map(|(name, _)| name).collect();
        let mut ordered = named.clone();
        ordered.sort();
        assert_eq!(named, ordered, "the environment is not in order");
    }
}
