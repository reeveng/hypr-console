//! What the desktop is made of, as `desktop.conf` says it.
//!
//! The manifest is the source of truth and everything else here is only the
//! engine that reads it. Anything installed or enabled outside it is invisible,
//! which is the point: a desktop assembled by hand is one nobody can put back
//! together.
//!
//! # Who owns what is inside a file
//!
//! A path in `[files]` says the file must be there. It has always been read as
//! saying more than that -- that what is in it is what the tree ships, so
//! anything else is drift -- and for most of them that is exactly right. Two
//! kinds of file it is wrong about, and both were being reported as changed on
//! every boot of a machine where nothing had changed: `bar.css`, which
//! `console-scale apply` writes at every login with the width the screen is
//! really standing at, and `zz-steamos-autologin.conf`, which
//! `steamos-session-select` rewrites on the way into Game Mode and back. Both
//! are ours to put there and neither is ours afterwards.
//!
//! A card that names two files that are always named teaches the person to read
//! past it, and that cost a morning once: an inputplumber upgrade laid its own
//! `50-legion_go.yaml` back over ours, the touchpad went dead, and the card said
//! so in the same sentence and the same colour as the two that mean nothing.
//!
//! So a path may carry one word after it. `theirs` says the manifest ships what
//! the file starts as and no more: it is installed when it is not there, it is
//! never compared, `console save` with nothing named does not sweep it back into
//! the tree, and a difference in it is not news. It is only accepted in
//! `[files]`, because a package or a unit has no inside for anybody to own.

use std::collections::{BTreeMap, BTreeSet};

use console_core_ini_files::heading;
use console_core_never::Never;

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

    pub fn named(name: &str) -> Result<Option<Self>, Never> {
        Ok(match name {
            "packages" => Some(Section::Packages),
            "build" => Some(Section::Build),
            "files" => Some(Section::Files),
            "services" => Some(Section::Services),
            "masked" => Some(Section::Masked),
            "elsewhere" => Some(Section::Elsewhere),
            _ => None,
        })
    }

    pub fn name(self) -> Result<&'static str, Never> {
        Ok(match self {
            Section::Packages => "packages",
            Section::Build => "build",
            Section::Files => "files",
            Section::Services => "services",
            Section::Masked => "masked",
            Section::Elsewhere => "elsewhere",
        })
    }
}

pub const THEIRS: &str = "theirs";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whose {
    Ours,
    Theirs,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    sections: BTreeMap<Section, Vec<String>>,
    theirs: BTreeSet<String>,
}

impl Manifest {
    pub fn read(text: &str) -> Result<Self, String> {
        text.lines()
            .map(|line| line.split('#').next().unwrap_or("").trim())
            .filter(|line| !line.is_empty())
            .try_fold(
                (Manifest::default(), None),
                |(held, current), line| {
                    let Ok(heading) = heading(line);

                    match heading {
                        Some(name) => {
                            let Ok(named) = Section::named(name);

                            match named {
                                Some(section) => {
                                    let Ok(opened) = held.opening(section);

                                    Ok((opened, Some(section)))
                                }
                                None => Err(format!("desktop.conf has a section called [{name}], which is not one this reads")),
                            }
                        }
                        None => match current {
                            Some(section) => {
                                let (name, whose) = said(section, line)?;
                                let Ok(holding) = held.holding(section, &name, whose);

                                Ok((holding, current))
                            }
                            None => Err(format!("desktop.conf has {line:?} before any section")),
                        },
                    }
                },
            )
            .map(|(held, _)| held)
    }

    pub fn of(&self, section: Section) -> Result<&[String], Never> {
        Ok(self.sections.get(&section).map_or(&[], Vec::as_slice))
    }

    pub fn whose(&self, path: &str) -> Result<Whose, Never> {
        Ok(match self.theirs.contains(path) {
            true => Whose::Theirs,
            false => Whose::Ours,
        })
    }

    pub fn sections(&self) -> Result<impl Iterator<Item = (Section, &[String])>, Never> {
        Ok(Section::EVERY
            .into_iter()
            .filter(|section| self.sections.contains_key(section))
            .map(|section| {
                let Ok(of) = self.of(section);

                (section, of)
            }))
    }

    fn opening(mut self, section: Section) -> Result<Self, Never> {
        self.sections.entry(section).or_default();

        Ok(self)
    }

    fn holding(mut self, section: Section, name: &str, whose: Whose) -> Result<Self, Never> {
        self.sections.entry(section).or_default().push(name.to_owned());

        match whose {
            Whose::Theirs => {
                let _ = self.theirs.insert(name.to_owned());
            }
            Whose::Ours => {},
        }

        Ok(self)
    }
}

