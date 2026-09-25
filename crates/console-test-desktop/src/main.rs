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
//! console-desktop describe --open C --until AT [--press S]
//!                            C alone and headless, until it has described
//!                            itself to AT and S has been drawn; no picture
//! console-desktop probe      what the nested compositor thinks
//! console-desktop stage      the staged copy, and nothing else
//! console-desktop clean      forget what no one is using
//! ```
//!
//! The compositor is started in a control group of its own, named after this
//! process, because killing it reaches nothing it started. `session-start`
//! backgrounds a pool, a bar, a keyboard and a wallpaper; when the compositor
//! goes they are reparented to the user manager and land in the control group
//! of whoever is logged in, where nothing can tell them from the desktop
//! someone is using. An afternoon of runs left eight hundred of them on this
//! laptop, sixty `pactl subscribe` among them -- four short of the number
//! pipewire-pulse serves before it starts refusing, which is the volume keys
//! going quiet for a reason no one would find. Stopping the scope takes all of
//! it without naming one process, which matters because the names are the
//! session's own.
//!
//! `just alone` wraps a whole run the same way and this is not that. A scope
//! made inside a scope is its sibling rather than its child, so the run's scope
//! never reaches this one -- and it is this one that has to hold, because the
//! panel tier is run as often by `cargo test -p console-panel` as by `just`.
//! What a scope cannot answer is a run killed outright, which leaves its own
//! standing; `session::swept` ends the scope of every stage whose pid is gone,
//! by the same arithmetic that deletes the stage.


use console_core_external_programs::Program;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_program_lifetime::{Wrapped, in_a_scope_of_its_own, nothing_left_in};
use console_waiting::{Schedule, Ready, until_handed};
use std::path::PathBuf;
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::Duration;

use console_test_desktop::nested::Wallpaper;
use console_test_desktop::staging::{Screen, Verbosity, environment, staged};
use console_test_desktop::connection::{Inside, Instance, Outcome};
use console_test_desktop::{Unnested, screen, scope_of, session, stage};
use console_test_stages::picture::{Picture, where_};

const KILLED_BY_A_SIGNAL: i32 = -1;


