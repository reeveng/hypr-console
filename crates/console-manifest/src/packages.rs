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
//! reads as installed, every check passes, and it leaves on somebody else's
//! errand. Three packages were found this way in one evening -- pw-record's,
//! notify-send's and pactl's -- each of them working only because something
//! unrelated had dragged it in, and the manifest is meant to be the answer to
//! exactly that.
//!
//! `Held::Borrowed` is that state said out loud, and `console apply` settles it
//! by telling pacman the desktop asked for the package too, which is true.

/// How the machine is holding a package the manifest names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// Installed, and something asked for it.
    Ok,
    /// Installed, but only because something else needed it, so it leaves when
    /// that does.
    Borrowed,
    Missing,
}

impl Held {
    pub fn name(self) -> &'static str {
        match self {
            Held::Ok => "ok",
            Held::Borrowed => "held as a dependency",
            Held::Missing => "missing",
        }
    }

    pub fn settled(self) -> bool {
        self == Held::Ok
    }
}

/// How the machine holds one package.
///
/// `asked_for` is what pacman was told to install and `installed` is everything
/// that got installed, dependencies and all, so the second contains the first.
pub fn held(installed: &[String], asked_for: &[String], package: &str) -> Held {
    let said = |names: &[String]| names.iter().any(|name| name == package);
    match (said(installed), said(asked_for)) {
        (_, true) => Held::Ok,
        (true, false) => Held::Borrowed,
        (false, false) => Held::Missing,
    }
}

/// The packages the manifest names that the machine has on somebody else's
/// word, which is what `apply` claims for the desktop.
pub fn borrowed<'a>(
    named: &'a [String],
    installed: &[String],
    asked_for: &[String],
) -> Vec<&'a str> {
    named
        .iter()
        .filter(|package| held(installed, asked_for, package) == Held::Borrowed)
        .map(String::as_str)
        .collect()
}

/// The packages the manifest names that are not on the machine at all.
pub fn missing<'a>(named: &'a [String], installed: &[String]) -> Vec<&'a str> {
    named
        .iter()
        .filter(|package| !installed.iter().any(|name| name == *package))
        .map(String::as_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(said: &[&str]) -> Vec<String> {
        said.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn a_package_somebody_asked_for_is_held() {
        let installed = names(&["glib2", "gtk4"]);
        let asked_for = names(&["gtk4"]);
        assert_eq!(held(&installed, &asked_for, "gtk4"), Held::Ok);
    }

    #[test]
    fn a_package_that_came_in_with_something_else_is_only_borrowed() {
        let installed = names(&["glib2", "gtk4"]);
        let asked_for = names(&["gtk4"]);
        assert_eq!(held(&installed, &asked_for, "glib2"), Held::Borrowed);
    }

    #[test]
    fn a_package_nothing_has_is_missing() {
        assert_eq!(held(&[], &[], "wtype"), Held::Missing);
    }

    /// Borrowed is a difference, because the package leaves when the thing that
    /// brought it in does and nothing here would have said so.
    #[test]
    fn only_a_package_somebody_asked_for_is_settled() {
        assert!(Held::Ok.settled());
        assert!(!Held::Borrowed.settled());
        assert!(!Held::Missing.settled());
    }

    #[test]
    fn what_apply_installs_and_what_it_claims_are_different_lists() {
        let named = names(&["glib2", "gtk4", "wtype"]);
        let installed = names(&["glib2", "gtk4"]);
        let asked_for = names(&["gtk4"]);
        assert_eq!(missing(&named, &installed), ["wtype"]);
        assert_eq!(borrowed(&named, &installed, &asked_for), ["glib2"]);
    }

    /// Installing a missing package makes pacman ask for it, so the two never
    /// overlap in one run and nothing is claimed that was just installed.
    #[test]
    fn nothing_is_both_missing_and_borrowed() {
        let named = names(&["glib2", "wtype"]);
        let installed = names(&["glib2"]);
        let asked_for = names(&[]);
        let missing = missing(&named, &installed);
        for package in borrowed(&named, &installed, &asked_for) {
            assert!(!missing.contains(&package));
        }
    }
}
