//! The tasks this tree runs on itself, each under the word somebody types.
//!
//! `cargo x <task>`, the way `mix <task>` is Elixir's: a task is a variant
//! here and a function in the program, the list `cargo x` prints is this
//! list, and one task that needs another asks for it rather than repeating
//! its lines.

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

pub const EVERY: [Task; 7] = [Task::Help, Task::Test, Task::Ready, Task::Reached, Task::Map, Task::Rules, Task::Alone];

pub fn help() -> Result<String, Never> {
    let lines: Vec<String> = EVERY
        .into_iter()
        .map(|task| {
            let Ok(word) = task.word();
            let Ok(about) = task.about();

            format!("cargo x {word:<10} {about}")
        })
        .collect();

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_task_is_found_again_by_its_word() {
        for task in EVERY {
            let Ok(word) = task.word();
            let Ok(found) = Task::from_word(word);

            assert_eq!(found, Some(task));
        }
    }

    #[test]
    fn a_word_that_is_no_task_is_none() {
        let Ok(found) = Task::from_word("deploy-everything");

        assert_eq!(found, None);
    }

    #[test]
    fn the_help_names_every_task() {
        let Ok(said) = help();

        assert_eq!(said.lines().count(), EVERY.len());
    }
}
