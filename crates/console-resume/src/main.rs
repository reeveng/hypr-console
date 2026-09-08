//! The words this program is started with, and what each of them does.
//!
//! `console-session.service` starts it with no words at all, and no words is the
//! whole of what this desktop asks of it: put back what was open, once for this
//! compositor, and then keep saving what is on the screen. The other modes are
//! here because somebody with a broken session needs a way to look at one and to
//! throw one away, and because `save` once is what a check presses.
//!
//! ## No words does not mean `load`
//!
//! It did in the fork, and `load` closes every window before it starts anything.
//! Under `Restart=always` that made a crash indistinguishable from a log-in: the
//! desktop was swept and rebuilt five seconds later, in front of whoever was
//! using it. It is also what an accidental `cargo run` did to this machine.
//!
//! So no words is [`Mode::Default`], which puts back only if
//! `console_resume::already` says this compositor has not had it done yet, and
//! watches either way. `load` is the word for doing it now regardless, and it is
//! a word somebody has to type.
//!
//! ## Nothing saved is not a fault, and asking for a session that is not there is
//!
//! `load` typed by a person names a session, and a name nobody saved is worth a
//! word and a status: they asked for something that is not there. The desktop's
//! own first start asks for the same thing and means something else -- a machine
//! that has never saved a session has nothing to put back, which is what a first
//! start *is*. Told those apart by the mode rather than by the file, because
//! the file says the same thing to both.
//!
//! It was one sentence and both, which made the ordinary first start of a fresh
//! device exit 1 without ever reaching the watching -- and `console-session`
//! carries `ExecStopPost=console-fell`, so it said so on the screen as a program
//! that had fallen over. `Restart=always` then started it again, the mark from
//! the first attempt made the second one a no-op, and it watched. The desktop
//! worked; the notice was true about the status and wrong about the desktop.
//!
//! ## The words are read here rather than by a parser crate
//!
//! The fork took clap, which is a dependency and a help screen for a program
//! nobody types. It is also what made `--help` harmless there and a swept desktop
//! here: with clap gone, an argument nothing recognises left no mode word, and no
//! mode word was `load`. A word this does not know now says which words it knows
//! and stops, and that is the only thing an unknown word may do.

use std::env;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

use console_core_never::Never;
use console_resume::already::Already;
use console_resume::session::{Duplicates, PutBack, Really, Restoring, Sessions};

const SAVE_INTERVAL: Duration = Duration::from_secs(60);
const ADJUSTING_FOR: Duration = Duration::from_secs(60);

const WHERE: &str = "CONSOLE_RESUME_PATH";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Default,
    Save,
    Watch,
    List,
    Load,
    Clear,
    Delete,
}

const EVERY: [(&str, Mode); 7] = [
    ("default", Mode::Default),
    ("save", Mode::Save),
    ("watch", Mode::Watch),
    ("list", Mode::List),
    ("load", Mode::Load),
    ("clear", Mode::Clear),
    ("delete", Mode::Delete),
];

fn mode(word: &str) -> Result<Option<Mode>, Never> {
    Ok(EVERY.iter().find(|(spelt, _mode)| *spelt == word).map(|(_spelt, mode)| *mode))
}

struct Asked {
    mode: Mode,
    name: String,
    save_interval: Duration,
    adjusting_for: Duration,
    really: Really,
    restoring: Restoring,
}

fn seconds(said: Option<&str>, unless: Duration) -> Result<Duration, Never> {
    Ok(match said.map(str::parse::<u64>) {
        Some(Ok(seconds)) => Duration::from_secs(seconds),
        Some(Err(_)) | None => unless,
    })
}

