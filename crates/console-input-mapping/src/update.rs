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

use console_core_internal_programs::InternalProgram;
use std::path::{Path, PathBuf};

use console_input_bindings::moved::Tasks;
use console_core_never::Never;
use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Arguments, Effect, Flag, Command, Event, FileWrite};

use crate::rows::{Part, question};

pub const ASKING: InternalProgram = InternalProgram::Asking;

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

impl Machine for Setup {
    type Input = Arguments;
    type State = Setting;
    type Request = Event<MappingEvent>;
    type Effect = Effect<MappingEffect>;

    fn initialize(arguments: &Arguments, _previous: Option<Setting>, effects: &mut Effects) -> Result<Setting, Never> {
        let Ok(opening) = initial(arguments);

        opening.offered(effects)
    }

    fn handle(state: Setting, event: Event<MappingEvent>, effects: &mut Effects) -> Result<Setting, Never> {
        let Ok(decided) = decide(&state, &event);

        decided.offered(effects)
    }
}

type Effects = Queue<Effect<MappingEffect>>;

fn initial(arguments: &Arguments) -> Result<Transition<Setting, Effect<MappingEffect>>, Never> {
    let Ok(after) = arguments.after(TABLE);

    let setting = match after {
        Some(said) => {
            let Ok(first) = arguments.flag(FIRST);
            let Ok(written) = arguments.flag(WRITTEN);
            let Ok(empty) = Empty::of(first, written);

            Setting::Initial { at: PathBuf::from(said), empty }
        }
        None => Setting::Nowhere,
    };

    let Ok(opening) = Transition::without_effects(setting);

    Ok(opening)
}

fn decide(state: &Setting, event: &Event<MappingEvent>) -> Result<Transition<Setting, Effect<MappingEffect>>, Never> {
    let Ok(turn) = match (state, event) {
        (Setting::Initial { at, empty }, Event::Opened) => {
            let Ok(nothing) = emptied(at);

            let effects = match empty {
                Empty::Write => vec![Effect::Write(nothing)],
                Empty::Leave => Vec::new(),
            };

            Transition::new(Setting::Set { at: at.clone() }, effects)
        }

        (Setting::Set { .. }, Event::Custom(MappingEvent::Requested(part))) => {
            let Ok(asked) = question(part);
            let Ok(word) = part.on.word();
            let Ok(asking) = Command::internal(ASKING, &[&part.slug, word]);

            Transition::new(
                state.clone(),
                vec![Effect::Custom(MappingEffect::Note(asked)), Effect::Run(asking)],
            )
        }

        (Setting::Set { .. }, Event::Custom(MappingEvent::Restore)) => {
            Transition::new(state.clone(), vec![Effect::Custom(MappingEffect::Sure)])
        }

        (Setting::Set { at }, Event::Custom(MappingEvent::Sure)) => {
            let Ok(nothing) = emptied(at);

            Transition::new(state.clone(), vec![Effect::Write(nothing)])
        }

        (Setting::Nowhere, Event::Custom(_)) => {
            Transition::new(state.clone(), vec![Effect::Custom(MappingEffect::Note(NOWHERE.to_string()))])
        }

        (_, _) => Transition::without_effects(state.clone()),
    };

    Ok(turn)
}

fn emptied(at: &Path) -> Result<FileWrite, Never> {
    let Ok(none) = Tasks::none();
    let Ok(contents) = none.serialize();

    Ok(FileWrite { path: at.to_path_buf(), contents })
}

#[cfg(test)]
mod tests {
    use console_core_state_machine::run;

    use super::*;

    fn opened(words: &[&str]) -> Result<Vec<Effect<MappingEffect>>, Never> {
        let Ok(arguments) = Arguments::of(words);
        let Ok(said) = run::<Setup>(&arguments, &[Event::Opened]);

        said.effects()
    }

    fn pressing(words: &[&str], heard: &[MappingEvent]) -> Result<Vec<Effect<MappingEffect>>, Never> {
        let mut words_said = vec![Event::Opened];

        words_said.extend(heard.iter().cloned().map(Event::Custom));

        let Ok(arguments) = Arguments::of(words);
        let Ok(said) = run::<Setup>(&arguments, &words_said);

        said.effects()
    }

    fn part() -> Result<Part, Never> {
        Ok(Part {
            slug: "open-the-menu".to_string(),
            action: console_input_controller::actions::Action::Menu,
            does: "Open the menu".to_string(),
            on: console_input_bindings::bound::Input::Pad,
            plays: Vec::new(),
            moved: false,
        })
    }

    fn nothing_written() -> Result<String, Never> {
        let Ok(none) = Tasks::none();

        none.serialize()
    }

    #[test]
    fn a_first_run_with_nothing_written_writes_the_empty_table() {
        let Ok(said) = opened(&[FIRST, TABLE, "/home/someone/.config/console/buttons.toml"]);
        let Ok(contents) = nothing_written();

        assert_eq!(said, vec![Effect::Write(FileWrite {
            path: PathBuf::from("/home/someone/.config/console/buttons.toml"),
            contents,
        })]);
    }

    #[test]
    fn a_first_run_over_a_table_someone_answered_writes_nothing() {
        assert_eq!(opened(&[FIRST, WRITTEN, TABLE, "/somewhere"]), Ok(Vec::new()));
    }

    #[test]
    fn opening_it_the_ordinary_way_writes_nothing() {
        assert_eq!(opened(&[TABLE, "/somewhere"]), Ok(Vec::new()));
    }

    #[test]
    fn putting_them_back_asks_before_it_writes() {
        let Ok(asked) = pressing(&[TABLE, "/somewhere"], &[MappingEvent::Restore]);

        assert_eq!(asked, vec![Effect::Custom(MappingEffect::Sure)]);

        let Ok(answered) = pressing(&[TABLE, "/somewhere"], &[MappingEvent::Restore, MappingEvent::Sure]);
        let Ok(contents) = nothing_written();

        assert_eq!(answered.last(), Some(&Effect::Write(FileWrite { path: PathBuf::from("/somewhere"), contents })));
    }

    #[test]
    fn a_machine_that_will_not_say_whose_buttons_these_are_writes_nothing() {
        assert_eq!(opened(&[FIRST]), Ok(Vec::new()));
        assert_eq!(opened(&[]), Ok(Vec::new()));
    }

    #[test]
    fn a_press_with_nowhere_to_write_says_so_rather_than_writing() {
        let Ok(asked) = pressing(&[FIRST], &[MappingEvent::Restore, MappingEvent::Sure]);

        assert_eq!(asked, vec![
            Effect::Custom(MappingEffect::Note(NOWHERE.to_string())),
            Effect::Custom(MappingEffect::Note(NOWHERE.to_string())),
        ]);
    }

    #[test]
    fn asking_for_a_button_puts_the_card_up_before_the_card_is_started() {
        let Ok(part) = part();
        let Ok(said) = pressing(&[TABLE, "/somewhere"], &[MappingEvent::Requested(part)]);
        let Ok(runs) = Command::internal(ASKING, &["open-the-menu", "pad"]);

        assert_eq!(said, vec![
            Effect::Custom(MappingEffect::Note("Press a button for \u{201c}Open the menu\u{201d}".to_string())),
            Effect::Run(runs),
        ]);
    }
}
