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
//! still exec'd and still holds the screen -- see `console_panel::handoff` for
//! why that half did not go away, and why it was not taken over here.
//!
//! **A closed panel costs nothing.** This is the whole of what a resident
//! process owes a handheld. With nothing on the screen this process is asleep
//! in one `poll` over the door, the panel it is holding and the signal that
//! asks it to stop, so the cost of being warm is the memory and not the
//! battery -- but only for as long as nothing is left running underneath. What a panel holds while it is up is a `pactl subscribe` or a
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
//! be a surface holding a reading no one refreshed, which is the one hazard
//! `docs/programs.md` names as the real one in a program that holds state. An
//! opening here runs the same code an exec ran, less the toolkit coming up.
//!
//! **One at a time, and never none between two.** A request that arrives while
//! something is up is drawn first and closes what was up after its first
//! frame, so the screen goes from one panel to the next without an empty
//! moment in between. It used to close first, and every listener on the
//! compositor's layers -- the bar lighting its icons most of all -- read that
//! moment as nothing being up, so a tap on the clock with settings open lit
//! the calendar, put it out and lit it again. The two are up together for one
//! frame, which is shorter than anything the picker's lock is there to stop:
//! it is about a hand driving a picker it cannot see, and nothing can be
//! pressed into the one going in the frame it takes to go.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;

use rustix::event::{PollFd, PollFlags, poll};

use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_panel::card::{Card, Finalizer};
use console_panel::picker;
use console_panel::handoff::{self, Request, CLOSE, DRAWN, GONE};
use console_panel::left_open::LeftOpen;
use console_panel::surface;

pub struct Panel {
    pub who: &'static str,
    pub card: fn(&[String]) -> Result<Card, Never>,
    pub program: InternalProgram,
}

pub const PANELS: &[Panel] = &[
    Panel { who: console_launcher::WHO, card: console_launcher::card, program: InternalProgram::Launcher },
    Panel { who: console_settings::WHO, card: console_settings::card, program: InternalProgram::SettingsPanel },
    Panel { who: console_music::WHO, card: console_music::card, program: InternalProgram::MusicPanel },
    Panel { who: console_notifications::WHO, card: console_notifications::card, program: InternalProgram::NotificationsPanel },
    Panel { who: console_input_mapping::WHO, card: console_input_mapping::card, program: InternalProgram::MappingPanel },
    Panel { who: console_calculator::WHO, card: console_calculator::card, program: InternalProgram::Calculator },
    Panel { who: console_calendar::WHO, card: console_calendar::card, program: InternalProgram::CalendarPanel },
    Panel { who: console_forecast::WHO, card: console_forecast::card, program: InternalProgram::ForecastPanel },
];

pub fn one(who: &str) -> Result<Option<&'static Panel>, Never> {
    Ok(PANELS.iter().find(|panel| panel.who == who))
}

struct Up {
    who: String,
    shut: surface::Close,
    thread: thread::JoinHandle<Result<(), Never>>,
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    while_it_is_up: OwnedFd,
    done: Option<Finalizer>,
}

