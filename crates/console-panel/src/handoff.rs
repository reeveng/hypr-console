//! Asking the program that holds the panels to put one up.
//!
//! Three days of the device's own timing file said the same thing every day:
//! the two largest stretches of an opening are `exec` and the toolkit coming
//! up, and everything a card does after that is a fraction of either. Neither
//! is work about the panel. They are the cost of starting from nothing, paid
//! again every time someone presses a button, on a machine that spent the
//! moment before asleep.
//!
//! So one program keeps the toolkit up and draws whichever panel is asked for,
//! and this is how it is asked. What is on the other side of the socket has
//! already opened the display, parsed the stylesheet and read the icon theme,
//! and none of that happens twice.
//!
//! **The program that asks is still the program that holds the screen.** The
//! one-picker lock is a `flock` on a file held open for as long as a process
//! lives, and it is released by the kernel however that process ends -- which
//! is what lets a picker that was killed outright leave nothing behind. A
//! host that outlived every panel would be a host that never let the lock go.
//! So the panel's own program stays: it takes the lock, asks for the surface,
//! and waits with nothing to draw until the surface is gone. Every road in --
//! the bar's `on-click`, a `.desktop` file, the daemon's table, a key bound in
//! the compositor -- reaches the same program under the same name, and what
//! `hyprctl` reports as being on the screen is the same namespace as before.
//!
//! **The host could hold the lock, and the lock is not what keeps the program
//! that asks.** A `flock` the host took for one panel would be let go on every
//! road out: a panel closed is a `close` the host answers, a drawing that fails
//! is the pipe the host already watches for the surface ending, and a host that
//! is killed is a process ending, which the kernel answers for a host exactly as
//! it does for a panel. What the file holds beside the lock is a pid, and the
//! pid is the half everything else uses: a picker coming asks the one there to
//! go with a SIGTERM and takes it off the screen with a SIGKILL when it will
//! not, and the paddle's `console-put-away` is that SIGTERM and nothing more.
//! Held by the host, every one of those lands on the process holding every
//! panel -- a panel put away is the host told to stop, and one slow to go is the
//! host killed. Teaching the host that a SIGTERM means "put away what is up" is
//! a host that no longer stops the way the service manager stops it, and the
//! bar and the daemon, which only start a program today, would each have to be
//! told whether their press opened something or closed it, because a door that
//! only opens and a second press that closes are the holder's to decide. All
//! of that would buy the asking program's own start, which measured against
//! `socat` knocking on the same socket is the loader and a few reads -- small
//! beside the card being built and drawn, which no road in skips.
//!
//! **A panel with no host is merely slower.** This is a daemon that can be
//! down, and nothing here may be written as though it cannot be: a socket that
//! refuses is a panel that draws in its own process, exactly as it did before
//! any of this, and says so on the journal rather than on the screen.
//!
//! **What the lock file says is said by the one holding it.** `picker::drawn`
//! and `picker::gone` write into the lock this process opened, and the host
//! has no such lock -- called there they would be two no-ops, and the door name
//! they write is what `alone` reads to decide that a door which only opens has
//! been asked twice. So the host says which of the two has happened and the
//! stand-in writes it, which is the same order it happened in when a panel was
//! one process.
//!
//! Lines, and no version number, for `console_events::wire`'s reasons:
//! both ends are built by the same `console apply` from the same commit, and
//! someone holding `socat` against the socket should be able to read what
//! went past. What is escaped here is not what is escaped there -- a topic is
//! one token and a space is what ends it, and these are a program's arguments,
//! which are allowed to hold anything at all except a nul.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use console_core_never::Never;

pub const SOCKET: &str = "panels.sock";

pub const OPEN: &str = "open";

pub const CLOSE: &str = "close";

pub const DRAWN: &str = "drawn";

pub const GONE: &str = "gone";

const NOTHING: &str = "-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub who: String,
    pub from: String,
    pub pressed: Option<String>,
    pub exec: Duration,
    pub tells: Option<String>,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawnBy {
    ByTheHost,
    Here,
}

pub fn where_() -> Result<Option<PathBuf>, Never> {
    let ours = console_core_places::runtime_ours()?;

    Ok(ours.map(|ours| ours.join(SOCKET)))
}

pub fn spelled(asked: &Request) -> Result<String, Never> {
    let mut said = String::from(OPEN);

    for word in [&asked.who, &asked.from] {
        let Ok(word) = escaped(word);

        said.push(' ');
        said.push_str(&word);
    }

    let stamp = match asked.pressed.clone() {
        Some(stamp) => stamp,
        None => NOTHING.to_string(),
    };
    let Ok(stamp) = escaped(&stamp);

    said.push(' ');
    said.push_str(&stamp);
    said.push(' ');
    said.push_str(&asked.exec.as_nanos().to_string());

    let tells = match asked.tells.clone() {
        Some(tells) => tells,
        None => NOTHING.to_string(),
    };
    let Ok(tells) = escaped(&tells);

    said.push(' ');
    said.push_str(&tells);

    for word in &asked.arguments {
        let Ok(word) = escaped(word);

        said.push(' ');
        said.push_str(&word);
    }

    Ok(said)
}

