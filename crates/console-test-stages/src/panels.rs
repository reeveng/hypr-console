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
//! the desktop stage's own does.
//!
//! ```no_run
//! # use console_test_stages::panels::Panel;
//! let Ok(mut panel) = Panel::opening("viewer-panel", &["/home/me/Pictures/beach.jpg"]);
//! let drawn = panel.drawn().expect("the viewer drew nothing");
//!
//! for card in &drawn {
//!     assert_eq!(console_test_stages::panels::every_offer_answered(card), Ok(()));
//! }
//! ```

use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_never::Never;
use console_core_number_conversion::{Float, fitted};
use console_panel::telling::{self, Bare, Line, Offers, Reachable, Spot, Told};

const DRAWN: f64 = 2.5;

const AFTER: f64 = 1.5;

const BETWEEN: f64 = 0.6;

pub const QUIET: f64 = 4.0;

struct Room(Option<std::fs::File>);

fn how_many() -> Result<usize, Never> {
    let all = std::thread::available_parallelism().map_or(SOME, std::num::NonZero::get);

    Ok(all.checked_div(A_SHARE).unwrap_or(SOME).max(SOME))
}

const A_SHARE: usize = 4;

const SOME: usize = 2;

impl Room {
    fn for_one_more(mine: u32) -> Result<Room, Never> {
        let Ok(mine) = fitted::<u32, usize>(mine);
        let Ok(how_many) = how_many();

        let slot = mine.checked_rem(how_many).unwrap_or(0);
        let at = std::env::temp_dir().join(format!("console-panel-stage-{slot}.lock"));

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

fn beside(program: &str) -> Result<PathBuf, Never> {
    let here = match std::env::current_exe() {
        Ok(at) => at.parent().and_then(|at| at.parent()).map(|at| at.join(program)),
        Err(fault) => {
            eprintln!("console-test-stages: where this program is: {fault}");

            None
        }
    };

    Ok(here.filter(|at| at.exists()).unwrap_or_else(|| PathBuf::from(program)))
}

fn built_since_the_panel_code(program: &Path) -> Result<(), String> {
    let when = |at: &Path| -> Option<std::time::SystemTime> {
        let Ok(about) = at.metadata() else { return None };

        let Ok(when) = about.modified() else { return None };

        Some(when)
    };

    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent();
    let library = crates.map(|at| at.join("console-panel").join("src"));

    let Some(library) = library else { return Ok(()) };

    let mut newest = None;
    let mut look = vec![library];

    while let Some(at) = look.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else { continue };

        for found in entries.flatten() {
            let at = found.path();

            match at.is_dir() {
                true => look.push(at),
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

    match (newest, when(program)) {
        (Some(edited), Some(built)) if built < edited => Err(format!(
            "{} was built before console-panel was last edited, so this would hold \
             today's rules against yesterday's panel: cargo build --workspace",
            program.display()
        )),
        _ => Ok(()),
    }
}

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

    pub fn press(&mut self, card: &Told, spot: &Spot) -> Result<(), String> {
        let Ok((across, down)) = spot.middle();

        match across >= 0 && down >= 0 {
            true => {
                self.presses.push(format!(
                    "console-point --in {} {across} {down} --click",
                    card.panel
                ));

                Ok(())
            }
            false => Err(format!(
                "({across}, {down}) is not a place inside {}",
                card.panel
            )),
        }
    }

    pub fn key(&mut self, key: &str) -> Result<(), String> {
        match key.is_empty() {
            true => Err("a key with no name".to_string()),
            false => {
                self.presses.push(format!("wtype -k {key}"));

                Ok(())
            }
        }
    }

    fn script(&self) -> Result<Option<(String, f64)>, Never> {
        let Some((first, rest)) = self.presses.split_first() else { return Ok(None) };

        let mut script = format!("sleep {DRAWN}; {first}");

        for command in rest {
            script.push_str(&format!("; sleep {BETWEEN}; {command}"));
        }

        let Ok(pressed) = fitted::<usize, u64>(self.presses.len());
        let Ok(many) = pressed.float();

        Ok(Some((script, DRAWN + many * BETWEEN + AFTER)))
    }

    pub fn drawn(&mut self) -> Result<Vec<Told>, String> {
        match &self.read {
            Some(read) => return Ok(read.clone()),
            None => {},
        }

        let Ok(_room) = Room::for_one_more(self.mine);
        let Ok(program) = beside(&self.program);

        match program.is_file() {
            true => built_since_the_panel_code(&program)?,
            false => {
                return Err(format!(
                    "{} is not in target/debug, so there would be nothing to open: \
                     cargo build --workspace",
                    program.display()
                ));
            }
        }

        std::fs::create_dir_all(&self.here).map_err(|fault| fault.to_string())?;

        let told = self.here.join("told.jsonl");
        let _ = std::fs::remove_file(&told);

        let opening = format!(
            "CONSOLE_PANEL_TELLS={} {} {}",
            told.display(),
            program.display(),
            self.args.join(" ")
        );

        let Ok(desktop) = beside("console-desktop");
        let mut nesting = Command::new(desktop);
        nesting.arg("shot").arg(self.here.join("screen.png"));

        nesting.arg("--bare");
        nesting.args(["--open", &opening]);
        nesting.args(["--until", &told.display().to_string()]);

        let Ok(script) = self.script();

        let waited = match script {
            Some((script, waited)) => {
                nesting.args(["--open", &script]);
                waited
            }
            None => AFTER,
        };

        nesting.args(["--settle", &format!("{waited:.1}")]);

        let said = nesting.output().map_err(|fault| fault.to_string())?;

        let read = std::fs::read_to_string(&told).map_err(|fault| {
            format!(
                "{} said nothing about what it drew ({fault}):\n{}",
                self.program,
                {
                    let Ok(why) = why(&said.stderr);

                    why
                }
            )
        })?;

        let every = telling::every(&read)?;

        match every.is_empty() {
            true => Err(format!("{} drew nothing at all", self.program)),
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

pub fn every_offer_answered(card: &Told) -> Result<(), String> {
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
        false => Err(format!(
            "{} offers something behind Y that no finger can reach: {}",
            card.panel,
            missing.join(", ")
        )),
    }
}

pub fn one_mark_for_one_subject(card: &Told) -> Result<(), String> {
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
        (false, _) => Err(format!("{} draws a mark for an offer it does not make", card.panel)),
        (true, _) => Err(format!(
            "{} draws {marks} marks for Y where a card with one subject wants one",
            card.panel
        )),
    }
}

pub fn every_mark_reachable(card: &Told) -> Result<(), String> {
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
        false => Err(format!(
            "{} draws these where a hand cannot land, in a room of {:?}: {}",
            card.panel,
            card.room,
            off.join(", ")
        )),
    }
}

pub fn a_way_out_is_drawn(card: &Told) -> Result<(), String> {
    let Ok(worn) = card.wearing("shut");

    match worn {
        Some(_) => Ok(()),
        None => Err(format!("{} draws no way out, on the {} tab", card.panel, card.tab)),
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

        assert_eq!(every_offer_answered(&card), Ok(()));
        assert_eq!(one_mark_for_one_subject(&card), Ok(()));
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

        assert_eq!(every_offer_answered(&card), Ok(()));
        assert_eq!(one_mark_for_one_subject(&card), Ok(()));
    }

    #[test]
    fn a_mark_hanging_off_the_room_is_a_mark_nobody_can_press() {
        let off = card(Vec::new(), vec![spot("shut", (982, 14), (56, 44))]);
        let on = card(Vec::new(), vec![spot("shut", (954, 14), (56, 44))]);

        assert!(every_mark_reachable(&off).is_err());
        assert_eq!(every_mark_reachable(&on), Ok(()));
    }

    #[test]
    fn a_card_with_no_way_out_is_a_card_a_finger_is_shut_into() {
        assert!(a_way_out_is_drawn(&card(Vec::new(), Vec::new())).is_err());
        assert_eq!(
            a_way_out_is_drawn(&card(Vec::new(), vec![spot("shut", (954, 14), (56, 44))])),
            Ok(())
        );
    }

    #[test]
    fn a_press_is_aimed_inside_the_panel_and_says_which_panel() {
        let Ok(mut panel) = Panel::opening("a-panel", &[]);
        let card = card(Vec::new(), vec![spot("shut", (954, 14), (56, 44))]);
        let Ok(worn) = card.wearing("shut");
        let shut = worn.expect("a way out to press");

        assert_eq!(panel.press(&card, shut), Ok(()));
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
