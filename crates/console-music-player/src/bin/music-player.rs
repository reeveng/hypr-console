//! The player, started once and then only ever asked.
//!
//! One argument, and it is the folder the music is under. The player is told
//! where the library is rather than reading it out of somebody else's settings:
//! `console_music_panel::library` used to open kew's `kewrc` to find out, and that
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
use console_music_player::bus::{Held, changed, serve};
use console_music_player::sounding::Sounding;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

static HELD: OnceLock<Arc<Mutex<Held>>> = OnceLock::new();

static ANSWERING: OnceLock<gio::DBusConnection> = OnceLock::new();

fn ended() -> Result<(), Never> {
    glib::idle_add_once(|| {
        let Ok(()) = onward();
    });

    Ok(())
}

fn onward() -> Result<(), Never> {
    let held = match HELD.get() {
        Some(held) => held,
        None => return Ok(()),
    };

    let next = match held.lock() {
        Ok(mut held) => held.finished(),
        Err(poisoned) => poisoned.into_inner().finished(),
    };

    let Ok(()) = next;

    match ANSWERING.get() {
        Some(connection) => changed(connection, held),
        None => Ok(()),
    }
}

fn main() -> Result<(), Never> {
    let folder = std::env::args().nth(1).map(PathBuf::from);
    let Ok(sounding) = Sounding::new(ended);
    let Ok(held) = Held::new(sounding, folder);
    let held = Arc::new(Mutex::new(held));

    let _the_song_ending_needs_to_find_this = HELD.set(Arc::clone(&held));

    let Ok(connection) = serve(&held);

    match connection {
        Some(connection) => {
            let _the_song_ending_says_so_on_this = ANSWERING.set(connection);
        },
        None => return Ok(()),
    }

    glib::MainLoop::new(None, false).run();

    Ok(())
}
