//! What Limine's menu offers, each under the name a one-shot boot asks for it by.
//!
//! The menu is the one surface the pad cannot reach, so the choice of entry
//! moves to where it can: a card writes `LoaderEntryOneShot` and the machine
//! restarts into that entry once. The variable takes the identifier Limine
//! derives, and the list Limine publishes of them, `LoaderEntries`, is the
//! menu as it stood at the last boot -- every snapshot made since is missing
//! from it and every one pruned since is still there. So the identifiers are
//! derived here from `limine.conf` as it is now, by Limine's own rule, which is
//! `bli_entry_id` in its `common/menu.c`:
//!
//!   - an entry's path is its titles from the top, each with `\`, `/` and `#`
//!     escaped, and a second sibling of the same title marked `#1`, a third
//!     `#2`;
//!   - in that path `/` becomes `.` and every byte outside `[A-Za-z0-9+_.@-]`
//!     becomes `-` -- a byte and not a character, so the `│` limine-snapper-sync
//!     puts in a snapshot's title is three dashes;
//!   - the second entry to arrive at an identifier already given gets `-2`,
//!     the third `-3`.
//!
//! A folder with something under it is not an entry. One with nothing under
//! it is, which is why `Other systems and bootloaders` is in the list Limine
//! published. An entry for BIOS is not offered on UEFI and is counted by
//! neither rule. `if_fw_type` and
//! `if_arch` skip an entry the same way and are not read here: nothing on this
//! machine writes them, and one that did would shift a duplicate's number
//! rather than lose an entry.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub const CONFIG: &str = "/boot/limine.conf";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub levels: Vec<Level>,
    pub identifier: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub title: String,
    pub comments: Vec<String>,
}

