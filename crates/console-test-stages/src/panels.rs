//! One panel, opened alone in the nested desktop and asked what it drew.
//!
//! The other stages here are about the machine: is the desktop up, what colour
//! is the screen, did the daemon run the thing the button says. This one is
//! about a surface, and it is the tier a change to a panel is tried in while it
//! is being written -- one program, one compositor, a few seconds, and no
//! device.
//!
//! What it can answer is the question nothing here could answer before: not *is
//! it drawn* but *could a hand use it*. The panel writes down every part of
//! itself a hand could land on and where, `console_panel::telling` reads it
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
//! nothing else running on it. Six panel checks run beside each other here,
//! each in a nested compositor of its own, queued a few at a time, so the
//! machine a check runs on is always busy and the guess was wrong often enough
//! to read as the panel being broken: the key went nowhere because nothing was
//! up to receive it, and the check said the mark was never drawn.
//!
//! `console-desktop pressing` is the wait said once. It takes the file the
//! panel writes a line to on every draw, waits for a line to be in it, counts
//! them, presses, and returns when there is one more -- and `--press` is the
//! nested desktop holding itself open until that whole chain has finished
//! rather than until a number of seconds has gone by. Nothing here sleeps now,
//! and a run on a loaded machine is slower rather than red.
//!
//! **A panel binary is not a dependency of the check that drives it**, so
//! `cargo test -p console-media-viewer` builds today's expectations and runs
//! them against whatever binary was left in `target/` -- and a panel a library
//! change has not reached fails in ways that read as the check being wrong.
//! That is what `built_since_the_panel_code` refuses, and what it compares
//! against is the library and the program's own file. The other programs under
//! `console-panel/src/bin` are not in it: they are separate binaries that
//! cannot change what this one draws, and counting them meant that editing the
//! bar's door stopped every panel check in the tree until somebody rebuilt the
//! world.
//!
//! ```no_run
//! # use console_test_stages::panels::Panel;
//! let Ok(mut panel) = Panel::opening("viewer-panel", &["/home/me/Pictures/beach.jpg"]);
//! let drawn = panel.drawn().expect("the viewer drew nothing");
//!
//! for card in &drawn {
//!     console_test_stages::panels::every_offer_answered(card).expect("a finger can reach it");
//! }
//! ```

use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_panel::telling::{self, Bare, Line, Offers, Reachable, Spot, Told};

use crate::Awry;

const THE_ONLY_ONE: usize = 0;

struct Room(Option<std::fs::File>);

fn how_many() -> Result<usize, Never> {
    let all = std::thread::available_parallelism().map_or(SOME, std::num::NonZero::get);

    Ok((all / A_SHARE).max(SOME))
}

const A_SHARE: std::num::NonZeroUsize = match std::num::NonZeroUsize::new(4) {
    Some(share) => share,
    None => std::num::NonZeroUsize::MIN,
};

const SOME: usize = 2;

impl Room {
    fn for_one_more(mine: u32) -> Result<Room, Never> {
        let Ok(mine) = fitted::<u32, usize>(mine);
        let Ok(how_many) = how_many();

        let Ok(round) = console_core_walking::Ring::round(how_many);

        let slot = match round {
            Some(ring) => {
                let Ok(slot) = ring.at(mine);

                slot
            }
            None => THE_ONLY_ONE,
        };
        let at = std::env::temp_dir().join(format!("console-panel-stage-{slot}.lock"));

        #[cfg_attr(
            dylint_lib = "explicit040_no_torn_write",
            allow(
                explicit040_no_torn_write,
                reason = "the lock two check runs queue on, whose whole point is the open file and not its bytes"
            )
        )]
        let held = match std::fs::OpenOptions::new().create(true).write(true).truncate(false).open(&at) {
            Ok(file) => Some(file),
            Err(fault) => {
                eprintln!("console-test-stages: {}: {fault}", at.display());

                None
            }
        };

        match &held {
            Some(file) => {
                // SAFETY: the descriptor is this file's, and open for the call.
                unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
            }
            None => {},
        }

        Ok(Room(held))
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        match &self.0 {
            Some(file) => {
                // SAFETY: as above, and this is the handle that took it.
                unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
            }
            None => {},
        }
    }
}