fn said(section: Section, entry: &str) -> Result<(String, Whose), String> {
    let mut words = entry.split_whitespace();
    let name = words.next().unwrap_or("").to_string();
    let rest: Vec<&str> = words.collect();
    let Ok(under) = section.name();

    Ok(match rest.as_slice() {
        [] => (name, Whose::Ours),
        [THEIRS] => match section {
            Section::Files => (name, Whose::Theirs),
            Section::Packages
            | Section::Build
            | Section::Services
            | Section::Masked
            | Section::Elsewhere => {
                return Err(format!(
                    "desktop.conf says {entry:?} under [{under}], and {THEIRS} is a word only a \
                     file takes: a package or a unit has no inside for anybody to own"
                ));
            }
        },
        _ => {
            return Err(format!(
                "desktop.conf says {entry:?} under [{under}], and the only word an entry takes \
                 after it is {THEIRS}"
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(read: &Manifest, section: Section) -> &[String] {
        let Ok(of) = read.of(section);

        of
    }

    fn sections(read: &Manifest) -> Vec<(Section, &[String])> {
        let Ok(sections) = read.sections();

        sections.collect()
    }

    fn name(section: Section) -> &'static str {
        let Ok(name) = section.name();

        name
    }

    #[test]
    fn entries_are_kept_under_the_section_they_were_written_in() {
        let read = Manifest::read("[packages]\nhyprland\nwofi\n\n[services]\nconsole.target\n")
            .expect("it reads");
        assert_eq!(of(&read, Section::Packages), ["hyprland", "wofi"]);
        assert_eq!(of(&read, Section::Services), ["console.target"]);
    }

    #[test]
    fn a_comment_is_dropped_wherever_it_sits() {
        let read = Manifest::read("# a heading\n[packages]\nhyprland  # the compositor\n#wofi\n")
            .expect("it reads");
        assert_eq!(of(&read, Section::Packages), ["hyprland"]);
    }

    #[test]
    fn a_section_written_twice_keeps_both_halves() {
        let read = Manifest::read("[packages]\none\n[files]\n/etc/a\n[packages]\ntwo\n")
            .expect("it reads");
        assert_eq!(of(&read, Section::Packages), ["one", "two"]);
    }

    #[test]
    fn the_public_copy_names_what_it_does_not_carry_and_the_engine_opens_it() {
        let read = Manifest::read(
            "[files]\n/usr/local/bin/launcher\n\n[elsewhere]\n/usr/local/bin/kew\n",
        )
        .expect("a published manifest opens");
        assert_eq!(of(&read, Section::Elsewhere), ["/usr/local/bin/kew"]);
        assert!(!sections(&read).iter().any(|(section, _)| *section == Section::Elsewhere));
    }

    #[test]
    fn a_file_something_else_writes_is_a_path_with_a_word_after_it() {
        let read = Manifest::read("[files]\n/etc/a\n/home/@user@/.config/console/bar.css theirs\n")
            .expect("it reads");
        assert_eq!(of(&read, Section::Files), ["/etc/a", "/home/@user@/.config/console/bar.css"]);

        let Ok(ours) = read.whose("/etc/a");
        let Ok(theirs) = read.whose("/home/@user@/.config/console/bar.css");

        assert_eq!(ours, Whose::Ours);
        assert_eq!(theirs, Whose::Theirs);
    }

    #[test]
    fn a_path_nobody_marked_is_ours_and_so_is_a_path_the_manifest_never_named() {
        let read = Manifest::read("[files]\n/etc/a\n").expect("it reads");
        let Ok(named) = read.whose("/etc/a");
        let Ok(never) = read.whose("/etc/somewhere-else");

        assert_eq!(named, Whose::Ours);
        assert_eq!(never, Whose::Ours);
    }

    #[test]
    fn a_word_after_a_path_that_nobody_reads_is_refused_rather_than_taken_as_part_of_it() {
        let fault = Manifest::read("[files]\n/etc/a mine\n").expect_err("no such word");
        assert!(fault.contains("theirs"), "{fault}");
        assert!(fault.contains("/etc/a mine"), "{fault}");
    }

    #[test]
    fn only_a_file_has_an_inside_for_anybody_to_own() {
        let fault = Manifest::read("[packages]\nhyprland theirs\n").expect_err("not a file");
        assert!(fault.contains("packages"), "{fault}");

        let fault = Manifest::read("[services]\nconsole.target theirs\n").expect_err("not a file");
        assert!(fault.contains("services"), "{fault}");
    }

    #[test]
    fn the_manifest_this_desktop_wears_marks_the_files_something_else_on_it_writes() {
        let held = include_str!("../../../desktop.conf");
        let read = Manifest::read(held).expect("desktop.conf reads");
        let Ok(bar) = read.whose("/home/@user@/.config/console/bar.css");
        let Ok(session) = read.whose("/etc/plasmalogin.conf.d/zz-steamos-autologin.conf");
        let Ok(hyprland) = read.whose("/home/@user@/.config/hypr/hyprland.lua");

        assert_eq!(bar, Whose::Theirs, "console-scale apply writes this at every login");
        assert_eq!(session, Whose::Theirs, "steamos-session-select rewrites this on the way out");
        assert_eq!(hyprland, Whose::Ours);
    }

    #[test]
    fn a_section_nobody_named_is_refused_rather_than_skipped() {
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
        assert_eq!(of(&read, Section::Masked), [] as [String; 0]);
        assert_eq!(sections(&read).len(), 1);
    }

    #[test]
    fn a_section_never_written_is_empty_rather_than_a_fault() {
        let read = Manifest::read("[packages]\none\n").expect("it reads");
        assert_eq!(of(&read, Section::Build), [] as [String; 0]);
    }

    #[test]
    fn sections_come_back_in_the_order_they_are_acted_on() {
        let read = Manifest::read("[services]\na\n[build]\nb\n[packages]\nc\n[files]\n/d\n")
            .expect("it reads");
        let order: Vec<&str> = sections(&read).into_iter().map(|(section, _)| name(section)).collect();
        assert_eq!(order, ["packages", "build", "files", "services"]);
    }

    #[test]
    fn the_manifest_this_desktop_is_actually_made_of_reads() {
        let held = include_str!("../../../desktop.conf");
        let read = Manifest::read(held).expect("desktop.conf reads");
        assert!(!of(&read, Section::Packages).is_empty());
        assert!(!of(&read, Section::Files).is_empty());
        for path in of(&read, Section::Files) {
            assert!(path.starts_with('/'), "{path:?} is not an absolute path");
        }
    }
}
