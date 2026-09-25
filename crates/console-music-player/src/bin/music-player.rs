//! The player, started once and then only ever asked.
//!
//! One argument, and it is the folder the music is under. The player is told
//! where the library is rather than reading it out of someone else's settings:
//! `console_music::library` used to open kew's `kewrc` to find out, and that
//! file goes wherever the fork goes.
//!
//! The loop is the bus. Nothing is drawn and no toolkit is started -- this is a
//! program whose whole surface is a set of method calls -- so what it costs to
//! have running is a thread carrying samples and a name on the session bus.
//! There was a glib main loop here, for no reason except that the bus was
//! gio's; `console-bus` hands over a socket to wait on instead, and waiting on
//! it is this loop.
//!
//! A song ending is the one thing that happens without anyone asking, beside
//! the clock moving on, which is only said. The thread that carries samples
//! says so, and what plays next is worked out
//! there and then, under the same lock every other answer is worked out under
//! -- the playlist belongs to whoever holds it. That was a hop through glib's
//! idle queue before, which was how the answer reached the one thread gio
//! would answer a call on; a lock has no such thread, and the hop was carrying
//! nothing.

use console_bus::connection::{Bus, ConnectionError, NameRequestResult};
use console_core_never::Never;
use console_music_player::bus::{PlayerState, changed, heard, locked, moved};
use console_music_player::sounding::{Progress, Sounding};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, OnceLock};

use console_music_player::answers;
use console_bus::connection::Sender;

struct Answering {
    held: Arc<Mutex<PlayerState>>,
    saying: Sender,
}

fn ended(answering: &Arc<OnceLock<Answering>>) -> Result<(), Never> {
    let answering = match answering.get() {
        Some(answering) => answering,
        None => return Ok(()),
    };

    let Ok(mut holding) = locked(&answering.held);
    let Ok(()) = holding.finished();
    let Ok(()) = holding.remember();

    drop(holding);

    changed(&answering.saying, &answering.held)
}

fn played_on(answering: &Arc<OnceLock<Answering>>) -> Result<(), Never> {
    match answering.get() {
        Some(answering) => moved(&answering.saying),
        None => Ok(()),
    }
}

fn main() -> ExitCode {
    match answering() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("music-player: {why}");

            ExitCode::FAILURE
        }
    }
}

fn answering() -> Result<(), ConnectionError> {
    let folder = std::env::args().nth(1).map(PathBuf::from);
    let mut bus = Bus::session()?;
    let got = bus.taking(answers::NAME)?;

    match got {
        NameRequestResult::PrimaryOwner | NameRequestResult::AlreadyOwner => {}
        NameRequestResult::InQueue | NameRequestResult::Exists => {
            eprintln!("music-player: another player has the name, so this one answers to nothing");

            return Ok(());
        }
    }

    let answering: Arc<OnceLock<Answering>> = Arc::new(OnceLock::new());
    let ending = Arc::clone(&answering);
    let Ok(sounding) = Sounding::new(move |progress| match progress {
        Progress::Ended => ended(&ending),
        Progress::ASecondPlayed => played_on(&ending),
    });
    let Ok(held) = PlayerState::new(sounding, folder);
    let held = Arc::new(Mutex::new(held));
    let Ok((mut hearing, saying)) = bus.apart();

    let _the_song_ending_finds_the_player_and_the_bus_here =
        answering.set(Answering { held: Arc::clone(&held), saying: saying.clone() });

    loop {
        let message = hearing.heard()?;
        let Ok(turn) = heard(&held, &message);

        match turn.say {
            Some(say) => {
                let _ = saying.say(&say);
            }
            None => {},
        }

        match turn.changed {
            console_music_player::bus::Modified::Yes => {
                let Ok(()) = changed(&saying, &held);
            }
            console_music_player::bus::Modified::No => {},
        }
    }
}
