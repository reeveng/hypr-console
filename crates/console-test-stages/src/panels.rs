//! One panel, opened alone in the nested desktop and asked what it drew.
//!
//! The other stages here are about the machine: is the desktop up, what color
//! is the screen, did the daemon run the thing the button says. This one is
//! about a surface, and it is the tier a change to a panel is tried in while it
//! is being written -- one program, one compositor, a few seconds, and no
//! device.
//!
//! What it can answer is the question nothing here could answer before: not *is
//! it drawn* but *could a hand use it*. The panel writes down every part of
//! itself a hand could land on and where, `console_panel::description` reads it
//! back, and a check holds that against what the rows were built to offer. The
//! two faults it was written for are the two it catches without pressing
//! anything at all: a row that offers something behind Y and draws no mark for
//! it, and a mark drawn at a place outside the room the compositor granted.
//!
//! Pressing is the second half and goes through `console-point`, the same way
//! the desktop stage's own does. A press is not finished when the key has been
//! sent: it is finished when the panel has drawn what the key did, so that is
//! what is waited for. This used to be three numbers -- sleep two and a half
//! seconds for the panel to come up, six tenths between presses, a second and
//! a half at the end -- and every one of them was a guess about a machine with
//! nothing else running on it.
//!
//! `console-desktop pressing` is the wait said once. It takes the file the
//! panel writes a line to on every draw, waits for a line to be in it, counts
//! the inputs already reported, presses, and waits for the panel to describe
//! their result. `--press` keeps the nested desktop open until the chain ends
//! rather than until a number of seconds has gone by. Nothing here sleeps now,
//! and a run on a loaded machine is slower rather than red.
//!
//! **One nested desktop at a time, and the lock is how.** Every check here
//! wants a compositor of its own, and for a long time they were let up a few at
//! a time -- a share of the cores, with a floor under it. What that bought was
//! the viewer's tier failing a different handful of its checks every run, on a
//! laptop with nothing else running, and passing all of them every time under
//! `--test-threads=1`. The failures read as the panel: the key reached nothing,
//! the page was never arrived at, a picture opened over the whole screen drew
//! no way out. What was happening is that a panel's first frame is drawn before
//! what it is going to show has arrived -- a folder is walked, a picture is
//! decoded -- and with compositors racing each other for one machine's graphics
//! that gap grows until the press lands in a card that is still empty, which
//! draws a frame and changes nothing at all. A press that waits for the line
//! saying the thing the check is about was tried against the same load and is
//! not enough: the second compositor is the whole of it.
//!
//! So a tier queues on one lock. It costs the wall clock several times over and
//! it is the only reason any of it means anything: a tier that is red for the
//! machine it ran on is worse than no tier. Why this was tried once before and
//! looked like it failed is worth keeping -- the share was lowered and the
//! floor underneath it was not, so compositors went on racing in pairs and the
//! checks went on failing.
//!
//! **A panel binary is not a dependency of the check that drives it**, so
//! `cargo test -p console-application-media-viewer` builds today's expectations and runs
//! them against whatever binary was left in `target/` -- and a panel a library
//! change has not reached fails in ways that read as the check being wrong.
//! That is what `built_since_the_panel_code` refuses, and what it compares
//! against is the library and the program's own file. The other programs under
//! `console-panel/src/bin` are not in it: they are separate binaries that
//! cannot change what this one draws, and counting them meant that editing the
//! bar's door stopped every panel check in the tree until someone rebuilt the
//! world.
//!
//! **What runs is the staged copy, found on the stage's own path.** A panel
//! draws itself now, and a surface that draws itself reads the palette out of
//! the tree it is installed in -- `console_core_color::palette::beside`, which
//! is the argument for why a stage differs from the device in where it is and
//! in nothing else. Opening `target/debug/console-application-files` by its absolute path
//! walks up from `target/` into a directory that has no `usr/local`, finds no
//! palette, and the panel says so on stderr and draws nothing at all. The
//! toolkit never met this because its colors were compiled into a stylesheet.
//! The binary in `target/` is still what is checked for staleness: it is the
//! same file, cloned into the stage.
//!
//! ```no_run
//! # use console_test_stages::panels::Panel;
//! let Ok(mut panel) = Panel::opening("console-application-media-viewer", &["/home/me/Pictures/beach.jpg"]);
//! let drawn = panel.descriptions().expect("the viewer drew nothing");
//!
//! for card in &drawn {
//!     console_test_stages::panels::every_offer_answered(card).expect("a finger can reach it");
//! }
//! ```

