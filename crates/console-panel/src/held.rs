//! Asking the program that holds the panels to put one up.
//!
//! Three days of the device's own timing file said the same thing every day:
//! the two largest stretches of an opening are `exec` and the toolkit coming
//! up, and everything a card does after that is a fraction of either. Neither
//! is work about the panel. They are the cost of starting from nothing, paid
//! again every time somebody presses a button, on a machine that spent the
//! moment before asleep.
//!
//! So one program keeps the toolkit up and draws whichever panel is asked for,
//! and this is how it is asked. What is on the other side of the socket has
//! already opened the display, parsed the stylesheet and read the icon theme,
//! and none of that happens twice.
//!
//! **The program that asks is still the program that holds the screen.** The
//! one-chooser lock is a `flock` on a file held open for as long as a process
//! lives, and it is released by the kernel however that process ends -- which
//! is what lets a chooser that was killed outright leave nothing behind. A
//! host that outlived every panel would be a host that never let the lock go.
//! So the panel's own program stays: it takes the lock, asks for the surface,
//! and waits with nothing to draw until the surface is gone. Every road in --
//! the bar's `on-click`, a `.desktop` file, the daemon's table, a key bound in
//! the compositor -- reaches the same program under the same name, and what
//! `hyprctl` reports as being on the screen is the same namespace as before.
//!
//! **A panel with no host is merely slower.** This is a daemon that can be
//! down, and nothing here may be written as though it cannot be: a socket that
//! refuses is a panel that draws in its own process, exactly as it did before
//! any of this, and says so on the journal rather than on the screen.
//!
//! **What the lock file says is said by the one holding it.** `chooser::drawn`
//! and `chooser::gone` write into the lock this process opened, and the host
//! has no such lock -- called there they would be two no-ops, and the door name
//! they write is what `alone` reads to decide that a door which only opens has
//! been asked twice. So the host says which of the two has happened and the
//! stand-in writes it, which is the same order it happened in when a panel was
//! one process.
//!
//! Lines, and no version number, for `console_event_broker::wire`'s reasons:
//! both ends are built by the same `console apply` from the same commit, and
//! somebody holding `socat` against the socket should be able to read what
//! went past. What is escaped here is not what is escaped there -- a topic is
//! one token and a space is what ends it, and these are a program's arguments,
//! which are allowed to hold anything at all except a nul.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use console_never::Never;

pub const SOCKET: &str = "panels.sock";

pub const OPEN: &str = "open";

pub const CLOSE: &str = "close";

pub const DRAWN: &str = "drawn";

pub const GONE: &str = "gone";

const NOTHING: &str = "-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub who: String,
    pub from: String,
    pub pressed: Option<String>,
    pub exec: Duration,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawn {
    ByTheHost,
    Here,
}

pub fn where_() -> Result<Option<PathBuf>, Never> {
    let run = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(run) if !run.is_empty() => run,
        Ok(_) | Err(_) => return Ok(None),
    };

    Ok(Some(PathBuf::from(run).join("console").join(SOCKET)))
}

pub fn spelt(asked: &Asked) -> Result<String, Never> {
    let mut said = String::from(OPEN);

    for word in [&asked.who, &asked.from] {
        let Ok(word) = escaped(word);

        said.push(' ');
        said.push_str(&word);
    }

    let stamp = asked.pressed.clone().unwrap_or_else(|| NOTHING.to_string());
    let Ok(stamp) = escaped(&stamp);

    said.push(' ');
    said.push_str(&stamp);
    said.push(' ');
    said.push_str(&asked.exec.as_nanos().to_string());

    for word in &asked.argv {
        let Ok(word) = escaped(word);

        said.push(' ');
        said.push_str(&word);
    }

    Ok(said)
}

