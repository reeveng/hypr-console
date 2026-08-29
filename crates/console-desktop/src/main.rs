//! The Legion Go's desktop, running on this machine, in a window.
//!
//! ```text
//! console-desktop verify     does the compositor config still parse
//! console-desktop run        the desktop, nested, at the device's size
//! console-desktop shot FILE  a picture of it
//! console-desktop probe      what the nested compositor thinks
//! console-desktop stage      the staged copy, and nothing else
//! console-desktop clean      forget what nobody is using
//! ```

use std::path::PathBuf;
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::Duration;

use console_desktop::staging::{environment, staged};
use console_desktop::talking::Inside;
use console_desktop::{screen, session, stage};
use console_stage::picture::{Picture, where_};

/// What was asked for on the command line.
struct Asked {
    command: String,
    file: Option<PathBuf>,
    seconds: Option<f64>,
    open: Vec<String>,
    sample: Vec<String>,
    window: bool,
}

fn asked(words: Vec<String>) -> Asked {
    let every = |what: &str| {
        words
            .iter()
            .enumerate()
            .filter(|(_, word)| *word == what)
            .filter_map(|(at, _)| words.get(at + 1).cloned())
            .collect::<Vec<String>>()
    };
    let bare: Vec<&String> = words
        .iter()
        .enumerate()
        .filter(|(_, word)| !word.starts_with("--"))
        .filter(|(at, _)| *at == 0 || !words[at - 1].starts_with("--"))
        .map(|(_, word)| word)
        .collect();
    Asked {
        command: bare
            .first()
            .map_or_else(|| "run".to_string(), |word| (*word).clone()),
        file: bare.get(1).map(PathBuf::from),
        seconds: every("--seconds")
            .first()
            .and_then(|said| said.parse().ok()),
        open: every("--open"),
        sample: every("--sample"),
        window: words.iter().any(|word| word == "--window"),
    }
}

fn main() -> ExitCode {
    let asked = asked(std::env::args().skip(1).collect());
    let done = match asked.command.as_str() {
        "clean" => clean(),
        "stage" => staged(false, false).map(|_| 0),
        "verify" => verify(),
        "probe" => run(&asked, None, true),
        "shot" => match asked.file.clone() {
            Some(file) => run(&asked, Some(file), false),
            None => Err("a picture wants somewhere to be written".to_string()),
        },
        _ => run(&asked, None, false),
    };
    // A stage is one session's, and the session is over.
    if asked.command != "stage" {
        let _ = std::fs::remove_dir_all(stage());
    }
    match done {
        Ok(code) => ExitCode::from(code),
        Err(why) => {
            eprintln!("{why}");
            ExitCode::from(1)
        }
    }
}

fn clean() -> Result<u8, String> {
    let every = [stage()]
        .into_iter()
        .chain(session::abandoned())
        .chain(session::dead_instances());
    for path in every {
        let _ = std::fs::remove_dir_all(path);
    }
    Ok(0)
}

fn verify() -> Result<u8, String> {
    let nested = staged(true, false)?;
    let mut asking = Command::new("Hyprland");
    asking.args(["--verify-config", "-c"]).arg(&nested);
    for (name, value) in environment() {
        asking.env(name, value);
    }
    let done = asking.output().map_err(|fault| fault.to_string())?;
    let said = String::from_utf8_lossy(&done.stdout).trim().to_string();
    let complained = String::from_utf8_lossy(&done.stderr).trim().to_string();
    println!("{}", if said.is_empty() { complained } else { said });
    Ok(u8::from(!done.status.success()))
}

