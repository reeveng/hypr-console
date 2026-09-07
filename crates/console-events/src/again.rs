//! The pool's words turned back into *ask again*.
//!
//! A bar module does not want the line a source said, it wants to know that
//! its reading is stale. `watching_layers` was that shape before the pool
//! existed -- a channel of nothing, one word per reason to look -- and four
//! programs were written against it, so this is the same channel with the pool
//! behind it. Moving one over is a change of one call.
//!
//! **What is worth asking after stays outside the pool.** `console-events`
//! relays a line as a line and never parses one, and the bar keeps different
//! lines from the wallpaper -- so [`layers`] does its deciding here, on the
//! near side of the socket, and anything that wants to disagree asks
//! [`crate::listening`] itself the way `console-sky` does.
//!
//! **Getting into the pool is a word.** What these carry is *ask again* rather
//! than an answer, so a gap in one is a reading quietly out of date with
//! nothing on the way to correct it -- which is what every watch this replaced
//! meant by saying something the moment it connected. `bar-door` is the one
//! with no tick underneath it and so the one that would stay wrong.

use std::sync::mpsc::Sender;

use console_core_never::Never;
use console_onscreen::Worth;
use console_program_contract::Topic;

use crate::listening::{self, Heard};

pub fn layers(say: Sender<()>) -> Result<(), Never> {
    let Ok(listening) = listening::listen(&[Topic::Compositor]);

    let _ = std::thread::spawn(move || {
        let Ok(heard) = listening.heard();

        for heard in heard.iter() {
            let worth = match &heard {
                Heard::GotIn => Worth::Asking,
                Heard::Said(changed) => {
                    let Ok(worth) = console_onscreen::worth_asking_after(&changed.said);

                    worth
                }
            };

            match worth {
                Worth::Asking => {
                    let Ok(()) = say.send(()) else { return };
                }
                Worth::Ignoring => {},
            }
        }
    });

    Ok(())
}

pub fn about(topic: &Topic, say: Sender<()>) -> Result<(), Never> {
    let Ok(listening) = listening::listen(std::slice::from_ref(topic));

    let _ = std::thread::spawn(move || {
        let Ok(heard) = listening.heard();

        for _heard in heard.iter() {
            let Ok(()) = say.send(()) else { return };
        }
    });

    Ok(())
}
