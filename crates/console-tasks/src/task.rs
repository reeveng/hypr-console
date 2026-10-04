//! The tasks this tree runs on itself, each under the word somebody types.
//!
//! `cargo x <task>`, the way `mix <task>` is Elixir's: a task is a variant
//! here and a function in the program, the list `cargo x` prints is this
//! list, and one task that needs another asks for it rather than repeating
//! its lines. That list is drawn by `console-core-arguments` off this enum,
//! so `cargo x`, `cargo x help` and `cargo x --help` are one usage, and
//! `cargo x` refuses a word that is no task with that usage under it.

use console_core_arguments::{Command, Operands, Subcommand};
use console_core_never::Never;
use console_core_words::Words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Task {
    #[words(word = "help", about = "what this can do")]
    Help,
    #[words(word = "test", about = "every test that can run on this machine")]
    Test,
    #[words(word = "ready", about = "everything that must hold before a deploy")]
    Ready,
    #[words(word = "reached", about = "the crates a change since a tree can reach, as cargo's flags")]
    Reached,
    #[words(word = "map", about = "draw how the running desktop is connected")]
    Map,
    #[words(word = "rules", about = "the EXPLICIT_* rules, enforced, over cargo's flags or the workspace")]
    Rules,
    #[words(word = "alone", about = "a command in a control group of its own")]
    Alone,
}

impl Subcommand for Task {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(Task::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        Task::about(self)
    }
}

pub const COMMAND: Command = Command {
    name: "cargo x",
    about: "The tasks this tree runs on itself, each under the word somebody types.",
    flags: &[],
    operands: Operands::Verbatim("ARGUMENT"),
};

pub fn help() -> Result<String, Never> {
    console_core_arguments::usage::<Task>(&COMMAND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_word_finds_every_task_again() -> Result<(), console_core_arguments::ValidationError> {
        for task in Task::VARIANTS {
            let Ok(word) = task.word();
            let line = console_core_arguments::read_with::<Task, &str>(&COMMAND, &[word])?;

            assert_eq!(line.subcommand(), Ok(Some(*task)));
        }

        Ok(())
    }

    #[test]
    fn the_reader_refuses_a_word_that_is_no_task() {
        let read = console_core_arguments::read_with::<Task, &str>(&COMMAND, &["deploy-everything"]);

        assert_eq!(
            read.map(|line| line.subcommand()).map_err(|refusal| refusal.reason),
            Err(console_core_arguments::Reason::NoSuchSubcommand("deploy-everything".to_string()))
        );
    }

    #[test]
    fn a_task_has_what_follows_it_as_the_person_types_it() -> Result<(), console_core_arguments::ValidationError> {
        let line = console_core_arguments::read_with::<Task, &str>(&COMMAND, &["rules", "--locked", "-p", "console-tasks"])?;
        let Ok(handed) = line.operands();

        assert_eq!(line.subcommand(), Ok(Some(Task::Rules)));
        assert_eq!(handed, ["--locked", "-p", "console-tasks"]);

        Ok(())
    }

    #[test]
    fn the_help_names_every_task() {
        let Ok(said) = help();

        for task in Task::VARIANTS {
            let Ok(word) = task.word();

            assert!(said.lines().any(|line| line.trim_start().starts_with(word)), "{word} is not in {said}");
        }
    }
}
