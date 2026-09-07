//! The Legion Go's desktop, running on this machine, in a window.
//!
//! ```text
//! console-desktop verify     does the compositor config still parse
//! console-desktop run        the desktop, nested, at the device's size
//! console-desktop shot FILE  a picture of it, once the screen has stopped
//!                            changing, or --settle N seconds after
//! console-desktop shot FILE --bare   the same, without a ground to paint
//! console-desktop shot FILE --until AT   and not before AT has a line in it
//! console-desktop probe      what the nested compositor thinks
//! console-desktop stage      the staged copy, and nothing else
//! console-desktop clean      forget what nobody is using
//! ```


use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_waiting::{Patience, Seen, until};
use std::path::PathBuf;
use std::process::{Child, ExitCode, Stdio};
use std::time::Duration;

use console_test_desktop::nested::Wallpaper;
use console_test_desktop::staging::{Screen, Told, environment, staged};
use console_test_desktop::talking::{Inside, Waited};
use console_test_desktop::{screen, session, stage};
use console_test_stages::picture::{Picture, where_};

struct Asked {
    command: String,
    file: Option<PathBuf>,
    seconds: Option<f64>,
    settle: Option<f64>,
    until: Option<PathBuf>,
    open: Vec<String>,
    sample: Vec<String>,
    window: bool,
    bare: bool,
}

fn asked(words: Vec<String>) -> Result<Asked, Never> {
    let every = |what: &str| {
        words
            .iter()
            .enumerate()
            .filter(|(_, word)| *word == what)
            .filter_map(|(at, _)| words.get(at.saturating_add(1)).cloned())
            .collect::<Vec<String>>()
    };
    let bare: Vec<&String> = words
        .iter()
        .enumerate()
        .filter(|(_, word)| !word.starts_with("--"))
        .filter(|(at, _)| {
            *at == 0
                || words
                    .get(at.saturating_sub(1))
                    .is_none_or(|before| !before.starts_with("--"))
        })
        .map(|(_, word)| word)
        .collect();

    Ok(Asked {
        command: bare
            .first()
            .map_or_else(|| "run".to_string(), |word| (*word).clone()),
        file: bare.get(1).map(PathBuf::from),
        seconds: every("--seconds").first().and_then(|said| match said.parse() {
            Ok(seconds) => Some(seconds),
            Err(fault) => {
                eprintln!("console-desktop: --seconds {said}: {fault}");

                None
            }
        }),
        settle: every("--settle").first().and_then(|said| match said.parse() {
            Ok(seconds) => Some(seconds),
            Err(fault) => {
                eprintln!("console-desktop: --settle {said}: {fault}");

                None
            }
        }),
        until: every("--until").first().map(PathBuf::from),
        open: every("--open"),
        sample: every("--sample"),
        window: words.iter().any(|word| word == "--window"),
        bare: words.iter().any(|word| word == "--bare"),
    })
}

fn main() -> ExitCode {
    let Ok(asked) = asked(std::env::args().skip(1).collect());

    let done = match asked.command.as_str() {
        "clean" => clean(),
        "stage" => staged(Told::Aloud, Screen::InAWindow, Wallpaper::Started).map(|_| 0),
        "verify" => verify(),
        "probe" => run(&asked, None, Doing::Probing),
        "shot" => match asked.file.clone() {
            Some(file) => run(&asked, Some(file), Doing::Running),
            None => Err("a picture wants somewhere to be written".to_string()),
        },
        _ => run(&asked, None, Doing::Running),
    };

    match asked.command == "stage" {
        true => {},
        false => {
            let Ok(here) = stage();
            let _ = std::fs::remove_dir_all(here);
        }
    }

    match done {
        Ok(code) => ExitCode::from(code),
        Err(why) => {
            eprintln!("{why}");
            ExitCode::from(1)
        }
    }
}

fn out_of_the_way() -> Result<(), Never> {
    match std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        true => return Ok(()),
        false => {},
    }

    let Ok(mut asking) = Program::Hyprctl.command();

    let _ = asking
        .arg("eval")
        .arg(
            r#"hl.window_rule({ name = "the nested desktop stays out of the way", match = { class = "aquamarine" }, workspace = "special:console-desktop silent" })"#,
        )
        .output();

    Ok(())
}

fn clean() -> Result<u8, String> {
    let Ok(here) = stage();
    let Ok(abandoned) = session::abandoned();
    let Ok(dead) = session::dead_instances();

    let every = [here].into_iter().chain(abandoned).chain(dead);

    for path in every {
        let _ = std::fs::remove_dir_all(path);
    }

    Ok(0)
}

