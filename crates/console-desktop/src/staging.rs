//! Every file the desktop reads, in one place, pointing at each other.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::{HOME, nested, root, screen, stage};

/// How much of this machine's screen a window may take before it is worth giving
/// density up to fit. A picture of the device is 2560x1600, which is larger than
/// most laptops.
pub const ROOM: f64 = 0.9;

/// What is executable, decided the way the installer decides it.
///
/// By a shebang or an ELF header or the directory, because a file's mode in git
/// is not what the device installs it with. Copying the modes out of the
/// checkout instead gave a stage where settings-panel was not executable, so
/// --open started nothing and the picture was of a desktop with no panel in it.
///
/// Written here rather than reached for, because the engine is a binary and its
/// insides are its own. The engine's own tests assert the same rule.
pub fn mode_of(live: &str, head: &[u8]) -> u32 {
    match live {
        path if path.contains("/bin/") || path.contains("/sbin/") => 0o755,
        _ => match head {
            [b'#', b'!', ..] | [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    }
}

/// Every file under a directory, deepest last, following nothing.
pub fn walk(at: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(at) else { return found };
    for path in entries.flatten().map(|entry| entry.path()) {
        match path.is_dir() && !path.is_symlink() {
            true => found.extend(walk(&path)),
            false => found.push(path),
        }
    }
    found.sort();
    found
}

fn copied(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
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

/// The programs the device compiles for itself, as this machine has them.
///
/// They are not in files/, because the device builds them, so a stage made of
/// files/ alone has a bin directory with the menu, the panel, the guide and the
/// daemon missing from it. Whatever is beside this program is what cargo built,
/// which is what somebody working on it wants to look at.
pub fn built() -> Vec<(String, PathBuf)> {
    let Some(beside) = std::env::current_exe().ok().and_then(|at| at.parent().map(Path::to_path_buf))
    else {
        return Vec::new();
    };
    let held = std::fs::read_to_string(root().join("desktop.conf")).unwrap_or_default();
    section(&held, "build")
        .into_iter()
        .map(|name| (name.clone(), beside.join(name)))
        .filter(|(_, at)| at.is_file())
        .collect()
}

/// What one section of the manifest names.
fn section(held: &str, wanted: &str) -> Vec<String> {
    held.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .fold((Vec::new(), String::new()), |(mut out, at), line| {
            match line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                Some(name) => (out, name.to_string()),
                None => {
                    if at == wanted {
                        out.push(line.to_string());
                    }
                    (out, at)
                }
            }
        })
        .0
}

/// Every absolute path inside a staged file, pointing back into the stage.
fn rewritten(said: &str, here: &str) -> String {
    said.replace(HOME, &format!("{here}/home"))
        .replace("/usr/local", &format!("{here}/usr/local"))
        .replace("/usr/share", &format!("{here}/usr/share"))
}

/// How much of this machine's screen a window may have.
///
/// Asked of the compositor running it rather than assumed, because the whole
/// point is that the machine looking is not the machine being looked at. If it
/// cannot be asked, nothing is given up and the window is the device's own size,
/// which is right on a screen large enough and obvious on one that is not.
pub fn room_here() -> (u32, u32) {
    let go = screen();
    let said = std::process::Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
        .ok()
        .map(|done| String::from_utf8_lossy(&done.stdout).into_owned())
        .unwrap_or_default();
    let Ok(monitors) = serde_json::from_str::<serde_json::Value>(&said) else {
        return go.pixels();
    };
    let largest = |what: &str| {
        monitors
            .as_array()?
            .iter()
            .filter_map(|monitor| {
                let size = monitor.get(what)?.as_f64()?;
                let scale = monitor.get("scale")?.as_f64()?;
                Some(size / scale)
            })
            .fold(f64::NAN, f64::max)
            .into()
    };
    match (largest("width"), largest("height")) {
        (Some(wide), Some(tall)) if wide.is_finite() && tall.is_finite() => {
            ((wide * ROOM) as u32, (tall * ROOM) as u32)
        }
        _ => go.pixels(),
    }
}

/// The staged copy, and the nested config inside it.
pub fn staged(quiet: bool, headless: bool) -> Result<PathBuf, String> {
    let here = stage();
    let fault = |what: &'static str| move |e: std::io::Error| format!("{what}: {e}");
    let _ = std::fs::remove_dir_all(&here);
    std::fs::create_dir_all(&here).map_err(fault("the stage"))?;
    let files = root().join("files");
    copied(&files.join("home/@user@"), &here.join("home")).map_err(fault("the home"))?;
    copied(&files.join("usr"), &here.join("usr")).map_err(fault("the system"))?;

    let said_here = here.display().to_string();
    for path in walk(&here) {
        if path.is_symlink() {
            continue;
        }
        let Ok(was) = std::fs::read_to_string(&path) else { continue };
        let now = rewritten(&was, &said_here);
        if now != was {
            std::fs::write(&path, now).map_err(fault("a staged file"))?;
        }
    }

    for (name, at) in built() {
        let _ = std::fs::copy(&at, here.join("usr/local/bin").join(&name));
    }

    let start = here.join("usr/local/bin/session-start");
    std::fs::write(&start, nested::SESSION_START).map_err(fault("session-start"))?;

    for path in walk(&here) {
        if path.is_symlink() {
            continue;
        }
        let head: Vec<u8> = std::fs::read(&path).unwrap_or_default().into_iter().take(4).collect();
        let live = path.strip_prefix(&here).unwrap_or(&path).display().to_string();
        let mode = mode_of(&format!("/{live}"), &head);
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));
    }

    let go = screen();
    let device_config = here.join("home/.config/hypr/hyprland.lua");
    let said = match headless {
        // A picture of the device is the device's own pixels, all of them.
        // Nothing is given up here: this is the one that gets measured.
        true => nested::headless(&go),
        false => {
            let scale = go.cut_to(room_here());
            if !quiet && (scale - go.scale).abs() > f64::EPSILON {
                let (wide, tall) = go.pixels();
                eprintln!(
                    "this screen cannot hold {wide}x{tall}, so the window is at a scale of \
                     {scale:.2} rather than the device's {}",
                    go.scale
                );
            }
            nested::in_a_window(&go, scale)
        }
    };
    let config = here.join("home/.config/hypr/nested.lua");
    std::fs::write(&config, nested::config(&said, &device_config.display().to_string()))
        .map_err(fault("the nested config"))?;
    if !quiet {
        println!("staged in {}", here.display());
    }
    Ok(config)
}