pub fn read(line: &str) -> Result<Option<Request>, Never> {
    let mut words = line.split(' ');

    match words.next() {
        Some(OPEN) => {},
        Some(_) | None => return Ok(None),
    }

    let who = match words.next() {
        Some(who) => who,
        None => return Ok(None),
    };

    let from = match words.next() {
        Some(from) => from,
        None => return Ok(None),
    };

    let pressed = match words.next() {
        Some(pressed) => pressed,
        None => return Ok(None),
    };

    let exec = match words.next() {
        Some(exec) => exec,
        None => return Ok(None),
    };

    let tells = match words.next() {
        Some(tells) => tells,
        None => return Ok(None),
    };

    let Ok(exec) = took(exec);
    let Ok(who) = plain(who);
    let Ok(from) = plain(from);
    let Ok(stamp) = plain(pressed);
    let Ok(tells) = plain(tells);

    let arguments: Vec<String> = words
        .map(|word| {
            let Ok(word) = plain(word);

            word
        })
        .collect();

    match who.is_empty() {
        true => return Ok(None),
        false => {},
    }

    Ok(Some(Request {
        who,
        from,
        pressed: match stamp.is_empty() {
            true => None,
            false => Some(stamp),
        },
        exec,
        tells: match tells.is_empty() {
            true => None,
            false => Some(tells),
        },
        arguments,
    }))
}

fn took(said: &str) -> Result<Duration, Never> {
    Ok(match said.trim().parse::<u64>() {
        Ok(nanos) => Duration::from_nanos(nanos),
        Err(_) => Duration::ZERO,
    })
}

fn escaped(said: &str) -> Result<String, Never> {
    let mut written = String::with_capacity(said.len());

    for letter in said.chars() {
        match letter {
            '\\' => written.push_str("\\\\"),
            ' ' => written.push_str("\\s"),
            '\n' => written.push_str("\\n"),
            other => written.push(other),
        }
    }

    match written.is_empty() {
        true => Ok(NOTHING.to_string()),
        false => Ok(written),
    }
}

fn plain(said: &str) -> Result<String, Never> {
    match said == NOTHING {
        true => return Ok(String::new()),
        false => {},
    }

    let mut written = String::with_capacity(said.len());
    let mut letters = said.chars();

    while let Some(letter) = letters.next() {
        match letter {
            '\\' => match letters.next() {
                Some('\\') => written.push('\\'),
                Some('s') => written.push(' '),
                Some('n') => written.push('\n'),
                Some(other) => written.push(other),
                None => {},
            },
            other => written.push(other),
        }
    }

    Ok(written)
}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "a signal handler is handed nothing and may allocate nothing: the descriptor it writes `close` down has to be a number it can read with one load"
    )
)]
static TELLING: AtomicI32 = AtomicI32::new(-1);

pub fn stood_in(who: &str, arguments: &[String]) -> Result<DrawnBy, Never> {
    let Ok(where_) = where_();

    let Ok(drawn) = match where_ {
        Some(at) => stood_in_at(&at, who, arguments),
        None => {
            eprintln!("{who}: XDG_RUNTIME_DIR names nothing, so there is no host to ask");

            Ok(DrawnBy::Here)
        }
    };

    match drawn {
        DrawnBy::Here => {
            let Ok(()) = crate::picker::taken_over();
        }
        DrawnBy::ByTheHost => {},
    }

    Ok(drawn)
}

pub fn stood_in_at(at: &Path, who: &str, arguments: &[String]) -> Result<DrawnBy, Never> {
    let asking = match UnixStream::connect(at) {
        Ok(asking) => asking,
        Err(fault) => {
            eprintln!("{who}: {}: {fault}, so this panel draws itself", at.display());

            return Ok(DrawnBy::Here);
        }
    };

    let Ok(asked) = asking_for(who, arguments);
    let Ok(said) = spelled(&asked);

    let mut writing = match asking.try_clone() {
        Ok(writing) => writing,
        Err(fault) => {
            eprintln!("{who}: {fault}, so this panel draws itself");

            return Ok(DrawnBy::Here);
        }
    };

    match writeln!(writing, "{said}") {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("{who}: {fault}, so this panel draws itself");

            return Ok(DrawnBy::Here);
        }
    }

    let _ = writing.flush();

    TELLING.store(writing.as_raw_fd(), Ordering::SeqCst);

    let Ok(()) = answers_being_asked_to_stop();

    waited(who, asking)
}

fn waited(who: &str, asking: UnixStream) -> Result<DrawnBy, Never> {
    let reading = BufReader::new(asking);
    let mut drawn = DrawnBy::Here;

    for line in reading.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_fault) => break,
        };

        match line.trim() {
            DRAWN => {
                let Ok(()) = crate::picker::drawn();

                drawn = DrawnBy::ByTheHost;
            }
            GONE => {
                let Ok(()) = crate::picker::gone();

                return Ok(DrawnBy::ByTheHost);
            }
            said => eprintln!("{who}: the host said {said:?}, which is nothing this knows"),
        }
    }

    Ok(match drawn {
        DrawnBy::ByTheHost => DrawnBy::ByTheHost,
        DrawnBy::Here => {
            eprintln!("{who}: the host went away before it drew anything, so this panel does");

            DrawnBy::Here
        }
    })
}

