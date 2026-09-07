//! Put a picture on the screen, and keep the right one there.
//!
//!     console-sky        keep the right picture up
//!     console-sky --now  put the right one up and stop
//!
//! It wakes for three reasons and no others. The compositor says something has
//! covered the wallpaper or stopped covering it; the weather has an answer;
//! or enough time has gone by that the sun has moved. Between those it is
//! asleep. So is the wallpaper daemon, whenever anything is in front of the
//! picture, because what it was handed then is one frame that lasts for ever.
//!
//! `--now` is for the settings panel, which has just written down what she
//! asked for and would rather she saw it happen than waited for the next time
//! this came round.
//!
//! Nothing here decides anything. What picture answers a rainy dusk is
//! `console_wallpaper::choose`, whether the wallpaper is covered is
//! `console_wallpaper::covered`, where the sun is is `console_wallpaper::sun`,
//! and what to do about all three -- including how long to sleep before
//! looking again -- is `console_wallpaper::keeping`, which is a
//! `console_program_contract::Program`. This is the loop that asks the machine
//! and the one line that tells the wallpaper daemon.

use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use console_events::listening;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_program_contract::{Argv, Doing, Program as _, Topic, Turn as Turn_, Word};
use console_wallpaper::choose::{self, Outside, Set, Turn, Wanted};
use console_wallpaper::keeping::{self, Chosen, Going, Heard, Its, Painted, Sky, Sun};
use console_wallpaper::{covered, here, moon, place, sun, weather};
use console_wallpaper::covered::Worth;
use console_wallpaper::weather::Weather;

const ASK_AGAIN: Duration = Duration::from_secs(1200);

const ASK_SOONER: Duration = Duration::from_secs(60);

enum Woke {
    Compositor,
    Weather(Option<Weather>),
}

fn main() -> ExitCode {
    let told: Vec<String> = std::env::args().skip(1).collect();
    let Ok(argv) = Argv::of(&told.iter().map(String::as_str).collect::<Vec<&str>>());

    match run(&argv) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}

fn run(argv: &Argv) -> Result<(), String> {
    let table = read_table()?;
    let mut sky = Sun::opening(argv).state;

    let (say, woken) = channel();

    match sky.going {
        Going::Once => {
            let Ok(asked) = Wanted::asked();

            let Ok(pinned) = choose::pinned(&table.pictures, &asked);

            match pinned {
                Some(_) => {},
                None => {
                    let Ok(at) = here::here();

                    let Ok(now) = weather::now(&at);

                    sky.weather = now;
                }
            }
        }
        Going::KeepGoing => {
            let Ok(()) = listen(say.clone());

            let Ok(()) = ask_the_weather(say);
        }
    }

    loop {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let Ok(at) = here::here();

        let Ok(moon) = moon::moon(seconds);

        let Ok(season) = sun::season(&at, seconds);

        let Ok(band) = sun::sky(&at, seconds);

        let here = Outside { moon, season, sky: band, weather: sky.weather };

        let Ok(asked) = Wanted::asked();

        let Ok(turn) = Turn::at(seconds);

        let Ok(wanted) = choose::wanted(&table.pictures, &asked, &here, turn);

        let chosen = match wanted {
            Some(picture) => {
                let Ok(found) = place::picture(&picture.name);

                found.map(|(moving, still)| Chosen {
                    still: still.is_file().then_some(still),
                    moving,
                })
            }
            None => None,
        };

        let Ok(covered) = covered::now();

        let looked = Heard::Looked { seconds, covered, chosen };
        let Turn_ { now, doings } = Sun::heard(&sky, &Word::Its(looked));

        sky = now;

        let Ok(mut waiting) = keeping::wake(&doings);

        for doing in &doings {
            let Ok(carried) = carry(&mut sky, doing);

            match carried {
                Carried::Stopped => return Ok(()),
                Carried::Again(sooner) => waiting = Some(sooner),
                Carried::Went => {},
            }
        }

        let Some(waiting) = waiting else { return Ok(()) };

        match woken.recv_timeout(waiting) {
            Ok(Woke::Weather(said)) => {
                let Ok(()) = told_the_weather(&mut sky, said);
            }
            Ok(Woke::Compositor) | Err(RecvTimeoutError::Timeout) => (),
            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "nothing is left to wake this daemon, so there is no longer a thing to wait for; it falls back to looking again rather than spinning on a dead channel"
                )
            )]
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(keeping::LOOK_AGAIN),
        }
    }
}