#[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    allow(
        explicit048_no_unreal_state,
        reason = "what someone typed, one field per word: a window and a file and a number of seconds are asked for together or not at all, and the combinations are the command lines rather than states"
    )
)]
struct Arguments {
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

fn asked(words: Vec<String>) -> Result<Arguments, Never> {
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

    Ok(Arguments {
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
        "stage" => staged(Verbosity::Aloud, Screen::InAWindow, Wallpaper::Started).map(|_| 0),
        "verify" => verify(),
        "probe" => run(&asked, None, Action::Probing),
        "describe" => run(&asked, None, Action::Describing),
        "pressing" => pressing(&asked),
        "shot" => match asked.file.clone() {
            Some(file) => run(&asked, Some(file), Action::Running),
            None => Err(Unnested::NowhereToWrite),
        },
        _ => run(&asked, None, Action::Running),
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

fn pressing(asked: &Arguments) -> Result<u8, Unnested> {
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
        session::Wrote::Some => {},
        session::Wrote::None => eprintln!(
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
        session::Wrote::Some => 0,
        session::Wrote::None => {
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

    let Ok(_said) = console_compositor::request(
        console_compositor::Request::Eval,
        r#"hl.window_rule({ name = "the nested desktop stays out of the way", match = { class = "aquamarine" }, workspace = "special:console-desktop silent" })"#,
    );

    Ok(())
}

fn clean() -> Result<u8, Unnested> {
    let Ok(here) = stage();
    let _ = std::fs::remove_dir_all(here);
    let Ok(()) = session::swept();
    let Ok(dead) = session::dead_instances();

    for path in dead {
        let _ = std::fs::remove_dir_all(path);
    }

    Ok(0)
}

fn verify() -> Result<u8, Unnested> {
    let nested = staged(Verbosity::Quietly, Screen::InAWindow, Wallpaper::Started)?;
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
enum Action {
    Probing,
    Running,
    Describing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    ByThis,
    ByTheCompositor,
}

fn run(asked: &Arguments, shot: Option<PathBuf>, probe: Action) -> Result<u8, Unnested> {
    let headless = !asked.window;
    let showing = match (headless, &shot, probe) {
        (true, Some(_), Action::Probing | Action::Running | Action::Describing) | (true, None, Action::Describing) => Screen::Headless,
        (true, None, Action::Probing | Action::Running) | (false, Some(_) | None, Action::Probing | Action::Running | Action::Describing) => Screen::InAWindow,
    };
    let ended = match shot.is_none() && asked.seconds.is_none() && probe == Action::Running {
        true => Ended::ByTheCompositor,
        false => Ended::ByThis,
    };
    let wallpaper = match ended {
        Ended::ByThis => Wallpaper::Started,
        Ended::ByTheCompositor => Wallpaper::LeftOut,
    };
    let nested = staged(Verbosity::Quietly, showing, wallpaper)?;
    let Ok(where_) = environment();
    let Ok(()) = out_of_the_way();
    let Ok(here) = stage();
    let Ok(named) = scope_of(&here);
    let Ok(arguments) =
        Program::Hyprland.words(vec!["-c".to_string(), nested.display().to_string()]);
    let Ok((wrapped, arguments)) = in_a_scope_of_its_own(named.as_deref(), &arguments);

    match wrapped {
        Wrapped::InAScope => {},
        Wrapped::AsItWasHandedIn => eprintln!(
            "console-desktop: no user manager here, so what this session starts \
             outlives it"
        ),
    }

    let (program, rest) = match arguments.split_first() {
        Some(said) => said,
        None => return Err(Unnested::NoCompositor),
    };

    let mut compositor = {
        let Ok(_held) = session::Starting::now();
        let Ok(running) = session::instances();
        let mut asking = Command::new(program);

        asking.args(rest).env_remove("HYPRLAND_INSTANCE_SIGNATURE");

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
            let Ok(()) = nothing_left(named.as_deref());

            return Err(Unnested::NoCompositor);
        }
    };

    eprintln!("the desktop is on {socket}");

    match ended {
        Ended::ByTheCompositor => {
            let done = compositor.0.wait().map_err(Unnested::Machine)?;
            let Ok(()) = session::left_behind(&signature);
            let Ok(()) = nothing_left(named.as_deref());

            return Ok(u8::from(!done.success()));
        }
        Ended::ByThis => {},
    }

    let Ok(inside) = Inside::new(where_, Instance { socket: &socket, signature: &signature });

    let go = screen()?;
    let Ok(looked_at) = looked_at(asked);

    let Ok(made) = match headless {
        true => inside.make_the_screen(&go),
        false => Ok(Outcome::Happened),
    };

    match made {
        Outcome::Happened => {},
        Outcome::RanOut => {
            let Ok(()) = stop(&mut compositor.0, &signature, &inside, named.as_deref());

            return Err(Unnested::NoScreen);
        }
    }

    let Ok(waited) = inside.wait_for_screen(looked_at);

    match waited {
        Outcome::RanOut => {
            let Ok(()) = stop(&mut compositor.0, &signature, &inside, named.as_deref());

            return Err(Unnested::NoScreen);
        }
        Outcome::Happened => {},
    }

    match asked.bare {
        true => {}
        false => {
            let Ok(here) = stage();
            let Ok(()) =
                inside.paint_the_background(&here.join("usr/share/backgrounds/console.webp"));
        }
    }

    let Ok(opened) = opening(asked, &inside);
    let Ok(()) = looking(asked, &inside, probe);
    let Ok(monitors) = recording(asked, &inside);

    match &shot {
        Some(file) => {
            picturing(asked, &inside, file, &monitors, &go)?;
        }
        None => {},
    }

    match asked.seconds {
        Some(seconds) => {
            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "`--seconds N` is how long someone asked to be left looking at the desktop; nothing else ends this wait, because nothing else is meant to"
                )
            )]
            std::thread::sleep(Duration::from_secs_f64(seconds));
        }
        None => {},
    }

    let Ok(()) = stop(&mut compositor.0, &signature, &inside, named.as_deref());
    let Ok(()) = nothing_left_running(opened);

    Ok(0)
}

fn opening(asked: &Arguments, inside: &Inside) -> Result<Vec<(String, Child)>, Never> {
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
                Outcome::RanOut => eprintln!("nothing that was asked for reached the screen"),
                Outcome::Happened => {},
            }

            let Ok(()) = inside.show_a_window();
            let Ok(()) = say_what_died(&mut opened);
        }
    }

    Ok(opened)
}

fn looking(asked: &Arguments, inside: &Inside, probe: Action) -> Result<(), Never> {
    match probe {
        Action::Probing => {
            for question in ["monitors", "workspaces", "clients"] {
                println!("== {question}");
                let Ok(said) = inside.hyprctl(&[question, "-j"]);

                println!("{}", said.chars().take(1200).collect::<String>());
            }
        }
        Action::Running | Action::Describing => {},
    }

    match &asked.until {
        Some(at) => {
            let Ok(wrote) = session::wait_for_written(at, session::A_LINE);

            match wrote {
                session::Wrote::Some => {},
                session::Wrote::None => {
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

    match (asked.settle, probe) {
        (Some(seconds), Action::Probing | Action::Running | Action::Describing) => {
            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "`--settle N` is a person asking for N seconds; the elapsing is the whole of what was asked for, and the default below is the poll it overrides"
                )
            )]
            std::thread::sleep(Duration::from_secs_f64(seconds));
        }
        (None, Action::Describing) => {},
        (None, Action::Probing | Action::Running) => {
            let Ok(_still) = inside.wait_for_a_still_screen();
        },
    }

    Ok(())
}

fn recording(asked: &Arguments, inside: &Inside) -> Result<String, Never> {
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

    Ok(monitors)
}

fn looked_at(asked: &Arguments) -> Result<&'static str, Never> {
    Ok(match asked.window {
        true => "WAYLAND-1",
        false => "HEADLESS-1",
    })
}