use rustix::fs::flock;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_directory_listing::Descend;
use console_core_geometry::Point;
use console_core_never::Never;
use console_panel::description::{self, Bare, Description, Line, Offers, Reachable, Spot};
use console_repository::RelativePath;

use crate::{Error, RuntimeName, missing_is_removed, remove_scratch};

struct Room(Option<std::fs::File>);

const ONE_AT_A_TIME: &str = "console-panel-stage.lock";

impl Room {
  fn for_one_more() -> Result<Room, Never> {
    let Ok(runtime) = console_core_places::runtime_or_user();
    let at = runtime.join(ONE_AT_A_TIME);

    #[cfg_attr(
      dylint_lib = "explicit040_no_torn_write",
      allow(
        explicit040_no_torn_write,
        reason = "the lock two check runs queue on, whose whole point is the open file and not its bytes"
      )
    )]
    let held = match std::fs::OpenOptions::new()
      .create(true)
      .write(true)
      .truncate(false)
      .open(&at)
    {
      Ok(file) => Some(file),
      Err(fault) => {
        eprintln!("console-test-stages: {}: {fault}", at.display());

        None
      }
    };

    match &held {
      Some(file) => match flock(file, rustix::fs::FlockOperation::LockExclusive) {
        Ok(()) => {}
        Err(fault) => {
          eprintln!(
            "console-test-stages: {}: waiting for the lock: {fault}",
            at.display()
          );
        }
      },
      None => {}
    }

    Ok(Room(held))
  }
}

impl Drop for Room {
  fn drop(&mut self) {
    match &self.0 {
      Some(file) => match flock(file, rustix::fs::FlockOperation::Unlock) {
        Ok(()) => {}
        Err(fault) => {
          eprintln!("console-test-stages: the lock this run took would not come off: {fault}");
        }
      },
      None => {}
    }
  }
}

const PROGRAMS: &str = "bin";

fn built_since_the_panel_code(program: &Path) -> Result<(), Error> {
  let when = |at: &Path| -> Option<std::time::SystemTime> {
    let about = match at.metadata() {
      Ok(about) => about,
      Err(_unreadable) => return None,
    };

    match bool::then_some(about.is_file(), ()) {
      Some(()) => {}
      None => return None,
    }

    let when = match about.modified() {
      Ok(when) => when,
      Err(_unstamped) => return None,
    };

    Some(when)
  };

  let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent();
  let library = crates.map(|at| at.join("console-panel").join("src"));

  let library_3 = match library {
    Some(library_2) => library_2,
    None => return Ok(()),
  };

  let Ok(listing) = console_core_directory_listing::recursive(&library_3, |at| {
    match ::core::primitive::bool::then_some(
      at.file_name().is_some_and(|named| named == PROGRAMS),
      (),
    ) {
      ::core::option::Option::Some(()) => Descend::Past,
      ::core::option::Option::None => Descend::Into,
    }
  });

  let mut newest = listing
    .filter_map(|entry| match entry {
      Ok(at) => Some(at),
      Err(_unreadable) => None,
    })
    .filter(|at| at.extension().is_some_and(|it| it == "rs" || it == "css"))
    .filter_map(|at| when(&at))
    .max();

  let its_own = program
    .file_name()
    .and_then(|named| match RelativePath::new(Path::new(named)) {
      Ok(source) => Some(library_3.join(PROGRAMS).join(source).with_extension("rs")),
      Err(_no_source_name) => None,
    });

  match its_own {
    Some(at) => newest = newest.max(when(&at)),
    None => {}
  }

  match (newest, when(program)) {
    (Some(edited), Some(built)) => match ::core::primitive::bool::then_some(built < edited, ()) {
      ::core::option::Option::Some(()) => Err(Error::Stale(program.to_path_buf())),
      ::core::option::Option::None => Ok(()),
    },
    (Some(_), None) | (None, Some(_)) | (None, None) => Ok(()),
  }
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "each panel this process opens needs a directory no other one is using, and checks are separate functions run beside each other with nothing above them to do the counting"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "parallel panel stage instances in one process use distinct scratch paths under that process user runtime"
  )
)]
static ONE_AFTER_ANOTHER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub struct Panel {
  program: String,
  arguments: Vec<String>,
  presses: Vec<Press>,
  here: PathBuf,
  read: Option<Vec<Description>>,
}

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
struct Press {
  command: String,
  count: u64,
}

