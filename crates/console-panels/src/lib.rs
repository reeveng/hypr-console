//! One program, holding every panel.
//!
//! Three days of the device's own timing file said the same thing every day:
//! the two largest stretches of any opening are `exec` and the toolkit coming
//! up. Neither is work about the panel. A card is built and filled and drawn in
//! a fraction of either, so no amount of work on how a card is built moves the
//! number a thumb waits -- and the thing that does is not starting a process at
//! all.
//!
//! So this one stays up. It opens the display once, parses the stylesheet once,
//! reads the icon theme once, and draws whichever panel it is asked for. What
//! reaches it is a request on a socket from the panel's own program, which is
//! still exec'd and still holds the screen -- see `console_panel::held` for why
//! that half did not go away.
//!
//! **A closed panel costs nothing.** This is the whole of what a resident
//! process owes a handheld. A GTK loop with no surface mapped has no frame
//! clock to tick and sits in `poll`, so the cost of being warm is the memory
//! and not the battery -- but only for as long as nothing is left running
//! underneath. What a panel holds while it is up is a `pactl subscribe` or a
//! `busctl monitor` per watch, a thread per actor, and a window; today those
//! cost nothing because the process dies. Here they are released by name, in
//! `shut`, and `console-events`' own head is the argument for why that matters:
//! twenty-five orphaned subscriptions were once found alive on this device, the
//! oldest four hours old.
//!
//! **Nothing is kept between openings.** The window is destroyed and built
//! again rather than hidden and shown, and the panel's state is built by the
//! same call its `main` used to make. That is deliberate and it costs a few
//! milliseconds of the cheapest stretch there is: a surface that survived would
//! be a surface holding a reading nobody refreshed, which is the one hazard
//! `docs/programs.md` names as the real one in a program that holds state. An
//! opening here runs the same code an exec ran, less the toolkit coming up.
//!
//! **One at a time.** The chooser's lock already promises that and this does
//! not lean on it: a request that arrives while something is up closes what is
//! up first, which is what `chooser::alone` does between processes and has to
//! keep meaning here.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::rc::Rc;

use console_core_never::Never;
use console_panel::card::{Card, Done};
use console_panel::held::{self, Asked, CLOSE, DRAWN, GONE};
use console_panel::panel::{self, Over};
use gtk4::glib;
use gtk4::prelude::*;

pub mod arriving;

use arriving::{Again, Watching};

pub struct Panel {
    pub who: &'static str,
    pub card: fn(&[String]) -> Result<Card, Never>,
}

pub const PANELS: &[Panel] = &[
    Panel { who: console_launcher::WHO, card: console_launcher::card },
    Panel { who: console_settings::WHO, card: console_settings::card },
    Panel { who: console_files::WHO, card: console_files::card },
    Panel { who: console_music_panel::WHO, card: console_music_panel::card },
    Panel { who: console_media_viewer::WHO, card: console_media_viewer::card },
    Panel { who: console_downloads::WHO, card: console_downloads::card },
    Panel { who: console_notifications::WHO, card: console_notifications::card },
    Panel { who: console_input_mapping::WHO, card: console_input_mapping::card },
];

pub fn one(who: &str) -> Result<Option<&'static Panel>, Never> {
    Ok(PANELS.iter().find(|panel| panel.who == who))
}

struct Up {
    panel: Rc<panel::Panel>,
    telling: UnixStream,
    watching: Option<Watching>,
    done: Option<Done>,
}

struct Holding {
    up: RefCell<Option<Up>>,
}