fn picturing(
    asked: &Arguments,
    inside: &Inside,
    file: &std::path::Path,
    monitors: &str,
    go: &console_screen::Screen,
) -> Result<(), Unnested> {
    let Ok(looked_at) = looked_at(asked);
    let Ok(mut asking) = inside.command("grim");

    let taken = asking
        .args(["-o", looked_at])
        .arg(file)
        .output()
        .map_err(Unnested::Machine)?;

    match taken.status.success() {
        true => {
            println!("{}", file.display());
            let Ok(()) = say_the_colors(file, &asked.sample, monitors, go);
        }
        false => {
            eprintln!(
                "no picture: {}",
                String::from_utf8_lossy(&taken.stderr).trim()
            );
        }
    }

    Ok(())
}

const GOING: Duration = Duration::from_secs(3);

fn asked_to_stop(process: &Child) -> Result<(), Never> {
    let Ok(which) = fitted(process.id());

    console_program_lifetime::signal(which, rustix::process::Signal::TERM)
}

fn nothing_left_running(opened: Vec<(String, Child)>) -> Result<(), Never> {
    let mut going: Vec<(String, Child)> = Vec::new();

    for (command, mut process) in opened {
        match process.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                let Ok(()) = asked_to_stop(&process);

                going.push((command, process));
            }
            Err(_unasked) => {
                let Ok(()) = asked_to_stop(&process);

                going.push((command, process));
            }
        }
    }

    let Ok(patience) = Schedule::asking_every(GOING, Duration::from_millis(100));
    let Ok(all_gone) = until_handed(patience, &mut going, |going| {
        going.retain_mut(|(_, process)| !matches!(process.try_wait(), Ok(Some(_))));

        Ok(match going.is_empty() {
            true => Ready::Yes,
            false => Ready::NotYet,
        })
    });

    match all_gone {
        Outcome::Happened => return Ok(()),
        Outcome::RanOut => {},
    }

    for (command, mut process) in going {
        eprintln!("console-desktop: {command} would not stop, and was killed");
        let _ = process.kill();
        let _ = process.wait();
    }

    Ok(())
}

fn nothing_left(named: Option<&str>) -> Result<(), Never> {
    match named {
        Some(unit) => nothing_left_in(unit),
        None => Ok(()),
    }
}

fn stop(
    compositor: &mut Child,
    signature: &str,
    inside: &Inside,
    named: Option<&str>,
) -> Result<(), Never> {
    let Ok(()) = inside.stop_the_wallpaper();
    let Ok(()) = inside.stop_the_bar();
    let Ok(which) = fitted(compositor.id());
    let Ok(()) = console_program_lifetime::signal(which, rustix::process::Signal::TERM);

    let Ok(patience) = Schedule::asking_every(Duration::from_secs(10), Duration::from_millis(100));
    let Ok(ended) = until_handed(patience, compositor, |compositor| {
        Ok(match compositor.try_wait().is_ok_and(|ended| ended.is_some()) {
            true => Ready::Yes,
            false => Ready::NotYet,
        })
    });

    match ended {
        Outcome::Happened => {
            let Ok(()) = session::left_behind(signature);
            let Ok(()) = nothing_left(named);

            return Ok(());
        }
        Outcome::RanOut => {},
    }

    let _ = compositor.kill();
    let _ = compositor.wait();
    let Ok(()) = session::left_behind(signature);
    let Ok(()) = nothing_left(named);

    Ok(())
}

fn say_what_died(opened: &mut [(String, Child)]) -> Result<(), Never> {
    for (command, process) in opened {
        let ended = match process.try_wait() {
            Ok(Some(ended)) => ended,
            Ok(None) => continue,
            Err(_unasked) => continue,
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

    let monitors = match console_compositor::read_monitors(monitors) {
        Ok(monitors) => monitors,
        Err(_nothing_to_read) => return Ok(declared),
    };

    let asked = monitors.first().and_then(|first| {
        let Ok(logical) = first.logical();

        logical
    });

    Ok(match asked {
        Some(asked) => asked,
        None => declared,
    })
}

fn say_the_colors(
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
        Err(_unreadable) => return Ok(()),
    };

    for place in sample {
        match place == "most" {
            true => {
                let Ok(most) = picture.most_common();

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
            (Err(_not_a_number), _) | (_, Err(_not_a_number)) => continue,
        };

        match where_(&picture, Point { x: across, y: down }, logical) {
            Ok(color) => println!("  {:<12} #{color}", format!("{across},{down}")),
            Err(why) => eprintln!("  {place}: {why}"),
        }
    }

    Ok(())
}
