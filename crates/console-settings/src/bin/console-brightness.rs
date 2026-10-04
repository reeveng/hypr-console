//! Screen brightness, in steps, within the range this panel can actually show.
//!
//!     console-brightness up | down | get
//!     console-brightness dim | undim
//!     console-brightness follow | switched-on | follow-or-not
//!
//! `undim` also puts the panel back on, which is the machine's only way out of
//! a screen that has gone dark and stayed dark. See `panel_on`.
//!
//! `dim` and `undim` are the pair the idle daemon runs, and they are here
//! rather than in its configuration because putting a screen back where it was
//! means having remembered where that was. A `brightnessctl -s` in a config
//! file would remember it in a place nothing else on this machine can read,
//! and would restore over the top of someone who reached for the rocker while
//! it was dim.
//!
//! `get` is the same range read the other way round, in points of a hundred, so
//! the settings panel can draw a bar of it. Nothing else may work out what full
//! is: a second opinion about this screen is two numbers that part company the
//! day either of them moves. `console_settings::screen` is that one opinion,
//! and the panel reads it there rather than running this.
//!
//! `up` and `down` say where they got to, the way the volume rocker does. A
//! press under L2 happens with a game in front of it and the settings panel
//! shut, so without a notification the only report of it is the screen itself --
//! which is the one thing someone adjusting the screen cannot judge, because
//! it is what their eyes have just adapted to. `dim` and `undim` say nothing:
//! no one pressed them, and a machine that woke you to tell you it had dimmed
//! itself would be worse than one that did it quietly.
//!
//! `up` and `down` also teach. A press is a person saying how bright they want
//! the screen in the light they are sitting in, so the reading and the level
//! are kept as a sample and `follow` wears what was learned when the light
//! changes. `console_settings::learned` argues for the whole of that; what is
//! here is the two ends of it, because this is the program a press already
//! runs and the one that already knows what a level means on this panel.
//!
//! `switched-on` is the unit's own question, and it answers no on a machine with
//! no light sensor in it. Most machines have none, and a desktop that started
//! a daemon to read one anyway would be a unit failing in the journal of every
//! device that is not this one.

use std::path::Path;
use std::time::Duration;

use console_core_arguments::{Operands, Subcommand, read_with};
use console_core_external_programs::Program;
use console_core_iteration::Step;
use console_core_words::Words;
use console_core_never::Never;
use console_notifications::saying::{StatePath, Notification, Content, raise_kept};
use console_settings::learned::{self, Band, Following, Standing};
use console_settings::light::{self, Lit};
use console_settings::screen::{self, Moved, Panel, Was, Way, remembered};
use console_waiting::Schedule;

const UNIT: &str = "console-light.service";

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
    name: "console-brightness",
    about: "screen brightness, in steps, within the range this panel can actually show",
    flags: &[],
    operands: Operands::None,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Brightness {
    #[words(word = "up", about = "one step brighter, and learn it for the light this is")]
    Up,
    #[words(word = "down", about = "one step dimmer, and learn it for the light this is")]
    Down,
    #[words(word = "get", about = "how bright it is, in points of a hundred")]
    Get,
    #[words(word = "dim", about = "go dim, remembering where it was")]
    Dim,
    #[words(word = "undim", about = "go back to the level dim remembers, and put the panel back on")]
    Undim,
    #[words(word = "follow", about = "wear what it learns for the light there is now")]
    Follow,
    #[words(word = "switched-on", about = "nothing said; the exit code is whether to follow the light at all")]
    SwitchedOn,
    #[words(word = "follow-or-not", about = "switch following the light on or off")]
    FollowOrNot,
}

impl Subcommand for Brightness {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(Brightness::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        Brightness::about(self)
    }
}

const ROUND: Duration = Duration::from_secs(60);

const ASKING: Duration = Duration::from_secs(4);

fn main() -> std::process::ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let read = read_with::<Brightness, String>(&COMMAND, &words).and_then(|line| line.require_subcommand());

    let asked = match read {
        Ok(asked) => asked,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return std::process::ExitCode::from(code);
        }
    };

    let Ok(found) = screen::here();

    let panel = match found {
        Some(panel) => panel,
        None => {
            eprintln!("console-brightness: no backlight under {}", screen::UNDER);

            return std::process::ExitCode::FAILURE;
        }
    };

    let Ok(reading) = panel.now();

    let now = match reading {
        Some(now) => now,
        None => {
            eprintln!("console-brightness: {} would not say how bright it is", panel.at.display());

            return std::process::ExitCode::FAILURE;
        }
    };

    let way = match asked {
        Brightness::Up => Way::Up,
        Brightness::Down => Way::Down,
        Brightness::Get => {
            let Ok(points) = panel.as_points(now);

            println!("{points}");

            return std::process::ExitCode::SUCCESS;
        }
        Brightness::Dim => {
            let Ok(done) = dim(&panel, now);

            return done;
        }
        Brightness::Undim => {
            let Ok(done) = undim(&panel, now);

            return done;
        }
        Brightness::Follow => {
            let Ok(done) = follow(&panel);

            return done;
        }
        Brightness::SwitchedOn => {
            let Ok(done) = switched_on();

            return done;
        }
        Brightness::FollowOrNot => {
            let Ok(done) = follow_or_not();

            return done;
        }
    };

    let Ok(going) = panel.step_from(now, way);

    let Ok(moved) = panel.set(going);

    match moved {
        Moved::No => {
            eprintln!("console-brightness: the screen would not take it");
            return std::process::ExitCode::FAILURE;
        }
        Moved::Yes => {},
    }

    let Ok(()) = notify(&panel, going);
    let Ok(()) = learn(&panel, going);

    std::process::ExitCode::SUCCESS
}