impl Panel {
  pub fn opening(program: &str, arguments: &[&str]) -> Result<Panel, Never> {
    let mine = ONE_AFTER_ANOTHER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let Ok(runtime) = console_core_places::runtime_or_user();
    let name = RuntimeName::panel(mine)?;
    let here = runtime.join(name);

    Ok(Panel {
      program: program.to_string(),
      arguments: arguments.iter().map(|said| (*said).to_string()).collect(),
      presses: Vec::new(),
      here,
      read: None,
    })
  }

  pub fn press(&mut self, card: &Description, spot: &Spot) -> Result<(), Error> {
    let Ok((across, down)) = spot.middle();

    match ::core::primitive::bool::then_some(across >= 0 && down >= 0, ()) {
      ::core::option::Option::Some(()) => {
        self.presses.push(Press {
          command: format!("console-point --in {} {across} {down} --click", card.panel),
          count: 1,
        });

        Ok(())
      }
      ::core::option::Option::None => Err(Error::NotInside(
        Point { x: across, y: down },
        card.panel.clone(),
      )),
    }
  }

  pub fn key(&mut self, key: &str) -> Result<(), Error> {
    self.keys(&[key])
  }

  pub fn keys(&mut self, keys: &[&str]) -> Result<(), Error> {
    match ::core::primitive::bool::then_some(
      keys.is_empty() || keys.iter().any(|key| key.is_empty()),
      (),
    ) {
      ::core::option::Option::Some(()) => Err(Error::UnnamedKey),
      ::core::option::Option::None => {
        let Ok(count) = console_core_number_conversion::fitted(keys.len());
        self.presses.push(Press {
          command: format!("wtype -k {}", keys.join(" -k ")),
          count,
        });

        Ok(())
      }
    }
  }

  fn script(&self, told: &Path) -> Result<Option<String>, Never> {
    let Ok(desktop) = console_core_internal_programs::beside_this_program("console-desktop");

    let every: Vec<String> = self
      .presses
      .iter()
      .map(|press| {
        format!(
          "{} pressing {} --press-count {} --then '{}'",
          desktop.display(),
          told.display(),
          press.count,
          press.command
        )
      })
      .collect();

    Ok(
      match ::core::primitive::bool::then_some(every.is_empty(), ()) {
        ::core::option::Option::Some(()) => None,
        ::core::option::Option::None => Some(every.join(" && ")),
      },
    )
  }