const PROGRAMS: &str = "bin";

fn built_since_the_panel_code(program: &Path) -> Result<(), Awry> {
    let when = |at: &Path| -> Option<std::time::SystemTime> {
        let about = match at.metadata() {
            Ok(about) => about,
            Err(_fault) => return None,
        };

        let when = match about.modified() {
            Ok(when) => when,
            Err(_fault) => return None,
        };

        Some(when)
    };

    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent();
    let library = crates.map(|at| at.join("console-panel").join("src"));

    let library = match library {
        Some(library) => library,
        None => return Ok(()),
    };

    let mut newest = None;
    let mut look = vec![library.clone()];

    while let Some(at) = look.pop() {
        let entries = match std::fs::read_dir(&at) {
            Ok(entries) => entries,
            Err(_fault) => continue,
        };

        for found in entries.flatten() {
            let at = found.path();

            match at.is_dir() {
                true => match at.file_name().is_some_and(|named| named == PROGRAMS) {
                    true => {},
                    false => look.push(at),
                },
                false => {
                    let named = at.extension().is_some_and(|it| it == "rs" || it == "css");

                    match named {
                        true => newest = newest.max(when(&at)),
                        false => {},
                    }
                }
            }
        }
    }

    let its_own = program
        .file_name()
        .map(|named| library.join(PROGRAMS).join(named).with_extension("rs"));

    match its_own {
        Some(at) => newest = newest.max(when(&at)),
        None => {},
    }

    match (newest, when(program)) {
        (Some(edited), Some(built)) if built < edited => {
            Err(Awry::Stale(program.to_path_buf()))
        }
        _ => Ok(()),
    }
}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "each panel this process opens needs a directory no other one is using, and checks are separate functions run beside each other with nothing above them to do the counting"
    )
)]
static ONE_AFTER_ANOTHER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub struct Panel {
    program: String,
    mine: u32,
    args: Vec<String>,
    presses: Vec<String>,
    here: PathBuf,
    read: Option<Vec<Told>>,
}

impl Panel {
    pub fn opening(program: &str, args: &[&str]) -> Result<Panel, Never> {
        let mine = ONE_AFTER_ANOTHER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let here = std::env::temp_dir()
            .join(format!("console-panel-{program}-{}-{mine}", std::process::id()));

        Ok(Panel {
            program: program.to_string(),
            mine,
            args: args.iter().map(|said| (*said).to_string()).collect(),
            presses: Vec::new(),
            here,
            read: None,
        })
    }

    pub fn press(&mut self, card: &Told, spot: &Spot) -> Result<(), Awry> {
        let Ok((across, down)) = spot.middle();

        match across >= 0 && down >= 0 {
            true => {
                self.presses.push(format!(
                    "console-point --in {} {across} {down} --click",
                    card.panel
                ));

                Ok(())
            }
            false => Err(Awry::NotInside(
                Point { across, down },
                card.panel.clone(),
            )),
        }
    }

    pub fn key(&mut self, key: &str) -> Result<(), Awry> {
        match key.is_empty() {
            true => Err(Awry::NamelessKey),
            false => {
                self.presses.push(format!("wtype -k {key}"));

                Ok(())
            }
        }
    }

    fn script(&self, told: &Path) -> Result<Option<String>, Never> {
        let Ok(desktop) = crate::beside("console-desktop");

        let every: Vec<String> = self
            .presses
            .iter()
            .map(|command| {
                format!(
                    "{} pressing {} --then '{command}'",
                    desktop.display(),
                    told.display()
                )
            })
            .collect();

        Ok(match every.is_empty() {
            true => None,
            false => Some(every.join("; ")),
        })
    }

