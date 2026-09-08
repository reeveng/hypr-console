//! What the machine said last time it was asked.
//!
//! `Page::meanwhile` is the tab as it can be drawn before the machine has
//! answered anything, and it works because most of a tab is known in advance:
//! the three power profiles are the same three whatever powerprofilesctl says,
//! and all the answer decides is which of them is marked.
//!
//! Some tabs are not like that. Sound is whatever is plugged in and whatever is
//! playing, Wi-Fi is whatever is in the air, Bluetooth is whatever has ever
//! been paired. There is nothing to draw in advance, so those tabs went up
//! empty and filled in, which is the whole card changing height a moment after
//! it appeared -- and it appeared under a thumb already moving down it.
//!
//! They are not unknowable, though. They were known last time, and last time is
//! a far better guess than nothing: the speakers are the speakers, and the
//! networks in this room are the networks that were in this room. So what a
//! command says is written down as it answers, and the tab's `meanwhile` builds
//! its rows out of what was written down.
//!
//! The same builder fed an older answer, never a second opinion about what the
//! tab looks like. A hand-written `meanwhile` is a second list somebody has to
//! remember to change when the first one changes; this one cannot drift,
//! because there is only one list and the two readings go into it.
//!
//! Under the cache and not beside the [`notes`](crate::notes), deliberately.
//! A note is something the desktop remembers about itself and could not work
//! out again -- which tab it was left on, how much room it was granted. This is
//! the machine's own answer to a question anybody can ask again, so it belongs
//! where a thing that can be rebuilt belongs. Somebody who clears the cache
//! gets a panel that opens the way it did before there was one, which is the
//! only thing any of this is allowed to cost.

use std::path::PathBuf;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Base;

use crate::running;

fn beside(note: &str) -> Result<Option<PathBuf>, Never> {
    let ours = Base::Cache.ours()?;

    let Ok(whose) = whose();
    let Ok(filed) = filed(note);

    Ok(ours.map(|ours| ours.join("asked").join(format!("{whose}.{filed}"))))
}

fn whose() -> Result<String, Never> {
    Ok(std::env::args()
        .next()
        .and_then(|argv0| {
            std::path::Path::new(&argv0).file_name().and_then(|name| name.to_str()).map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "console-panel".to_string()))
}

fn filed(note: &str) -> Result<String, Never> {
    let filed: String = note
        .chars()
        .map(|letter| match letter.is_ascii_alphanumeric() {
            true => letter.to_ascii_lowercase(),
            false => '-',
        })
        .collect();

    Ok(match filed.is_empty() {
        true => "asked".to_string(),
        false => filed,
    })
}

pub fn last(note: &str) -> Result<String, Never> {
    let Ok(beside) = beside(note);

    let path = match beside {
        Some(path) => path,
        None => return Ok(String::new()),
    };

    let said = match std::fs::read_to_string(path) {
        Ok(said) => said,
        Err(_fault) => return Ok(String::new()),
    };

    Ok(said)
}

pub fn said(note: &str, program: Program, rest: &[&str]) -> Result<String, Never> {
    let Ok(said) = running::said(program, rest);
    let Ok(()) = keep(note, &said);

    Ok(said)
}

fn keep(note: &str, said: &str) -> Result<(), Never> {
    let Ok(last) = last(note);

    match last == said {
        true => return Ok(()),
        false => {},
    }

    let Ok(beside) = beside(note);

    let path = match beside {
        Some(path) => path,
        None => return Ok(()),
    };

    let holding = match path.parent() {
        Some(holding) => holding,
        None => return Ok(()),
    };

    match std::fs::create_dir_all(holding) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console: {}: keeping what a tab last said: {fault}", holding.display());

            return Ok(());
        }
    }

    let _ = std::fs::write(path, said);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_is_filed_under_a_name_any_filesystem_would_take() {
        assert_eq!(filed("sinks"), Ok("sinks".to_string()));
        assert_eq!(
            filed("bluetooth AA:BB:CC:DD:EE:FF"),
            Ok("bluetooth-aa-bb-cc-dd-ee-ff".to_string())
        );
        assert_eq!(filed("opens audio/x-opus+ogg"), Ok("opens-audio-x-opus-ogg".to_string()));
    }

    #[test]
    fn a_question_with_no_name_is_still_a_file() {
        assert_eq!(filed(""), Ok("asked".to_string()));
        assert_eq!(filed("///"), Ok("---".to_string()));
    }

    #[test]
    fn a_question_nobody_has_asked_says_nothing() {
        assert_eq!(
            last("a question nothing on this machine has ever asked"),
            Ok(String::new())
        );
    }
}
