//! The line, put on a queue and written by a thread of its own.
//!
//! A stopwatch that costs what it measures measures itself. The line is short
//! and the write is one `write` to a descriptor opened `O_APPEND`, but the rest
//! of it is not: the first line a process writes has to make the directory,
//! open the file and ask how long it already is, and every line after it pays
//! for a `write` on the thread that has just finished drawing a window and is
//! about to draw the next one. On a handheld that is the frame somebody sees.
//!
//! So nothing on the timed thread touches the disk. It renders the line, hands
//! it to a queue and goes back to what it was doing; one thread per process,
//! started when the first line is written and never stopped, holds the file
//! open for the life of the process and does the writing.
//!
//! ## When the queue is full
//!
//! The handover is `try_send`, so a thread being timed never waits on the one
//! writing -- and a queue that is full means the disk is behind by four
//! thousand lines, which has never happened and would be worth knowing about.
//! The line is then written here, on the thread that made it, rather than
//! dropped: a stopwatch that quietly loses the slow openings is a stopwatch
//! that says the machine is fast.
//!
//! ## When a process is about to go
//!
//! A queued line is in this process and nowhere else, so a program that writes
//! one and exits at once can exit before the thread has written it. Anything
//! whose whole run is one wait -- going to Game Mode, coming back, the desktop
//! starting -- calls `settled` before it returns, which waits for the queue to
//! empty and no longer. A panel does not need to: it writes its opening and
//! then stays up for as long as somebody is looking at it.
//!
//! ## When the file is long
//!
//! It is kept. `CAP` is ten gigabytes and the device has the room: what these
//! lines are for is the question *is this getting slower*, which cannot be
//! asked of a file that keeps a week. The rotation that is left is a last stop
//! against a program stuck in a loop, not a retention policy.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::where_;

pub const CAP: u64 = 10 << 30;

const QUEUE: usize = 4096;

enum Asked {
    Line(String),
    Settled(SyncSender<()>),
}

pub fn line(said: &str) -> Result<(), Never> {
    let mut whole = String::with_capacity(said.len().saturating_add(1));
    whole.push_str(said);
    whole.push('\n');

    let Ok(held) = writer();

    let say = match held {
        Some(say) => say,
        None => {
            let Ok(()) = by_hand(&whole);

            return Ok(());
        }
    };

    match say.try_send(Asked::Line(whole.clone())) {
        Ok(()) => {}
        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
            let Ok(()) = by_hand(&whole);
        }
    }

    Ok(())
}

pub fn settled() -> Result<(), Never> {
    let Ok(held) = writer();

    let say = match held {
        Some(say) => say,
        None => return Ok(()),
    };

    let (told, back) = sync_channel(0);

    match say.send(Asked::Settled(told)) {
        Ok(()) => {
            let _ = back.recv();
        }
        Err(_) => {}
    }

    Ok(())
}

fn writer() -> Result<Option<&'static SyncSender<Asked>>, Never> {
    static WRITER: OnceLock<Option<SyncSender<Asked>>> = OnceLock::new();

    let held = WRITER.get_or_init(|| {
        let Ok(started) = start();

        started
    });

    Ok(held.as_ref())
}

fn start() -> Result<Option<SyncSender<Asked>>, Never> {
    let (say, heard) = sync_channel(QUEUE);

    let Ok(at) = where_();

    let at = match at {
        Some(at) => at,
        None => return Ok(None),
    };

    let started = std::thread::Builder::new()
        .name("wait-times".to_string())
        .spawn(move || keep(&heard, &at));

    match started {
        Ok(_) => Ok(Some(say)),
        Err(fault) => {
            eprintln!("console-response-times: no thread to write with: {fault}");

            Ok(None)
        }
    }
}

fn keep(heard: &Receiver<Asked>, at: &Path) -> Result<(), Never> {
    let mut store: Option<Store> = None;

    for asked in heard {
        match asked {
            Asked::Line(said) => {
                let Ok(held) = written(store, at, &said);

                store = held;
            }
            Asked::Settled(told) => {
                let _ = told.send(());
            }
        }
    }

    Ok(())
}

struct Store {
    file: File,
    long: u64,
}

