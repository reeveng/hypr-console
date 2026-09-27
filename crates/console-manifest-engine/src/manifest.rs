//! What the desktop is made of, as `desktop.conf` says it.
//!
//! The manifest is the source of truth and everything else here is only the
//! engine that reads it. Anything installed or enabled outside it is invisible,
//! which is the point: a desktop assembled by hand is one no one can put back
//! together.
//!
//! # Who owns what is inside a file
//!
//! A path in `[files]` says the file must be there. It has always been read as
//! saying more than that -- that what is in it is what the tree ships, so
//! anything else is drift -- and for most of them that is exactly right. Two
//! kinds of file it is wrong about, and both were being reported as changed on
//! every boot of a machine where nothing had changed: the bar's own width,
//! which was written at every login by `console-scale` with the size the screen
//! was really standing at, and `zz-steamos-autologin.conf`, which
//! `steamos-session-select` rewrites on the way into Game Mode and back. Both
//! are ours to put there and neither is ours afterwards. The first of the two
//! is gone -- the bar asks the compositor how wide the screen is now -- and the
//! word stays, because the second is still true and machines.conf carries it.
//!
//! A card that names two files that are always named teaches the person to read
//! past it, and that cost a morning once: an inputplumber upgrade laid its own
//! `50-legion_go.yaml` back over ours, the touchpad went dead, and the card said
//! so in the same sentence and the same color as the two that mean nothing.
//!
//! So a path may carry one word after it. `once` says the manifest ships what
//! the file starts as and no more: it is installed when it is not there, it is
//! never compared, `console save` with nothing named does not sweep it back into
//! the tree, and a difference in it is not news. It is only accepted in
//! `[files]`, because a package or a unit has no inside for anyone to own.
//!
//! The word was `theirs` until the day it was named for what it does, and the
//! machine keeps every commit it applied. Pruning reads those commits to learn
//! what it placed, so a manifest read as [`Reading::Recorded`] still takes the
//! old word and means the same thing by it. A manifest being applied is read as
//! [`Reading::Today`] and refuses it, because a word nobody writes any more is
//! one somebody wrote by mistake.

use std::collections::{BTreeMap, BTreeSet};

use console_core_ini_files::{heading, without_a_comment};
use console_core_never::Never;

use crate::unapplied::Unapplied;

pub use console_manifest_migrations::Section;

pub const ONCE: &str = "once";

pub const ONCE_AS_IT_WAS: &str = "theirs";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    Today,
    Recorded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    Always,
    Once,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    sections: BTreeMap<Section, Vec<String>>,
    written_once: BTreeSet<String>,
}

pub use console_repository::MARK;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Configuration<'a>(pub &'a str);

impl Manifest {
    pub fn read(text: &str) -> Result<Self, Unapplied> {
        Manifest::read_as(text, Reading::Today)
    }

    pub fn read_as(text: &str, reading: Reading) -> Result<Self, Unapplied> {
        Manifest::default().folding(Configuration(MARK), text, reading)
    }

    pub fn and(self, configuration: Configuration<'_>, text: &str) -> Result<Self, Unapplied> {
        self.and_as(configuration, text, Reading::Today)
    }

    pub fn and_as(self, configuration: Configuration<'_>, text: &str, reading: Reading) -> Result<Self, Unapplied> {
        let Configuration(file) = configuration;
        let added = Manifest::default().folding(configuration, text, reading)?;

        for section in Section::EVERY {
            let Ok(held) = self.of(section);
            let Ok(coming) = added.of(section);
            let already: BTreeSet<&String> = held.iter().collect();
            let twice = coming.iter().find(|entry| already.contains(entry));
            let Ok(under) = section.name();

            match twice {
                Some(entry) => {
                    return Err(Unapplied::TwiceSaid(
                        file.to_string(),
                        entry.to_string(),
                        under.to_string(),
                    ));
                }
                None => {},
            }
        }

        let mut held = self;

        for (section, entries) in &added.sections {
            let Ok(opened) = held.opening(*section);

            held = opened;

            for entry in entries {
                let Ok(written) = added.write_policy(entry);
                let Ok(holding) = held.with_entry(*section, entry, written);

                held = holding;
            }
        }

        Ok(held)
    }