pub fn entries(config: &str) -> Result<Vec<Entry>, Never> {
    let Ok(headings) = headings(config);
    let after = headings.iter().skip(1).map(|heading| Some(heading.depth)).chain(std::iter::once(None));

    Ok(headings
        .iter()
        .zip(after)
        .fold(Walk::default(), |walk, (heading, next)| {
            let Ok(walked) = walk.met(heading, next);

            walked
        })
        .entries)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Heading {
    depth: u32,
    title: String,
    protocol: Option<String>,
    comments: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Folder,
    Bootable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Firmware {
    Offered,
    Skipped,
}

const LEVEL: char = '/';
const OPENED: char = '+';
const LONGEST: u32 = 232;

fn headings(config: &str) -> Result<Vec<Heading>, Never> {
    Ok(config.lines().map(str::trim).fold(Vec::new(), |mut found: Vec<Heading>, line| {
        let Ok(depth): Result<u32, Never> = fitted(line.chars().take_while(|at| *at == LEVEL).count());

        match (depth, found.last_mut()) {
            (0, Some(heading)) => {
                let Ok(()) = heading.read(line);
            }
            (0, None) => {}
            (_, _) => {
                let Ok(heading) = Heading::new(depth, line);

                found.push(heading);
            }
        }

        found
    }))
}

impl Heading {
    fn new(depth: u32, line: &str) -> Result<Heading, Never> {
        let named = line.trim_start_matches(LEVEL);
        let title = match named.strip_prefix(OPENED) {
            Some(opened) => opened,
            None => named,
        };

        Ok(Heading { depth, title: title.trim().to_string(), protocol: None, comments: Vec::new() })
    }

    fn read(&mut self, line: &str) -> Result<(), Never> {
        match line.split_once(':') {
            Some((key, value)) => match key.trim().to_ascii_lowercase().as_str() {
                "protocol" => self.protocol = Some(value.trim().to_string()),
                "comment" => self.comments.push(value.trim().to_string()),
                _ => {}
            },
            None => {}
        }

        Ok(())
    }

    fn firmware(&self, shape: Shape) -> Result<Firmware, Never> {
        Ok(match (shape, self.protocol.as_deref()) {
            (Shape::Bootable, Some("bios" | "bios_chainload")) => Firmware::Skipped,
            (Shape::Bootable, Some(_) | None) => Firmware::Offered,
            (Shape::Folder, Some(_) | None) => Firmware::Offered,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Walk {
    above: Vec<Above>,
    siblings: Vec<Vec<String>>,
    given: Vec<String>,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Above {
    level: Level,
    segment: String,
}

impl Walk {
    fn met(mut self, heading: &Heading, next: Option<u32>) -> Result<Walk, Never> {
        let level = heading.depth.saturating_sub(1);
        let Ok(held) = index(level);
        let shape = match next.map(|depth| depth > heading.depth) {
            Some(true) => Shape::Folder,
            Some(false) | None => Shape::Bootable,
        };

        self.above.truncate(held);
        self.siblings.resize(held.saturating_add(1), Vec::new());

        let Ok(firmware) = heading.firmware(shape);

        Ok(match firmware {
            Firmware::Skipped => self,
            Firmware::Offered => {
                let Ok(walked) = self.offered(heading, shape, level);

                walked
            }
        })
    }

    fn offered(mut self, heading: &Heading, shape: Shape, level: u32) -> Result<Walk, Never> {
        let Ok(segment) = self.segment(&heading.title, level);
        let Ok(held) = index(level);
        let above = Above { level: Level { title: heading.title.clone(), comments: heading.comments.clone() }, segment };

        match self.siblings.get_mut(held) {
            Some(siblings) => siblings.push(heading.title.clone()),
            None => {}
        }

        match shape {
            Shape::Folder => self.above.push(above),
            Shape::Bootable => {
                let Ok(entry) = self.entry(above);

                self.entries.push(entry);
            }
        }

        Ok(self)
    }

    fn segment(&self, title: &str, level: u32) -> Result<String, Never> {
        let Ok(held) = index(level);
        let Ok(before): Result<u32, Never> = fitted(match self.siblings.get(held) {
            Some(siblings) => siblings.iter().filter(|sibling| *sibling == title).count(),
            None => 0,
        });
        let Ok(escaped) = escaped(title);

        Ok(match before {
            0 => escaped,
            _ => format!("{escaped}#{before}"),
        })
    }

    fn entry(&mut self, this: Above) -> Result<Entry, Never> {
        let path = self.above.iter().chain(std::iter::once(&this)).map(|above| above.segment.as_str()).collect::<Vec<_>>().join("/");
        let Ok(base) = identifier(&path);
        let Ok(before): Result<u32, Never> = fitted(self.given.iter().filter(|given| **given == base).count());

        self.given.push(base.clone());

        Ok(Entry {
            levels: self.above.iter().map(|above| above.level.clone()).chain(std::iter::once(this.level)).collect(),
            identifier: match before {
                0 => base,
                _ => format!("{base}-{}", before.saturating_add(1)),
            },
        })
    }
}

fn escaped(title: &str) -> Result<String, Never> {
    Ok(title.chars().fold(String::new(), |mut escaped, at| {
        match at {
            '\\' | '/' | '#' => escaped.push('\\'),
            _ => {}
        }

        escaped.push(at);

        escaped
    }))
}

fn identifier(path: &str) -> Result<String, Never> {
    let Ok(longest) = index(LONGEST);
    let mapped = path
        .bytes()
        .take(longest)
        .map(|byte| match byte {
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'+' | b'_' | b'.' | b'@' | b'-' => char::from(byte),
            b'/' => '.',
            _ => '-',
        })
        .collect::<String>();

    Ok(match mapped.as_str() {
        "" | "." | ".." => format!("{mapped}-"),
        _ => mapped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE: &str = "timeout: 10
default_entry: 2
remember_last_entry: yes

# CachyOS Limine theme
# Author: somebody (https://example.org)
comment: machine-id=0000
/+CachyOS
  //linux-cachyos-deckify
  ### This kernel entry is auto-generated by limine-entry-tool
  comment: Kernel version: 7.2.3-1-cachyos-deckify
  protocol: linux

     //Snapshots
     ### Auto-generated by limine-snapper-sync
     comment: 2 snapshots
     ///184 │ 2026-09-23 06:54:01
     comment: console apply e79c18f5
     ////linux-cachyos-deckify
     comment: Kernel version: 7.2.3-1-cachyos-deckify
     protocol: linux
     ///6   │ 2026-08-27 14:47:13
     comment: Fresh CachyOS Installation
     ////linux-cachyos-deckify
     protocol: linux

/+Other systems and bootloaders
/EFI fallback
comment: Default EFI loader
protocol: efi
path: boot():/EFI/BOOT/BOOTX64.EFI
";

    fn identifiers(config: &str) -> Vec<String> {
        let Ok(found) = entries(config);

        found.into_iter().map(|entry| entry.identifier).collect()
    }

    #[test]
    fn the_device_menu_is_named_the_way_limine_published_it() {
        assert_eq!(
            identifiers(DEVICE),
            vec![
                "CachyOS.linux-cachyos-deckify",
                "CachyOS.Snapshots.184-----2026-09-23-06-54-01.linux-cachyos-deckify",
                "CachyOS.Snapshots.6-------2026-08-27-14-47-13.linux-cachyos-deckify",
                "Other-systems-and-bootloaders",
                "EFI-fallback",
            ]
        );
    }

    #[test]
    fn a_snapshot_keeps_the_apply_it_stands_before() {
        let Ok(found) = entries(DEVICE);

        assert_eq!(
            found.get(1).and_then(|entry| entry.levels.get(2)),
            Some(&Level {
                title: "184 │ 2026-09-23 06:54:01".to_string(),
                comments: vec!["console apply e79c18f5".to_string()],
            })
        );
    }

    #[test]
    fn a_second_sibling_of_one_title_is_marked_in_the_path() {
        let config = "/Linux\nprotocol: linux\n/Linux\nprotocol: linux\n";

        assert_eq!(identifiers(config), vec!["Linux", "Linux-1"]);
    }

    #[test]
    fn two_paths_that_map_to_one_identifier_are_numbered_from_two() {
        let config = "/a b\nprotocol: linux\n/a:b\nprotocol: linux\n/a?b\nprotocol: linux\n";

        assert_eq!(identifiers(config), vec!["a-b", "a-b-2", "a-b-3"]);
    }

    #[test]
    fn a_bios_entry_is_not_offered_and_does_not_count_as_a_sibling() {
        let config = "/Linux\nprotocol: bios\n/Linux\nprotocol: linux\n";

        assert_eq!(identifiers(config), vec!["Linux"]);
    }

    #[test]
    fn a_slash_in_a_title_is_escaped_rather_than_read_as_a_level() {
        let config = "/A/B\nprotocol: linux\n";

        assert_eq!(identifiers(config), vec!["A-.B"]);
    }

    #[test]
    fn a_long_path_is_cut_where_limine_cuts_it() {
        let title = "x".repeat(300);
        let config = format!("/{title}\nprotocol: linux\n");

        assert_eq!(identifiers(&config), vec!["x".repeat(232)]);
    }
}