  pub fn descriptions(&mut self) -> Result<Vec<Description>, Error> {
    match &self.read {
      Some(read) => return Ok(read.clone()),
      None => {}
    }

    let Ok(_room) = Room::for_one_more();
    let Ok(program) = console_core_internal_programs::beside_this_program(&self.program);

    let built = std::fs::metadata(&program).is_ok_and(|entry| entry.is_file());

    match bool::then_some(built, ()) {
      Some(()) => built_since_the_panel_code(&program)?,
      None => return Err(Error::NotBuilt(program)),
    }

    std::fs::create_dir_all(&self.here).map_err(Error::Machine)?;

    let told = self.here.join("told.jsonl");
    missing_is_removed(std::fs::remove_file(&told)).map_err(Error::Machine)?;

    let opening = format!(
      "CONSOLE_PANEL_TELLS={} {} {}",
      told.display(),
      self.program,
      self.arguments.join(" ")
    );

    let Ok(desktop) = console_core_internal_programs::beside_this_program("console-desktop");
    let mut nesting = Command::new(desktop);
    nesting.arg("describe");

    nesting.arg("--bare");
    nesting.args(["--open", &opening]);
    nesting.args(["--until", &told.display().to_string()]);

    let Ok(script) = self.script(&told);

    match script {
      Some(script_2) => {
        nesting.args(["--press", &script_2]);
      }
      None => {}
    }

    let said = nesting.output().map_err(Error::Machine)?;

    match ::core::primitive::bool::then_some(said.status.success(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        let Ok(why) = why(&said.stderr);

        return Err(Error::NotNested(self.program.clone(), why));
      }
    }

    let read = std::fs::read_to_string(&told).map_err(|fault| {
      let Ok(why) = why(&said.stderr);

      Error::SaidNothingDrawn(self.program.clone(), fault, why)
    })?;

    let every = description::every(&read)?;

    match ::core::primitive::bool::then_some(every.is_empty(), ()) {
      ::core::option::Option::Some(()) => Err(Error::DrewNothing(self.program.clone())),
      ::core::option::Option::None => {
        self.read = Some(every.clone());

        Ok(every)
      }
    }
  }
}

fn why(said: &[u8]) -> Result<String, Never> {
  let heard_before = |line: &&str| {
    !line.is_empty()
      && !line.starts_with("> ")
      && !line.starts_with("(EE) ")
      && !line.starts_with("(WW) ")
      && !line.starts_with("(II) ")
      && !line.starts_with("The XKEYBOARD keymap compiler")
      && !line.starts_with("Errors from xkbcomp")
      && !line.starts_with("unable to lock lockfile")
      && !line.starts_with("Hyprland already running")
  };

  let all = match std::str::from_utf8(said) {
    Ok(text) => text,
    Err(fault) => return Ok(format!("the nested panel printed invalid UTF-8: {fault}")),
  };
  let worth: Vec<&str> = all
    .lines()
    .map(str::trim_end)
    .filter(heard_before)
    .collect();

  let Ok(many) = console_core_number_conversion::fitted::<_, u32>(worth.len());
  let Ok(from) = console_core_number_conversion::index(many.saturating_sub(TOLD_LINES));

  Ok(match worth.get(from..) {
    Some(said_2) => said_2.join("\n"),
    None => "and nothing to say about why".to_string(),
  })
}

const TOLD_LINES: u32 = 12;

impl Drop for Panel {
  fn drop(&mut self) {
    let Ok(()) = remove_scratch(&self.here);
  }
}

pub fn every_offer_answered(card: &Description) -> Result<(), Error> {
  let missing: Vec<String> = card
    .lines
    .iter()
    .filter(|line| line.offers == Offers::Yes && line.bare == Bare::No)
    .filter(|line| {
      let Ok(worn) = line.spot("else");

      worn.is_none()
    })
    .map(|line| {
      let Ok(said) = said_of(line);

      said
    })
    .collect();

  match ::core::primitive::bool::then_some(missing.is_empty(), ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(Error::OfferUnanswered(card.panel.clone(), missing)),
  }
}

pub fn one_mark_for_one_subject(card: &Description) -> Result<(), Error> {
  let Ok(marks) = console_core_number_conversion::fitted::<_, u32>(
    card
      .lines
      .iter()
      .filter(|line| {
        let Ok(worn) = line.spot("else");

        worn.is_some()
      })
      .count(),
  );
  let offered = card.lines.iter().any(|line| line.offers == Offers::Yes);

  match (offered, marks) {
    (false, 0) | (true, 1) => Ok(()),
    (false, _) => Err(Error::MarkWithoutOffer(card.panel.clone())),
    (true, _) => Err(Error::TooManyMarks(card.panel.clone(), marks)),
  }
}

pub fn every_mark_reachable(card: &Description) -> Result<(), Error> {
  let Ok(every) = card.every_spot();
  let off: Vec<String> = every
    .into_iter()
    .filter(|spot| {
      let Ok(reachable) = description::reachable(spot, card.room);

      reachable == Reachable::No
    })
    .map(|spot| format!("{} at {:?} of {:?}", spot.name, spot.at, spot.big))
    .collect();

  match ::core::primitive::bool::then_some(off.is_empty(), ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(Error::OutOfReach(card.panel.clone(), card.room, off)),
  }
}