const FLAGS: [&str; 4] = ["save-interval", "load-time", "simulate", "adjust-clients-only"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Known {
    Yes,
    No,
}

fn known_flag(word: &str) -> Result<Known, Never> {
    let said = match word.strip_prefix("--") {
        Some(said) => said,
        None => return Ok(Known::Yes),
    };

    let named = match said.split_once('=') {
        Some((named, _value)) => named,
        None => said,
    };

    Ok(match FLAGS.contains(&named) {
        true => Known::Yes,
        false => Known::No,
    })
}

fn asked(words: &[String]) -> Result<Asked, String> {
    for word in words {
        let Ok(known) = known_flag(word);

        match known {
            Known::Yes => {},
            Known::No => {
                return Err(format!("{word:?} is not one of --{}", FLAGS.join(", --")));
            },
        }
    }

    let mut mode_and_name = words.iter().filter(|word| !word.starts_with("--"));

    let flag = |named: &str| {
        words
            .iter()
            .find_map(|word| word.strip_prefix(&format!("--{named}=")))
            .map(str::to_string)
    };

    let set = |named: &str| words.iter().any(|word| word == &format!("--{named}"));

    let said = match mode_and_name.next() {
        Some(said) => said.as_str(),
        None => "default",
    };

    let Ok(known) = mode(said);

    let mode = match known {
        Some(mode) => mode,
        None => {
            let every: Vec<&str> = EVERY.iter().map(|(spelt, _mode)| *spelt).collect();

            return Err(format!("{said:?} is not one of {}", every.join(", ")));
        },
    };

    let Ok(save_interval) = seconds(flag("save-interval").as_deref(), SAVE_INTERVAL);

    match save_interval.is_zero() {
        true => return Err("a save interval of nought is never".to_string()),
        false => {},
    }

    let Ok(adjusting_for) = seconds(flag("load-time").as_deref(), ADJUSTING_FOR);

    Ok(Asked {
        mode,
        name: mode_and_name.next().cloned().unwrap_or_else(|| "default".to_string()),
        save_interval,
        adjusting_for,
        really: match set("simulate") {
            true => Really::Simulated,
            false => Really::Truly,
        },
        restoring: match set("adjust-clients-only") {
            true => Restoring::MovingWhatIsOpen,
            false => Restoring::StartingItAgain,
        },
    })
}

fn where_sessions_live() -> Result<PathBuf, String> {
    match env::var(WHERE) {
        Ok(said) => return Ok(PathBuf::from(said)),
        Err(_it_was_not_said) => {},
    }

    let Ok(ours) = console_core_places::Base::Share.ours();

    match ours {
        Some(ours) => Ok(ours.join(console_resume::OURS)),
        None => Err("there is no home to keep a session in".to_string()),
    }
}

fn putting_back(sessions: &Sessions, name: &str) -> Result<(), String> {
    let Ok(already) = console_resume::already::asked();

    match already {
        Already::PutBack => {
            println!("this compositor has had its windows put back already");

            return Ok(());
        },
        Already::CannotTell => {
            eprintln!(
                "nothing says which compositor this is, so a restart cannot be told from a \
                 log-in and nothing is put back"
            );

            return Ok(());
        },
        Already::NotYet => {},
    }

    console_resume::already::said()?;

    let put_back = sessions.load(name)?;

    match put_back {
        PutBack::Windows => {},
        PutBack::NothingSaved(at) => {
            println!("{} has nothing saved in it yet, so there is nothing to put back", at.display());
        },
    }

    Ok(())
}

fn run() -> Result<(), String> {
    let words: Vec<String> = env::args().skip(1).collect();

    let asked = asked(&words)?;

    let at = where_sessions_live()?;

    create_dir_all(&at).map_err(|fault| format!("{}: making it: {fault}", at.display()))?;

    let sessions = Sessions {
        at,
        adjusting_for: asked.adjusting_for,
        really: asked.really,
        restoring: asked.restoring,
        duplicates: Duplicates::OnePerProgram,
    };

    match asked.mode {
        Mode::Clear => sessions.clear()?,
        Mode::Default => putting_back(&sessions, &asked.name)?,
        Mode::Load => {
            let put_back = sessions.load(&asked.name)?;

            match put_back {
                PutBack::Windows => {},
                PutBack::NothingSaved(at) => {
                    return Err(format!(
                        "{} has no session in it, so the screen is left as it is",
                        at.display()
                    ));
                },
            }
        },
        Mode::Watch => {},
        Mode::Save => sessions.save(&asked.name)?,
        Mode::Delete => sessions.delete(&asked.name)?,
        Mode::List => {
            let Ok(saved) = sessions.list();

            match saved.first() {
                Some(_there_are_some) => {
                    for name in saved {
                        println!(" - {name}");
                    }
                },
                None => println!("(no sessions saved)"),
            }
        },
    }

    match asked.mode {
        Mode::Default | Mode::Watch => sessions.watch(&asked.name, asked.save_interval),
        Mode::Save | Mode::List | Mode::Load | Mode::Clear | Mode::Delete => Ok(()),
    }
}

fn main() {
    match run() {
        Ok(()) => {},
        Err(why) => {
            eprintln!("console-resume: {why}");

            exit(1);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(said: &[&str]) -> Vec<String> {
        said.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn no_words_at_all_is_what_the_unit_starts() {
        let asked = asked(&words(&[]));

        assert_eq!(asked.as_ref().map(|asked| asked.mode), Ok(Mode::Default));
        assert_eq!(asked.map(|asked| asked.name), Ok("default".to_string()));
    }

    #[test]
    fn no_words_is_not_the_mode_that_closes_every_window() {
        let asked = asked(&words(&[]));

        assert_ne!(
            asked.map(|asked| asked.mode),
            Ok(Mode::Load),
            "a bare run of this program must not sweep the desktop"
        );
    }

    #[test]
    fn a_word_this_does_not_know_is_refused_rather_than_read_as_no_word_at_all() {
        for unknown in ["--help", "--version", "sideways", "--save-duplicate-pids"] {
            assert!(
                asked(&words(&[unknown])).is_err(),
                "{unknown} was read as no word at all, which is the mode that sweeps the desktop"
            );
        }
    }

    #[test]
    fn a_mode_nobody_here_says_names_the_ones_that_are_said() {
        let said = asked(&words(&["sideways"]));

        assert_eq!(said.map(|asked| asked.mode).map_err(|why| why.contains("delete")), Err(true));
    }

    #[test]
    fn the_mode_is_a_word_and_the_name_is_the_word_after_it() {
        let asked = asked(&words(&["load", "yesterday"]));

        assert_eq!(asked.as_ref().map(|asked| asked.mode), Ok(Mode::Load));
        assert_eq!(asked.map(|asked| asked.name), Ok("yesterday".to_string()));
    }

    #[test]
    fn a_flag_is_read_wherever_it_stands_among_the_words() {
        let asked = asked(&words(&["save", "--load-time=5", "nightly", "--simulate"]));

        assert_eq!(asked.as_ref().map(|asked| asked.mode), Ok(Mode::Save));
        assert_eq!(asked.as_ref().map(|asked| asked.name.clone()), Ok("nightly".to_string()));
        assert_eq!(asked.as_ref().map(|asked| asked.adjusting_for), Ok(Duration::from_secs(5)));
        assert_eq!(asked.map(|asked| asked.really), Ok(Really::Simulated));
    }

    #[test]
    fn saving_every_nought_seconds_is_refused_rather_than_spun_on() {
        assert!(asked(&words(&["--save-interval=0"])).is_err());
    }
}
