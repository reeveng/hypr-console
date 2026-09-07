//! What the desktop is made of, as `desktop.conf` says it.
//!
//! The manifest is the source of truth and everything else here is only the
//! engine that reads it. Anything installed or enabled outside it is invisible,
//! which is the point: a desktop assembled by hand is one nobody can put back
//! together.

use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest(BTreeMap<Section, Vec<String>>);

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
                                let Ok(holding) = held.holding(section, line);

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
        Ok(self.0.get(&section).map_or(&[], Vec::as_slice))
    }

    pub fn sections(&self) -> Result<impl Iterator<Item = (Section, &[String])>, Never> {
        Ok(Section::EVERY
            .into_iter()
            .filter(|section| self.0.contains_key(section))
            .map(|section| {
                let Ok(of) = self.of(section);

                (section, of)
            }))
    }

    fn opening(mut self, section: Section) -> Result<Self, Never> {
        self.0.entry(section).or_default();

        Ok(self)
    }

    fn holding(mut self, section: Section, entry: &str) -> Result<Self, Never> {
        self.0.entry(section).or_default().push(entry.to_owned());

        Ok(self)
    }
}

fn heading(line: &str) -> Result<Option<&str>, Never> {
    Ok(line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')))
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
            "[files]\n/usr/local/bin/launcher\n\n[elsewhere]\n/usr/local/bin/hyprsession\n",
        )
        .expect("a published manifest opens");
        assert_eq!(of(&read, Section::Elsewhere), ["/usr/local/bin/hyprsession"]);
        assert!(!sections(&read).iter().any(|(section, _)| *section == Section::Elsewhere));
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
