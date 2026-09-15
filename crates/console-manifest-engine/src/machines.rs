//! What is true on one machine and not on the others.
//!
//! `desktop.conf` is this desktop, and for a year it was also this handheld:
//! the pad's own device definition, the rule that silences its motors, the
//! Steam session the left button crosses to. Every one of those lines is right
//! and every one of them is about a Lenovo Legion Go, so a machine that is not
//! one had no way to apply the manifest without being handed another machine's
//! hardware.
//!
//! The answer is not a file per machine. A desktop that cannot be installed
//! until somebody writes a file for their laptop is a desktop with one
//! installation, and a manifest that grows a section per device is a manifest
//! nobody can read. So this is the exceptions, most machines match nothing in
//! it, and what they get is `desktop.conf` and nothing else.
//!
//! A machine is named by what its firmware calls itself. `matches` is read
//! against `product_name` and `product_family` under `/sys/class/dmi/id`, case
//! folded, a part of the name being enough -- which is the question omarchy's
//! `omarchy-hw-match` asks for each of its own hardware fixes, and the same one
//! InputPlumber's device files ask in their `dmi_data` block. Two answers
//! already agree on where this fact lives, and the kernel's own panel
//! orientation quirks and systemd's hwdb are the same table a layer down.
//!
//! What is not here, and must not arrive: a condition, a default, a value
//! computed from another value. A block is selected or it is not, and what is
//! inside it is the manifest's own vocabulary, laid down after `desktop.conf`
//! in the same order by the same code. The moment this file can compute, the
//! machine stops being readable as a description of itself.
//!
//! [`here`] is what this machine gets, which is one block or none. What any
//! machine gets is a different question and is asked on a laptop rather than on
//! a machine: `the_tree` crosses every file under `files/` against the manifest,
//! and a path carried for the handheld is carried, so that test reads both files
//! and this one does not have to answer for it.

use std::collections::BTreeMap;
use std::path::Path;

use console_core_ini_files::{Key, Under, field, heading};
use console_core_never::Never;

use crate::unapplied::Unapplied;

pub const AT: &str = "machines.conf";

pub const MATCHES: &str = "matches";

pub const FIRMWARE: &str = "/sys/class/dmi/id";

pub const ASKED: [&str; 2] = ["product_name", "product_family"];

const JOINED: char = '.';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Named<'a>(pub &'a str);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub named: String,
    pub matches: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fits {
    ThisMachine,
    Another,
}

pub fn every(said: &str) -> Result<Vec<Machine>, Unapplied> {
    let mut every: Vec<Machine> = Vec::new();

    for line in said.lines() {
        let Ok(heading) = heading(line);

        let named = match heading {
            Some(named) => named,
            None => continue,
        };

        match named.contains(JOINED) {
            true => continue,
            false => {},
        }

        let Ok(matches) = field(said, Under(named), Key(MATCHES));

        let matches = match matches {
            Some(matches) => matches,
            None => {
                return Err(Unapplied::NoMatches(named.to_owned()));
            }
        };

        every.push(Machine { named: named.to_owned(), matches: matches.to_owned() });
    }

    Ok(every)
}

pub fn called(firmware: &Path) -> Result<Vec<String>, Never> {
    let mut every: Vec<String> = Vec::new();

    for asked in ASKED {
        let said = match std::fs::read_to_string(firmware.join(asked)) {
            Ok(said) => said,
            Err(_firmware_that_will_not_say) => continue,
        };

        let said = said.trim().to_lowercase();

        match said.is_empty() {
            true => {},
            false => every.push(said),
        }
    }

    Ok(every)
}

impl Machine {
    pub fn fits(&self, called: &[String]) -> Result<Fits, Never> {
        let wanted = self.matches.trim().to_lowercase();

        Ok(match called.iter().any(|called| called.contains(&wanted)) {
            true => Fits::ThisMachine,
            false => Fits::Another,
        })
    }
}

pub fn entries(said: &str) -> Result<BTreeMap<String, Vec<(String, String)>>, Never> {
    let mut held: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut under: Option<(String, String)> = None;

    for line in said.lines() {
        let Ok(heading) = heading(line);

        match heading {
            Some(named) => {
                under = named
                    .split_once(JOINED)
                    .map(|(machine, section)| (machine.to_owned(), section.to_owned()));
            }
            None => {
                let Ok(entry) = console_core_ini_files::without_a_comment(line);

                match (under.as_ref(), entry.is_empty()) {
                    (Some((machine, section)), false) => {
                        held.entry(machine.clone())
                            .or_default()
                            .push((section.clone(), entry.to_owned()));
                    }
                    (Some(_), true) | (None, _) => {},
                }
            }
        }
    }

    Ok(held)
}

