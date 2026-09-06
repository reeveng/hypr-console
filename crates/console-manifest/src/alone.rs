//! One writer at a time.
//!
//! `apply` is the one operation that rewrites the whole machine: pacman, a
//! cargo build, sixty files, the profiles, then systemd. It can be started
//! from two places -- by hand on the device, and over ssh by
//! `console-deploy` -- and nothing has ever stopped the two from being
//! started a minute apart.
//!
//! Two of them interleaved is not two applies. One sweeps the staged copies
//! the other is about to swap in; one restarts a service against files the
//! other has half written; the `Deploy` each is holding names the other's
//! kept copies as the way back. Every guard in `laying` is written against a
//! run that stopped, and none of them is written against a second run.
//!
//! So a run that writes takes this first, and a second one is refused rather
//! than let in. Reading is never blocked: `check` and `list` change nothing,
//! and somebody watching an apply from the other end of an ssh link is exactly
//! who wants to run one.
//!
//! # Why this is a socket and not a lock file
//!
//! It was `flock` on `/run/console/apply.lock`, for a good reason: the kernel
//! drops the lock when the process ends however it ended, so the power cut
//! this code already worries about leaves nothing behind to explain. That is
//! the right instinct and it was held the wrong way round. An advisory lock
//! is on the *inode*, and the second apply does not ask for the inode -- it
//! asks for the path. Remove the file, by hand or by anything that tidies
//! `/run`, and the holder goes on holding a lock on an inode with no name
//! while the next apply makes a fresh file, locks that instead, and runs.
//! `a_lock_whose_file_was_removed_still_refuses_the_next_one` is where that
//! was found, and no arrangement of `flock` closes it: there is no shared
//! inode left to contend for.
//!
//! An abstract-namespace socket has no filesystem entry at all, so there is
//! nothing to remove. The name lives in the kernel, binding it twice fails on
//! the second, and it goes when the process does -- which is every property
//! the lock file was chosen for, and the one it could not keep. It needs no
//! `unsafe`, and nothing has to be cleaned up on the way out.
//!
//! It is per network namespace, which is the one limit worth knowing: an
//! apply run inside a namespace of its own would not see one on the host.
//! Nothing here runs that way, and an apply that did would have a great deal
//! else wrong with it.
//!
//! It is also invisible to `ls`. `ss -xl | grep console-apply` is how you see
//! who holds it.

use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener};

pub const NAME: &str = "console/apply";

#[derive(Debug)]
pub struct Alone {
    _holding: UnixListener,
}

pub fn taking() -> Result<Alone, String> {
    named(NAME)
}

pub fn named(name: &str) -> Result<Alone, String> {
    let who = SocketAddr::from_abstract_name(name.as_bytes())
        .map_err(|fault| format!("{name} is not a name this kernel will hold: {fault}"))?;

    match UnixListener::bind_addr(&who) {
        Ok(holding) => Ok(Alone { _holding: holding }),
        Err(_) => Err("another console apply is running on this machine.".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_name_of_our_own(what: &str) -> String {
        format!("console/test-{}-{what}", std::process::id())
    }

    #[test]
    fn a_second_writer_is_refused() {
        let name = a_name_of_our_own("refused");
        let first = named(&name);
        assert!(first.is_ok());
        assert!(named(&name).is_err());
    }

    #[test]
    fn giving_it_back_lets_the_next_one_in() {
        let name = a_name_of_our_own("again");
        let first = named(&name);
        assert!(first.is_ok());
        drop(first);
        assert!(named(&name).is_ok());
    }

    #[test]
    fn two_different_names_are_two_different_locks() {
        let one = named(&a_name_of_our_own("one"));
        let other = named(&a_name_of_our_own("other"));
        assert!(one.is_ok() && other.is_ok(), "an unrelated name was refused");
    }

    #[test]
    fn the_refusal_says_what_is_actually_happening() {
        let name = a_name_of_our_own("said");
        let first = named(&name);
        assert!(first.is_ok());
        let said = named(&name).expect_err("the second one was let in");
        assert!(
            said.contains("another console apply is running"),
            "the refusal does not say another apply is running: {said:?}"
        );
        assert!(!said.contains("os error"), "the refusal hands over an errno: {said:?}");
    }

    #[test]
    fn the_lock_is_not_a_file_anybody_can_remove() {
        let name = a_name_of_our_own("nofile");
        let held = named(&name);
        assert!(held.is_ok());
        for was in ["/run/console/apply.lock", "/run/console"] {
            assert!(
                !std::path::Path::new(was).exists(),
                "{was} is on the filesystem again, and a lock somebody can rm is a lock two \
                 applies can hold at once"
            );
        }
        assert!(named(&name).is_err(), "the name stopped excluding once nothing named it");
    }
}
