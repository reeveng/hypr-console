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
//! lines from the wallpaper -- so the deciding happens here, on the near side
//! of the socket, and anything that wants to disagree asks
//! [`crate::listening`] itself the way `console-sky` does.
//!
//! **[`about`] cannot be called without saying what the lines mean, and it
//! could.** It used to turn every line on a topic into *ask again* and ask
//! nobody what the lines were, which is the one thing a subscriber must not be
//! handed: a reading answered by asking its own source is heard by the source
//! as a change, and the bar read the volume forty-seven times a second for as
//! long as it was up. The filter is an argument now rather than a thing a
//! caller might remember, and a watch that truly wants every line says
//! [`anything`] out loud. [`layers`] is the same call with the one filter the
//! compositor's own words already settle.
//!
//! **Getting into the pool is a word.** What these carry is *ask again* rather
//! than an answer, so a gap in one is a reading quietly out of date with
//! nothing on the way to correct it -- which is what every watch this replaced
//! meant by saying something the moment it connected. `bar-door` is the one
//! with no tick underneath it and so the one that would stay wrong.

use std::sync::mpsc::Sender;

use console_core_never::Never;
use console_program_contract::Topic;

use crate::listening::{self, Heard, Listening};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Asking,
    Ignoring,
}

pub type Worthwhile = fn(&str) -> Result<Worth, Never>;

pub fn anything(_every_line_is_a_reason: &str) -> Result<Worth, Never> {
    Ok(Worth::Asking)
}

pub fn layers(say: Sender<()>) -> Result<(), Never> {
    let Ok(listening) = listening::listen(&[Topic::Compositor]);

    saying(listening, surfaces, say)
}

pub fn about(topic: &Topic, worth: Worthwhile, say: Sender<()>) -> Result<(), Never> {
    let Ok(listening) = listening::listen(std::slice::from_ref(topic));

    saying(listening, worth, say)
}

fn surfaces(line: &str) -> Result<Worth, Never> {
    let Ok(worth) = console_onscreen::worth_asking_after(line);

    Ok(match worth {
        console_onscreen::Worth::Asking => Worth::Asking,
        console_onscreen::Worth::Ignoring => Worth::Ignoring,
    })
}

fn saying(listening: Listening, worth: Worthwhile, say: Sender<()>) -> Result<(), Never> {
    let _ = std::thread::spawn(move || {
        let Ok(heard) = listening.heard();

        for heard in heard.iter() {
            let asking = match &heard {
                Heard::GotIn => Worth::Asking,
                Heard::Said(changed) => {
                    let Ok(asking) = worth(&changed.said);

                    asking
                }
            };

            match asking {
                Worth::Asking => {
                    match say.send(()) {
                        Ok(()) => {},
                        Err(_nobody_is_listening) => return,
                    }
                }
                Worth::Ignoring => {},
            }
        }
    });

    Ok(())
}