pub fn a_way_out_is_drawn(card: &Description) -> Result<(), Error> {
  let Ok(worn) = card.spot("shut");

  match worn {
    Some(_) => Ok(()),
    None => Err(Error::NoWayOut(card.panel.clone(), card.tab.clone())),
  }
}

pub fn every_row_draws_what_it_carries(card: &Description) -> Result<(), Error> {
  let missing: Vec<String> = card
    .lines
    .iter()
    .flat_map(|line| {
      let Ok(said) = said_of(line);
      let words = match (line.says.is_empty(), line.drew.is_empty()) {
        (false, true) => Some(format!("{said} draws none of its words")),
        (true, _) | (false, false) => None,
      };

      let drew: std::collections::BTreeSet<&String> = line.drew.iter().collect();
      let cells = line
        .cells
        .iter()
        .filter(move |cell| !drew.contains(cell))
        .map(move |cell| format!("{said} draws no cell {cell:?}"));

      words.into_iter().chain(cells.collect::<Vec<_>>())
    })
    .collect();

  match ::core::primitive::bool::then_some(missing.is_empty(), ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(Error::Hidden(card.panel.clone(), missing)),
  }
}

fn said_of(line: &Line) -> Result<String, Never> {
  Ok(
    match ::core::primitive::bool::then_some(line.says.is_empty(), ()) {
      ::core::option::Option::Some(()) => format!("row {}", line.at),
      ::core::option::Option::None => format!("row {} ({})", line.at, line.says),
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  fn spot(name: &str, at: (i32, i32), big: (i32, i32)) -> Result<Spot, Never> {
    Ok(Spot {
      name: name.to_string(),
      at,
      big,
      scrolls: description::Scrolls::No,
    })
  }

  fn line(at: u32, offers: Offers, bare: Bare, spots: Vec<Spot>) -> Result<Line, Never> {
    Ok(Line {
      at,
      says: String::new(),
      aside: String::new(),
      offers,
      bare,
      heading: console_panel::description::Heading::No,
      standing: console_panel::description::Standing::No,
      spots,
      cells: Vec::new(),
      drew: Vec::new(),
    })
  }

  fn week(cells: &[&str], drew: &[&str]) -> Result<Line, Never> {
    let Ok(line) = line(0, Offers::No, Bare::No, Vec::new());

    Ok(Line {
      cells: cells.iter().map(|cell| (*cell).to_string()).collect(),
      drew: drew.iter().map(|word| (*word).to_string()).collect(),
      ..line
    })
  }

  fn card(lines: Vec<Line>, spots: Vec<Spot>) -> Result<Description, Never> {
    Ok(Description {
      panel: "a-panel".to_string(),
      tab: "One".to_string(),
      out: description::Output::No,
      room: (1024, 600),
      spots,
      lines,
      presses: 0,
      keyboard: description::KeyboardFocus::Here,
    })
  }

  fn shut_at(at: (i32, i32)) -> Result<Description, Never> {
    let Ok(shut) = spot("shut", at, (56, 44));

    card(Vec::new(), vec![shut])
  }

  #[test]
  fn a_week_whose_days_are_carried_and_not_drawn_is_the_fault() {
    let Ok(week) = week(&["1", "2", "3"], &[]);
    let Ok(card) = card(vec![week], Vec::new());

    assert!(matches!(
      every_row_draws_what_it_carries(&card),
      Err(crate::Error::Hidden(..))
    ));
  }

  #[test]
  fn a_week_that_draws_every_day_it_carries_holds() -> Result<(), Failure> {
    let Ok(week) = week(&["1", "2", "3"], &["1", "2", "3"]);
    let Ok(card) = card(vec![week], Vec::new());

    every_row_draws_what_it_carries(&card)?;

    Ok(())
  }

  #[test]
  fn a_row_whose_words_never_reach_the_screen_is_the_fault() {
    let Ok(line) = line(0, Offers::No, Bare::No, Vec::new());
    let quiet = Line {
      says: "Wi-Fi".to_string(),
      ..line
    };
    let Ok(card) = card(vec![quiet], Vec::new());

    assert!(matches!(
      every_row_draws_what_it_carries(&card),
      Err(crate::Error::Hidden(..))
    ));
  }

  #[test]
  fn a_row_offering_something_with_no_mark_on_it_is_the_fault() {
    let Ok(offering) = line(0, Offers::Yes, Bare::No, Vec::new());
    let Ok(card) = card(vec![offering], Vec::new());

    assert!(matches!(
      every_offer_answered(&card),
      Err(crate::Error::OfferUnanswered(..))
    ));
  }

  #[test]
  fn the_mark_may_be_on_the_row_beside_the_one_that_offers() -> Result<(), Failure> {
    let Ok(bare) = line(0, Offers::Yes, Bare::Yes, Vec::new());
    let Ok(mark) = spot("else", (900, 300), (40, 30));
    let Ok(marked) = line(1, Offers::Yes, Bare::No, vec![mark]);
    let Ok(card) = card(vec![bare, marked], Vec::new());

    every_offer_answered(&card)?;
    one_mark_for_one_subject(&card)?;

    Ok(())
  }

  #[test]
  fn a_card_with_one_subject_and_a_mark_on_every_line_is_a_crowd() {
    let Ok(first) = spot("else", (900, 300), (40, 30));
    let Ok(second) = spot("else", (900, 360), (40, 30));
    let Ok(one) = line(0, Offers::Yes, Bare::No, vec![first]);
    let Ok(other) = line(1, Offers::Yes, Bare::No, vec![second]);
    let Ok(card) = card(vec![one, other], Vec::new());

    assert!(matches!(
      one_mark_for_one_subject(&card),
      Err(crate::Error::TooManyMarks(_, 2))
    ));
  }

  #[test]
  fn a_panel_that_offers_nothing_needs_no_mark() -> Result<(), Failure> {
    let Ok(plain) = line(0, Offers::No, Bare::No, Vec::new());
    let Ok(card) = card(vec![plain], Vec::new());

    every_offer_answered(&card)?;
    one_mark_for_one_subject(&card)?;

    Ok(())
  }

  #[test]
  fn a_mark_hanging_off_the_room_is_a_mark_no_one_can_press() -> Result<(), Failure> {
    let Ok(off) = shut_at((982, 14));
    let Ok(on) = shut_at((954, 14));

    assert!(matches!(
      every_mark_reachable(&off),
      Err(crate::Error::OutOfReach(..))
    ));

    every_mark_reachable(&on)?;

    Ok(())
  }

  #[test]
  fn a_card_with_no_way_out_is_a_card_a_finger_is_shut_into() -> Result<(), Failure> {
    let Ok(shut_in) = card(Vec::new(), Vec::new());
    let Ok(way_out) = shut_at((954, 14));

    assert!(matches!(
      a_way_out_is_drawn(&shut_in),
      Err(crate::Error::NoWayOut(..))
    ));

    a_way_out_is_drawn(&way_out)?;

    Ok(())
  }

  #[test]
  fn a_press_is_aimed_inside_the_panel_and_says_which_panel() -> Result<(), Failure> {
    let Ok(mut panel) = Panel::opening("a-panel", &[]);
    let Ok(card) = shut_at((954, 14));
    let Ok(worn) = card.spot("shut");
    let shut = worn.ok_or("a way out to press")?;

    panel.press(&card, shut)?;

    assert_eq!(
      panel.presses,
      vec![Press {
        command: "console-point --in a-panel 982 36 --click".to_string(),
        count: 1,
      }]
    );

    Ok(())
  }

  #[test]
  fn a_press_at_a_place_outside_the_panel_is_refused() -> Result<(), Failure> {
    let Ok(mut panel) = Panel::opening("a-panel", &[]);
    let Ok(card) = shut_at((-80, 14));
    let Ok(worn) = card.spot("shut");
    let shut = worn.ok_or("a way out to press")?;

    assert!(matches!(
      panel.press(&card, shut),
      Err(crate::Error::NotInside(..))
    ));

    Ok(())
  }
}
