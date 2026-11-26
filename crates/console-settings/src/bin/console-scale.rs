//! How big everything on the screen is.
//!
//!     console-scale                 which size the screen is standing at
//!     console-scale tiny|smaller|normal|bigger|huge
//!                                   put it there, and remember it
//!     console-scale left|upright|right|over
//!                                   which way up it stands, and remember that
//!     console-scale apply           wear what was remembered
//!
//! The size and the way up are one eval and not two. A density here is a rung
//! divided into the width of the panel, and turning the panel changes which of
//! its two sides that is -- so a turn that did not carry the size with it would
//! leave the desktop at a density no one chose, and the screen is described
//! whole or not at all.
//!
//! `apply` restores each display's saved zoom and rotation at login and after
//! the compositor reloads its configuration.
//!
//! The screen it is put on is the one the compositor says it is driving, and not
//! the one the tree declares. Those were the same thing for as long as there was
//! one machine: `declared()` reads a block written for a 1600x2560 panel turned
//! a quarter, and a laptop handed that through `eval` comes up rotated at two
//! and a half times the size, which is a session no one can use rather than a
//! size no one asked for. A machine with no compositor to ask is told so and
//! nothing is changed.
//!
//! **A rung is one screen's, in one shape.** Which of a panel's two sides is
//! its width is what a turn changes, so the same word is a different density
//! either way up and choosing a size after turning is saying what *this* way up
//! should be rather than correcting the other. The turn is read first, the turn
//! decides the shape, and the shape decides which rung is put on -- which is
//! also why turning back finds the size that was chosen there.
//!
//! A shape no one has chosen a size in yet wears the other shape's word rather
//! than the density that is already on the screen. A rung is a canvas across,
//! so carrying the word over is what keeps everything the same size to an eye
//! through a turn, and carrying the density over is what makes a turn quietly
//! change the size as well -- which is the thing the turn was described whole
//! in one eval to avoid. The word is the answer someone gave; the density is
//! arithmetic about a screen they have not seen yet.
//!
//! **An answer is one screen's.** A word typed here is about the panel the
//! compositor is driving -- the built-in one where there is one, which is the
//! screen someone holding a handheld is looking at -- and it is kept under
//! that screen's own connector by `console_settings::screens`. `apply` is the
//! other half: it walks every screen the compositor has, puts each one back to
//! what was remembered for it, and leaves alone the ones nothing was ever
//! chosen for. A machine that kept one word for the whole of itself put the
//! handheld's quarter turn on the monitor someone plugged into it, which is
//! not a setting behaving oddly -- it is one answer being asked of two
//! different questions.
//!
//! What `hyprctl` is told is `eval`, and that is not a preference. A
//! Lua-configured compositor answers `hyprctl keyword` with *"keyword can't
//! work with non-legacy parsers. Use eval."* -- the same trap the `screen.md`
//! note in Orbit describes for `dispatch`, where the command every example on
//! the internet gives comes back with a complaint and the only symptom is a
//! setting that does nothing.

use std::process::ExitCode;

use console_compositor::setup::{OutputRule, OutputScale, Outputs, TouchUpdate};
use console_core_arguments::{Flag, Operands, Subcommand, Takes, read_with};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_settings::screens::{Output, Unnamed};
use console_settings::size::{self, Size};
use console_settings::turning::{self, Turn};

const BAR: &str = "console-bar.service";

const HOME: &str = "console-home.service";

const APPLY: &str = "apply";

const OUTPUT: Flag = Flag {
  spelling: "--output",
  takes: Takes::Value("CONNECTOR"),
  about: "change this connected display's zoom or rotation",
};

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-scale",
  about: "how big everything on the screen is, and which way up it stands; with nothing, the size it is standing at",
  flags: &[OUTPUT],
  operands: Operands::None,
};

#[path = "console-scale/display.rs"]
mod display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wanted {
  Sized(Size),
  Turned(Turn),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scale {
  Wear(Wanted),
  Apply,
}

impl Subcommand for Scale {
  fn variants() -> Result<impl Iterator<Item = Self>, Never> {
    let sized = Size::VARIANTS.iter().copied().map(Wanted::Sized);
    let turned = Turn::VARIANTS.iter().copied().map(Wanted::Turned);

    Ok(sized.chain(turned).map(Scale::Wear).chain([Scale::Apply]))
  }

  fn spelling(self) -> Result<&'static str, Never> {
    match self {
      Scale::Wear(Wanted::Sized(size)) => size.written(),
      Scale::Wear(Wanted::Turned(turn)) => turn.written(),
      Scale::Apply => Ok(APPLY),
    }
  }

  fn about(self) -> Result<&'static str, Never> {
    Ok(match self {
      Scale::Wear(Wanted::Sized(_size)) => "draw everything at this size, and remember it",
      Scale::Wear(Wanted::Turned(_turn)) => "stand the screen this way up, and remember it",
      Scale::Apply => "wear what each screen remembers",
    })
  }
}

