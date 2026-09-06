//! What a press on the setup screen decides.
//!
//! Two of the three things this screen does write the file every button on the
//! front of the machine is read out of, and the rule that matters is that a
//! press is never one of them: putting every button back asks first, and only
//! the answer writes. The first-run write is the same rule from the other
//! side -- `--first` is the desktop saying nobody has answered yet, and a
//! table already on disk is somebody who has, so the flag alone is not enough
//! to overwrite one.
//!
//! Where the table is comes in as a word, because it is under `HOME` and this
//! is not allowed to look.

use std::path::{Path, PathBuf};

use console_gamepad::jobs::Jobs;
use console_never::Never;
use console_program_contract::{
    Argv, Doing, Given, Opening, Program, Runs, Turn, Word, Writing,
};

use crate::rows::{Part, question};

pub const ASKING: &str = "console-asking";

pub const TABLE: &str = "--table";

pub const FIRST: &str = "--first";

pub const WRITTEN: &str = "--written";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Empty {
    Write,
    Leave,
}

impl Empty {
    pub fn of(first: Given, written: Given) -> Result<Self, Never> {
        Ok(match (first, written) {
            (Given::Yes, Given::No) => Empty::Write,
            (Given::Yes, Given::Yes) | (Given::No, _) => Empty::Leave,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    Opening { at: PathBuf, empty: Empty },
    Set { at: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Asked(Part),
    PutBack,
    Sure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Its {
    Note(String),
    Sure,
}

pub struct Setup;

impl Program for Setup {
    type State = Setting;
    type Hears = Heard;
    type Does = Its;

    fn opening(argv: &Argv) -> Opening<Setting> {
        let Ok(after) = argv.after(TABLE);
        let at = PathBuf::from(after.unwrap_or_default());
        let Ok(first) = argv.given(FIRST);
        let Ok(written) = argv.given(WRITTEN);
        let Ok(empty) = Empty::of(first, written);
        let Ok(opening) = Opening::holding(Setting::Opening { at, empty });

        opening
    }

    fn heard(state: &Setting, word: &Word<Heard>) -> Turn<Setting, Its> {
        let Ok(turn) = match (state, word) {
            (Setting::Opening { at, empty }, Word::Opened) => {
                let Ok(nothing) = emptied(at);

                let doings = match empty {
                    Empty::Write => vec![Doing::Write(nothing)],
                    Empty::Leave => Vec::new(),
                };

                Turn::doing(Setting::Set { at: at.clone() }, doings)
            }

            (Setting::Set { .. }, Word::Its(Heard::Asked(part))) => {
                let Ok(asked) = question(part);
                let Ok(asking) = Runs::ours(ASKING, &[&part.slug]);

                Turn::doing(
                    state.clone(),
                    vec![Doing::Its(Its::Note(asked)), Doing::Ask(asking)],
                )
            }

            (Setting::Set { .. }, Word::Its(Heard::PutBack)) => {
                Turn::doing(state.clone(), vec![Doing::Its(Its::Sure)])
            }

            (Setting::Set { at }, Word::Its(Heard::Sure)) => {
                let Ok(nothing) = emptied(at);

                Turn::doing(state.clone(), vec![Doing::Write(nothing)])
            }

            (_, _) => Turn::nothing(state.clone()),
        };

        turn
    }
}

fn emptied(at: &Path) -> Result<Writing, Never> {
    let Ok(none) = Jobs::none();
    let Ok(what) = none.written();

    Ok(Writing { at: at.to_path_buf(), what })
}

#[cfg(test)]
mod tests {
    use console_program_contract::told;

    use super::*;

    fn opened(words: &[&str]) -> Vec<Doing<Its>> {
        let Ok(argv) = Argv::of(words);
        let Ok(said) = told::<Setup>(&argv, &[Word::Opened]);
        let Ok(doings) = said.doings();

        doings
    }

    fn pressing(words: &[&str], heard: &[Heard]) -> Vec<Doing<Its>> {
        let mut words_said = vec![Word::Opened];

        words_said.extend(heard.iter().cloned().map(Word::Its));

        let Ok(argv) = Argv::of(words);
        let Ok(said) = told::<Setup>(&argv, &words_said);
        let Ok(doings) = said.doings();

        doings
    }

    fn part() -> Part {
        Part {
            slug: "open-the-menu".to_string(),
            does: "Open the menu".to_string(),
            plays: Vec::new(),
            moved: false,
        }
    }

    fn nothing_written() -> String {
        let Ok(none) = Jobs::none();
        let Ok(what) = none.written();

        what
    }

    #[test]
    fn a_first_run_with_nothing_written_writes_the_empty_table() {
        let said = opened(&[FIRST, TABLE, "/home/somebody/.config/console/buttons.toml"]);

        assert_eq!(said, vec![Doing::Write(Writing {
            at: PathBuf::from("/home/somebody/.config/console/buttons.toml"),
            what: nothing_written(),
        })]);
    }

    #[test]
    fn a_first_run_over_a_table_somebody_answered_writes_nothing() {
        assert!(opened(&[FIRST, WRITTEN, TABLE, "/somewhere"]).is_empty());
    }

    #[test]
    fn opening_it_the_ordinary_way_writes_nothing() {
        assert!(opened(&[TABLE, "/somewhere"]).is_empty());
    }

    #[test]
    fn putting_them_back_asks_before_it_writes() {
        let asked = pressing(&[TABLE, "/somewhere"], &[Heard::PutBack]);

        assert_eq!(asked, vec![Doing::Its(Its::Sure)]);

        let answered = pressing(&[TABLE, "/somewhere"], &[Heard::PutBack, Heard::Sure]);

        assert_eq!(answered.last(), Some(&Doing::Write(Writing {
            at: PathBuf::from("/somewhere"),
            what: nothing_written(),
        })));
    }

    #[test]
    fn asking_for_a_button_puts_the_card_up_before_the_card_is_started() {
        let said = pressing(&[TABLE, "/somewhere"], &[Heard::Asked(part())]);
        let Ok(runs) = Runs::ours(ASKING, &["open-the-menu"]);

        assert_eq!(said, vec![
            Doing::Its(Its::Note("Press the button for open the menu".to_string())),
            Doing::Ask(runs),
        ]);
    }
}