fn lit() -> Result<Option<Lit>, Never> {
    let Ok(found) = light::here();

    match found {
        Some(sensor) => sensor.now(),
        None => Ok(None),
    }
}

fn levels(home: &Path) -> Result<Option<learned::Levels>, Never> {
    let Ok(standing) = learned::load(home);

    Ok(match standing {
        Standing::Loaded(levels) => Some(levels),
        Standing::Invalid(why) => {
            eprintln!("console-brightness: what this machine had learned would not read: {why}");

            None
        }
    })
}

fn learn(panel: &Panel, going: i64) -> Result<(), Never> {
    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => return Ok(()),
    };

    let Ok(now) = lit();

    let now = match now {
        Some(now) => now,
        None => return Ok(()),
    };

    let Ok(found) = levels(&home);

    let held = match found {
        Some(held) => held,
        None => return Ok(()),
    };

    let Ok(band) = learned::band(now);
    let Ok(parts) = panel.as_parts(going);
    let Ok(taught) = held.taught(band, parts);

    match learned::keep(&home, &taught) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-brightness: {fault}"),
    }

    Ok(())
}

fn switched_on() -> Result<std::process::ExitCode, Never> {
    let Ok(found) = light::here();

    match found {
        Some(_a_machine_with_eyes) => {},
        None => return Ok(std::process::ExitCode::FAILURE),
    }

    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => return Ok(std::process::ExitCode::FAILURE),
    };

    let Ok(asked) = learned::following(&home);

    Ok(match asked {
        Following::Yes => std::process::ExitCode::SUCCESS,
        Following::No => std::process::ExitCode::FAILURE,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Dimmed {
    Yes,
    No,
}

fn follow_or_not() -> Result<std::process::ExitCode, Never> {
    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => {
            eprintln!("console-brightness: no HOME, so there is no one to remember for");

            return Ok(std::process::ExitCode::FAILURE);
        }
    };

    let Ok(at) = learned::asked_at(&home);
    let Ok(standing) = learned::following(&home);
    let Ok(other) = standing.other();
    let Ok(written) = other.as_str();

    match at.parent().map(std::fs::create_dir_all) {
        Some(Err(fault)) => {
            eprintln!("console-brightness: {}: {fault}, so nothing was changed", at.display());

            return Ok(std::process::ExitCode::FAILURE);
        }
        Some(Ok(())) | None => {},
    }

    match console_core_atomic_writes::whole(&at, format!("{written}\n").as_bytes()) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-brightness: {fault}, so nothing was changed");

            return Ok(std::process::ExitCode::FAILURE);
        }
    }

    let Ok(mut asking) = Program::Systemctl.command();
    let done = asking.args(["--user", "restart", UNIT]).status();

    Ok(match done.is_ok_and(|how| how.success()) {
        true => std::process::ExitCode::SUCCESS,
        false => {
            eprintln!(
                "console-brightness: {UNIT} would not restart, so the screen is still following \
                 what it was. It is written down, and the next start of the desktop will wear it."
            );

            std::process::ExitCode::FAILURE
        }
    })
}

fn dimmed() -> Result<Dimmed, Never> {
    let Ok(remembered) = remembered();

    let kept = match remembered {
        Some(kept) => kept,
        None => return Ok(Dimmed::No),
    };

    Ok(match kept.exists() {
        true => Dimmed::Yes,
        false => Dimmed::No,
    })
}

fn moved_to(sensor: &light::Sensor, acted: Option<Band>) -> Result<Option<Band>, Never> {
    let Ok(dimmed) = dimmed();

    match dimmed {
        Dimmed::Yes => return Ok(None),
        Dimmed::No => {},
    }

    let Ok(now) = sensor.now();

    let now = match now {
        Some(now) => now,
        None => return Ok(None),
    };

    let Ok(band) = learned::band(now);

    Ok(match acted == Some(band) {
        true => None,
        false => Some(band),
    })
}

fn wear(panel: &Panel, home: &Path, band: Band) -> Result<(), Never> {
    let Ok(found) = levels(home);

    let held = match found {
        Some(held) => held,
        None => return Ok(()),
    };

    let Ok(wanting) = held.level_for(band);

    let asked = match wanting {
        Some(asked) => asked,
        None => return Ok(()),
    };

    let Ok(want) = panel.part(asked);
    let Ok(ceiling) = panel.ceiling();
    let Ok(floor) = panel.floor();
    let Ok(moved) = panel.set(want.clamp(floor, ceiling));

    match moved {
        Moved::Yes => {},
        Moved::No => eprintln!("console-brightness: the screen would not take it"),
    }

    Ok(())
}

