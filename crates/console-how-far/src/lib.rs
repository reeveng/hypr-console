//! A line that says how far along a long thing is, for whoever is watching it.
//!
//! Two things on this device take minutes and say almost nothing while they do
//! it: an apply, and a run of the checks against the handheld. Both of them
//! used to leave the person waiting with a cursor and a guess, and the guess
//! was always the same one -- that it had hung, or that it had broken something
//! and was too far in to say so.
//!
//! # Drawn the way pacman draws
//!
//! Not because of how it looks. Pacman is the long thing everybody on an Arch
//! machine has already watched a hundred times without once wondering whether
//! it was still alive, and what earns that is not the hashes: it is that pacman
//! never says anything it does not know. Its counters come out of a transaction
//! that was decided before the first byte moved, so `(2/14)` is a fact rather
//! than an estimate. It names the thing it is on, by name, so a slow one can be
//! told from a stuck one. It ends every line it finishes, so what has already
//! happened stays on the screen and can be read back. And it never fills the
//! bar and then sits there, because the bar reaching the end is the only thing
//! it uses to say the end has come.
//!
//! Those are the four rules here, and the shape follows from them. It is
//! pacman's own, off its own format strings -- `(%*zu/%*zu) %ls%-*s`, then
//! `[%s]`, then ` %3d%%`: the counters padded to the width of their total so
//! the line does not jitter as they climb, the name on the left, the bar on the
//! right, redrawn over itself with a carriage return while it fills, and ended
//! with a newline when it is full so it becomes history.
//!
//! Fixed widths, where pacman measures the terminal. Pacman has to: a package
//! name and a version can be most of a line and it cannot know in advance. What
//! is drawn here is named by the callers, the longest of those names is known
//! where they are written, and a column wide enough for all of them is
//! arithmetic instead of an ioctl. Eighty columns is the whole budget and
//! `every_line_fits_a_terminal_eighty_wide` is what holds it.
//!
//! # Two streams, because they are two different questions
//!
//! The bar is drawn on stderr and everything said on the way is printed on
//! stdout, which is where each already was. That means either can be a pipe
//! while the other is a screen, so each is asked on its own whether it is being
//! watched, and neither is sent an escape sequence that would end up in
//! somebody's log. A stream that is not a screen gets a plain line per finished
//! item and no redrawing at all.

use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_u16};

use std::io::IsTerminal;
use std::io::Write;

pub const CELLS: usize = 22;

pub const AROUND: usize = 9;

pub const WHOLE: u16 = 100;

pub const ROOM: usize = 80;

const ERASE: &str = "\u{1b}[K";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watched {
    Screen,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Stays,
    Again,
}

pub fn watched() -> Result<Watched, Never> {
    Ok(match std::io::stderr().is_terminal() {
        true => Watched::Screen,
        false => Watched::Not,
    })
}

