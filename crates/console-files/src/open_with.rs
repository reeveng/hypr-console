//! Which applications open a kind of file, and starting one on it.
//!
//! This was two calls into gio -- `AppInfo::recommended_for_type` and
//! `AppInfo::launch` -- and neither of them was gio's answer either. What a
//! .desktop file claims is `MimeType=`, which `console-applications` already
//! reads and is the crate that says what a desktop file means; what starting
//! one is is `console_panel::running::left_running`, which is how the menu has
//! always started an application and puts it in a scope of its own so that a
//! panel closing does not take it with it.
//!
//! **The field codes are the part gio was doing quietly.** `Exec` does not name
//! the file: it names where the file goes, as `%f` or `%u` for one and `%F` or
//! `%U` for a list, and an entry that takes no file at all names nothing. The
//! rest of this tree strips them, because the menu starts an application with
//! no file. Here the file is the whole point, so the code is replaced where it
//! stands and appended when there is none -- which is what every launcher does
//! with an entry that forgot to say.
//!
//! What is not read is the order gio would have put them in: `mimeapps.list`
//! says which application is the default for a type and which have been used
//! before. What this draws is every application that claims the type, by name,
//! which is a list a person reads rather than a machine's preference. The
//! default is a different question and it is the settings panel's.

use std::path::{Path, PathBuf};

use console_applications::entry::{DesktopEntry, Worth};
use console_core_external_programs::Program;
use console_core_never::Never;

const A_FILE: [&str; 4] = ["%f", "%F", "%u", "%U"];

pub fn programs(kind: &str) -> Result<Vec<(String, String)>, Never> {
    let Ok(roots) = console_core_places::applications();
    let Ok(files) = console_applications::entry::files(&roots);

    let mut found: Vec<(String, String)> = Vec::new();

    for at in files {
        let said = match std::fs::read_to_string(&at) {
            Ok(said) => said,
            Err(_nothing_to_read) => continue,
        };

        let Ok(entry) = DesktopEntry::read(&said);
        let Ok(worth) = entry.worth();

        match worth {
            Worth::Skipping => continue,
            Worth::Drawing => {},
        }

        let Ok(opens) = entry.opens();

        match opens.iter().any(|claimed| claimed == kind) {
            true => {},
            false => continue,
        }

        let Ok(says) = entry.says();

        match says {
            Some(says) => found.push((says.to_string(), at.display().to_string())),
            None => {},
        }
    }

    found.sort_by(|(one, _), (two, _)| one.cmp(two));
    found.dedup_by(|(one, _), (two, _)| one == two);

    Ok(found)
}

pub fn started(at: &str, path: &Path) -> Result<(), Never> {
    let said = match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(fault) => {
            eprintln!("files: {at}: {fault}");

            return Ok(());
        }
    };

    let Ok(entry) = DesktopEntry::read(&said);

    let command = match entry.command.filter(|command| !command.is_empty()) {
        Some(command) => command,
        None => {
            eprintln!("files: {at}: nothing to run");

            return Ok(());
        }
    };

    let Ok(words) = console_applications::words::split(command);

    let words = match words {
        Some(words) => words,
        None => {
            eprintln!("files: {at}: {command:?} is not a command");

            return Ok(());
        }
    };

    let Ok(mut arguments) = holding(&words, path);

    match entry.terminal.map(str::to_lowercase).as_deref() == Some("true") {
        true => {
            let Ok(alacritty) = Program::Alacritty.name();

            arguments.insert(0, alacritty.to_string());
            arguments.insert(1, "-e".to_string());
        }
        false => {},
    }

    console_panel::running::left_running(&arguments)
}

pub fn holding(words: &[String], path: &Path) -> Result<Vec<String>, Never> {
    let said = path.display().to_string();
    let mut arguments: Vec<String> = Vec::new();
    let mut put = Presence::Nowhere;

    for word in words {
        match A_FILE.contains(&word.as_str()) {
            true => {
                arguments.push(said.clone());
                put = Presence::InIt;
            },
            false => arguments.push(word.clone()),
        }
    }

    match put {
        Presence::InIt => {},
        Presence::Nowhere => arguments.push(said),
    }

    Ok(arguments)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
    InIt,
    Nowhere,
}

pub fn every(roots: &[PathBuf]) -> Result<Vec<PathBuf>, Never> {
    console_applications::entry::files(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(said: &[&str]) -> Vec<String> {
        said.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn the_file_goes_where_the_entry_says_it_goes() {
        assert_eq!(
            holding(&words(&["gimp", "%U"]), Path::new("/home/ada/beach.jpg")),
            Ok(words(&["gimp", "/home/ada/beach.jpg"]))
        );
        assert_eq!(
            holding(&words(&["mpv", "--fullscreen", "%f"]), Path::new("/x/film.mkv")),
            Ok(words(&["mpv", "--fullscreen", "/x/film.mkv"]))
        );
    }

    #[test]
    fn an_entry_that_says_nowhere_is_handed_the_file_at_the_end() {
        assert_eq!(
            holding(&words(&["librewolf"]), Path::new("/x/page.html")),
            Ok(words(&["librewolf", "/x/page.html"]))
        );
    }

    #[test]
    fn a_word_that_only_looks_like_a_code_is_left_alone() {
        assert_eq!(
            holding(&words(&["run", "--rate=%50", "%f"]), Path::new("/x/y")),
            Ok(words(&["run", "--rate=%50", "/x/y"]))
        );
    }
}
