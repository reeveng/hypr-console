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
//! them, presses, and returns when there is one more -- and `--press` is the
//! nested desktop holding itself open until that whole chain has finished
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
//! `cargo test -p console-media-viewer` builds today's expectations and runs
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
//! in nothing else. Opening `target/debug/files` by its absolute path
//! walks up from `target/` into a directory that has no `usr/local`, finds no
//! palette, and the panel says so on stderr and draws nothing at all. The
//! toolkit never met this because its colors were compiled into a stylesheet.
//! The binary in `target/` is still what is checked for staleness: it is the
//! same file, cloned into the stage.
//!
//! ```no_run
//! # use console_test_stages::panels::Panel;
//! let Ok(mut panel) = Panel::opening("viewer", &["/home/me/Pictures/beach.jpg"]);
//! let drawn = panel.drawn().expect("the viewer drew nothing");
//!
//! for card in &drawn {
//!     console_test_stages::panels::every_offer_answered(card).expect("a finger can reach it");
//! }
//! ```

use rustix::fs::flock;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_geometry::Point;
use console_core_never::Never;
use console_panel::description::{self, Bare, Line, Offers, Reachable, Spot, Description};

use crate::Error;

struct Room(Option<std::fs::File>);

const ONE_AT_A_TIME: &str = "console-panel-stage.lock";

impl Room {
    fn for_one_more() -> Result<Room, Never> {
        let at = std::env::temp_dir().join(ONE_AT_A_TIME);

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
            Some(file) => match flock(file, rustix::fs::FlockOperation::LockExclusive) {
                Ok(()) => {},
                Err(fault) => {
                    eprintln!("console-test-stages: {}: waiting for the lock: {fault}", at.display());
                },
            },
            None => {},
        }