pub fn serve() -> Result<(), Never> {
    let Ok(where_) = held::where_();

    let at = match where_ {
        Some(at) => at,
        None => {
            eprintln!("console-panels: XDG_RUNTIME_DIR names nothing, so there is nowhere to listen");

            return Ok(());
        }
    };

    let Ok(free) = nobody_is_there(&at);

    match free {
        Free::Taken => {
            eprintln!("console-panels: {} is answering already", at.display());

            return Ok(());
        }
        Free::Yes => {},
    }

    let Ok(toolkit) = panel::toolkit();

    match toolkit {
        panel::Toolkit::Up => {},
        panel::Toolkit::None => return Ok(()),
    }

    let listening = match UnixListener::bind(&at) {
        Ok(listening) => listening,
        Err(fault) => {
            eprintln!("console-panels: {}: {fault}", at.display());

            return Ok(());
        }
    };

    match listening.set_nonblocking(true) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-panels: {fault}"),
    }

    let waiting = glib::MainLoop::new(None, false);

    match gtk4::gdk::Display::default() {
        Some(screen) => {
            let over = waiting.clone();
            screen.connect_closed(move |_, _| over.quit());
        }
        None => {},
    }

    let holding = Rc::new(Holding { up: RefCell::new(None) });
    let taking = Rc::clone(&holding);
    let door = listening.as_raw_fd();

    let Ok(_the_door_is_watched_for_as_long_as_this_runs) =
        arriving::when_there_is_something(door, move || {
            let Ok(()) = taken(&taking, &listening);

            Again::Yes
        });

    let told = Rc::clone(&holding);
    let over = waiting.clone();
    let Ok(()) = console_panel::asked::stops_when_asked(move || {
        let Ok(()) = nothing_is_up(&told);

        over.quit();
    });

    eprintln!("console-panels: holding the panels, listening at {}", at.display());

    waiting.run();

    let _ = std::fs::remove_file(&at);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Free {
    Yes,
    Taken,
}

fn nobody_is_there(at: &Path) -> Result<Free, Never> {
    match UnixStream::connect(at) {
        Ok(_answered) => return Ok(Free::Taken),
        Err(_) => {},
    }

    let _ = std::fs::remove_file(at);

    Ok(Free::Yes)
}

fn taken(holding: &Rc<Holding>, listening: &UnixListener) -> Result<(), Never> {
    loop {
        let asking = match listening.accept() {
            Ok((asking, _)) => asking,
            Err(_nothing_more_is_waiting) => return Ok(()),
        };

        let Ok(()) = asked_of(holding, asking);
    }
}

fn asked_of(holding: &Rc<Holding>, asking: UnixStream) -> Result<(), Never> {
    let heard = match asking.try_clone() {
        Ok(heard) => heard,
        Err(fault) => {
            eprintln!("console-panels: {fault}");

            return Ok(());
        }
    };

    let mut reading = BufReader::new(heard);
    let mut line = String::new();

    match reading.read_line(&mut line) {
        Ok(0) | Err(_) => return Ok(()),
        Ok(_) => {},
    }

    let Ok(asked) = held::read(line.trim_end());

    let asked = match asked {
        Some(asked) => asked,
        None => {
            eprintln!("console-panels: {:?} is not a request", line.trim_end());

            return Ok(());
        }
    };

    put_up(holding, asked, asking, reading)
}

