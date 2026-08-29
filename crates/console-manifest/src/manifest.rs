//! What the desktop is made of, as `desktop.conf` says it.
//!
//! The manifest is the source of truth and everything else here is only the
//! engine that reads it. Anything installed or enabled outside it is invisible,
//! which is the point: a desktop assembled by hand is one nobody can put back
//! together.

use std::collections::BTreeMap;

/// The sections a manifest may hold, in the order they are acted on.
///
/// Packages first, because a file may belong to one. Built programs next,
/// because compiling needs the toolchain the packages brought. Then files,
/// then the units that run them.
///
/// `Elsewhere` is acted on by nothing. It is how the public copy names the two
/// forks it does not carry, so that a unit starting a program nothing installs
/// stays the failure it should be. Read rather than refused, because a copy of
/// this desktop that its own engine will not open is not a copy of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Packages,
    Build,
    Files,
    Services,
    Masked,
    Elsewhere,
}

impl Section {
    pub const EVERY: [Section; 5] = [
        Section::Packages,
        Section::Build,
        Section::Files,
        Section::Services,
        Section::Masked,
    ];

    pub fn named(name: &str) -> Option<Self> {
        match name {
            "packages" => Some(Section::Packages),
            "build" => Some(Section::Build),
            "files" => Some(Section::Files),
            "services" => Some(Section::Services),
            "masked" => Some(Section::Masked),
            "elsewhere" => Some(Section::Elsewhere),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Section::Packages => "packages",
            Section::Build => "build",
            Section::Files => "files",
            Section::Services => "services",
            Section::Masked => "masked",
            Section::Elsewhere => "elsewhere",
        }
    }
}

/// The whole inventory, read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest(BTreeMap<Section, Vec<String>>);

impl Manifest {
    /// The manifest as sections of entries, comments and blank lines dropped.
    ///
    /// A section nobody wrote a name for is refused rather than skipped. A
    /// typo in a heading would otherwise take everything under it out of the
    /// inventory silently, and the machine would keep whatever those lines
    /// were about while the manifest said nothing about it.
    pub fn read(text: &str) -> Result<Self, String> {
        text.lines()
            .map(|line| line.split('#').next().unwrap_or("").trim())
            .filter(|line| !line.is_empty())
            .try_fold(
                (Manifest::default(), None),
                |(held, current), line| match heading(line) {
                    Some(name) => match Section::named(name) {
                        Some(section) => Ok((held.opening(section), Some(section))),
                        None => Err(format!("desktop.conf has a section called [{name}], which is not one this reads")),
                    },
                    None => match current {
                        Some(section) => Ok((held.holding(section, line), current)),
                        None => Err(format!("desktop.conf has {line:?} before any section")),
                    },
                },
            )
            .map(|(held, _)| held)
    }

    pub fn of(&self, section: Section) -> &[String] {
        self.0.get(&section).map_or(&[], Vec::as_slice)
    }

    /// The sections that are acted on, in the order they are acted on.
    ///
    /// `Elsewhere` is not among them: it is read so the manifest opens, and
    /// then it is nobody's work.
    pub fn sections(&self) -> impl Iterator<Item = (Section, &[String])> {
        Section::EVERY
            .into_iter()
            .filter(|section| self.0.contains_key(section))
            .map(|section| (section, self.of(section)))
    }

    #[must_use]
    fn opening(mut self, section: Section) -> Self {
        self.0.entry(section).or_default();
        self
    }

    #[must_use]
    fn holding(mut self, section: Section, entry: &str) -> Self {
        self.0.entry(section).or_default().push(entry.to_owned());
        self
    }
}

fn heading(line: &str) -> Option<&str> {
    line.strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_kept_under_the_section_they_were_written_in() {
        let read = Manifest::read("[packages]\nhyprland\nwofi\n\n[services]\nconsole.target\n")
            .expect("it reads");
        assert_eq!(read.of(Section::Packages), ["hyprland", "wofi"]);
        assert_eq!(read.of(Section::Services), ["console.target"]);
    }

    #[test]
    fn a_comment_is_dropped_wherever_it_sits() {
        let read = Manifest::read("# a heading\n[packages]\nhyprland  # the compositor\n#wofi\n")
            .expect("it reads");
        assert_eq!(read.of(Section::Packages), ["hyprland"]);
    }

    #[test]
    fn a_section_written_twice_keeps_both_halves() {
        // The manifest groups packages under prose headings, so a section is
        // reopened all the time and neither half may be lost.
        let read = Manifest::read("[packages]\none\n[files]\n/etc/a\n[packages]\ntwo\n")
            .expect("it reads");
        assert_eq!(read.of(Section::Packages), ["one", "two"]);
    }

    #[test]
    fn the_public_copy_names_what_it_does_not_carry_and_the_engine_opens_it() {
        let read = Manifest::read(
            "[files]\n/usr/local/bin/launcher\n\n[elsewhere]\n/usr/local/bin/hyprsession\n",
        )
        .expect("a published manifest opens");
        assert_eq!(read.of(Section::Elsewhere), ["/usr/local/bin/hyprsession"]);
        // Read so the manifest opens, and then nobody's work.
        assert!(
            !read
                .sections()
                .any(|(section, _)| section == Section::Elsewhere)
        );
    }

    #[test]
    fn a_section_nobody_named_is_refused_rather_than_skipped() {
        // A typo in a heading would otherwise take everything under it out of
        // the inventory, silently, and the machine would keep it anyway.
        let fault = Manifest::read("[packagez]\nhyprland\n").expect_err("no such section");
        assert!(fault.contains("packagez"), "{fault}");
    }

    #[test]
    fn an_entry_before_any_section_is_refused() {
        let fault = Manifest::read("hyprland\n[packages]\n").expect_err("nowhere to put it");
        assert!(fault.contains("hyprland"), "{fault}");
    }

    #[test]
    fn an_empty_section_is_read_as_empty_and_not_as_absent() {
        let read = Manifest::read("[masked]\n").expect("it reads");
        assert_eq!(read.of(Section::Masked), [] as [String; 0]);
        assert_eq!(read.sections().count(), 1);
    }

    #[test]
    fn a_section_never_written_is_empty_rather_than_a_fault() {
        let read = Manifest::read("[packages]\none\n").expect("it reads");
        assert_eq!(read.of(Section::Build), [] as [String; 0]);
    }

    #[test]
    fn sections_come_back_in_the_order_they_are_acted_on() {
        // Packages before build, because compiling needs the toolchain they
        // bring; build before files, because a built program is a file.
        let read = Manifest::read("[services]\na\n[build]\nb\n[packages]\nc\n[files]\n/d\n")
            .expect("it reads");
        let order: Vec<&str> = read.sections().map(|(section, _)| section.name()).collect();
        assert_eq!(order, ["packages", "build", "files", "services"]);
    }

    #[test]
    fn the_manifest_this_desktop_is_actually_made_of_reads() {
        let held = include_str!("../../../desktop.conf");
        let read = Manifest::read(held).expect("desktop.conf reads");
        assert!(!read.of(Section::Packages).is_empty());
        assert!(!read.of(Section::Files).is_empty());
        for path in read.of(Section::Files) {
            assert!(path.starts_with('/'), "{path:?} is not an absolute path");
        }
    }
}