        Ok(Room(held))
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        match &self.0 {
            Some(file) => match flock(file, rustix::fs::FlockOperation::Unlock) {
                Ok(()) => {},
                Err(fault) => {
                    eprintln!("console-test-stages: the lock this run took would not come off: {fault}");
                },
            },
            None => {},
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

        let when = match about.modified() {
            Ok(when) => when,
            Err(_unstamped) => return None,
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
            Err(_unreadable) => continue,
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
        (Some(edited), Some(built)) => match built < edited {
            true => Err(Error::Stale(program.to_path_buf())),
            false => Ok(()),
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
static ONE_AFTER_ANOTHER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub struct Panel {
    program: String,
    arguments: Vec<String>,
    presses: Vec<String>,
    here: PathBuf,
    read: Option<Vec<Description>>,
}

impl Panel {
    pub fn opening(program: &str, arguments: &[&str]) -> Result<Panel, Never> {
        let mine = ONE_AFTER_ANOTHER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let here = std::env::temp_dir()
            .join(format!("console-panel-{program}-{}-{mine}", std::process::id()));

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

        match across >= 0 && down >= 0 {
            true => {
                self.presses.push(format!(
                    "console-point --in {} {across} {down} --click",
                    card.panel
                ));

                Ok(())
            }
            false => Err(Error::NotInside(
                Point { x: across, y: down },
                card.panel.clone(),
            )),
        }
    }

    pub fn key(&mut self, key: &str) -> Result<(), Error> {
        match key.is_empty() {
            true => Err(Error::UnnamedKey),
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

    pub fn drawn(&mut self) -> Result<Vec<Description>, Error> {
        match &self.read {
            Some(read) => return Ok(read.clone()),
            None => {},
        }

        let Ok(_room) = Room::for_one_more();
        let Ok(program) = crate::beside(&self.program);

        match program.is_file() {
            true => built_since_the_panel_code(&program)?,
            false => return Err(Error::NotBuilt(program)),
        }

        std::fs::create_dir_all(&self.here).map_err(Error::Machine)?;

        let told = self.here.join("told.jsonl");
        let _ = std::fs::remove_file(&told);

        let opening = format!(
            "CONSOLE_PANEL_TELLS={} {} {}",
            told.display(),
            self.program,
            self.arguments.join(" ")
        );

        let Ok(desktop) = crate::beside("console-desktop");
        let mut nesting = Command::new(desktop);
        nesting.arg("describe");

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

        let said = nesting.output().map_err(Error::Machine)?;

        match said.status.success() {
            true => {},
            false => {
                let Ok(why) = why(&said.stderr);

                return Err(Error::NotNested(self.program.clone(), why));
            },
        }

        let read = std::fs::read_to_string(&told).map_err(|fault| {
            let Ok(why) = why(&said.stderr);

            Error::SaidNothingDrawn(self.program.clone(), fault, why)
        })?;

        let every = description::every(&read)?;

        match every.is_empty() {
            true => Err(Error::DrewNothing(self.program.clone())),
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

    let Ok(many) = console_core_number_conversion::fitted::<_, u32>(worth.len());
    let Ok(from) = console_core_number_conversion::index(many.saturating_sub(TOLD_LINES));

    Ok(match worth.get(from..) {
        Some(said) => said.join("\n"),
        None => "and nothing to say about why".to_string(),
    })
}

const TOLD_LINES: u32 = 12;

impl Drop for Panel {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

pub fn every_offer_answered(card: &Description) -> Result<(), Error> {
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
        false => Err(Error::OfferUnanswered(card.panel.clone(), missing)),
    }
}

pub fn one_mark_for_one_subject(card: &Description) -> Result<(), Error> {
    let Ok(marks) = console_core_number_conversion::fitted::<_, u32>(card
        .lines
        .iter()
        .filter(|line| {
            let Ok(worn) = line.wearing("else");

            worn.is_some()
        })
        .count());
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

    match off.is_empty() {
        true => Ok(()),
        false => Err(Error::OutOfReach(card.panel.clone(), card.room, off)),
    }
}

pub fn a_way_out_is_drawn(card: &Description) -> Result<(), Error> {
    let Ok(worn) = card.wearing("shut");

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

    match missing.is_empty() {
        true => Ok(()),
        false => Err(Error::Hidden(card.panel.clone(), missing)),
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
        Spot { name: name.to_string(), at, big, scrolls: description::Scrolls::No }
    }

    fn line(at: u32, offers: Offers, bare: Bare, spots: Vec<Spot>) -> Line {
        Line {
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
        }
    }

    fn week(cells: &[&str], drew: &[&str]) -> Line {
        Line {
            cells: cells.iter().map(|cell| (*cell).to_string()).collect(),
            drew: drew.iter().map(|word| (*word).to_string()).collect(),
            ..line(0, Offers::No, Bare::No, Vec::new())
        }
    }

    #[test]
    fn a_week_whose_days_are_carried_and_not_drawn_is_the_fault() {
        let card = card(vec![week(&["1", "2", "3"], &[])], Vec::new());

        assert!(every_row_draws_what_it_carries(&card).is_err());
    }

    #[test]
    fn a_week_that_draws_every_day_it_carries_holds() {
        let card = card(vec![week(&["1", "2", "3"], &["1", "2", "3"])], Vec::new());

        assert!(every_row_draws_what_it_carries(&card).is_ok());
    }

    #[test]
    fn a_row_whose_words_never_reach_the_screen_is_the_fault() {
        let quiet = Line { says: "Wi-Fi".to_string(), ..line(0, Offers::No, Bare::No, Vec::new()) };
        let card = card(vec![quiet], Vec::new());

        assert!(every_row_draws_what_it_carries(&card).is_err());
    }

    fn card(lines: Vec<Line>, spots: Vec<Spot>) -> Description {
        Description {
            panel: "a-panel".to_string(),
            tab: "One".to_string(),
            out: description::Output::No,
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
    fn a_mark_hanging_off_the_room_is_a_mark_no_one_can_press() {
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