pub fn talking() -> Result<Watched, Never> {
    Ok(match std::io::stdout().is_terminal() {
        true => Watched::Screen,
        false => Watched::Not,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Escaping {
    Yes,
    No,
}

pub fn plain(line: &str) -> Result<String, Never> {
    let mut said = String::new();
    let mut escaping = Escaping::No;

    for one in line.chars() {
        match (escaping, one) {
            (Escaping::No, '\u{1b}') => escaping = Escaping::Yes,
            (Escaping::No, one) => said.push(one),
            (Escaping::Yes, one) => {
                escaping = match one.is_ascii_alphabetic() {
                    true => Escaping::No,
                    false => Escaping::Yes,
                };
            }
        }
    }

    Ok(said)
}

pub fn fraction(done: usize, many: usize) -> Result<f64, Never> {
    Ok(match many {
        0 => 0.0,
        many => {
            let Ok(done) = done.float();
            let Ok(many) = many.float();

            done / many
        }
    })
}

pub fn percent(part: f64) -> Result<u16, Never> {
    let Ok(percent) = toward_zero_u16(part.clamp(0.0, 1.0) * f64::from(WHOLE));

    Ok(percent.min(WHOLE))
}

pub fn counted(at: usize, many: usize) -> Result<String, Never> {
    let wide = format!("{many}").chars().count();

    Ok(format!("({at:>wide$}/{many})"))
}

pub fn caption(doing: &str, now: &str) -> Result<String, Never> {
    Ok(match now.is_empty() {
        true => doing.to_string(),
        false => format!("{doing} {now}"),
    })
}

pub fn room(many: usize) -> Result<usize, Never> {
    let Ok(widest) = counted(many, many);

    Ok(ROOM
        .saturating_sub(widest.chars().count())
        .saturating_sub(CELLS)
        .saturating_sub(AROUND))
}

pub fn fitted(said: &str, room: usize) -> Result<String, Never> {
    let over = said.chars().count().saturating_sub(room);

    Ok(match over {
        0 => said.to_string(),
        over => {
            let tail: String = said.chars().skip(over.saturating_add(1)).collect();

            format!("…{tail}")
        }
    })
}

pub fn line(at: usize, many: usize, said: &str, into: u16) -> Result<String, Never> {
    let Ok(counted) = counted(at, many);
    let Ok(room) = room(many);
    let Ok(said) = fitted(said, room);
    let full = usize::from(into.min(WHOLE))
        .saturating_mul(CELLS)
        .checked_div(usize::from(WHOLE))
        .unwrap_or(0);
    let empty = CELLS.saturating_sub(full);

    Ok(format!(
        "{counted} {said:<room$} [{}{}] {:>3}%",
        "#".repeat(full),
        "-".repeat(empty),
        into.min(WHOLE)
    ))
}

#[derive(Debug)]
pub struct Bar {
    watched: Watched,
    talking: Watched,
    at: usize,
    many: usize,
    into: u16,
    doing: String,
    now: String,
}

impl Bar {
    pub fn of(many: usize) -> Result<Self, Never> {
        let Ok(watched) = watched();
        let Ok(talking) = talking();

        Ok(Bar {
            watched,
            talking,
            at: 0,
            many,
            into: 0,
            doing: String::new(),
            now: String::new(),
        })
    }

    pub fn unwatched(many: usize) -> Result<Self, Never> {
        Ok(Bar {
            watched: Watched::Not,
            talking: Watched::Not,
            at: 0,
            many,
            into: 0,
            doing: String::new(),
            now: String::new(),
        })
    }

    pub fn many(&mut self, many: usize) -> Result<(), Never> {
        self.many = many;

        Ok(())
    }

    pub fn on(&mut self, doing: &str) -> Result<(), Never> {
        self.at = self.at.saturating_add(1);
        self.into = 0;
        self.doing = doing.to_string();
        self.now = String::new();

        self.moved()
    }

    pub fn onto(&mut self, doing: &str, into: u16) -> Result<(), Never> {
        self.at = self.at.saturating_add(1);
        self.into = into.min(WHOLE);
        self.doing = doing.to_string();
        self.now = String::new();

        self.moved()
    }

    pub fn filling(&mut self, into: u16, now: &str) -> Result<(), Never> {
        self.into = self.into.max(into.min(WHOLE));
        self.now = now.to_string();

        self.moved()
    }

    pub fn full(&mut self) -> Result<(), Never> {
        self.into = WHOLE;
        self.now = String::new();

        self.drawing(Ending::Stays)
    }

    pub fn stays(&mut self, doing: &str) -> Result<(), Never> {
        self.doing = doing.to_string();
        self.into = WHOLE;
        self.now = String::new();

        self.drawing(Ending::Stays)
    }

    pub fn say(&self, line: &str) -> Result<(), Never> {
        let mut out = std::io::stdout();
        let _ = match self.talking {
            Watched::Screen => writeln!(out, "\r{ERASE}{line}"),
            Watched::Not => writeln!(out, "{line}"),
        };
        let _ = out.flush();

        self.moved()
    }

    pub fn wiped(&self) -> Result<(), Never> {
        let mut out = std::io::stderr();
        let _ = match self.watched {
            Watched::Screen => write!(out, "\r{ERASE}"),
            Watched::Not => Ok(()),
        };
        let _ = out.flush();

        Ok(())
    }

    pub fn line(&self) -> Result<String, Never> {
        let Ok(said) = caption(&self.doing, &self.now);

        line(self.at, self.many, &said, self.into)
    }

    fn moved(&self) -> Result<(), Never> {
        match self.watched {
            Watched::Screen => self.drawing(Ending::Again),
            Watched::Not => Ok(()),
        }
    }

    fn drawing(&self, ending: Ending) -> Result<(), Never> {
        let Ok(drawn) = self.line();
        let mut out = std::io::stderr();
        let _ = match (self.watched, ending) {
            (Watched::Screen, Ending::Stays) => writeln!(out, "\r{ERASE}{drawn}"),
            (Watched::Screen, Ending::Again) => write!(out, "\r{ERASE}{drawn}"),
            (Watched::Not, Ending::Stays) => {
                let Ok(said) = caption(&self.doing, &self.now);
                let Ok(counted) = counted(self.at, self.many);

                writeln!(out, "  {counted} {said}")
            }
            (Watched::Not, Ending::Again) => Ok(()),
        };
        let _ = out.flush();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(bar: &Bar) -> String {
        let Ok(line) = bar.line();

        line
    }

    fn bar(many: usize) -> Bar {
        let Ok(bar) = Bar::unwatched(many);

        bar
    }

    #[test]
    fn the_counter_is_padded_to_the_width_of_its_total_so_it_does_not_jitter() {
        assert_eq!(counted(1, 14), Ok("( 1/14)".to_string()));
        assert_eq!(counted(14, 14), Ok("(14/14)".to_string()));
        assert_eq!(counted(3, 9), Ok("(3/9)".to_string()));
        assert_eq!(counted(7, 120), Ok("(  7/120)".to_string()));
    }

    #[test]
    fn a_line_is_the_counter_the_name_the_bar_and_the_number() {
        let Ok(said) = line(2, 14, "installing console-fonts", 50);

        assert!(said.starts_with("( 2/14) installing console-fonts"), "{said}");
        assert!(said.contains(&"#".repeat(11)), "{said}");
        assert!(said.contains(" ["), "the bar is not set off from the name: {said}");
        assert!(said.ends_with("]  50%"), "{said}");
    }

    #[test]
    fn every_line_fits_a_terminal_eighty_wide() {
        for (at, many, into) in
            [(1_usize, 9_usize, 0_u16), (14, 14, 100), (120, 120, 7), (7, 1000, 50)]
        {
            let Ok(said) =
                caption("keeping the release", "home/@user@/.config/waybar/style.css");
            let Ok(drawn) = line(at, many, &said, into);
            let wide = drawn.chars().count();

            assert_eq!(wide, ROOM, "{wide} columns rather than {ROOM}: {drawn}");
        }
    }

    #[test]
    fn a_short_name_is_padded_so_the_bar_stands_in_the_same_column_all_run() {
        let Ok(one) = line(1, 14, "sweeping", 0);
        let Ok(other) = line(2, 14, "installing console-fonts", 50);

        assert_eq!(one.find('['), other.find('['), "{one}\n{other}");
    }

    #[test]
    fn a_name_too_long_for_the_column_keeps_its_tail() {
        let Ok(room) = room(14);
        let Ok(said) = fitted("writing files home/@user@/.config/waybar/a/long/way/style.css", room);

        assert!(said.starts_with('…'), "{said}");
        assert!(said.ends_with("style.css"), "{said}");
        assert!(said.chars().count() <= room, "{said}");
    }

    #[test]
    fn a_thing_with_nothing_to_say_about_itself_is_named_by_its_own_name() {
        assert_eq!(caption("sweeping", ""), Ok("sweeping".to_string()));
    }

    #[test]
    fn a_bar_never_runs_backwards_inside_one_item() {
        let mut bar = bar(3);
        let Ok(()) = bar.on("building");
        let Ok(()) = bar.filling(40, "console-panel");
        let Ok(()) = bar.filling(10, "console-panel");

        assert!(drawn(&bar).contains(" 40%"), "{}", drawn(&bar));
    }

    #[test]
    fn the_next_item_starts_the_bar_empty_again() {
        let mut bar = bar(3);
        let Ok(()) = bar.on("building");
        let Ok(()) = bar.filling(90, "console-panel");
        let Ok(()) = bar.on("writing files");

        assert!(drawn(&bar).starts_with("(2/3) writing files"), "{}", drawn(&bar));
        assert!(drawn(&bar).ends_with("]   0%"), "{}", drawn(&bar));
    }

    #[test]
    fn an_item_that_carries_the_fill_over_never_draws_an_empty_frame_first() {
        let mut bar = bar(3);
        let Ok(()) = bar.on("one");
        let Ok(()) = bar.filling(60, "");
        let Ok(()) = bar.onto("two", 60);

        assert!(drawn(&bar).starts_with("(2/3) two"), "{}", drawn(&bar));
        assert!(drawn(&bar).ends_with("]  60%"), "{}", drawn(&bar));
    }

    #[test]
    fn an_item_that_is_finished_is_full() {
        let mut bar = bar(3);
        let Ok(()) = bar.on("building");
        let Ok(()) = bar.filling(40, "console-panel");
        let Ok(()) = bar.full();

        assert!(drawn(&bar).ends_with("] 100%"), "{}", drawn(&bar));
        assert!(drawn(&bar).contains(&"#".repeat(CELLS)), "{}", drawn(&bar));
    }

    #[test]
    fn a_number_past_the_end_fills_the_bar_and_no_further() {
        let Ok(said) = line(1, 1, "done", 400);

        assert!(said.ends_with("] 100%"), "{said}");
        assert_eq!(said.chars().filter(|one| *one == '#').count(), CELLS);
    }

    #[test]
    fn the_colour_a_program_writes_is_not_part_of_what_it_said() {
        assert_eq!(
            plain("\u{1b}[0m\u{1b}[1m\u{1b}[32m   Compiling\u{1b}[0m console-panel v0.1.0"),
            Ok("   Compiling console-panel v0.1.0".to_string())
        );
        assert_eq!(plain("\u{1b}[1;34m::\u{1b}[0m plain"), Ok(":: plain".to_string()));
        assert_eq!(plain("nothing to strip"), Ok("nothing to strip".to_string()));
        assert_eq!(plain("\u{1b}[K"), Ok(String::new()));
    }

    #[test]
    fn nothing_is_divided_by_a_total_of_nothing() {
        assert_eq!(fraction(0, 0), Ok(0.0));
        assert_eq!(fraction(1, 4), Ok(0.25));
        assert_eq!(fraction(4, 4), Ok(1.0));
    }

    #[test]
    fn a_part_of_a_whole_is_a_number_between_none_of_it_and_all_of_it() {
        assert_eq!(percent(0.0), Ok(0));
        assert_eq!(percent(0.5), Ok(50));
        assert_eq!(percent(1.0), Ok(WHOLE));
        assert_eq!(percent(-3.0), Ok(0));
        assert_eq!(percent(9.0), Ok(WHOLE));
    }
}