    pub fn drawn(&mut self) -> Result<Vec<Told>, Awry> {
        match &self.read {
            Some(read) => return Ok(read.clone()),
            None => {},
        }

        let Ok(_room) = Room::for_one_more(self.mine);
        let Ok(program) = crate::beside(&self.program);

        match program.is_file() {
            true => built_since_the_panel_code(&program)?,
            false => return Err(Awry::NotBuilt(program)),
        }

        std::fs::create_dir_all(&self.here).map_err(Awry::Machine)?;

        let told = self.here.join("told.jsonl");
        let _ = std::fs::remove_file(&told);

        let opening = format!(
            "CONSOLE_PANEL_TELLS={} {} {}",
            told.display(),
            program.display(),
            self.args.join(" ")
        );

        let Ok(desktop) = crate::beside("console-desktop");
        let mut nesting = Command::new(desktop);
        nesting.arg("shot").arg(self.here.join("screen.png"));

        nesting.arg("--bare");
        nesting.args(["--open", &opening]);
        nesting.args(["--until", &told.display().to_string()]);

        let Ok(script) = self.script(&told);

        match script {
            Some(script) => {
                nesting.args(["--press", &script]);
            }
            None => {},
        }

        let said = nesting.output().map_err(Awry::Machine)?;

        let read = std::fs::read_to_string(&told).map_err(|fault| {
            let Ok(why) = why(&said.stderr);

            Awry::SaidNothingDrawn(self.program.clone(), fault, why)
        })?;

        let every = telling::every(&read)?;

        match every.is_empty() {
            true => Err(Awry::DrewNothing(self.program.clone())),
            false => {
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

    let all = String::from_utf8_lossy(said);
    let worth: Vec<&str> = all.lines().map(str::trim_end).filter(heard_before).collect();

    let from = worth.len().saturating_sub(TOLD_LINES);

    Ok(match worth.get(from..) {
        Some(said) => said.join("\n"),
        None => "and nothing to say about why".to_string(),
    })
}

const TOLD_LINES: usize = 12;

impl Drop for Panel {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

pub fn every_offer_answered(card: &Told) -> Result<(), Awry> {
    let missing: Vec<String> = card
        .lines
        .iter()
        .filter(|line| line.offers == Offers::Yes && line.bare == Bare::No)
        .filter(|line| {
            let Ok(worn) = line.wearing("else");

            worn.is_none()
        })
        .map(|line| {
            let Ok(said) = said_of(line);

            said
        })
        .collect();

    match missing.is_empty() {
        true => Ok(()),
        false => Err(Awry::OfferUnanswered(card.panel.clone(), missing)),
    }
}

pub fn one_mark_for_one_subject(card: &Told) -> Result<(), Awry> {
    let marks = card
        .lines
        .iter()
        .filter(|line| {
            let Ok(worn) = line.wearing("else");

            worn.is_some()
        })
        .count();
    let offered = card.lines.iter().any(|line| line.offers == Offers::Yes);

    match (offered, marks) {
        (false, 0) | (true, 1) => Ok(()),
        (false, _) => Err(Awry::MarkWithoutOffer(card.panel.clone())),
        (true, _) => Err(Awry::TooManyMarks(card.panel.clone(), marks)),
    }
}

pub fn every_mark_reachable(card: &Told) -> Result<(), Awry> {
    let Ok(every) = card.every_spot();
    let off: Vec<String> = every
        .into_iter()
        .filter(|spot| {
            let Ok(reachable) = telling::reachable(spot, card.room);

            reachable == Reachable::No
        })
        .map(|spot| format!("{} at {:?} of {:?}", spot.name, spot.at, spot.big))
        .collect();

    match off.is_empty() {
        true => Ok(()),
        false => Err(Awry::OutOfReach(card.panel.clone(), card.room, off)),
    }
}

pub fn a_way_out_is_drawn(card: &Told) -> Result<(), Awry> {
    let Ok(worn) = card.wearing("shut");

    match worn {
        Some(_) => Ok(()),
        None => Err(Awry::NoWayOut(card.panel.clone(), card.tab.clone())),
    }
}

fn said_of(line: &Line) -> Result<String, Never> {
    Ok(match line.says.is_empty() {
        true => format!("row {}", line.at),
        false => format!("row {} ({})", line.at, line.says),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot(name: &str, at: (i32, i32), big: (i32, i32)) -> Spot {
        Spot { name: name.to_string(), at, big, scrolls: telling::Scrolls::No }
    }

    fn line(at: usize, offers: Offers, bare: Bare, spots: Vec<Spot>) -> Line {
        Line {
            at,
            says: String::new(),
            aside: String::new(),
            offers,
            bare,
            heading: console_panel::telling::Heading::No,
            standing: console_panel::telling::Standing::No,
            spots,
        }
    }

    fn card(lines: Vec<Line>, spots: Vec<Spot>) -> Told {
        Told {
            panel: "a-panel".to_string(),
            tab: "One".to_string(),
            out: telling::Out::No,
            room: (1024, 600),
            spots,
            lines,
        }
    }

    #[test]
    fn a_row_offering_something_with_no_mark_on_it_is_the_fault() {
        let card = card(vec![line(0, Offers::Yes, Bare::No, Vec::new())], Vec::new());
        assert!(every_offer_answered(&card).is_err());
    }

    #[test]
    fn the_mark_may_be_on_the_row_beside_the_one_that_offers() {
        let bare = line(0, Offers::Yes, Bare::Yes, Vec::new());
        let marked = line(1, Offers::Yes, Bare::No, vec![spot("else", (900, 300), (40, 30))]);
        let card = card(vec![bare, marked], Vec::new());

        every_offer_answered(&card).expect("the mark beside it answers");
        one_mark_for_one_subject(&card).expect("one mark, one subject");
    }

    #[test]
    fn a_card_with_one_subject_and_a_mark_on_every_line_is_a_crowd() {
        let card = card(
            vec![
                line(0, Offers::Yes, Bare::No, vec![spot("else", (900, 300), (40, 30))]),
                line(1, Offers::Yes, Bare::No, vec![spot("else", (900, 360), (40, 30))]),
            ],
            Vec::new(),
        );

        assert!(one_mark_for_one_subject(&card).is_err());
    }

    #[test]
    fn a_panel_that_offers_nothing_needs_no_mark() {
        let card = card(vec![line(0, Offers::No, Bare::No, Vec::new())], Vec::new());

        every_offer_answered(&card).expect("nothing is offered");
        one_mark_for_one_subject(&card).expect("nothing is offered");
    }

    #[test]
    fn a_mark_hanging_off_the_room_is_a_mark_nobody_can_press() {
        let off = card(Vec::new(), vec![spot("shut", (982, 14), (56, 44))]);
        let on = card(Vec::new(), vec![spot("shut", (954, 14), (56, 44))]);

        assert!(every_mark_reachable(&off).is_err());
        every_mark_reachable(&on).expect("a mark inside the room");
    }

    #[test]
    fn a_card_with_no_way_out_is_a_card_a_finger_is_shut_into() {
        assert!(a_way_out_is_drawn(&card(Vec::new(), Vec::new())).is_err());
        a_way_out_is_drawn(&card(Vec::new(), vec![spot("shut", (954, 14), (56, 44))]))
            .expect("a way out");
    }

    #[test]
    fn a_press_is_aimed_inside_the_panel_and_says_which_panel() {
        let Ok(mut panel) = Panel::opening("a-panel", &[]);
        let card = card(Vec::new(), vec![spot("shut", (954, 14), (56, 44))]);
        let Ok(worn) = card.wearing("shut");
        let shut = worn.expect("a way out to press");

        panel.press(&card, shut).expect("a place inside the panel");
        assert_eq!(panel.presses, vec!["console-point --in a-panel 982 36 --click".to_string()]);
    }

    #[test]
    fn a_press_at_a_place_outside_the_panel_is_refused() {
        let Ok(mut panel) = Panel::opening("a-panel", &[]);
        let card = card(Vec::new(), vec![spot("shut", (-80, 14), (56, 44))]);
        let Ok(worn) = card.wearing("shut");
        let shut = worn.expect("a way out to press");

        assert!(panel.press(&card, shut).is_err());
    }
}
