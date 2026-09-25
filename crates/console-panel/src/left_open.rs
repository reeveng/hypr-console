//! The panel that was up when the machine went down, so it comes back up.
//!
//! A restart that nobody asked for -- the battery running out, the button held,
//! a deploy restarting the host -- should put back what was on the screen, the
//! way a book comes back at its page. So the host writes which panel it put up
//! and what it was asked with, every time it puts one up, and takes it away
//! when a person puts the panel away: B, a row that closes it, or its own
//! button pressed a second time. Restart and Shut Down are rows that close the
//! panel as they run, so what they leave behind is nothing, and the machine
//! that comes back from them comes back to the desktop rather than to Settings.
//!
//! What is not a person putting it away is left alone. The host being asked to
//! stop and the panel's program being ended are what a machine going down looks
//! like from here, and forgetting then is forgetting the one time it mattered.
//!
//! Put back is the panel's own program started again with the same arguments,
//! which is every road in: it takes the lock, asks the host, and the host draws
//! it as though it had been pressed.

use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_program_lifetime::{in_a_scope_of_its_own, let_go};

use crate::notes::{self, Note};

pub const NOTE: Note<'static> = Note { program: "panels", called: "left-open" };

const BETWEEN: char = '\0';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOpen {
    pub who: String,
    pub arguments: Vec<String>,
}

pub fn spelled(open: &LeftOpen) -> Result<String, Never> {
    let words: Vec<&str> = std::iter::once(open.who.as_str()).chain(open.arguments.iter().map(String::as_str)).collect();

    Ok(words.join(&BETWEEN.to_string()))
}

pub fn read(said: &str) -> Result<Option<LeftOpen>, Never> {
    let mut words = said.split(BETWEEN);

    Ok(match words.next() {
        Some("") | None => None,
        Some(who) => Some(LeftOpen { who: who.to_string(), arguments: words.map(str::to_string).collect() }),
    })
}

pub fn opened(open: &LeftOpen) -> Result<(), Never> {
    let Ok(said) = spelled(open);

    notes::write(NOTE, &said)
}

pub fn put_away() -> Result<(), Never> {
    notes::write(NOTE, "")
}

pub fn left() -> Result<Option<LeftOpen>, Never> {
    let Ok(said) = notes::read(NOTE);

    match said {
        Some(said) => read(&said),
        None => Ok(None),
    }
}

pub fn started(program: InternalProgram, arguments: Vec<String>) -> Result<(), Never> {
    let Ok(at) = program.at();
    let whole: Vec<String> = std::iter::once(at.to_string_lossy().into_owned()).chain(arguments).collect();
    let Ok((_either_way_it_starts, arguments)) = in_a_scope_of_its_own(None, &whole);

    let (named, rest) = match arguments.split_first() {
        Some((named, rest)) => (named, rest),
        None => return Ok(()),
    };

    let mut starting = std::process::Command::new(named);
    starting.args(rest).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());

    let Ok(()) = console_response_times::not_a_press(&mut starting);

    match let_go(&mut starting) {
        Ok(started) => crate::running::kept(started),
        Err(fault) => {
            eprintln!("console-panels: putting {} back: {fault}", at.display());

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_and_its_arguments_come_back_as_they_went_in() {
        let open = LeftOpen { who: "settings-panel".to_string(), arguments: vec!["Sound".to_string(), "two words".to_string(), String::new()] };
        let Ok(said) = spelled(&open);

        assert_eq!(read(&said), Ok(Some(open)));
    }

    #[test]
    fn a_panel_put_away_leaves_nothing_to_put_back() {
        assert_eq!(read(""), Ok(None));
    }
}