fn read_file(at: &std::path::Path) -> Result<String, Never> {
  Ok(match console_core_atomic_writes::text_or_empty(at) {
    Ok(message) => message,
    Err(fault) => {
      eprintln!("console-scale: {fault}");

      String::new()
    }
  })
}

fn standing() -> Result<ExitCode, Never> {
  let Ok(monitors) = query_monitors();

  let monitors_3 = match monitors {
    Some(monitors_2) => monitors_2,
    None => return Ok(ExitCode::FAILURE),
  };

  let Ok(standing) = size::current_size(&monitors_3);
  let Ok(shown) = console_screen::shown(&monitors_3);

  Ok(match (standing, shown) {
    (Some(size), _) => {
      let Ok(written) = size.written();

      println!("{written}");

      ExitCode::SUCCESS
    }
    (None, Some(screen)) => {
      println!("{}", screen.scale);

      ExitCode::SUCCESS
    }
    (None, None) => {
      eprintln!("console-scale: the compositor said nothing about a screen");

      ExitCode::FAILURE
    }
  })
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  match words.as_slice() {
    [command, arguments @ ..] => match command.as_str() {
      "display" => {
        let Ok(status) = display::run(arguments);
        return status;
      }
      _scale => {}
    },
    [] => {}
  }

  let (asked, connector) = match read_with::<Scale, String>(&COMMAND, &words) {
    Ok(line) => {
      let Ok(asked) = line.subcommand();
      let Ok(connector) = line.value(OUTPUT);

      (asked, connector.map(str::to_owned))
    }
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  let Ok(said) = console_core_places::home();

  let home = match said {
    Some(home) => home,
    None => {
      eprintln!("console-scale: no HOME, so there is no one to remember for");

      return ExitCode::FAILURE;
    }
  };

  let wanted = match asked {
    Some(Scale::Wear(wanted)) => wanted,
    Some(Scale::Apply) => {
      let Ok(gone) = applied(&home);

      return gone;
    }
    None => {
      let Ok(standing) = standing();

      return standing;
    }
  };

  let editing = match display::edit() {
    Ok(editing) => editing,
    Err(fault) => {
      let Ok(status) = display::failed(&fault.to_string());
      return status;
    }
  };
  let outputs = match console_compositor::ask(Outputs) {
    Ok(outputs) => outputs,
    Err(fault) => {
      let Ok(status) = display::failed(&fault.to_string());
      return status;
    }
  };

  let output = {
    let Ok(optional_output) = display::selected(&outputs, connector.as_deref());

    match optional_output {
      Some(matched_output) => matched_output,
      ::core::option::Option::None => {
        let Ok(status) = display::failed("the display is no longer connected");
        return status;
      }
    }
  };

  let screen = {
    let Ok(optional_screen) = display::screen(output);

    match optional_screen {
      Some(matched_screen) => matched_screen,
      ::core::option::Option::None => {
        let Ok(status) = display::failed("the display has no usable mode");
        return status;
      }
    }
  };

  let named = &output.connector;

  let wanted_turn = match wanted {
    Wanted::Turned(turn) => Some(turn),
    Wanted::Sized(_a_rung) => None,
  };

  let kept = match remembers(&home, Output(named), &screen, wanted_turn) {
    Ok(kept) => kept,
    Err(why) => {
      eprintln!("console-scale: {why}, so there is nowhere to remember this");

      return ExitCode::FAILURE;
    }
  };

  let (size, turn) = match wanted {
    Wanted::Sized(size) => (Some(size), kept.turn),
    Wanted::Turned(turn) => (kept.size, Some(turn)),
  };

  let (remembering, said_2) = match wanted {
    Wanted::Sized(size_2) => {
      let Ok(written) = size_2.written();

      (kept.size_at, written.to_string())
    }
    Wanted::Turned(turn_2) => {
      let Ok(written) = turn_2.written();

      (kept.turn_at, written.to_string())
    }
  };

  match remembering.parent() {
    Some(holding) => match std::fs::create_dir_all(holding) {
      Ok(()) => {}
      Err(fault) => {
        let Ok(status) = display::failed(&format!("{}: {fault}", holding.display()));
        return status;
      }
    },
    None => {}
  }

  let Ok(previous) = output.rule(&outputs);
  let Ok(rule) = configuration(previous, &screen, size, turn);

  match display::apply(rule, output.id) {
    Err(fault) => {
      let Ok(status) = display::restored(previous, rule, output.id, &fault.to_string());
      return status;
    }
    ::core::result::Result::Ok(_) => {}
  }

  match console_core_atomic_writes::whole(&remembering, format!("{said_2}\n").as_bytes()) {
    Err(fault) => {
      let Ok(status) = display::restored(previous, rule, output.id, &fault.to_string());
      return status;
    }
    ::core::result::Result::Ok(_) => {}
  }

  drop(editing);
  let Ok(mut asking) = Program::Systemctl.command();

  match asking.args(["--user", "restart", BAR, HOME]).status() {
    Ok(status) => match status.code() {
      Some(0) => {}
      Some(_) | None => eprintln!("console-scale: restarting {BAR} and {HOME} returned {status}"),
    },
    Err(fault) => eprintln!("console-scale: restarting {BAR} and {HOME} failed: {fault}"),
  }

  ExitCode::SUCCESS
}

struct Remembers {
  size: Option<Size>,
  turn: Option<Turn>,
  size_at: std::path::PathBuf,
  turn_at: std::path::PathBuf,
}

fn remembers(
  home: &std::path::Path,
  panel: Output<'_>,
  screen: &console_screen::Screen,
  wanted: Option<Turn>,
) -> Result<Remembers, Unnamed> {
  let turn_at = turning::at(home, panel)?;
  let Ok(written) = read_file(&turn_at);
  let Ok(turn) = Turn::of(&written);

  let standing = match wanted.or(turn) {
    Some(turn_2) => {
      let Ok(turned) = turning::rotate(screen, turn_2);

      turned
    }
    None => *screen,
  };
  let Ok(shape) = standing.shape();

  let size_at = size::at(home, panel, shape)?;
  let Ok(written_2) = read_file(&size_at);
  let Ok(said) = Size::of(&written_2);

  let size = match said {
    Some(size) => Some(size),
    None => {
      let Ok(other) = shape.other();
      let at = size::at(home, panel, other)?;
      let Ok(written_3) = read_file(&at);
      let Ok(said_2) = Size::of(&written_3);

      said_2
    }
  };

  Ok(Remembers {
    size,
    turn,
    size_at,
    turn_at,
  })
}

fn applied(home: &std::path::Path) -> Result<ExitCode, Never> {
  let _editing = match display::edit_for_restore() {
    Ok(_editing) => _editing,
    Err(fault) => {
      console_core_logging::warn!(
        "console-scale: could not restore saved display settings: {fault}"
      );
      return Ok(ExitCode::SUCCESS);
    }
  };

  let outputs = match console_compositor::ask(Outputs) {
    Ok(outputs) => outputs,
    Err(fault) => {
      eprintln!("console-scale: {fault}");
      return Ok(ExitCode::SUCCESS);
    }
  };

  for output in &outputs {
    let screen = {
      let Ok(optional_screen) = display::screen(output);

      match optional_screen {
        Some(matched_screen) => matched_screen,
        ::core::option::Option::None => continue,
      }
    };

    let kept = match remembers(home, Output(&output.connector), &screen, None) {
      Ok(kept) => kept,
      Err(fault) => {
        eprintln!("console-scale: {fault}");
        continue;
      }
    };

    match { ::core::primitive::bool::then_some(kept.size.is_none() && kept.turn.is_none(), ()) } {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    let Ok(previous) = output.rule(&outputs);
    let Ok(rule) = configuration(previous, &screen, kept.size, kept.turn);

    {
      match display::apply(rule, output.id) {
        Err(fault) => {
          let Ok(_) = display::restored(previous, rule, output.id, &fault.to_string());
        }
        ::core::result::Result::Ok(_) => {}
      };
    }
  }

  Ok(ExitCode::SUCCESS)
}

#[must_use]
fn configuration<'a>(
  previous: OutputRule<'a>,
  screen: &console_screen::Screen,
  size: Option<Size>,
  turn: Option<Turn>,
) -> core::result::Result<OutputRule<'a>, Never> {
  let standing = match turn {
    Some(turn_2) => {
      let Ok(turned) = turning::rotate(screen, turn_2);

      turned
    }
    None => *screen,
  };
  let scale = match size {
    Some(size_2) => {
      let Ok(scale) = size_2.scale_on(&standing);

      let Ok(adjusted) =
        console_settings::display::pixel_aligned_scale(previous.mode, OutputScale(scale));
      adjusted
    }
    None => OutputScale(standing.scale),
  };
  let Ok(transform) = standing.transform.number();
  Ok(OutputRule {
    scale,
    transform,
    touch: TouchUpdate::from(
      previous.connector.starts_with("eDP-") && transform != previous.transform,
    ),
    ..previous
  })
}

fn query_monitors() -> Result<Option<Vec<console_compositor::Monitor>>, Never> {
  Ok(
    match console_compositor::ask(console_compositor::Monitors) {
      Ok(monitors) => Some(monitors),
      Err(why) => {
        eprintln!("console-scale: {why}");

        None
      }
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_arguments::ValidationError;

  #[test]
  fn every_size_and_every_turn_is_a_word_it_reads() -> Result<(), ValidationError> {
    let sized = Size::VARIANTS.iter().copied().map(Wanted::Sized);
    let turned = Turn::VARIANTS.iter().copied().map(Wanted::Turned);

    for wanted in sized.chain(turned) {
      let Ok(word) = Scale::Wear(wanted).spelling();
      let line = read_with::<Scale, &str>(&COMMAND, &[word]);
      let read = line?;

      assert_eq!(read.subcommand(), Ok(Some(Scale::Wear(wanted))), "{word}");
    }

    Ok(())
  }
}