    fn folding(self, configuration: Configuration<'_>, text: &str, reading: Reading) -> Result<Self, Unapplied> {
        let Configuration(file) = configuration;

        text.lines()
            .map(|line| {
                let Ok(said) = without_a_comment(line);

                said
            })
            .filter(|line| !line.is_empty())
            .try_fold(
                (self, None),
                |(held, current), line| {
                    let Ok(heading) = heading(line);

                    match heading {
                        Some(name) => {
                            let Ok(named) = Section::from_name(name);

                            match named {
                                Some(section) => {
                                    let Ok(opened) = held.opening(section);

                                    Ok((opened, Some(section)))
                                }
                                None => Err(Unapplied::NoSuchSection(
                                    file.to_string(),
                                    name.to_string(),
                                )),
                            }
                        }
                        None => match current {
                            Some(section) => {
                                let (name, written) = said_in(configuration, section, line, reading)?;
                                let Ok(holding) = held.with_entry(section, &name, written);

                                Ok((holding, current))
                            }
                            None => Err(Unapplied::BeforeAnySection(
                                file.to_string(),
                                line.to_string(),
                            )),
                        },
                    }
                },
            )
            .map(|(held, _)| held)
    }

    pub fn of(&self, section: Section) -> Result<&[String], Never> {
        Ok(self.sections.get(&section).map_or(&[], Vec::as_slice))
    }

