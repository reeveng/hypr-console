//! Put a picture on the screen, and keep the right one there.
//!
//!     console-wallpaper        keep the right picture up
//!     console-wallpaper --now  put the right one up and stop
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
//! The weather is asked for rather than told, so the one number here is how
//! long to wait before asking again: twenty minutes when there was an answer,
//! and a minute when there was not, widening to twenty over the same steps a
//! console service's restarts widen over. A handheld is carried out of range
//! of a network and left there, and the first minute after the wifi drops is
//! worth another try while the rest of the night is not -- a question nobody
//! can answer, asked fourteen hundred times before morning, is a radio kept
//! awake for nothing. What the picture is drawn from in the meantime is the
//! last answer, which `keeping` holds rather than throwing away.
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
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use console_events::subscription::{self, Received};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_program_contract::{Arguments, Effect, Program as _, Topic, Update, Event};
use console_program_lifetime::threads;
use console_wallpaper::choose::{self, Outside, Set, Turn, Wanted};
use console_wallpaper::keeping::{self, Chosen, Going, WallpaperEvent, WallpaperEffect, Rendered, Sky, Sun};
use console_wallpaper::{covered, place};
use console_weather::{conditions as weather, here};
use console_wallpaper::covered::Worth;
use console_weather::conditions::Weather;

const ASK_AGAIN: Duration = Duration::from_secs(1200);

const ASK_SOONER: Duration = Duration::from_secs(60);

enum Woke {
    Compositor,
    Weather(Option<Weather>),
}

fn main() -> ExitCode {
    let told: Vec<String> = std::env::args().skip(1).collect();
    let Ok(arguments) = Arguments::of(&told.iter().map(String::as_str).collect::<Vec<&str>>());

    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
enum Untabled {
    Read(std::path::PathBuf, std::io::Error),
    Unparsed(std::path::PathBuf, toml::de::Error),
}

impl std::fmt::Display for Untabled {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Untabled::Read(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            Untabled::Unparsed(at, fault) => {
                write!(to, "{} does not parse: {fault}", at.display())
            }
        }
    }
}

impl std::error::Error for Untabled {}

fn run(arguments: &Arguments) -> Result<(), Untabled> {
    let table = read_table()?;
    let mut sky = Sun::init(arguments).state;
    let Ok(mut kept) = here::CachedLocation::none();
    let mut told = weather::Notified::default();
    let mut answering = covered::Notified::default();

    let (say, woken) = channel();

    match sky.going {
        Going::Once => {
            let Ok(asked) = Wanted::asked();

            let Ok(pinned) = choose::pinned(&table.pictures, &asked);

            match pinned {
                Some(_) => {},
                None => {
                    let Ok(at) = kept.here(Instant::now());

                    let Ok(now) = weather::now(&at, &mut told);

                    sky.weather = now;
                }
            }
        }
        Going::KeepGoing => {
            let Ok(()) = subscribe(say.clone());

            let Ok(()) = ask_the_weather(say);
        }
    }

    loop {
        let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(since) => since.as_secs_f64(),
            Err(before) => {
                eprintln!(
                    "console-wallpaper: this machine says the time is before 1970 ({before}), so the \
                     sky is drawn for the epoch until the clock is set"
                );

                0.0
            }
        };
        let Ok(at) = kept.here(Instant::now());

        let Ok(here) = Outside::at(&at, seconds, sky.weather);

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

        let Ok(covered) = covered::now(&mut answering);

        let looked = WallpaperEvent::Looked { seconds, covered, chosen };
        let Update { state, effects } = Sun::update(&sky, &Event::Custom(looked));

        sky = state;

        let Ok(mut waiting) = keeping::wake(&effects);

        for effect in &effects {
            let Ok(carried) = carry(&mut sky, effect);

            match carried {
                Carried::Stopped => return Ok(()),
                Carried::Again(sooner) => waiting = Some(sooner),
                Carried::Went => {},
            }
        }

        let waiting = match waiting {
            Some(waiting) => waiting,
            None => return Ok(()),
        };

        match woken.recv_timeout(waiting) {
            Ok(Woke::Weather(said)) => {
                let Ok(()) = told_the_weather(&mut sky, said);
            }
            Ok(Woke::Compositor) => {
                for queued in woken.try_iter() {
                    match queued {
                        Woke::Weather(said) => {
                            let Ok(()) = told_the_weather(&mut sky, said);
                        }
                        Woke::Compositor => {},
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => (),
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

fn carry(sky: &mut Sky, effect: &Effect<WallpaperEffect>) -> Result<Carried, Never> {
    match effect {
        Effect::Custom(WallpaperEffect::Refresh(moving)) => {
            place::refresh(moving)?;

            Ok(Carried::Went)
        }

        Effect::Custom(WallpaperEffect::Paint(picture)) => {
            let went = paint(picture)?;

            let Update { state, effects } = Sun::update(
                sky,
                &Event::Custom(WallpaperEvent::Rendered { at: picture.clone(), went }),
            );

            *sky = state;

            let waking = keeping::wake(&effects)?;

            Ok(match waking {
                Some(sooner) => Carried::Again(sooner),
                None => Carried::Went,
            })
        }

        Effect::Stop(_) => Ok(Carried::Stopped),

        Effect::Custom(WallpaperEffect::Again(_))
        | Effect::Run(_)
        | Effect::Stream(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_) => Ok(Carried::Went),
    }
}

fn told_the_weather(sky: &mut Sky, said: Option<Weather>) -> Result<(), Never> {
    let Update { state, .. } = Sun::update(sky, &Event::Custom(WallpaperEvent::Weather(said)));

    *sky = state;

    Ok(())
}

fn ask_the_weather(say: Sender<Woke>) -> Result<(), Never> {
    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let Ok(mut kept) = here::CachedLocation::none();
        let mut told = weather::Notified::default();
        let mut sooner = ASK_SOONER;

        loop {
            let Ok(at) = kept.here(Instant::now());

            let Ok(said) = weather::now(&at, &mut told);

            let again = match said {
                Some(_) => {
                    sooner = ASK_SOONER;

                    ASK_AGAIN
                }
                None => {
                    let waiting = sooner;

                    sooner = sooner.saturating_mul(2).min(ASK_AGAIN);

                    waiting
                }
            };

            match say.send(Woke::Weather(said)) {
                Ok(()) => {},
                Err(_no_one_is_listening) => return,
            }

            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "the sky is asked rather than told: a forecast service has nothing to subscribe to, and how long a handheld waits before asking again is the whole of what this decides"
                )
            )]
            std::thread::sleep(again);
        }
    }));

    Ok(())
}