fn verify() -> Result<u8, String> {
    let nested = staged(Told::Quietly, Screen::InAWindow, Wallpaper::Started)?;
    let Ok(mut asking) = Program::Hyprland.command();

    asking.args(["--verify-config", "-c"]).arg(&nested);

    let Ok(where_) = environment();

    for (name, value) in where_ {
        asking.env(name, value);
    }

    let done = asking.output().map_err(|fault| fault.to_string())?;
    let said = String::from_utf8_lossy(&done.stdout).trim().to_string();
    let complained = String::from_utf8_lossy(&done.stderr).trim().to_string();
    println!("{}", match said.is_empty() {
        true => complained,
        false => said,
    });
    Ok(u8::from(!done.status.success()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Doing {
    Probing,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    ByThis,
    ByTheCompositor,
}

fn run(asked: &Asked, shot: Option<PathBuf>, probe: Doing) -> Result<u8, String> {
    let headless = !asked.window;
    let showing = match headless && shot.is_some() {
        true => Screen::Headless,
        false => Screen::InAWindow,
    };
    let ended = match shot.is_none() && asked.seconds.is_none() && probe == Doing::Running {
        true => Ended::ByTheCompositor,
        false => Ended::ByThis,
    };
    let wallpaper = match ended {
        Ended::ByThis => Wallpaper::Started,
        Ended::ByTheCompositor => Wallpaper::LeftOut,
    };
    let nested = staged(Told::Quietly, showing, wallpaper)?;
    let Ok(where_) = environment();
    let Ok(()) = out_of_the_way();

    let mut compositor = {
        let Ok(_held) = session::Starting::now();
        let Ok(was) = session::sockets();
        let Ok(running) = session::instances();
        let Ok(mut asking) = Program::Hyprland.command();

        asking
            .arg("-c")
            .arg(&nested)
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE");

        for (name, value) in &where_ {
            asking.env(name, value);
        }

        let started = asking.spawn().map_err(|fault| fault.to_string())?;
        let Ok(socket) = session::wait_for_socket(&was);
        let Ok(signature) = session::wait_for_instance(&running);

        (started, socket, signature)
    };

    let (Some(socket), Some(signature)) = (compositor.1.clone(), compositor.2.clone()) else {
        let _ = compositor.0.kill();
        return Err("the nested compositor never came up".to_string());
    };

    eprintln!("the desktop is on {socket}");

    match ended {
        Ended::ByTheCompositor => {
            let done = compositor.0.wait().map_err(|fault| fault.to_string())?;
            let Ok(()) = session::left_behind(&signature);

            return Ok(u8::from(!done.success()));
        }
        Ended::ByThis => {},
    }

    let Ok(inside) = Inside::new(where_, &socket, &signature);

    let go = screen()?;
    let looked_at = match headless {
        true => "HEADLESS-1",
        false => "WAYLAND-1",
    };

    match headless {
        true => {
            let Ok(()) = inside.make_the_screen(&go);
        }
        false => {},
    }

    let Ok(waited) = inside.wait_for_screen(looked_at);

    match waited {
        Waited::RanOut => {
            let Ok(()) = stop(&mut compositor.0, &signature, &inside);

            return Err("the screen never appeared".to_string());
        }
        Waited::Happened => {},
    }

    match asked.bare {
        true => {}
        false => {
            let Ok(here) = stage();
            let Ok(()) =
                inside.paint_the_background(&here.join("usr/share/backgrounds/console.webp"));
        }
    }

    let Ok(was) = inside.surfaces();

    let mut opened: Vec<(String, Child)> = asked
        .open
        .iter()
        .filter_map(|command| {
            let Ok(mut asking) = inside.command("sh");

            let started = match asking
                .args(["-c", command])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(started) => started,
                Err(fault) => {
                    eprintln!("console-desktop: {command}: {fault}");

                    return None;
                }
            };

            Some((command.clone(), started))
        })
        .collect();

    match opened.is_empty() {
        true => {},
        false => {
            let Ok(waited) = inside.wait_for_something(&was);

            match waited {
                Waited::RanOut => eprintln!("nothing that was asked for reached the screen"),
                Waited::Happened => {},
            }

            let Ok(()) = inside.show_a_window();
            let Ok(()) = say_what_died(&mut opened);
        }
    }

    match probe {
        Doing::Probing => {
            for question in ["monitors", "workspaces", "clients"] {
                println!("== {question}");
                let Ok(said) = inside.hyprctl(&[question, "-j"]);

                println!("{}", said.chars().take(1200).collect::<String>());
            }
        }
        Doing::Running => {},
    }

    match &asked.until {
        Some(at) => {
            let Ok(wrote) = session::wait_for_written(at, session::A_LINE);

            match wrote {
                session::Wrote::Something => {},
                session::Wrote::Nothing => {
                    eprintln!(
                        "console-desktop: nothing wrote a line to {} in {}s",
                        at.display(),
                        session::A_LINE.as_secs()
                    );
                }
            }
        }
        None => {},
    }

    match asked.settle {
        Some(seconds) => {
            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "`--settle N` is a person asking for N seconds; the elapsing is the whole of what was asked for, and the default below is the poll it overrides"
                )
            )]
            std::thread::sleep(Duration::from_secs_f64(seconds));
        }
        None => {
            let Ok(_still) = inside.wait_for_a_still_screen();
        },
    }

    match &shot {
        Some(file) => {
            let Ok(mut asking) = inside.command("grim");

            let taken = asking
                .args(["-o", looked_at])
                .arg(file)
                .output()
                .map_err(|fault| fault.to_string())?;

            match taken.status.success() {
                true => {
                    println!("{}", file.display());
                    let Ok(()) = say_the_colours(file, &asked.sample, &go);
                }
                false => {
                    eprintln!(
                        "no picture: {}",
                        String::from_utf8_lossy(&taken.stderr).trim()
                    );
                }
            }
        }
        None => {},
    }

    match asked.seconds {
        Some(seconds) => {
            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "`--seconds N` is how long somebody asked to be left looking at the desktop; nothing else ends this wait, because nothing else is meant to"
                )
            )]
            std::thread::sleep(Duration::from_secs_f64(seconds));
        }
        None => {},
    }

    let Ok(()) = stop(&mut compositor.0, &signature, &inside);
    let Ok(()) = nothing_left_running(opened);

    Ok(0)
}

