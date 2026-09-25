//! What a press on the setup screen decides.
//!
//! Two of the three things this screen does write the file every button on the
//! front of the machine is read out of, and the rule that matters is that a
//! press is never one of them: putting every button back asks first, and only
//! the answer writes. The first-run write is the same rule from the other
//! side -- `--first` is the desktop saying no one has answered yet, and a
//! table already on disk is someone who has, so the flag alone is not enough
//! to overwrite one.
//!
//! Where the table is comes in as a word, because it is under `HOME` and this
//! is not allowed to look. A machine that will not say whose buttons these are
//! hands over no word at all, and that is a state of its own rather than an
//! empty path carried around as though it were somewhere: an empty path is a
//! write that goes nowhere and reports nothing. `Nowhere` writes nothing and
//! the first press says why.

use std::path::{Path, PathBuf};

use console_input_bindings::moved::Tasks;
use console_core_never::Never;
use console_program_contract::{
    Arguments, Effect, Flag, Initial, Program, Command, Update, Event, FileWrite,
};

use crate::rows::{Part, question};

pub const ASKING: &str = "console-asking";

pub const TABLE: &str = "--table";

pub const FIRST: &str = "--first";

pub const WRITTEN: &str = "--written";

pub const NOWHERE: &str = "Can't identify this controller";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Empty {
    Write,
    Leave,
}

