//! What pacman has, and on whose word it has it.
//!
//! A package is on this machine for one of two reasons: something asked for it,
//! or something else needed it. pacman writes that reason down, and it is the
//! reason rather than the presence that decides whether the package survives.
//! Anything held only as a dependency goes the moment the thing that pulled it
//! in is removed, and `pacman -Qdtq | pacman -Rns -` is a line people run on a
//! quiet afternoon.
//!
//! So a package named in the manifest and held as a dependency is not held. It
//! reads as installed, every check passes, and it leaves on someone else's
//! errand. Three packages were found this way in one evening -- pw-record's,
//! notify-send's and pactl's -- each of them working only because something
//! unrelated had dragged it in, and the manifest is meant to be the answer to
//! exactly that.
//!
//! `PackageState::Borrowed` is that state said out loud, and `console apply` settles it
//! by telling pacman the desktop asked for the package too, which is true.

use std::collections::HashSet;

use console_core_never::Never;
use console_core_words::Words;

use crate::settled::Settled;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum PackageState {
    #[words(name = "ok")]
    Ok,
    #[words(name = "held as a dependency")]
    Borrowed,
    #[words(name = "missing")]
    Missing,
}

impl PackageState {
    pub fn settled(self) -> Result<Settled, Never> {
        Ok(match self == PackageState::Ok {
            true => Settled::Yes,
            false => Settled::No,
        })
    }
}

pub fn held(installed: &[String], asked_for: &[String], package: &str) -> Result<PackageState, Never> {
    let said = |names: &[String]| names.iter().any(|name| name == package);

    Ok(match (said(installed), said(asked_for)) {
        (_, true) => PackageState::Ok,
        (true, false) => PackageState::Borrowed,
        (false, false) => PackageState::Missing,
    })
}

pub fn borrowed<'a>(
    named: &'a [String],
    installed: &[String],
    asked_for: &[String],
) -> Result<Vec<&'a str>, Never> {
    Ok(named
        .iter()
        .filter(|package| {
            let Ok(held) = held(installed, asked_for, package);

            held == PackageState::Borrowed
        })
        .map(String::as_str)
        .collect())
}

pub fn missing<'a>(named: &'a [String], installed: &[String]) -> Result<Vec<&'a str>, Never> {
    let known: HashSet<&str> = installed.iter().map(String::as_str).collect();

    Ok(named
        .iter()
        .filter(|package| !known.contains(package.as_str()))
        .map(String::as_str)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(said: &[&str]) -> Vec<String> {
        said.iter().map(|name| name.to_string()).collect()
    }

    fn holding(installed: &[String], asked_for: &[String], package: &str) -> PackageState {
        let Ok(held) = held(installed, asked_for, package);

        held
    }

    fn lacking<'a>(named: &'a [String], installed: &[String]) -> Vec<&'a str> {
        let Ok(missing) = missing(named, installed);

        missing
    }

    fn lent<'a>(named: &'a [String], installed: &[String], asked_for: &[String]) -> Vec<&'a str> {
        let Ok(borrowed) = borrowed(named, installed, asked_for);

        borrowed
    }

    #[test]
    fn a_package_someone_asked_for_is_held() {
        let installed = names(&["glib2", "gtk4"]);
        let asked_for = names(&["gtk4"]);
        assert_eq!(holding(&installed, &asked_for, "gtk4"), PackageState::Ok);
    }

    #[test]
    fn a_package_that_came_in_with_something_else_is_only_borrowed() {
        let installed = names(&["glib2", "gtk4"]);
        let asked_for = names(&["gtk4"]);
        assert_eq!(holding(&installed, &asked_for, "glib2"), PackageState::Borrowed);
    }

    #[test]
    fn a_package_nothing_has_is_missing() {
        assert_eq!(holding(&[], &[], "wtype"), PackageState::Missing);
    }

    #[test]
    fn only_a_package_someone_asked_for_is_settled() {
        let Ok(asked_for) = PackageState::Ok.settled();
        let Ok(borrowed) = PackageState::Borrowed.settled();
        let Ok(missing) = PackageState::Missing.settled();

        assert_eq!(asked_for, Settled::Yes);
        assert_eq!(borrowed, Settled::No);
        assert_eq!(missing, Settled::No);
    }

    #[test]
    fn what_apply_installs_and_what_it_claims_are_different_lists() {
        let named = names(&["glib2", "gtk4", "wtype"]);
        let installed = names(&["glib2", "gtk4"]);
        let asked_for = names(&["gtk4"]);
        assert_eq!(lacking(&named, &installed), ["wtype"]);
        assert_eq!(lent(&named, &installed, &asked_for), ["glib2"]);
    }

    #[test]
    fn nothing_is_both_missing_and_borrowed() {
        let named = names(&["glib2", "wtype"]);
        let installed = names(&["glib2"]);
        let asked_for = names(&[]);
        let missing = lacking(&named, &installed);
        for package in lent(&named, &installed, &asked_for) {
            assert!(!missing.contains(&package));
        }
    }
}
