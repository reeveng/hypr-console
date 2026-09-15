//! The player, started once and then only ever asked.
//!
//! One argument, and it is the folder the music is under. The player is told
//! where the library is rather than reading it out of somebody else's settings:
//! `console_music::library` used to open kew's `kewrc` to find out, and that
//! file goes wherever the fork goes.
//!
//! The loop is glib's because the bus is gio's. Nothing is drawn and no toolkit
//! is started -- this is a program whose whole surface is a set of method calls
//! -- so what it costs to have running is a thread carrying samples and a name
//! on the session bus.
//!
//! A song ending is the one thing that happens without anybody asking. The
//! thread that carries samples cannot answer it, because what plays next is the
//! playlist's and the playlist belongs to whoever holds the lock, so the thread
//! says only that the song ended and the answer is worked out here on the main
//! context, in the same place every other answer is worked out.

use console_core_never::Never;
use console_music_player::bus::{Held, changed, locked, serve};
use console_music_player::sounding::Sounding;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

struct Answering {
    held: Arc<Mutex<Held>>,
    connection: gio::DBusConnection,
}

fn ended(answering: &Arc<OnceLock<Answering>>) -> Result<(), Never> {
    let answering = Arc::clone(answering);

    glib::idle_add_once(move || {
        let Ok(()) = onward(&answering);
    });

    Ok(())
}

fn onward(answering: &Arc<OnceLock<Answering>>) -> Result<(), Never> {
    let answering = match answering.get() {
        Some(answering) => answering,
        None => return Ok(()),
    };

    let Ok(mut holding) = locked(&answering.held);
    let Ok(()) = holding.finished();
    let Ok(()) = holding.remember();

    drop(holding);

    changed(&answering.connection, &answering.held)
}

fn main() -> Result<(), Never> {
    let folder = std::env::args().nth(1).map(PathBuf::from);
    let answering: Arc<OnceLock<Answering>> = Arc::new(OnceLock::new());
    let ending = Arc::clone(&answering);
    let Ok(sounding) = Sounding::new(move || ended(&ending));
    let Ok(held) = Held::new(sounding, folder);
    let held = Arc::new(Mutex::new(held));

    let Ok(connection) = serve(&held);

    match connection {
        Some(connection) => {
            let _the_song_ending_finds_the_player_and_the_bus_here =
                answering.set(Answering { held, connection });
        },
        None => return Ok(()),
    }

    glib::MainLoop::new(None, false).run();

    Ok(())
}