const GOING: Duration = Duration::from_secs(3);

fn nothing_left_running(opened: Vec<(String, Child)>) -> Result<(), Never> {
    let mut going: Vec<(String, Child)> = Vec::new();

    for (command, mut process) in opened {
        match process.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let Ok(which) = fitted(process.id());

                // SAFETY: a signal to a process this started, by the pid it
                // was given when it started.
                unsafe { libc::kill(which, libc::SIGTERM) };

                going.push((command, process));
            }
        }
    }

    let Ok(patience) = Patience::asking_every(GOING, Duration::from_millis(100));
    let Ok(all_gone) = until(patience, || {
        going.retain_mut(|(_, process)| !matches!(process.try_wait(), Ok(Some(_))));

        Ok(match going.is_empty() {
            true => Seen::Yes,
            false => Seen::NotYet,
        })
    });

    match all_gone {
        Waited::Happened => return Ok(()),
        Waited::RanOut => {},
    }

    for (command, mut process) in going {
        eprintln!("console-desktop: {command} would not stop, and was killed");
        let _ = process.kill();
        let _ = process.wait();
    }

    Ok(())
}

fn stop(compositor: &mut Child, signature: &str, inside: &Inside) -> Result<(), Never> {
    let Ok(()) = inside.stop_the_wallpaper();
    let Ok(()) = inside.stop_the_bar();
    let Ok(which) = fitted(compositor.id());

    // SAFETY: a signal to the compositor this started, by its own pid.
    unsafe { libc::kill(which, libc::SIGTERM) };

    let Ok(patience) = Patience::asking_every(Duration::from_secs(10), Duration::from_millis(100));
    let Ok(ended) = until(patience, || {
        Ok(match compositor.try_wait().is_ok_and(|ended| ended.is_some()) {
            true => Seen::Yes,
            false => Seen::NotYet,
        })
    });

    match ended {
        Waited::Happened => {
            let Ok(()) = session::left_behind(signature);

            return Ok(());
        }
        Waited::RanOut => {},
    }

    let _ = compositor.kill();
    let _ = compositor.wait();
    let Ok(()) = session::left_behind(signature);

    Ok(())
}

fn say_what_died(opened: &mut [(String, Child)]) -> Result<(), Never> {
    for (command, process) in opened {
        let Ok(Some(ended)) = process.try_wait() else {
            continue;
        };

        let mut said = String::new();

        match process.stderr.take() {
            Some(mut voice) => {
                let _ = std::io::Read::read_to_string(&mut voice, &mut said);
            }
            None => {},
        }

        let said = said.trim().to_string();
        eprintln!(
            "{command} ended with {}: {}",
            ended.code().unwrap_or(-1),
            match said.is_empty() {
                true => "nothing said".to_string(),
                false => said,
            }
        );
    }

    Ok(())
}

fn say_the_colours(
    shot: &std::path::Path,
    sample: &[String],
    go: &console_screen::Screen,
) -> Result<(), Never> {
    match sample.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let Ok(picture) = Picture::read(shot) else {
        return Ok(());
    };

    for place in sample {
        match place == "most" {
            true => {
                let Ok(most) = picture.commonest();

                println!("  most of it   #{most}");
                continue;
            }
            false => {},
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

    Ok(())
}