fn read_table() -> Result<Set, Untabled> {
    let Ok(at) = place::table();

    let held = std::fs::read_to_string(&at)
        .map_err(|fault| Untabled::Read(at.clone(), fault))?;

    toml::from_str(&held).map_err(|fault| Untabled::Unparsed(at, fault))
}

fn paint(picture: &Path) -> Result<Rendered, Never> {
    let Ok(mut asking) = Program::Awww.command();

    let told = asking
        .arg("img")
        .arg(picture)
        .args(["--resize", "crop", "--transition-type", "none"])
        .output();

    match told {
        Ok(done) => match done.status.success() {
            true => up(picture),
            false => {
                eprintln!(
                    "the wallpaper would not take {}: {}",
                    picture.display(),
                    String::from_utf8_lossy(&done.stderr).trim()
                );

                Ok(Rendered::No)
            }
        },
        Err(fault) => {
            eprintln!("the wallpaper daemon could not be told: {fault}");

            Ok(Rendered::No)
        }
    }
}

fn up(picture: &Path) -> Result<Rendered, Never> {
    let name = match picture.to_str() {
        Some(name) => name,
        None => return Ok(Rendered::No),
    };

    let Ok(mut asking) = Program::Awww.command();

    let said = match asking.arg("query").output() {
        Ok(said) => said,
        Err(_would_not_start) => return Ok(Rendered::No),
    };

    Ok(match String::from_utf8_lossy(&said.stdout).contains(name) {
        true => Rendered::Yes,
        false => Rendered::No,
    })
}

fn subscribe(say: Sender<Woke>) -> Result<(), Never> {
    let subscriber = subscription::connect(&[Topic::Compositor])?;

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let Ok(received) = subscriber.received();

        for event in received.iter() {
            let worth = match &event {
                Received::Connected => Worth::Waking,
                Received::Event(change) => {
                    let Ok(worth) = covered::worth_waking_for(&change.text);

                    worth
                }
            };

            match worth {
                Worth::Waking => {
                    match say.send(Woke::Compositor) {
                        Ok(()) => {},
                        Err(_no_one_is_listening) => return,
                    }
                }
                Worth::Ignoring => {},
            }
        }
    }));

    Ok(())
}