fn written(store: Option<Store>, at: &Path, said: &str) -> Result<Option<Store>, Never> {
    let mut store = match store {
        Some(store) => store,
        None => {
            let Ok(held) = opened(at);

            match held {
                Some(held) => held,
                None => return Ok(None),
            }
        }
    };

    match store.file.write_all(said.as_bytes()) {
        Ok(()) => {}
        Err(fault) => {
            eprintln!("console-response-times: {}: {fault}", at.display());

            return Ok(None);
        }
    }

    let Ok(long) = fitted(said.len());

    store.long = store.long.saturating_add(long);

    match store.long >= CAP {
        true => {
            let Ok(()) = set_aside(at);

            Ok(None)
        }
        false => Ok(Some(store)),
    }
}

fn opened(at: &Path) -> Result<Option<Store>, Never> {
    match at.parent() {
        Some(above) => match std::fs::create_dir_all(above) {
            Ok(()) => {}
            Err(fault) => {
                eprintln!("console-response-times: {}: {fault}", above.display());

                return Ok(None);
            }
        },
        None => {}
    }

    let file = match OpenOptions::new().create(true).append(true).open(at) {
        Ok(file) => file,
        Err(fault) => {
            eprintln!("console-response-times: {}: {fault}", at.display());

            return Ok(None);
        }
    };
    let long = match file.metadata() {
        Ok(about) => about.len(),
        Err(fault) => {
            eprintln!(
                "console-response-times: {}: how long it already is would not be said: {fault}. The \
                 ten gigabytes are counted from here instead",
                at.display()
            );

            0
        }
    };

    Ok(Some(Store { file, long }))
}

fn set_aside(at: &Path) -> Result<(), Never> {
    let _ = std::fs::rename(at, at.with_extension("jsonl.old"));

    Ok(())
}

fn by_hand(said: &str) -> Result<(), Never> {
    let Ok(at) = where_();

    let at = match at {
        Some(at) => at,
        None => return Ok(()),
    };

    let Ok(held) = opened(&at);

    let mut store = match held {
        Some(store) => store,
        None => return Ok(()),
    };

    let _ = store.file.write_all(said.as_bytes());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn somewhere(named: &str) -> PathBuf {
        std::env::temp_dir().join(format!("console-waited-{named}-{}", std::process::id()))
    }

    #[test]
    fn the_file_is_opened_once_and_held_for_every_line_after_the_first() {
        let at = somewhere("held");
        let _ = std::fs::remove_file(&at);
        let Ok(store) = written(None, &at, "one\n");

        assert!(store.is_some(), "the first line did not open the file");

        let Ok(store) = written(store, &at, "two\n");

        assert!(store.is_some(), "the second line let go of the file");
        assert_eq!(std::fs::read_to_string(&at).unwrap_or_default(), "one\ntwo\n");
        let _ = std::fs::remove_file(&at);
    }

    #[test]
    fn a_line_written_by_hand_lands_where_the_thread_would_have_put_it() {
        let at = somewhere("by-hand");
        let _ = std::fs::remove_file(&at);
        let Ok(store) = written(None, &at, "written on the thread that timed it\n");

        assert!(store.is_some());
        assert!(std::fs::read_to_string(&at).unwrap_or_default().ends_with('\n'));
        let _ = std::fs::remove_file(&at);
    }

    #[test]
    fn the_thread_writes_what_it_was_given_and_answers_once_the_queue_is_empty() {
        let at = somewhere("queue");
        let _ = std::fs::remove_file(&at);
        let (say, heard) = sync_channel(8);
        let there = at.clone();
        let thread = std::thread::spawn(move || keep(&heard, &there));
        say.send(Asked::Line("one\n".to_string())).expect("a queue with room in it");
        let (told, back) = sync_channel(0);
        say.send(Asked::Settled(told)).expect("a queue with room in it");
        back.recv().expect("the thread says when it has caught up");
        assert_eq!(std::fs::read_to_string(&at).unwrap_or_default(), "one\n");
        drop(say);
        let _ = thread.join();
        let _ = std::fs::remove_file(&at);
    }

    #[test]
    fn waiting_for_a_queue_nothing_was_put_on_returns() {
        let Ok(()) = settled();
        let Ok(()) = settled();
    }
}
