//! What a unit names, and what has to be told a file changed.

use std::collections::BTreeSet;

/// A program that reads a file once, when it starts, has to be told the file
/// changed.
///
/// Writing it and saying "Done." leaves the machine looking applied and
/// behaving as it did before, which is worse than not writing it at all: the
/// next person to look believes the new file is what is running.
///
/// Only what has been watched doing it. The compositor belongs here too and is
/// not here yet: `hyprctl reload` wants the running instance named, and an
/// entry that quietly does nothing is the fault this table is for.
pub const WAKES: [Wake; 1] = [Wake {
    under: "/.config/waybar/",
    run: "pkill -SIGUSR2 -x waybar",
    name: "the bar",
}];

#[derive(Debug, Clone, Copy)]
pub struct Wake {
    pub under: &'static str,
    pub run: &'static str,
    pub name: &'static str,
}

/// Everything that has to be woken because one of these files was written.
pub fn woken_by<'a>(written: impl IntoIterator<Item = &'a String> + Clone) -> Vec<Wake> {
    WAKES
        .into_iter()
        .filter(|wake| written.clone().into_iter().any(|path| path.contains(wake.under)))
        .collect()
}

/// Every file a unit names on a command line, so rewriting one restarts it.
///
/// The program it runs is the obvious one. An argument is the one that was
/// missed, and the background is why: `awww img /usr/share/backgrounds/...`
/// reads that picture once, when the unit starts. A new picture written under
/// a daemon already holding the old one is a picture nobody is ever shown, and
/// the apply that wrote it says nothing about that.
pub fn named_by(unit: &str) -> BTreeSet<String> {
    unit.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| key.starts_with("Exec") && key[4..].chars().all(char::is_alphanumeric))
        .flat_map(|(_, command)| command.split_whitespace())
        .map(|word| word.trim_start_matches(['-', '@', ':', '+', '!']))
        .filter(|word| word.starts_with('/'))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_a_unit_runs_is_named_by_it() {
        let unit = "[Service]\nExecStart=/usr/local/bin/stick-scroll\n";
        assert!(named_by(unit).contains("/usr/local/bin/stick-scroll"));
    }

    #[test]
    fn an_argument_is_named_too() {
        // The one that was missed. A new wallpaper written under a daemon
        // already holding the old one is a picture nobody is ever shown.
        let unit = "[Service]\nExecStart=/usr/bin/awww img /usr/share/backgrounds/legion.webp\n";
        let named = named_by(unit);
        assert!(named.contains("/usr/share/backgrounds/legion.webp"), "{named:?}");
    }

    #[test]
    fn every_kind_of_exec_line_is_read() {
        let unit = "ExecStartPre=/a\nExecStart=/b\nExecStop=/c\nExecReload=/d\n";
        assert_eq!(named_by(unit).len(), 4);
    }

    #[test]
    fn the_prefixes_systemd_allows_are_not_part_of_the_path() {
        let unit = "ExecStartPre=-/usr/bin/rm\nExecStart=+@!/usr/bin/thing\n";
        let named = named_by(unit);
        assert!(named.contains("/usr/bin/rm"), "{named:?}");
        assert!(named.contains("/usr/bin/thing"), "{named:?}");
    }

    #[test]
    fn a_word_that_is_not_a_path_is_not_a_file() {
        let unit = "ExecStart=/usr/bin/thing --flag value -x\n";
        assert_eq!(named_by(unit), BTreeSet::from(["/usr/bin/thing".to_string()]));
    }

    #[test]
    fn a_setting_that_merely_starts_with_exec_is_not_an_exec_line() {
        let unit = "Execute_this=/nope\nExecStart=/yes\n";
        assert_eq!(named_by(unit), BTreeSet::from(["/yes".to_string()]));
    }

    #[test]
    fn the_bar_is_woken_when_its_own_configuration_is_written() {
        let written = vec!["/home/player/.config/waybar/config.jsonc".to_string()];
        assert_eq!(woken_by(&written).len(), 1);
        let elsewhere = vec!["/home/player/.config/wofi/config".to_string()];
        assert!(woken_by(&elsewhere).is_empty());
    }
}
