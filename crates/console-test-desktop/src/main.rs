//! The Legion Go's desktop, running on this machine, in a window.
//!
//! ```text
//! console-desktop verify     does the compositor config still parse
//! console-desktop run        the desktop, nested, at the device's size
//! console-desktop shot FILE  a picture of it, once the screen has stopped
//!                            changing, or --settle N seconds after
//! console-desktop shot FILE --bare   the same, without a ground to paint
//! console-desktop shot FILE --until AT   and not before AT has a line in it
//! console-desktop shot FILE --press S   run S inside, and do not go on until
//!                            it has finished
//! console-desktop pressing AT --then C  once AT has been drawn to, press C,
//!                            and not return until it has been drawn to again
//! console-desktop shot FILE --clients AT what windows it had, written to AT
//! console-desktop shot FILE --monitors AT what screen it was, written to AT
//! console-desktop probe      what the nested compositor thinks
//! console-desktop stage      the staged copy, and nothing else
//! console-desktop clean      forget what nobody is using
//! ```


use console_core_external_programs::Program;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_waiting::{Patience, Seen, until};
use std::path::PathBuf;
use std::process::{Child, ExitCode, Stdio};
use std::time::Duration;

use console_test_desktop::nested::Wallpaper;
use console_test_desktop::staging::{Screen, Told, environment, staged};
use console_test_desktop::talking::{Inside, Instance, Waited};
use console_test_desktop::{Unnested, screen, session, stage};
use console_test_stages::picture::{Picture, where_};

const KILLED_BY_A_SIGNAL: i32 = -1;


