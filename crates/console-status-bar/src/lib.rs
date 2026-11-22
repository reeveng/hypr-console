//! What the bar says about the machine, and which menu you are in.
//!
//! The desktop bar can move between the top and sides. Its preferences select
//! the built-in readings and applets shown on that edge.
//!
//! waybar has modules of its own for the sound, the network, the bluetooth and
//! the battery, and they read the machine perfectly well. What they cannot do
//! is wear a class that something outside them decides, and every one of these
//! icons opens a tab of the settings panel. So the icon could not say whether
//! the thing it opens is already on the screen, which the two doors on the left
//! have said since the day they were written.
//!
//! These are those four readings, said by us, so that the icon lights while its
//! own tab is in front.
//!
//! Every `bar-*` program is built here, which it was not: `bar-door` came out
//! of `console-panel` and `bar-updating` out of `console-notifications`, each
//! beside the thing it asks about rather than beside the thing it feeds. The
//! question either one asks is someone else's -- what is on the screen, how
//! far an apply has got -- and the library it asks it with is still theirs.
//! What is this crate's is the answer: a line of waybar's JSON, with a class
//! the stylesheet paints. So they are here for what they produce, and the
//! prefix names one crate again.

pub mod applets;
pub mod clock;
pub mod dwindling;
pub mod lock_screen;
pub mod measuring;
pub mod notifications;
pub mod panels;
pub mod preferences;
pub mod reading;
pub mod settings;
pub mod showing;
pub mod state;
pub mod watch;

use std::fs::File;
use std::io;
use std::path::Path;

fn read_optional(at: &Path, limit: u32) -> io::Result<Option<String>> {
  use rustix::fs::{Mode, OFlags, open};

  let file = match open(
    at,
    OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
    Mode::empty(),
  ) {
    Ok(file) => File::from(file),
    Err(rustix::io::Errno::NOENT) => return Ok(None),
    Err(error) => return Err(io::Error::from(error)),
  };

  let metadata = file.metadata()?;

  match metadata.is_file().then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "expected a regular file",
      ));
    }
  }

  let text = console_core_bounded_reads::text(file, limit)?;

  Ok(Some(text))
}