enum Carried {
    Went,
    Stopped,
    Again(Duration),
}

fn carry(sky: &mut Sky, doing: &Doing<Its>) -> Result<Carried, Never> {
    match doing {
        Doing::Its(Its::Freshen(moving)) => {
            place::freshen(moving)?;

            Ok(Carried::Went)
        }

        Doing::Its(Its::Paint(picture)) => {
            let went = paint(picture)?;

            let Turn_ { now, doings } = Sun::heard(
                sky,
                &Word::Its(Heard::Painted { at: picture.clone(), went }),
            );

            *sky = now;

            let waking = keeping::wake(&doings)?;

            Ok(match waking {
                Some(sooner) => Carried::Again(sooner),
                None => Carried::Went,
            })
        }

        Doing::Stop(_) => Ok(Carried::Stopped),

        Doing::Its(Its::Again(_))
        | Doing::Ask(_)
        | Doing::Watch(_)
        | Doing::AskWhoever(_)
        | Doing::Start(_)
        | Doing::Listen(_)
        | Doing::Deafen(_)
        | Doing::Write(_)
        | Doing::Say(_)
        | Doing::Print(_) => Ok(Carried::Went),
    }
}

fn told_the_weather(sky: &mut Sky, said: Option<Weather>) -> Result<(), Never> {
    let Turn_ { now, .. } = Sun::heard(sky, &Word::Its(Heard::Weather(said)));

    *sky = now;

    Ok(())
}

fn ask_the_weather(say: Sender<Woke>) -> Result<(), Never> {
    std::thread::spawn(move || {
        loop {
            let Ok(at) = here::here();

            let Ok(said) = weather::now(&at);

            let again = match said.is_some() {
                true => ASK_AGAIN,
                false => ASK_SOONER,
            };

            let Ok(()) = say.send(Woke::Weather(said)) else { return };

            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "the sky is asked rather than told: a forecast service has nothing to subscribe to, and how long a handheld waits before asking again is the whole of what this decides"
                )
            )]
            std::thread::sleep(again);
        }
    });

    Ok(())
}

fn read_table() -> Result<Set, String> {
    let Ok(at) = place::table();

    let held = std::fs::read_to_string(&at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;
    toml::from_str(&held).map_err(|fault| format!("{} does not parse: {fault}", at.display()))
}

fn paint(picture: &Path) -> Result<Painted, Never> {
    let Ok(mut asking) = Program::Awww.command();

    let told = asking
        .arg("img")
        .arg(picture)
        .args(["--resize", "crop", "--transition-type", "none"])
        .output();

    match told {
        Ok(done) if done.status.success() => up(picture),
        Ok(done) => {
            eprintln!(
                "the wallpaper would not take {}: {}",
                picture.display(),
                String::from_utf8_lossy(&done.stderr).trim()
            );

            Ok(Painted::No)
        }
        Err(fault) => {
            eprintln!("the wallpaper daemon could not be told: {fault}");

            Ok(Painted::No)
        }
    }
}

fn up(picture: &Path) -> Result<Painted, Never> {
    let Some(name) = picture.to_str() else {
        return Ok(Painted::No);
    };

    let Ok(mut asking) = Program::Awww.command();

    let Ok(said) = asking.arg("query").output() else {
        return Ok(Painted::No);
    };

    Ok(match String::from_utf8_lossy(&said.stdout).contains(name) {
        true => Painted::Yes,
        false => Painted::No,
    })
}

fn listen(say: Sender<Woke>) -> Result<(), Never> {
    let listening = listening::listen(&[Topic::Compositor])?;

    let _ = std::thread::spawn(move || {
        let Ok(heard) = listening.heard();

        for heard in heard.iter() {
            let worth = match &heard {
                listening::Heard::GotIn => Worth::Waking,
                listening::Heard::Said(changed) => {
                    let Ok(worth) = covered::worth_waking_for(&changed.said);

                    worth
                }
            };

            match worth {
                Worth::Waking => {
                    let Ok(()) = say.send(Woke::Compositor) else { return };
                }
                Worth::Ignoring => {},
            }
        }
    });

    Ok(())
}
