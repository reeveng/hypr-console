//! A descriptor a loop can be woken down.
//!
//! The rest of this crate is about a thread that has to stop between two asks.
//! This is the other half of the same subject and the better half: a loop that
//! is already waiting on descriptors -- a compositor's socket, a bus, a
//! keyboard -- and a thread beside it that has something to say. A pipe is what
//! joins the two. The thread writes a byte, `poll` returns, and the loop looks
//! at what changed; there is no duration anywhere in it and nothing is asked
//! twice.
//!
//! It is here because two programs had written the same twenty lines of
//! `libc::pipe` and `from_raw_fd`, and a third was about to. What each of them
//! wanted was not a pipe, it was to be woken, and saying that once is what
//! keeps the `unsafe` in one place with its reason beside it.
//!
//! **What comes down it carries nothing.** A byte is *look again*, never a
//! message: what actually changed is behind a lock the writer and the loop
//! share, or is simply re-read. That is deliberate -- a pipe that carried
//! meaning would be a queue that can fill, and a queue that fills blocks the
//! thread that was only trying to say "something happened".
//!
//! **Both ends refuse to block, and that is the whole of what was wrong with
//! the two copies this replaced.** A loop polling a compositor's socket and
//! this pipe together is woken by either, and then drains the pipe -- so on
//! every wake that was the compositor's, the drain is a `read` on a pipe
//! no one has written to, which waits for a byte that is not coming. It is a
//! desktop that stops answering the first time two things happen in the wrong
//! order, and it looks like a compositor fault rather than a read. The far end
//! is the same argument upside down: a pipe left full blocks the source thread
//! that was only saying "look again", and a full pipe already says that. So
//! neither end waits, and a drain that finds nothing is an answer.
//!
//! They close across an exec as well. The loop that holds one of these is the
//! kind of program that starts panels, and a descriptor a panel inherits is a
//! pipe that never reports its writer gone.

use rustix::io::read;
use rustix::pipe::{pipe_with, PipeFlags};
use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;

use console_core_never::Never;

#[derive(Debug)]
pub struct Woken {
    pub waiting: OwnedFd,
    pub saying: File,
}

pub fn pipe() -> Result<Woken, io::Error> {
    let (waiting, saying) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)?;
    let saying = File::from(saying);

    Ok(Woken { waiting, saying })
}

pub fn drained(waiting: &OwnedFd) -> Result<(), Never> {
    let mut heard = [0_u8; 64];

    let _ = read(waiting, &mut heard);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn a_byte_written_down_it_is_a_byte_the_loop_can_read() {
        let woken = match pipe() {
            Ok(woken) => woken,
            Err(why) => panic!("a pipe should be made: {why}"),
        };
        let mut saying = woken.saying;

        match saying.write_all(&[1]) {
            Ok(()) => {}
            Err(why) => panic!("a byte should go down it: {why}"),
        }

        assert_eq!(drained(&woken.waiting), Ok(()));
    }

    #[test]
    fn draining_one_no_one_wrote_to_answers_rather_than_waiting_for_a_byte() {
        let woken = match pipe() {
            Ok(woken) => woken,
            Err(why) => panic!("a pipe should be made: {why}"),
        };

        assert_eq!(drained(&woken.waiting), Ok(()));
    }
}