    pub fn write_policy(&self, path: &str) -> Result<Written, Never> {
        Ok(match self.written_once.contains(path) {
            true => Written::Once,
            false => Written::Always,
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

    fn with_entry(mut self, section: Section, name: &str, written: Written) -> Result<Self, Never> {
        self.sections.entry(section).or_default().push(name.to_owned());

        match written {
            Written::Once => {
                let _ = self.written_once.insert(name.to_owned());
            }
            Written::Always => {},
        }

        Ok(self)
    }
}

fn said_in(
    configuration: Configuration<'_>,
    section: Section,
    entry: &str,
    reading: Reading,
) -> Result<(String, Written), Unapplied> {
    let Configuration(file) = configuration;

    let mut words = entry.split_whitespace();

    let name = match words.next() {
        Some(name) => name.to_string(),
        None => String::new(),
    };

    let rest: Vec<&str> = words.collect();
    let Ok(under) = section.name();

    Ok(match (rest.as_slice(), reading) {
        ([], _) => (name, Written::Always),
        ([ONCE], _) | ([ONCE_AS_IT_WAS], Reading::Recorded) => match section {
            Section::Files => (name, Written::Once),
            Section::Packages
            | Section::Build
            | Section::Services
            | Section::Masked
            | Section::Elsewhere => {
                return Err(Unapplied::OnceIsForFiles(
                    file.to_string(),
                    entry.to_string(),
                    under.to_string(),
                ));
            }
        },
        ([ONCE_AS_IT_WAS], Reading::Today) | (_, Reading::Today | Reading::Recorded) => {
            return Err(Unapplied::OnlyOnce(
                file.to_string(),
                entry.to_string(),
                under.to_string(),
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    fn sections(read: &Manifest) -> Result<Vec<(Section, &[String])>, Never> {
        let Ok(sections) = read.sections();

        Ok(sections.collect())
    }

    #[test]
    fn entries_are_kept_under_the_section_they_were_written_in() -> Result<(), Failure> {
        let read = Manifest::read("[packages]\nhyprland\nwofi\n\n[services]\nconsole.target\n")?;
        let Ok(packages) = read.of(Section::Packages);
        let Ok(services) = read.of(Section::Services);

        assert_eq!(packages, ["hyprland", "wofi"]);
        assert_eq!(services, ["console.target"]);

        Ok(())
    }

    #[test]
    fn a_comment_is_dropped_wherever_it_sits() -> Result<(), Failure> {
        let read = Manifest::read("# a heading\n[packages]\nhyprland  # the compositor\n#wofi\n")?;
        let Ok(packages) = read.of(Section::Packages);

        assert_eq!(packages, ["hyprland"]);

        Ok(())
    }

    #[test]
    fn a_section_written_twice_keeps_both_halves() -> Result<(), Failure> {
        let read = Manifest::read("[packages]\none\n[files]\n/etc/a\n[packages]\ntwo\n")?;
        let Ok(packages) = read.of(Section::Packages);

        assert_eq!(packages, ["one", "two"]);

        Ok(())
    }

    #[test]
    fn the_public_copy_names_what_it_does_not_carry_and_the_engine_opens_it() -> Result<(), Failure> {
        let read = Manifest::read(
            "[files]\n/usr/local/bin/launcher\n\n[elsewhere]\n/usr/local/bin/kew\n",
        )?;
        let Ok(every) = sections(&read);
        let Ok(elsewhere) = read.of(Section::Elsewhere);

        assert_eq!(elsewhere, ["/usr/local/bin/kew"]);
        assert!(!every.iter().any(|(section, _)| *section == Section::Elsewhere));

        Ok(())
    }

    #[test]
    fn a_file_something_else_writes_is_a_path_with_a_word_after_it() -> Result<(), Failure> {
        let read = Manifest::read("[files]\n/etc/a\n/home/@user@/.config/console/bar.css once\n")?;
        let Ok(files) = read.of(Section::Files);

        assert_eq!(files, ["/etc/a", "/home/@user@/.config/console/bar.css"]);

        let Ok(replaced) = read.write_policy("/etc/a");
        let Ok(kept) = read.write_policy("/home/@user@/.config/console/bar.css");

        assert_eq!(replaced, Written::Always);
        assert_eq!(kept, Written::Once);

        Ok(())
    }

    #[test]
    fn a_recorded_manifest_says_once_in_the_word_it_used_to_have() -> Result<(), Failure> {
        let said = "[files]\n/etc/plasmalogin.conf.d/zz-steamos-autologin.conf theirs\n";
        let recorded = Manifest::read_as(said, Reading::Recorded)?;
        let Ok(written) = recorded.write_policy("/etc/plasmalogin.conf.d/zz-steamos-autologin.conf");

        assert_eq!(written, Written::Once);

        Ok(())
    }

    #[test]
    fn the_word_it_used_to_have_is_refused_in_a_manifest_being_applied() {
        let said = "[files]\n/etc/plasmalogin.conf.d/zz-steamos-autologin.conf theirs\n";

        assert!(matches!(Manifest::read(said), Err(Unapplied::OnlyOnce(..))));
    }

    #[test]
    fn a_path_no_one_marked_is_ours_and_so_is_a_path_the_manifest_never_named() -> Result<(), Failure> {
        let read = Manifest::read("[files]\n/etc/a\n")?;
        let Ok(named) = read.write_policy("/etc/a");
        let Ok(never) = read.write_policy("/etc/somewhere-else");

        assert_eq!(named, Written::Always);
        assert_eq!(never, Written::Always);

        Ok(())
    }

    #[test]
    fn a_word_after_a_path_that_no_one_reads_is_refused_rather_than_taken_as_part_of_it() -> Result<(), Failure> {
        let fault = match Manifest::read("[files]\n/etc/a mine\n") {
            Ok(_) => return Err(Failure::from("no such word, and it was taken")),
            Err(fault) => fault,
        };

        assert!(fault.to_string().contains("once"), "{fault}");
        assert!(fault.to_string().contains("/etc/a mine"), "{fault}");

        Ok(())
    }

    #[test]
    fn only_a_file_has_an_inside_for_anyone_to_own() -> Result<(), Failure> {
        let fault = match Manifest::read("[packages]\nhyprland once\n") {
            Ok(_) => return Err(Failure::from("not a file, and it was taken")),
            Err(fault) => fault,
        };

        assert!(fault.to_string().contains("packages"), "{fault}");

        let fault = match Manifest::read("[services]\nconsole.target once\n") {
            Ok(_) => return Err(Failure::from("not a file, and it was taken")),
            Err(fault) => fault,
        };

        assert!(fault.to_string().contains("services"), "{fault}");

        Ok(())
    }

    #[test]
    fn the_manifest_this_desktop_wears_marks_the_files_something_else_on_it_writes() -> Result<(), Failure> {
        let held = include_str!("../../../desktop.conf");
        let read = Manifest::read(held)?;
        let Ok(hyprland) = read.write_policy("/home/@user@/.config/console/hypr/hyprland.lua");

        assert_eq!(hyprland, Written::Always);

        Ok(())
    }

    #[test]
    fn a_machines_own_block_marks_them_too_and_is_read_by_the_same_code() -> Result<(), Failure> {
        let held = include_str!("../../../machines.conf");
        let Ok(mine) = crate::machines::of(held, crate::machines::Named("legion-go"));
        let read = Manifest::read(&mine)?;
        let Ok(session) = read.write_policy("/etc/plasmalogin.conf.d/zz-steamos-autologin.conf");

        assert_eq!(session, Written::Once, "steamos-session-select rewrites this on the way out");

        let Ok(profiles) = read.write_policy("/home/@user@/.librewolf/profiles.ini");

        assert_eq!(profiles, Written::Once, "the browser keeps its own list of profiles");

        Ok(())
    }

    #[test]
    fn a_line_in_both_files_is_one_of_them_being_wrong() -> Result<(), Failure> {
        let read = Manifest::read("[files]\n/etc/a\n")?;
        let fault = match read.and(Configuration("machines.conf"), "[files]\n/etc/a\n") {
            Ok(_) => return Err(Failure::from("twice, and it was taken")),
            Err(fault) => fault,
        };


        assert!(fault.to_string().contains("/etc/a"), "{fault}");
        assert!(fault.to_string().contains("machines.conf"), "{fault}");

        Ok(())
    }

    #[test]
    fn a_machine_with_no_block_gets_the_manifest_and_nothing_else() -> Result<(), Failure> {
        let read = Manifest::read("[files]\n/etc/a\n")?;
        let whole = read.and(Configuration("machines.conf"), "")?;
        let Ok(files) = whole.of(Section::Files);

        assert_eq!(files, ["/etc/a"]);

        Ok(())
    }

    #[test]
    fn a_section_no_one_named_is_refused_rather_than_skipped() -> Result<(), Failure> {
        let fault = match Manifest::read("[packagez]\nhyprland\n") {
            Ok(_) => return Err(Failure::from("no such section, and it was taken")),
            Err(fault) => fault,
        };

        assert!(fault.to_string().contains("packagez"), "{fault}");

        Ok(())
    }

    #[test]
    fn an_entry_before_any_section_is_refused() -> Result<(), Failure> {
        let fault = match Manifest::read("hyprland\n[packages]\n") {
            Ok(_) => return Err(Failure::from("nowhere to put it, and it was taken")),
            Err(fault) => fault,
        };

        assert!(fault.to_string().contains("hyprland"), "{fault}");

        Ok(())
    }

    #[test]
    fn an_empty_section_is_read_as_empty_and_not_as_absent() -> Result<(), Failure> {
        let read = Manifest::read("[masked]\n")?;
        let Ok(every) = sections(&read);
        let Ok(masked) = read.of(Section::Masked);

        assert!(masked.is_empty(), "{masked:?}");
        assert_eq!(every.len(), 1);

        Ok(())
    }

    #[test]
    fn a_section_never_written_is_empty_rather_than_a_fault() -> Result<(), Failure> {
        let read = Manifest::read("[packages]\none\n")?;
        let Ok(build) = read.of(Section::Build);

        assert!(build.is_empty(), "{build:?}");

        Ok(())
    }

    #[test]
    fn sections_come_back_in_the_order_they_are_acted_on() -> Result<(), Failure> {
        let read = Manifest::read("[services]\na\n[build]\nb\n[packages]\nc\n[files]\n/d\n")?;
        let Ok(every) = sections(&read);
        let order: Vec<&str> = every
            .into_iter()
            .map(|(section, _)| {
                let Ok(name) = section.name();

                name
            })
            .collect();

        assert_eq!(order, ["packages", "build", "files", "services"]);

        Ok(())
    }

    #[test]
    fn the_manifest_this_desktop_is_actually_made_of_reads() -> Result<(), Failure> {
        let held = include_str!("../../../desktop.conf");
        let read = Manifest::read(held)?;
        let Ok(packages) = read.of(Section::Packages);
        let Ok(files) = read.of(Section::Files);

        assert!(!packages.is_empty());
        assert!(!files.is_empty());

        for path in files {
            assert!(path.starts_with('/'), "{path:?} is not an absolute path");
        }

        Ok(())
    }
}
