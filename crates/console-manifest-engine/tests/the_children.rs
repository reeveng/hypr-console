//! Nothing holds a child process without saying how long it means to hold it.
//!
//! The fault this is here for did not look like a fault for six days. A
//! settings panel opened in a nested desktop starts `pactl subscribe` to hear
//! about the volume, and a nested desktop is torn down rather than closed, so
//! the panel died on a path that never reached the line that stopped its
//! watcher. One orphan per run, sixty-five runs, and then pipewire-pulse --
//! which serves sixty-four clients and refuses the sixty-fifth -- turned away
//! every `pactl` on the machine. What that looked like from a keyboard was the
//! volume keys not working.
//!
//! The panel's mistake is not visible at the line that made it. `spawn` hands
//! back a `Child`, a `Child` that is dropped is not killed, and a field holding
//! one says nothing about whether that is meant. The lifetime crates have
//! the answers as types -- `BoundToParent`, which dies with whoever started it,
//! `Detached`, which is meant not to, and `Forked`, which dies with whoever
//! started it, takes what it started along, and can be interrupted by a value
//! -- and every one of them survives the paths no one thought about, because
//! the kernel's signal and the drop are both under each. What this test does is
//! make them the only answers available: a `Child` named anywhere else is an
//! answer given by not answering.
//!
//! One thing it deliberately does not look at: it reads the type, not `spawn`.
//! A command that is spawned, read to the end and waited for inside one
//! function is over before that function returns and cannot be left behind --
//! `ffmpeg` decoding a wallpaper is that, and so is the `notify-send` that is
//! asked a question with a timeout on it. What is kept is what has to be
//! answered for, and keeping one means naming the type.

mod reading;

use reading::{Failure, missing, naming, read};

const EXCUSED: [&str; 6] = [
  "console-program-lifetime",
  "console-operating-system-linux",
  "console-test-desktop",
  "console-core-cascading-style-sheets",
  "console-core-iso-media-boxes",
  "console-core-video-decoding-matroska-container",
];

#[test]
fn nothing_holds_a_child_without_saying_how_long_for() -> Result<(), Failure> {
  let word = format!("{}{}", "Chi", "ld");
  let holding = naming(&word, &EXCUSED)?;

  assert!(
    holding.is_empty(),
    "these hold a process without saying whether it outlives them; use \
         the platform's BoundToParent or console_program_lifetime's Detached: {holding:?}"
  );

  Ok(())
}

#[test]
fn the_lifetime_types_still_declare_their_guarantees() -> Result<(), Failure> {
  let said = read("crates/console-program-lifetime/src/lib.rs")?;
  let native = read("crates/console-operating-system-linux/src/program_lifetime/mod.rs")?;
  let forks = read("crates/console-operating-system-linux/src/program_lifetime/forks.rs")?;

  assert!(
    native.contains("pub struct BoundToParent"),
    "the one that dies with us is gone"
  );
  assert!(
    said.contains("pub struct Detached"),
    "the one that is meant not to is gone"
  );
  assert!(
    forks.contains("pub struct Forked"),
    "the one that can be interrupted is gone"
  );
  assert!(
    native.contains("PR_SET_PDEATHSIG"),
    "the kernel's half is gone, and the drop is alone again"
  );

  Ok(())
}

#[test]
fn every_excused_crate_is_a_crate() {
  let Ok(missing) = missing(&EXCUSED);

  assert!(
    missing.is_empty(),
    "excused from a rule but not in the tree: {missing:?}"
  );
}