impl Empty {
    pub fn of(first: Flag, written: Flag) -> Result<Self, Never> {
        Ok(match (first, written) {
            (Flag::Present, Flag::Absent) => Empty::Write,
            (Flag::Present, Flag::Present) | (Flag::Absent, _) => Empty::Leave,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    Nowhere,
    Initial { at: PathBuf, empty: Empty },
    Set { at: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingEvent {
    Requested(Part),
    Restore,
    Sure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingEffect {
    Note(String),
    Sure,
}

pub struct Setup;

impl Program for Setup {
    type State = Setting;
    type Event = MappingEvent;
    type Effect = MappingEffect;

    fn init(arguments: &Arguments) -> Initial<Setting> {
        let Ok(after) = arguments.after(TABLE);

        let setting = match after {
            Some(said) => {
                let Ok(first) = arguments.given(FIRST);
                let Ok(written) = arguments.given(WRITTEN);
                let Ok(empty) = Empty::of(first, written);

                Setting::Initial { at: PathBuf::from(said), empty }
            }
            None => Setting::Nowhere,
        };

        let Ok(opening) = Initial::new(setting);

        opening
    }

    fn update(state: &Setting, event: &Event<MappingEvent>) -> Update<Setting, MappingEffect> {
        let Ok(turn) = match (state, event) {
            (Setting::Initial { at, empty }, Event::Opened) => {
                let Ok(nothing) = emptied(at);

                let effects = match empty {
                    Empty::Write => vec![Effect::Write(nothing)],
                    Empty::Leave => Vec::new(),
                };

                Update::new(Setting::Set { at: at.clone() }, effects)
            }

            (Setting::Set { .. }, Event::Custom(MappingEvent::Requested(part))) => {
                let Ok(asked) = question(part);
                let Ok(word) = part.on.word();
                let Ok(asking) = Command::internal(ASKING, &[&part.slug, word]);

                Update::new(
                    state.clone(),
                    vec![Effect::Custom(MappingEffect::Note(asked)), Effect::Run(asking)],
                )
            }

            (Setting::Set { .. }, Event::Custom(MappingEvent::Restore)) => {
                Update::new(state.clone(), vec![Effect::Custom(MappingEffect::Sure)])
            }

            (Setting::Set { at }, Event::Custom(MappingEvent::Sure)) => {
                let Ok(nothing) = emptied(at);

                Update::new(state.clone(), vec![Effect::Write(nothing)])
            }

            (Setting::Nowhere, Event::Custom(_)) => {
                Update::new(state.clone(), vec![Effect::Custom(MappingEffect::Note(NOWHERE.to_string()))])
            }

            (_, _) => Update::none(state.clone()),
        };

        turn
    }
}

fn emptied(at: &Path) -> Result<FileWrite, Never> {
    let Ok(none) = Tasks::none();
    let Ok(contents) = none.written();

    Ok(FileWrite { path: at.to_path_buf(), contents })
}

#[cfg(test)]
mod tests {
    use console_program_contract::run;

    use super::*;

    fn opened(words: &[&str]) -> Vec<Effect<MappingEffect>> {
        let Ok(arguments) = Arguments::of(words);
        let Ok(said) = run::<Setup>(&arguments, &[Event::Opened]);
        let Ok(effects) = said.effects();

        effects
    }

    fn pressing(words: &[&str], heard: &[MappingEvent]) -> Vec<Effect<MappingEffect>> {
        let mut words_said = vec![Event::Opened];

        words_said.extend(heard.iter().cloned().map(Event::Custom));

        let Ok(arguments) = Arguments::of(words);
        let Ok(said) = run::<Setup>(&arguments, &words_said);
        let Ok(effects) = said.effects();

        effects
    }

    fn part() -> Part {
        Part {
            slug: "open-the-menu".to_string(),
            action: console_input_controller::actions::Action::Menu,
            does: "Open the menu".to_string(),
            on: console_input_bindings::bound::Input::Pad,
            plays: Vec::new(),
            moved: false,
        }
    }

    fn nothing_written() -> String {
        let Ok(none) = Tasks::none();
        let Ok(what) = none.written();

        what
    }

    #[test]
    fn a_first_run_with_nothing_written_writes_the_empty_table() {
        let said = opened(&[FIRST, TABLE, "/home/someone/.config/console/buttons.toml"]);

        assert_eq!(said, vec![Effect::Write(FileWrite {
            path: PathBuf::from("/home/someone/.config/console/buttons.toml"),
            contents: nothing_written(),
        })]);
    }

    #[test]
    fn a_first_run_over_a_table_someone_answered_writes_nothing() {
        assert!(opened(&[FIRST, WRITTEN, TABLE, "/somewhere"]).is_empty());
    }

    #[test]
    fn opening_it_the_ordinary_way_writes_nothing() {
        assert!(opened(&[TABLE, "/somewhere"]).is_empty());
    }

    #[test]
    fn putting_them_back_asks_before_it_writes() {
        let asked = pressing(&[TABLE, "/somewhere"], &[MappingEvent::Restore]);

        assert_eq!(asked, vec![Effect::Custom(MappingEffect::Sure)]);

        let answered = pressing(&[TABLE, "/somewhere"], &[MappingEvent::Restore, MappingEvent::Sure]);

        assert_eq!(answered.last(), Some(&Effect::Write(FileWrite {
            path: PathBuf::from("/somewhere"),
            contents: nothing_written(),
        })));
    }

    #[test]
    fn a_machine_that_will_not_say_whose_buttons_these_are_writes_nothing() {
        assert!(opened(&[FIRST]).is_empty());
        assert!(opened(&[]).is_empty());
    }

    #[test]
    fn a_press_with_nowhere_to_write_says_so_rather_than_writing() {
        let asked = pressing(&[FIRST], &[MappingEvent::Restore, MappingEvent::Sure]);

        assert_eq!(asked, vec![
            Effect::Custom(MappingEffect::Note(NOWHERE.to_string())),
            Effect::Custom(MappingEffect::Note(NOWHERE.to_string())),
        ]);
    }

    #[test]
    fn asking_for_a_button_puts_the_card_up_before_the_card_is_started() {
        let said = pressing(&[TABLE, "/somewhere"], &[MappingEvent::Requested(part())]);
        let Ok(runs) = Command::internal(ASKING, &["open-the-menu", "pad"]);

        assert_eq!(said, vec![
            Effect::Custom(MappingEffect::Note("Press a button for \u{201c}Open the menu\u{201d}".to_string())),
            Effect::Run(runs),
        ]);
    }
}