pub fn read(line: &str) -> Result<Option<Asked>, Never> {
    let mut words = line.split(' ');

    match words.next() {
        Some(OPEN) => {},
        Some(_) | None => return Ok(None),
    }

    let Some(who) = words.next() else { return Ok(None) };

    let Some(from) = words.next() else { return Ok(None) };

    let Some(pressed) = words.next() else { return Ok(None) };

    let Some(exec) = words.next() else { return Ok(None) };

    let Ok(exec) = took(exec);
    let Ok(who) = plain(who);
    let Ok(from) = plain(from);
    let Ok(stamp) = plain(pressed);

    let argv: Vec<String> = words
        .map(|word| {
            let Ok(word) = plain(word);

            word
        })
        .collect();

    match who.is_empty() {
        true => return Ok(None),
        false => {},
    }

    Ok(Some(Asked {
        who,
        from,
        pressed: match stamp.is_empty() {
            true => None,
            false => Some(stamp),
        },
        exec,
        argv,
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

static TELLING: AtomicI32 = AtomicI32::new(-1);

pub fn stood_in(who: &str, argv: &[String]) -> Result<Drawn, Never> {
    let Ok(where_) = where_();

    let Some(at) = where_ else {
        eprintln!("{who}: XDG_RUNTIME_DIR names nothing, so there is no host to ask");

        return Ok(Drawn::Here);
    };

    stood_in_at(&at, who, argv)
}

pub fn stood_in_at(at: &Path, who: &str, argv: &[String]) -> Result<Drawn, Never> {
    let asking = match UnixStream::connect(at) {
        Ok(asking) => asking,
        Err(fault) => {
            eprintln!("{who}: {}: {fault}, so this panel draws itself", at.display());

            return Ok(Drawn::Here);
        }
    };

    let Ok(asked) = asking_for(who, argv);
    let Ok(said) = spelt(&asked);

    let mut writing = match asking.try_clone() {
        Ok(writing) => writing,
        Err(fault) => {
            eprintln!("{who}: {fault}, so this panel draws itself");

            return Ok(Drawn::Here);
        }
    };

    match writeln!(writing, "{said}") {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("{who}: {fault}, so this panel draws itself");

            return Ok(Drawn::Here);
        }
    }

    let _ = writing.flush();

    TELLING.store(writing.as_raw_fd(), Ordering::SeqCst);

    let Ok(()) = answers_being_asked_to_stop();

    waited(who, asking)
}

fn waited(who: &str, asking: UnixStream) -> Result<Drawn, Never> {
    let reading = BufReader::new(asking);
    let mut drawn = Drawn::Here;

    for line in reading.lines() {
        let Ok(line) = line else { break };

        match line.trim() {
            DRAWN => {
                let Ok(()) = crate::chooser::drawn();

                drawn = Drawn::ByTheHost;
            }
            GONE => {
                let Ok(()) = crate::chooser::gone();

                return Ok(Drawn::ByTheHost);
            }
            said => eprintln!("{who}: the host said {said:?}, which is nothing this knows"),
        }
    }

    Ok(match drawn {
        Drawn::ByTheHost => Drawn::ByTheHost,
        Drawn::Here => {
            eprintln!("{who}: the host went away before it drew anything, so this panel does");

            Drawn::Here
        }
    })
}

fn asking_for(who: &str, argv: &[String]) -> Result<Asked, Never> {
    let Ok(pressed) = console_wait_times::press_said();
    let Ok(from) = console_wait_times::from_said();
    let Ok(exec) = console_wait_times::since_exec();

    Ok(Asked {
        who: who.to_string(),
        from,
        pressed,
        exec: exec.unwrap_or_default(),
        argv: argv.to_vec(),
    })
}

fn answers_being_asked_to_stop() -> Result<(), Never> {
    #[cfg_attr(
        dylint_lib = "explicit011_no_as_cast",
        allow(
            explicit011_no_as_cast,
            reason = "no trait turns a function into the number `signal` takes, which is `chooser::showing`'s reason for the same cast"
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

    fn asked(argv: &[&str]) -> Asked {
        Asked {
            who: "launcher".to_string(),
            from: "bar".to_string(),
            pressed: Some("12345".to_string()),
            exec: Duration::from_millis(3),
            argv: argv.iter().map(|word| (*word).to_string()).collect(),
        }
    }

    #[test]
    fn what_was_asked_for_is_what_arrives() {
        let asking = asked(&["--place", "2.1.3"]);
        let Ok(said) = spelt(&asking);

        assert_eq!(read(&said), Ok(Some(asking)));
    }

    #[test]
    fn an_argument_with_a_space_in_it_is_still_one_argument() {
        let asking = asked(&["Game Mode"]);
        let Ok(said) = spelt(&asking);
        let Ok(back) = read(&said);

        assert_eq!(back.map(|back| back.argv), Some(vec!["Game Mode".to_string()]));
    }

    #[test]
    fn a_path_holding_the_marks_that_spell_the_line_survives_it() {
        let awkward = "/home/someone/a folder\\with a mark/and a\nline";
        let asking = asked(&[awkward]);
        let Ok(said) = spelt(&asking);

        assert!(!said.contains('\n'), "a request is one line or it is two requests");
        let Ok(back) = read(&said);

        assert_eq!(back.map(|back| back.argv), Some(vec![awkward.to_string()]));
    }

    #[test]
    fn an_argument_that_says_nothing_is_still_an_argument() {
        let asking = asked(&["", "Sound"]);
        let Ok(said) = spelt(&asking);
        let Ok(back) = read(&said);

        assert_eq!(
            back.map(|back| back.argv),
            Some(vec![String::new(), "Sound".to_string()]),
            "an empty word dropped would shift every argument after it"
        );
    }

    #[test]
    fn a_panel_nobody_pressed_says_so_rather_than_saying_nought() {
        let asking = Asked { pressed: None, from: String::new(), ..asked(&[]) };
        let Ok(said) = spelt(&asking);

        assert_eq!(read(&said), Ok(Some(asking)), "a stamp of nought is a wait that ended long ago");
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
