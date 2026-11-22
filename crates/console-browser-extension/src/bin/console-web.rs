//! Pack the add-on this desktop puts in its browser, and put it in the profile
//! the browser will look through when it starts.
//!
//! ```text
//! console-web           pack it if anything has changed
//! console-web --always  pack it whether or not anything has changed
//! ```
//!
//! Run by `console apply`, in the same breath as the browsers are told which
//! engine to ask. Everything it touches is under her home -- it reads the
//! palette out of the profile and writes the add-on back into the same profile
//! -- so it is run as her and not as root.
//!
//! It does nothing at all on a machine where nothing has changed. A browser
//! installs an add-on by version and reinstalls it when the version goes up, so
//! packing the same files again under a new number every apply would be the
//! browser taking a new add-on every time the machine was told to catch up.

use std::path::Path;
use std::process::ExitCode;

use console_browser_extension::{PALETTE, source, stamp};
use console_core_arguments::{Command, Flag, Operands, Presence, Takes, read};
use console_core_atomic_writes::Stored;
use console_core_never::Never;

const ALWAYS: Flag = Flag {
  spelling: "--always",
  takes: Takes::None,
  about: "pack it whether or not anything has changed",
};

const COMMAND: Command = Command {
  name: "console-web",
  about: "pack the add-on this desktop puts in its browser into the profile, if anything has changed",
  flags: &[ALWAYS],
  operands: Operands::None,
};

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let line = match read(&COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let Ok(always) = line.presence(ALWAYS);

  let Ok(said) = console_core_places::home();

  let home = match said {
    Some(home) => home,
    None => {
      eprintln!("no HOME, so there is no one whose add-on this would be");

      return ExitCode::from(1);
    }
  };

  let at = home.join(PALETTE);

  let original = match std::fs::read_to_string(&at) {
    Ok(css) => css,
    Err(_unreadable) => {
      eprintln!("{}: no palette to dress the add-on in", at.display());
      return ExitCode::from(1);
    }
  };

  let Ok(dressed) = source::with_host_selector(&original);

  let palette = match dressed {
    Some(css) => css,
    None => {
      eprintln!("{}: not a palette this can read", at.display());
      return ExitCode::from(1);
    }
  };

  let Ok(xpi) = console_browser_extension::xpi(&home);
  let Ok(stamped) = console_browser_extension::stamp(&home);
  let Ok(hash) = source::hash(&palette);
  let Ok(held) = note_beside(&stamped);

  let packed_already = always == Presence::Absent
    && match std::fs::File::open(&xpi).and_then(|file| file.metadata()) {
      Ok(info) => info.is_file(),
      Err(_packed_extension_unavailable) => false,
    }
    && held.as_ref().is_some_and(|held_2| held_2.hash == hash);

  match ::core::primitive::bool::then_some(packed_already, ()) {
    ::core::option::Option::Some(()) => {
      println!("{}: already packed", xpi.display());
      return ExitCode::SUCCESS;
    }
    ::core::option::Option::None => {}
  }

  let was = held.map(|held_3| held_3.version).or_else(|| {
    let Ok(packed) = packed_version(&xpi);

    packed
  });

  let Ok(version) = stamp::next(was.as_deref());
  let Ok(files) = source::every(&version, source::Palette(&palette));
  let Ok(made) = console_browser_extension::pack::zip(&files);

  match console_core_atomic_writes::whole_with_folders(&xpi, &made) {
    Ok(()) => {}
    Err(why) => {
      eprintln!("{}: {why}", xpi.display());
      return ExitCode::from(1);
    }
  }

  let Ok(note) = stamp::format_stamp(&stamp::Stamp {
    hash,
    version: version.clone(),
  });

  match console_core_atomic_writes::whole_with_folders(&stamped, note.as_bytes()) {
    Ok(()) => {}
    Err(why) => {
      eprintln!("{}: {why}", stamped.display());
      return ExitCode::from(1);
    }
  }

  println!("{}: packed, version {version}", xpi.display());
  ExitCode::SUCCESS
}

fn note_beside(at: &Path) -> Result<Option<stamp::Stamp>, Never> {
  let Ok(said) = console_core_atomic_writes::read(at);

  Ok(match said {
    Stored::Text(said_2) => {
      let Ok(held) = stamp::read(&said_2);

      held
    }
    Stored::Absent => None,

    Stored::Failed(fault) => {
      eprintln!(
        "{}: reading the note beside the add-on: {fault}",
        at.display()
      );
      None
    }
  })
}

fn packed_version(at: &Path) -> Result<Option<String>, Never> {
  Ok(match std::fs::read(at) {
    Ok(bytes) => {
      let Ok(said) = stamp::version_of(&bytes);

      said
    }
    Err(fault) => match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => {
        eprintln!(
          "{}: reading the packed add-on for its version: {fault}",
          at.display()
        );
        None
      }
    },
  })
}