fn follow(panel: &Panel) -> Result<std::process::ExitCode, Never> {
    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => {
            eprintln!("console-brightness: no home, so nothing was ever learned");

            return Ok(std::process::ExitCode::FAILURE);
        }
    };

    let Ok(found) = light::here();

    let sensor = match found {
        Some(sensor) => sensor,
        None => {
            eprintln!("console-brightness: no light sensor under {}", light::UNDER);

            return Ok(std::process::ExitCode::FAILURE);
        }
    };

    let Ok(patience) = Schedule::asking_every(ROUND, ASKING);

    let watched = console_core_iteration::iterate(None, |acted: Option<Band>| {
        let Ok(found) = console_waiting::until_some(patience, || moved_to(&sensor, acted));

        Ok(Step::<_, std::process::ExitCode>::Again(match found {
            Some(band) => {
                let Ok(()) = wear(panel, &home, band);

                Some(band)
            }
            None => acted,
        }))
    });

    Ok(match watched {
        Ok(code) => code,
        Err(_endless) => std::process::ExitCode::FAILURE,
    })
}

fn notify(panel: &Panel, going: i64) -> Result<(), Never> {
    let points = panel.as_points(going)?;
    let words = screen::brightness_label(points)?;
    let notification = Notification::new(Content { summary: &words, body: "" })?;
    let notification = notification.lasting(1500)?;
    let notification = notification.valued(points)?;
    let kept = StatePath::new("brightness")?;
    let Ok(()) = raise_kept(notification, &kept);

    Ok(())
}

fn dim(panel: &Panel, now: i64) -> Result<std::process::ExitCode, Never> {
    let remembered = remembered()?;

    let kept = match remembered {
        Some(kept) => kept,
        None => {
            eprintln!("console-brightness: no XDG_RUNTIME_DIR, so nothing could be remembered");

            return Ok(std::process::ExitCode::FAILURE);
        }
    };

    match kept.exists() {
        true => return Ok(std::process::ExitCode::SUCCESS),
        false => {},
    }

    match console_core_atomic_writes::whole(&kept, format!("{now}\n").as_bytes()) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-brightness: could not write {}: {fault}", kept.display());

            return Ok(std::process::ExitCode::FAILURE);
        }
    }

    let dimmed = panel.dimmed()?;
    let moved = panel.set(dimmed)?;

    Ok(match moved {
        Moved::Yes => std::process::ExitCode::SUCCESS,
        Moved::No => std::process::ExitCode::FAILURE,
    })
}

fn kept_at(kept: &std::path::Path) -> Result<Option<i64>, Never> {
    console_core_atomic_writes::number(kept)
}

fn panel_on() -> Result<(), Never> {
    let done = console_compositor::request(
        console_compositor::Request::Dispatch,
        r#"hl.dsp.dpms({ action = "enable" })"#,
    )?;

    match done {
        console_compositor::DispatchResult::Success => {},
        console_compositor::DispatchResult::Failure(why) => {
            eprintln!("console-brightness: the panel would not come on: {why}");
        }
    }

    Ok(())
}

fn undim(panel: &Panel, now: i64) -> Result<std::process::ExitCode, Never> {
    panel_on()?;

    let remembered = remembered()?;

    let kept = match remembered {
        Some(kept) => kept,
        None => return Ok(std::process::ExitCode::SUCCESS),
    };

    let was = kept_at(&kept)?;
    let _ = std::fs::remove_file(&kept);

    let back = match was {
        Some(was) => panel.undimming(now, Was(was))?,
        None => None,
    };

    match back {
        Some(back) => {
            let moved = panel.set(back)?;

            Ok(match moved {
                Moved::Yes => std::process::ExitCode::SUCCESS,
                Moved::No => std::process::ExitCode::FAILURE,
            })
        },
        None => Ok(std::process::ExitCode::SUCCESS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_arguments::{Reason, ValidationError};

    #[test]
    fn each_thing_it_does_is_its_own_word_and_no_other_spelling_of_it() -> Result<(), ValidationError> {
        let up = read_with::<Brightness, &str>(&COMMAND, &["up"])?;
        let asking = read_with::<Brightness, &str>(&COMMAND, &["follow-or-not"])?;
        let shouted = read_with::<Brightness, &str>(&COMMAND, &["Up"]);

        assert_eq!(up.require_subcommand(), Ok(Brightness::Up));
        assert_eq!(asking.require_subcommand(), Ok(Brightness::FollowOrNot));
        assert_eq!(shouted.map_err(|refusal| refusal.reason), Err(Reason::NoSuchSubcommand("Up".to_string())));

        Ok(())
    }

    #[test]
    fn it_refuses_a_word_after_the_one_it_does_rather_than_ignoring_it() {
        let line = read_with::<Brightness, &str>(&COMMAND, &["up", "10"]);

        assert_eq!(line.map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("10".to_string())));
    }
}