struct Asked {
    command: String,
    file: Option<PathBuf>,
    seconds: Option<f64>,
    settle: Option<f64>,
    until: Option<PathBuf>,
    clients: Option<PathBuf>,
    monitors: Option<PathBuf>,
    open: Vec<String>,
    press: Option<String>,
    then: Option<String>,
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
        clients: every("--clients").first().map(PathBuf::from),
        monitors: every("--monitors").first().map(PathBuf::from),
        open: every("--open"),
        press: every("--press").first().cloned(),
        then: every("--then").first().cloned(),
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
        "pressing" => pressing(&asked),
        "shot" => match asked.file.clone() {
            Some(file) => run(&asked, Some(file), Doing::Running),
            None => Err(Unnested::NowhereToWrite),
        },
        _ => run(&asked, None, Doing::Running),
    };

    match asked.command.as_str() {
        "stage" | "pressing" => {},
        _ => {
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

fn pressing(asked: &Asked) -> Result<u8, Unnested> {
    let at = match &asked.file {
        Some(at) => at,
        None => return Err(Unnested::NothingToPress),
    };

    let command = match &asked.then {
        Some(command) => command,
        None => return Err(Unnested::NothingToPress),
    };

    let Ok(drew) = session::wait_for_written(at, session::A_LINE);

    match drew {
        session::Wrote::Something => {},
        session::Wrote::Nothing => eprintln!(
            "console-desktop: nothing had drawn to {} in {}s, and {command} is pressed anyway",
            at.display(),
            session::A_LINE.as_secs()
        ),
    }

    let Ok(before) = session::lines(at);

    let Ok(mut asking) = Program::Sh.command();

    let pressed = asking
        .args(["-c", command])
        .status()
        .map_err(Unnested::Machine)?;

    match pressed.success() {
        true => {},
        false => eprintln!("console-desktop: {command}: {pressed}"),
    }

    let Ok(again) = session::wait_for_more_than(at, before, session::A_LINE);

    Ok(match again {
        session::Wrote::Something => 0,
        session::Wrote::Nothing => {
            eprintln!(
                "console-desktop: {command} drew nothing new to {} in {}s",
                at.display(),
                session::A_LINE.as_secs()
            );

            1
        }
    })
}

fn out_of_the_way() -> Result<(), Never> {
    let Ok(instance) = console_compositor::instance();

    match instance {
        None => return Ok(()),
        Some(_a_compositor_to_stand_out_of_the_way_of) => {},
    }

    let Ok(_said) = console_compositor::told(
        console_compositor::Told::Eval,
        r#"hl.window_rule({ name = "the nested desktop stays out of the way", match = { class = "aquamarine" }, workspace = "special:console-desktop silent" })"#,
    );

    Ok(())
}

fn clean() -> Result<u8, Unnested> {
    let Ok(here) = stage();
    let Ok(abandoned) = session::abandoned();
    let Ok(dead) = session::dead_instances();

    let every = [here].into_iter().chain(abandoned).chain(dead);

    for path in every {
        let _ = std::fs::remove_dir_all(path);
    }

    Ok(0)
}

fn verify() -> Result<u8, Unnested> {
    let nested = staged(Told::Quietly, Screen::InAWindow, Wallpaper::Started)?;
    let Ok(mut asking) = Program::Hyprland.command();

    asking.args(["--verify-config", "-c"]).arg(&nested);

    let Ok(where_) = environment();

    for (name, value) in where_ {
        asking.env(name, value);
    }

    let done = asking.output().map_err(Unnested::Machine)?;
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

fn run(asked: &Asked, shot: Option<PathBuf>, probe: Doing) -> Result<u8, Unnested> {
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
        let Ok(running) = session::instances();
        let Ok(mut asking) = Program::Hyprland.command();

        asking
            .arg("-c")
            .arg(&nested)
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE");

        for (name, value) in &where_ {
            asking.env(name, value);
        }

        let started = asking.spawn().map_err(Unnested::Machine)?;
        let Ok(came) = session::wait_for_one(&running);

        (started, came)
    };

    let (socket, signature) = match compositor.1.clone() {
        Some(came) => (came.display, came.signature),
        None => {
            let _ = compositor.0.kill();
            return Err(Unnested::NoCompositor);
        }
    };

    eprintln!("the desktop is on {socket}");

    match ended {
        Ended::ByTheCompositor => {
            let done = compositor.0.wait().map_err(Unnested::Machine)?;
            let Ok(()) = session::left_behind(&signature);

            return Ok(u8::from(!done.success()));
        }
        Ended::ByThis => {},
    }

    let Ok(inside) = Inside::new(where_, Instance { socket: &socket, signature: &signature });

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

            return Err(Unnested::NoScreen);
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

    #[cfg_attr(
        dylint_lib = "explicit029_no_asking_per_item",
        allow(
            explicit029_no_asking_per_item,
            reason = "each is a different program the check asked to be opened, so there was never one process that could have opened all of them"
        )
    )]
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

    match &asked.press {
        Some(script) => {
            let Ok(mut asking) = inside.command("sh");

            match asking.args(["-c", script]).status() {
                Ok(done) => match done.success() {
                    true => {},
                    false => eprintln!("console-desktop: the presses ended {done}"),
                },
                Err(fault) => eprintln!("console-desktop: {script}: {fault}"),
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

    match &asked.clients {
        Some(at) => {
            let Ok(said) = inside.hyprctl(&["clients", "-j"]);

            match console_core_atomic_writes::whole(at, said.as_bytes()) {
                Ok(()) => {},
                Err(fault) => eprintln!("console-desktop: {}: {fault}", at.display()),
            }
        }
        None => {},
    }

    let Ok(monitors) = inside.hyprctl(&["monitors", "-j"]);

    match &asked.monitors {
        Some(at) => {
            match console_core_atomic_writes::whole(at, monitors.as_bytes()) {
                Ok(()) => {},
                Err(fault) => eprintln!("console-desktop: {}: {fault}", at.display()),
            }
        }
        None => {},
    }

    match &shot {
        Some(file) => {
            let Ok(mut asking) = inside.command("grim");

            let taken = asking
                .args(["-o", looked_at])
                .arg(file)
                .output()
                .map_err(Unnested::Machine)?;

            match taken.status.success() {
                true => {
                    println!("{}", file.display());
                    let Ok(()) = say_the_colours(file, &asked.sample, &monitors, &go);
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
        let ended = match process.try_wait() {
            Ok(Some(ended)) => ended,
            Ok(None) | Err(_) => continue,
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
            match ended.code() {
                Some(code) => code,
                None => KILLED_BY_A_SIGNAL,
            },
            match said.is_empty() {
                true => "nothing said".to_string(),
                false => said,
            }
        );
    }

    Ok(())
}

fn live(monitors: &str, go: &console_screen::Screen) -> Result<Size<u32>, Never> {
    let Ok(declared) = go.logical();

    let said = match console_compositor::read(monitors) {
        Ok(said) => said,
        Err(_nothing_to_read) => return Ok(declared),
    };

    let Ok(monitors) = console_compositor::monitors(&said);

    let asked = monitors.first().and_then(|first| {
        let Ok(logical) = first.logical();

        logical
    });

    Ok(match asked {
        Some(asked) => asked,
        None => declared,
    })
}

fn say_the_colours(
    shot: &std::path::Path,
    sample: &[String],
    monitors: &str,
    go: &console_screen::Screen,
) -> Result<(), Never> {
    match sample.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let Ok(logical) = live(monitors, go);

    let picture = match Picture::read(shot) {
        Ok(picture) => picture,
        Err(_fault) => return Ok(()),
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

        let (across, down) = match place.split_once(',') {
            Some((across, down)) => (across, down),
            None => continue,
        };

        let (across, down) = match (across.trim().parse::<f64>(), down.trim().parse::<f64>()) {
            (Ok(across), Ok(down)) => (across, down),
            (Err(_), _) | (_, Err(_)) => continue,
        };

        match where_(&picture, Point { across, down }, logical) {
            Ok(colour) => println!("  {:<12} #{colour}", format!("{across},{down}")),
            Err(why) => eprintln!("  {place}: {why}"),
        }
    }

    Ok(())
}