fn put_up(
    holding: &Rc<Holding>,
    asked: Asked,
    telling: UnixStream,
    reading: BufReader<UnixStream>,
) -> Result<(), Never> {
    let Ok(known) = one(&asked.who);

    let known = match known {
        Some(known) => known,
        None => {
            eprintln!("console-panels: nothing here draws {:?}", asked.who);

            let Ok(()) = say(&telling, GONE);

            return Ok(());
        }
    };

    let Ok(()) = nothing_is_up(holding);

    let Ok(()) = console_panel::opening::asked(
        &asked.who,
        asked.pressed.as_deref(),
        &asked.from,
        asked.exec,
    );

    let Ok(card) = (known.card)(&asked.argv);
    let Card { build, column, start, done } = card;

    let closing = Rc::clone(holding);
    let Ok(drawn) = panel::raised(
        &asked.who,
        build,
        column,
        start.as_deref(),
        Over::Told(Rc::new(move || {
            let Ok(()) = gone(&closing);
        })),
    );

    let Ok(()) = say(&telling, DRAWN);

    let heard = Rc::clone(holding);
    let fd = reading.get_ref().as_raw_fd();
    let listening = RefCell::new(reading);
    let Ok(watching) = arriving::when_there_is_something(fd, move || {
        let Ok(said) = a_word(&listening);

        match said {
            Word::Close | Word::Gone => {
                let Ok(()) = nothing_is_up(&heard);

                Again::No
            }
            Word::Nothing => Again::Yes,
        }
    });

    *holding.up.borrow_mut() =
        Some(Up { panel: drawn, telling, watching: Some(watching), done: Some(done) });

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Word {
    Close,
    Gone,
    Nothing,
}

fn a_word(listening: &RefCell<BufReader<UnixStream>>) -> Result<Word, Never> {
    let mut line = String::new();

    let read = match listening.try_borrow_mut() {
        Ok(mut reading) => reading.read_line(&mut line),
        Err(_) => return Ok(Word::Nothing),
    };

    Ok(match read {
        Ok(0) | Err(_) => Word::Gone,
        Ok(_) => match line.trim() == CLOSE {
            true => Word::Close,
            false => Word::Nothing,
        },
    })
}

fn gone(holding: &Rc<Holding>) -> Result<(), Never> {
    let up = holding.up.borrow_mut().take();

    let up = match up {
        Some(up) => up,
        None => return Ok(()),
    };

    let Ok(()) = let_go(up);

    Ok(())
}

fn nothing_is_up(holding: &Rc<Holding>) -> Result<(), Never> {
    let up = holding.up.borrow_mut().take();

    let up = match up {
        Some(up) => up,
        None => return Ok(()),
    };

    let Ok(()) = up.panel.shut();
    let Ok(()) = let_go(up);

    Ok(())
}

fn let_go(up: Up) -> Result<(), Never> {
    let Up { panel: _, telling, watching, done } = up;

    match watching {
        Some(watching) => {
            let Ok(()) = watching.stop();
        }
        None => {},
    }

    let Ok(()) = say(&telling, GONE);

    match done {
        Some(done) => {
            let Ok(()) = done();
        }
        None => {},
    }

    Ok(())
}

fn say(telling: &UnixStream, word: &str) -> Result<(), Never> {
    let mut writing = telling;

    match writeln!(writing, "{word}") {
        Ok(()) => {},
        Err(_the_panel_that_asked_has_gone) => return Ok(()),
    }

    let _ = writing.flush();

    Ok(())
}

fn drawn_by_hand(who: &str, argv: &[String]) -> Result<(), Never> {
    let Ok(known) = one(who);

    let known = match known {
        Some(known) => known,
        None => {
            eprintln!("console-panels: nothing here draws {who:?}");

            return Ok(());
        }
    };

    let Ok(card) = (known.card)(argv);

    panel::drawn_here(who, card)
}

pub fn asked_for(argv: &[String]) -> Result<(), Never> {
    match argv.split_first() {
        None => serve(),
        Some((who, rest)) => drawn_by_hand(who, rest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_panel_is_named_once() {
        let mut named: Vec<&str> = PANELS.iter().map(|panel| panel.who).collect();
        let many = named.len();

        named.sort_unstable();
        named.dedup();

        assert_eq!(named.len(), many, "two entries answering to one name: {named:?}");
    }

    #[test]
    fn a_panel_is_found_by_the_name_somebody_types() {
        let Ok(found) = one("launcher");

        assert!(found.is_some(), "the menu is on a button, a paddle, a key and the bar");

        let Ok(nothing) = one("console-panels");

        assert!(nothing.is_none(), "the host is not a panel it can be asked to draw");
    }

    #[test]
    fn every_name_here_is_the_program_a_road_in_already_names() {
        for panel in PANELS {
            assert!(!panel.who.is_empty());
            assert!(!panel.who.contains('/'), "{}: a namespace is a name, not a path", panel.who);
        }
    }
}