fn asking_for(who: &str, arguments: &[String]) -> Result<Request, Never> {
    let Ok(pressed) = console_response_times::press_said();
    let Ok(from) = console_response_times::from_said();
    let Ok(exec) = console_response_times::since_exec();
    let Ok(tells) = crate::description::where_to();

    Ok(Request {
        who: who.to_string(),
        from,
        pressed,
        exec: match exec {
            Some(exec) => exec,
            None => std::time::Duration::ZERO,
        },
        tells,
        arguments: arguments.to_vec(),
    })
}

fn answers_being_asked_to_stop() -> Result<(), Never> {
    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(
            explicit051_no_machine_width,
            reason = "`signal` takes a `sighandler_t`, which is the machine's width by the C ABI and not by choice here"
        )
    )]
    #[cfg_attr(
        dylint_lib = "explicit011_no_as_cast",
        allow(
            explicit011_no_as_cast,
            reason = "no trait turns a function into the number `signal` takes, which is `picker::showing`'s reason for the same cast"
        )
    )]
    let answer = telling as extern "C" fn(libc::c_int) as libc::sighandler_t;

    for number in [libc::SIGHUP, libc::SIGINT, libc::SIGTERM] {
        // SAFETY: the handler writes one line to a socket and nothing else,
        unsafe { libc::signal(number, answer) };
    }

    Ok(())
}

extern "C" fn telling(_number: libc::c_int) {
    let fd = TELLING.load(Ordering::SeqCst);

    match fd < 0 {
        true => return,
        false => {},
    }

    let said = b"close\n";

    // SAFETY: `write` is what a signal handler is allowed to call, and the fd
    unsafe { libc::write(fd, said.as_ptr().cast(), said.len()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(arguments: &[&str]) -> Request {
        Request {
            who: "launcher".to_string(),
            from: "bar".to_string(),
            pressed: Some("12345".to_string()),
            exec: Duration::from_millis(3),
            tells: None,
            arguments: arguments.iter().map(|word| (*word).to_string()).collect(),
        }
    }

    #[test]
    fn what_was_asked_for_is_what_arrives() {
        let asking = asked(&["--place", "2.1.3"]);
        let Ok(said) = spelled(&asking);

        assert_eq!(read(&said), Ok(Some(asking)));
    }

    #[test]
    fn where_the_asker_wants_the_panel_to_say_what_it_drew_reaches_the_host() {
        let asking = Request { tells: Some("/run/user/1000/told here.jsonl".to_string()), ..asked(&["General"]) };
        let Ok(said) = spelled(&asking);

        assert_eq!(read(&said), Ok(Some(asking)));
    }

    #[test]
    fn an_argument_with_a_space_in_it_is_still_one_argument() {
        let asking = asked(&["Game Mode"]);
        let Ok(said) = spelled(&asking);
        let Ok(back) = read(&said);

        assert_eq!(back.map(|back| back.arguments), Some(vec!["Game Mode".to_string()]));
    }

    #[test]
    fn a_path_holding_the_marks_that_spell_the_line_survives_it() {
        let awkward = "/home/someone/a folder\\with a mark/and a\nline";
        let asking = asked(&[awkward]);
        let Ok(said) = spelled(&asking);

        assert!(!said.contains('\n'), "a request is one line or it is two requests");
        let Ok(back) = read(&said);

        assert_eq!(back.map(|back| back.arguments), Some(vec![awkward.to_string()]));
    }

    #[test]
    fn an_argument_that_says_nothing_is_still_an_argument() {
        let asking = asked(&["", "Sound"]);
        let Ok(said) = spelled(&asking);
        let Ok(back) = read(&said);

        assert_eq!(
            back.map(|back| back.arguments),
            Some(vec![String::new(), "Sound".to_string()]),
            "an empty word dropped would shift every argument after it"
        );
    }

    #[test]
    fn a_panel_no_one_pressed_says_so_rather_than_saying_nought() {
        let asking = Request { pressed: None, from: String::new(), ..asked(&[]) };
        let Ok(said) = spelled(&asking);

        assert_eq!(read(&said), Ok(Some(asking)), "a stamp of zero is a wait that ended long ago");
    }

    #[test]
    fn a_line_that_is_not_a_request_is_not_read_as_one() {
        assert_eq!(read(""), Ok(None));
        assert_eq!(read("shut launcher"), Ok(None));
        assert_eq!(read("open"), Ok(None));
        assert_eq!(read("open launcher"), Ok(None));
        assert_eq!(read("open launcher bar 12345"), Ok(None));
        assert_eq!(read("open - bar 12345 0"), Ok(None), "a panel with no name is no panel");
    }

    #[test]
    fn the_socket_is_beside_the_pools() {
        let Ok(where_) = where_();

        match where_ {
            Some(at) => {
                assert!(at.ends_with("console/panels.sock"), "{}", at.display());
            }
            None => {},
        }
    }
}