/// A session that believes the staged copy is the whole system.
pub fn environment() -> Vec<(String, String)> {
    let here = stage();
    let at = |what: &str| here.join(what).display().to_string();
    let path = std::env::var("PATH").unwrap_or_default();
    vec![
        ("HOME".into(), at("home")),
        ("PATH".into(), format!("{}:{path}", at("usr/local/bin"))),
        ("XDG_CACHE_HOME".into(), at("home/.cache")),
        ("XDG_CONFIG_HOME".into(), at("home/.config")),
        ("XDG_DATA_DIRS".into(), format!("{}:/usr/local/share:/usr/share", at("usr/share"))),
        ("XDG_DATA_HOME".into(), at("home/.local/share")),
        ("XDG_STATE_HOME".into(), at("home/.local/state")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file's mode in git is not what the device installs it with.
    #[test]
    fn anything_under_bin_is_staged_able_to_run() {
        assert_eq!(mode_of("/usr/local/bin/launcher", b"#!/b"), 0o755);
        assert_eq!(mode_of("/usr/local/lib/console/palette.sh", b"#!/b"), 0o755);
        assert_eq!(mode_of("/home/@user@/.config/hypr/hyprland.lua", b"-- a"), 0o644);
    }

    /// A stylesheet that says the wallpaper's path has to find the picture that
    /// is going to be installed there.
    #[test]
    fn every_absolute_path_points_back_into_the_stage() {
        let said = rewritten("url(/usr/share/backgrounds/console.webp)\n/home/@user@/.cache", "/s");
        assert_eq!(said, "url(/s/usr/share/backgrounds/console.webp)\n/s/home/.cache");
    }

    /// A stage of files/ alone has no menu, no panel, no guide and no daemon.
    #[test]
    fn the_programs_the_device_builds_are_staged_too() {
        let held = std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf");
        assert!(section(&held, "build").contains(&"launcher".to_string()));
    }

    #[test]
    fn a_session_is_told_the_stage_is_the_whole_system() {
        let named: Vec<String> = environment().into_iter().map(|(name, _)| name).collect();
        let mut ordered = named.clone();
        ordered.sort();
        assert_eq!(named, ordered, "the environment is not in order");
    }
}