pub fn said_as_manifest(held: &[(String, String)]) -> Result<String, Never> {
    let mut out = String::new();
    let mut under: Option<&str> = None;

    for (section, entry) in held {
        match under == Some(section.as_str()) {
            true => {},
            false => {
                out.push_str(&format!("[{section}]\n"));

                under = Some(section);
            }
        }

        out.push_str(&format!("{entry}\n"));
    }

    Ok(out)
}

pub fn of(said: &str, machine: Named<'_>) -> Result<String, Never> {
    let Named(named) = machine;
    let Ok(entries) = entries(said);

    match entries.get(named) {
        Some(held) => said_as_manifest(held),
        None => Ok(String::new()),
    }
}

pub fn here(said: &str, firmware: &Path) -> Result<String, Unapplied> {
    let every = every(said)?;
    let Ok(called) = called(firmware);

    let mut out = String::new();

    for machine in &every {
        let Ok(fits) = machine.fits(&called);

        match fits {
            Fits::ThisMachine => {
                let Ok(held) = of(said, Named(&machine.named));

                out.push_str(&held);
            }
            Fits::Another => {},
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
[legion-go]
matches = Legion Go

[legion-go.files]
/usr/share/inputplumber/devices/50-legion_go.yaml
/etc/udev/rules.d/92-console-haptics.rules

[legion-go.masked]
cachyos-gamescope-autologin.service

[a-tablet]
matches = Surface Pro

[a-tablet.packages]
iptsd
";

    fn firmware(whose: &str, said: &[(&str, &str)]) -> std::path::PathBuf {
        let at = std::env::temp_dir()
            .join(format!("console-machines-{}-{whose}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);

        std::fs::create_dir_all(&at).expect("somewhere to stand a firmware");

        for (named, holds) in said {
            std::fs::write(at.join(named), format!("{holds}\n")).expect("a firmware file");
        }

        at
    }

    fn here(said: &str, firmware: &Path) -> String {
        match super::here(said, firmware) {
            Ok(out) => out,
            Err(fault) => panic!("{fault}"),
        }
    }

    #[test]
    fn a_machine_is_what_its_firmware_calls_it() {
        let at = firmware("handheld", &[("product_name", "83E1"), ("product_family", "Legion Go")]);

        assert_eq!(
            here(TABLE, &at),
            "[files]\n/usr/share/inputplumber/devices/50-legion_go.yaml\n\
             /etc/udev/rules.d/92-console-haptics.rules\n\
             [masked]\ncachyos-gamescope-autologin.service\n"
        );
    }

    #[test]
    fn a_machine_in_no_block_gets_nothing_rather_than_somebody_elses_hardware() {
        let at = firmware("laptop", &[
            ("product_name", "21MC001RCK"),
            ("product_family", "ThinkPad T14 Gen 5"),
        ]);

        assert_eq!(here(TABLE, &at), "", "a laptop needs no file in this repository");
    }

    #[test]
    fn a_part_of_the_name_is_enough_and_case_is_not_a_question() {
        let at = firmware("tablet", &[("product_name", "Surface Pro 9 for Business")]);

        assert_eq!(here(TABLE, &at), "[packages]\niptsd\n");
    }

    #[test]
    fn firmware_that_says_nothing_at_all_is_a_machine_with_no_block() {
        let at = firmware("nothing", &[]);

        assert_eq!(here(TABLE, &at), "");
    }

    #[test]
    fn a_machine_that_nothing_can_ever_be_is_a_fault_in_the_file() {
        let said = "[legion-go]\n\n[legion-go.files]\n/etc/a\n";

        assert!(
            super::here(said, Path::new("/nowhere")).is_err(),
            "a block with no matches line selects nothing and says nothing"
        );
    }

    #[test]
    fn the_blocks_are_read_in_the_manifests_own_vocabulary() {
        let at = firmware("vocabulary", &[("product_name", "Legion Go")]);
        let said = here(TABLE, &at);
        let read = crate::manifest::Manifest::read(&said).expect("the manifest reads a block");
        let Ok(files) = read.of(crate::manifest::Section::Files);

        assert_eq!(files.len(), 2);

        let Ok(masked) = read.of(crate::manifest::Section::Masked);

        assert_eq!(masked, ["cachyos-gamescope-autologin.service"]);
    }
}