/// The nested compositor, up, and whatever was asked of it.
fn run(asked: &Asked, shot: Option<PathBuf>, probe: bool) -> Result<u8, String> {
    let headless = !asked.window;
    let nested = staged(true, headless && shot.is_some())?;
    let where_ = environment();

    let mut compositor = {
        let _held = session::Starting::now();
        let (was, running) = (session::sockets(), session::instances());
        let mut asking = Command::new("Hyprland");
        asking
            .arg("-c")
            .arg(&nested)
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE");
        for (name, value) in &where_ {
            asking.env(name, value);
        }
        let started = asking.spawn().map_err(|fault| fault.to_string())?;
        (
            started,
            session::wait_for_socket(&was),
            session::wait_for_instance(&running),
        )
    };

    let (Some(socket), Some(signature)) = (compositor.1.clone(), compositor.2.clone()) else {
        let _ = compositor.0.kill();
        return Err("the nested compositor never came up".to_string());
    };
    eprintln!("the desktop is on {socket}");

    // Nothing to look at and nothing to take: this is somebody watching it.
    if shot.is_none() && asked.seconds.is_none() && !probe {
        let ended = compositor.0.wait().map_err(|fault| fault.to_string())?;
        session::left_behind(&signature);
        return Ok(u8::from(!ended.success()));
    }

    let inside = Inside::new(where_, &socket, &signature);
    let go = screen();
    let looked_at = match headless {
        true => "HEADLESS-1",
        false => "WAYLAND-1",
    };
    if headless {
        inside.make_the_screen(&go);
    }
    if !inside.wait_for_screen(looked_at) {
        stop(&mut compositor.0, &signature, &inside);
        return Err("the screen never appeared".to_string());
    }

    inside.paint_the_background(&stage().join("usr/share/backgrounds/console.webp"));

    let was = inside.surfaces();
    let opened: Vec<(String, Child)> = asked
        .open
        .iter()
        .filter_map(|command| {
            let started = inside
                .command("sh")
                .args(["-c", command])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .ok()?;
            Some((command.clone(), started))
        })
        .collect();
    if !opened.is_empty() {
        if !inside.wait_for_something(&was) {
            eprintln!("nothing that was asked for reached the screen");
        }
        inside.show_a_window();
        say_what_died(opened);
    }

    if probe {
        for question in ["monitors", "workspaces", "clients"] {
            println!("== {question}");
            let said = inside.hyprctl(&[question, "-j"]);
            println!("{}", said.chars().take(1200).collect::<String>());
        }
    }

    if let Some(file) = &shot {
        let taken = inside
            .command("grim")
            .args(["-o", looked_at])
            .arg(file)
            .output()
            .map_err(|fault| fault.to_string())?;
        match taken.status.success() {
            true => {
                println!("{}", file.display());
                say_the_colours(file, &asked.sample, &go);
            }
            false => {
                eprintln!(
                    "no picture: {}",
                    String::from_utf8_lossy(&taken.stderr).trim()
                );
            }
        }
    }

    if let Some(seconds) = asked.seconds {
        std::thread::sleep(Duration::from_secs_f64(seconds));
    }
    stop(&mut compositor.0, &signature, &inside);
    Ok(0)
}

fn stop(compositor: &mut Child, signature: &str, inside: &Inside) {
    // First, because both die of the compositor's leaving if they are still
    // holding a connection when it goes, in different ways and for different
    // reasons. See Inside::stop_the_wallpaper and Inside::stop_the_bar.
    inside.stop_the_wallpaper();
    inside.stop_the_bar();
    // SAFETY: a signal to the compositor this started, by its own pid.
    unsafe { libc::kill(compositor.id() as i32, libc::SIGTERM) };
    let by = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < by {
        if compositor.try_wait().is_ok_and(|ended| ended.is_some()) {
            session::left_behind(signature);
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = compositor.kill();
    let _ = compositor.wait();
    session::left_behind(signature);
}

/// A program asked for that is not there any more, and what it said.
///
/// Sent to nowhere, a program that cannot start leaves a picture of a desktop
/// without it and no reason why, and the picture is what gets believed.
fn say_what_died(opened: Vec<(String, Child)>) {
    for (command, mut process) in opened {
        let Ok(Some(ended)) = process.try_wait() else {
            continue;
        };
        // Read off the pipe rather than through wait_with_output, which has
        // nothing to give for a child that has already been reaped.
        let mut said = String::new();
        if let Some(mut voice) = process.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut voice, &mut said);
        }
        let said = said.trim().to_string();
        eprintln!(
            "{command} ended with {}: {}",
            ended.code().unwrap_or(-1),
            if said.is_empty() {
                "nothing said".to_string()
            } else {
                said
            }
        );
    }
}

/// What colour the screen is where it was asked about.
///
/// A screenshot nobody looks at agrees with anything. The wallpaper on the
/// device had not painted for days: hyprpaper read a config it no longer
/// understood, painted nothing, and reported success. What was on screen was the
/// compositor's own default, dark enough to pass for a background.
fn say_the_colours(shot: &std::path::Path, sample: &[String], go: &console_screen::Screen) {
    if sample.is_empty() {
        return;
    }
    let Ok(picture) = Picture::read(shot) else {
        return;
    };
    for place in sample {
        if place == "most" {
            println!("  most of it   #{}", picture.commonest());
            continue;
        }
        let Some((across, down)) = place.split_once(',') else {
            continue;
        };
        let (Ok(across), Ok(down)) = (across.trim().parse::<f64>(), down.trim().parse::<f64>())
        else {
            continue;
        };
        match where_(&picture, across, down, go) {
            Ok(colour) => println!("  {:<12} #{colour}", format!("{across},{down}")),
            Err(why) => eprintln!("  {place}: {why}"),
        }
    }
}