pub fn serve() -> Result<(), Never> {
    let Ok(where_) = handoff::where_();

    let at = match where_ {
        Some(at) => at,
        None => {
            eprintln!("console-panels: XDG_RUNTIME_DIR names nothing, so there is nowhere to listen");

            return Ok(());
        }
    };

    let Ok(free) = no_one_is_there(&at);

    match free {
        Free::Occupied => {
            eprintln!("console-panels: {} is answering already", at.display());

            return Ok(());
        }
        Free::Yes => {},
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

    let Ok(stopping) = console_panel::asked::told();

    eprintln!("console-panels: holding the panels, listening at {}", at.display());

    let Ok(()) = put_back();

    let mut up: Option<Up> = None;

    loop {
        let held = up.as_ref().map(|up| Descriptors {
            panel: up.reader.get_ref().as_fd(),
            gone: up.while_it_is_up.as_fd(),
        });
        let Ok(gone) = waited(&listening, stopping.as_ref(), held);

        match gone {
            PanelRequest::ToStop => {
                let Ok(()) = nothing_is_up(&mut up);

                break;
            }
            PanelRequest::ForAPanel => {
                let Ok(()) = taken(&mut up, &listening);
            }
            PanelRequest::ByThePanel => {
                let Ok(said) = a_word(&mut up);

                match said {
                    Word::Close | Word::Closed => {
                        let Ok(()) = nothing_is_up(&mut up);
                    }
                    Word::None => {},
                }
            }
            PanelRequest::PanelGone => {
                let Ok(()) = nothing_is_up(&mut up);
                let Ok(()) = console_panel::left_open::put_away();
            }
            PanelRequest::None => {},
        }
    }

    let _ = std::fs::remove_file(&at);

    Ok(())
}

fn put_back() -> Result<(), Never> {
    let Ok(left) = console_panel::left_open::left();

    let left = match left {
        Some(left) => left,
        None => return Ok(()),
    };

    let Ok(known) = one(&left.who);

    match known {
        Some(known) => console_panel::left_open::started(known.program, left.arguments),
        None => {
            eprintln!("console-panels: {:?} was left open and nothing here draws it", left.who);

            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelRequest {
    ForAPanel,
    ByThePanel,
    PanelGone,
    ToStop,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watching {
    Door,
    Stopping,
    Panel,
    Gone,
}

struct Descriptors<'a> {
    panel: BorrowedFd<'a>,
    gone: BorrowedFd<'a>,
}

fn waited(
    listening: &UnixListener,
    stopping: Option<&OwnedFd>,
    up: Option<Descriptors<'_>>,
) -> Result<PanelRequest, Never> {
    let mut which: Vec<Watching> = vec![Watching::Door];
    let mut watch: Vec<PollFd<'_>> =
        vec![PollFd::from_borrowed_fd(listening.as_fd(), PollFlags::IN)];

    match stopping {
        Some(stopping) => {
            which.push(Watching::Stopping);
            watch.push(PollFd::from_borrowed_fd(stopping.as_fd(), PollFlags::IN));
        }
        None => {},
    }

    match up {
        Some(up) => {
            which.push(Watching::Panel);
            watch.push(PollFd::from_borrowed_fd(up.panel, PollFlags::IN));
            which.push(Watching::Gone);
            watch.push(PollFd::from_borrowed_fd(up.gone, PollFlags::IN));
        }
        None => {},
    }

    match poll(&mut watch, None) {
        Ok(_) => {},
        Err(rustix::io::Errno::INTR) => return Ok(PanelRequest::None),
        Err(fault) => {
            eprintln!("console-panels: waiting: {fault}");

            return Ok(PanelRequest::ToStop);
        }
    }

    let said = watch
        .iter()
        .zip(which)
        .find(|(fd, _)| !fd.revents().is_empty())
        .map(|(_, which)| which);

    Ok(match said {
        Some(Watching::Door) => PanelRequest::ForAPanel,
        Some(Watching::Stopping) => PanelRequest::ToStop,
        Some(Watching::Panel) => PanelRequest::ByThePanel,
        Some(Watching::Gone) => PanelRequest::PanelGone,
        None => PanelRequest::None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Free {
    Yes,
    Occupied,
}

fn no_one_is_there(at: &Path) -> Result<Free, Never> {
    match UnixStream::connect(at) {
        Ok(_answered) => return Ok(Free::Occupied),
        Err(_no_one_is_listening) => {},
    }

    let _ = std::fs::remove_file(at);

    Ok(Free::Yes)
}

fn taken(up: &mut Option<Up>, listening: &UnixListener) -> Result<(), Never> {
    loop {
        let asking = match listening.accept() {
            Ok((asking, _)) => asking,
            Err(_nothing_more_is_waiting) => return Ok(()),
        };

        let Ok(()) = asked_of(up, asking);
    }
}

fn asked_of(up: &mut Option<Up>, asking: UnixStream) -> Result<(), Never> {
    let heard = match asking.try_clone() {
        Ok(heard) => heard,
        Err(fault) => {
            eprintln!("console-panels: {fault}");

            return Ok(());
        }
    };

    let mut reader = BufReader::new(heard);
    let mut line = String::new();

    match reader.read_line(&mut line) {
        Ok(0) => return Ok(()),
        Err(_the_read_failed) => return Ok(()),
        Ok(_) => {},
    }

    let Ok(asked) = handoff::read(line.trim_end());

    let asked = match asked {
        Some(asked) => asked,
        None => {
            eprintln!("console-panels: {:?} is not a request", line.trim_end());

            return Ok(());
        }
    };

    put_up(up, asked, asking, reader)
}

fn put_up(
    up: &mut Option<Up>,
    asked: Request,
    writer: UnixStream,
    reader: BufReader<UnixStream>,
) -> Result<(), Never> {
    let Ok(known) = one(&asked.who);

    let known = match known {
        Some(known) => known,
        None => {
            eprintln!("console-panels: nothing here draws {:?}", asked.who);

            let Ok(()) = say(&writer, GONE);

            return Ok(());
        }
    };

    let Ok(()) = console_panel::opening::asked(
        &asked.who,
        asked.pressed.as_deref(),
        console_panel::opening::Came(&asked.from),
        asked.exec,
    );

    let Ok(()) = console_panel::left_open::opened(&LeftOpen { who: asked.who.clone(), arguments: asked.arguments.clone() });
    let Ok(card) = (known.card)(&asked.arguments);
    let Card { build, column, start, done } = card;

    let (while_it_is_up, told_when_it_is_not) = match rustix::pipe::pipe() {
        Ok(ends) => ends,
        Err(fault) => {
            eprintln!("console-panels: nothing to hear a panel close on: {fault}");

            let Ok(()) = say(&writer, GONE);

            return Ok(());
        }
    };

    let (first_frame, drawn) = mpsc::channel();

    let Ok((shut, surface_thread)) = surface::run(
        &asked.who,
        build,
        column,
        start.as_deref(),
        told_when_it_is_not,
        first_frame,
        asked.tells.clone(),
    );

    match drawn.recv_timeout(picker::COMING) {
        Ok(()) => {},
        Err(RecvTimeoutError::Timeout) => {
            eprintln!("console-panels: {} drew nothing in time, and what it replaces goes anyway", asked.who);
        }
        Err(RecvTimeoutError::Disconnected) => {},
    }

    let Ok(()) = nothing_is_up(up);

    let Ok(()) = say(&writer, DRAWN);

    *up = Some(Up {
        who: asked.who.clone(),
        shut,
        thread: surface_thread,
        writer,
        reader,
        while_it_is_up,
        done: Some(done),
    });

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Word {
    Close,
    Closed,
    None,
}

fn a_word(up: &mut Option<Up>) -> Result<Word, Never> {
    let mut line = String::new();

    let up = match up {
        Some(up) => up,
        None => return Ok(Word::None),
    };

    Ok(match up.reader.read_line(&mut line) {
        Ok(0) => Word::Closed,
        Err(_the_read_failed) => Word::Closed,
        Ok(_) => match line.trim() == CLOSE {
            true => Word::Close,
            false => Word::None,
        },
    })
}

fn nothing_is_up(up: &mut Option<Up>) -> Result<(), Never> {
    let up = match up.take() {
        Some(up) => up,
        None => return Ok(()),
    };

    let Ok(mut closing) = console_response_times::Waiting::here(console_response_times::Wait { who: &up.who, what: "closing" });
    let Ok(()) = up.shut.shut();
    let Up { who: _, shut: _, thread, writer, reader: _, while_it_is_up: _, done } = up;
    let _ = thread.join();
    let Ok(()) = closing.mark("drawing");

    let Ok(()) = say(&writer, GONE);

    match done {
        Some(done) => {
            let Ok(()) = done();
        }
        None => {},
    }

    let Ok(()) = closing.mark("finishing");

    closing.done()
}

fn say(writer: &UnixStream, word: &str) -> Result<(), Never> {
    let mut writing = writer;

    match writeln!(writing, "{word}") {
        Ok(()) => {},
        Err(_the_panel_that_asked_has_gone) => return Ok(()),
    }

    let _ = writing.flush();

    Ok(())
}

fn drawn_by_hand(who: &str, arguments: &[String]) -> Result<(), Never> {
    let Ok(known) = one(who);

    let known = match known {
        Some(known) => known,
        None => {
            eprintln!("console-panels: nothing here draws {who:?}");

            return Ok(());
        }
    };

    let Ok(card) = (known.card)(arguments);

    surface::drawn_here(who, card)
}

pub fn asked_for(arguments: &[String]) -> Result<(), Never> {
    match arguments.split_first() {
        None => serve(),
        Some((who, rest)) => drawn_by_hand(who, rest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_that_put_itself_away_is_gone_without_a_word_from_whoever_asked_for_it() {
        let at = std::env::temp_dir().join(format!("console-panels-{}-gone", std::process::id()));
        let _ = std::fs::remove_file(&at);
        let listening = UnixListener::bind(&at).expect("nowhere to listen");
        let (asked_by, _still_waiting) = UnixStream::pair().expect("no socket to ask on");
        let (while_it_is_up, told_when_it_is_not) = rustix::pipe::pipe().expect("no pipe");

        drop(told_when_it_is_not);

        let heard = waited(
            &listening,
            None,
            Some(Descriptors { panel: asked_by.as_fd(), gone: while_it_is_up.as_fd() }),
        );
        let _ = std::fs::remove_file(&at);

        assert_eq!(
            heard,
            Ok(PanelRequest::PanelGone),
            "B put the panel away and the program that asked for it is still waiting to hear so; \
             reading a word from it instead is a host stuck until the next press, which then \
             closes a menu that is not there rather than opening one"
        );
    }

    #[test]
    fn every_panel_is_named_once() {
        let mut named: Vec<&str> = PANELS.iter().map(|panel| panel.who).collect();

        named.sort_unstable();
        let mut once = named.clone();
        once.dedup();

        assert_eq!(once, named, "two entries answering to one name: {named:?}");
    }

    #[test]
    fn a_panel_is_found_by_the_name_someone_types() {
        let Ok(found) = one("launcher");

        assert!(found.is_some(), "the menu is on a button, a paddle, a key and the bar");

        let Ok(nothing) = one("console-panels");

        assert!(nothing.is_none(), "the host is not a panel it can be asked to draw");
    }

    #[test]
    fn a_panel_left_open_is_put_back_by_starting_the_program_that_asks_for_it() {
        for panel in PANELS {
            assert_eq!(panel.program.name(), Ok(panel.who), "a restart would start the wrong program to put {} back", panel.who);
        }
    }

    #[test]
    fn every_name_here_is_the_program_a_road_in_already_names() {
        for panel in PANELS {
            assert!(!panel.who.is_empty());
            assert!(!panel.who.contains('/'), "{}: a namespace is a name, not a path", panel.who);
        }
    }
}
