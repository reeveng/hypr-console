//! What a unit names, and what has to be told a file changed.

use std::collections::BTreeSet;

use console_never::Never;

pub const WAKES: [Wake; 2] = [
    Wake {
        under: "/.config/waybar/",
        run: "pkill -SIGUSR2 -x waybar",
        name: "the bar",
    },
    Wake {
        under: "/usr/share/backgrounds/",
        run: "awww clear-cache",
        name: "the frames the wallpaper daemon kept",
    },
];

#[derive(Debug, Clone, Copy)]
pub struct Wake {
    pub under: &'static str,
    pub run: &'static str,
    pub name: &'static str,
}

pub fn woken_by<'a>(
    written: impl IntoIterator<Item = &'a String> + Clone,
) -> Result<Vec<Wake>, Never> {
    Ok(WAKES
        .into_iter()
        .filter(|wake| written.clone().into_iter().any(|path| path.contains(wake.under)))
        .collect())
}

pub fn named_by(unit: &str) -> Result<BTreeSet<String>, Never> {
    Ok(unit.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| {
            key.strip_prefix("Exec").is_some_and(|rest| rest.chars().all(char::is_alphanumeric))
        })
        .flat_map(|(_, command)| command.split_whitespace())
        .map(|word| word.trim_start_matches(['-', '@', ':', '+', '!']))
        .filter(|word| word.starts_with('/'))
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(unit: &str) -> BTreeSet<String> {
        let Ok(named) = named_by(unit);

        named
    }

    fn woken(written: &[String]) -> Vec<Wake> {
        let Ok(woken) = woken_by(written);

        woken
    }

    #[test]
    fn the_program_a_unit_runs_is_named_by_it() {
        let unit = "[Service]\nExecStart=/usr/local/bin/stick-scroll\n";
        assert!(named(unit).contains("/usr/local/bin/stick-scroll"));
    }

    #[test]
    fn an_argument_is_named_too() {
        let unit = "[Service]\nExecStart=/usr/bin/awww img /usr/share/backgrounds/console.webp\n";
        let named = named(unit);
        assert!(named.contains("/usr/share/backgrounds/console.webp"), "{named:?}");
    }

    #[test]
    fn every_kind_of_exec_line_is_read() {
        let unit = "ExecStartPre=/a\nExecStart=/b\nExecStop=/c\nExecReload=/d\n";
        assert_eq!(named(unit).len(), 4);
    }

    #[test]
    fn the_prefixes_systemd_allows_are_not_part_of_the_path() {
        let unit = "ExecStartPre=-/usr/bin/rm\nExecStart=+@!/usr/bin/thing\n";
        let named = named(unit);
        assert!(named.contains("/usr/bin/rm"), "{named:?}");
        assert!(named.contains("/usr/bin/thing"), "{named:?}");
    }

    #[test]
    fn a_word_that_is_not_a_path_is_not_a_file() {
        let unit = "ExecStart=/usr/bin/thing --flag value -x\n";
        assert_eq!(named(unit), BTreeSet::from(["/usr/bin/thing".to_string()]));
    }

    #[test]
    fn a_setting_that_merely_starts_with_exec_is_not_an_exec_line() {
        let unit = "Execute_this=/nope\nExecStart=/yes\n";
        assert_eq!(named(unit), BTreeSet::from(["/yes".to_string()]));
    }

    #[test]
    fn the_bar_is_woken_when_its_own_configuration_is_written() {
        let written = vec!["/home/@user@/.config/waybar/config.jsonc".to_string()];
        assert_eq!(woken(&written).len(), 1);
        let elsewhere = vec!["/home/@user@/.config/wofi/config".to_string()];
        assert!(woken(&elsewhere).is_empty());
    }

    #[test]
    fn the_kept_frames_go_when_a_background_is_written() {
        let written = vec!["/usr/share/backgrounds/console.webp".to_string()];
        let woken = woken(&written);
        assert_eq!(woken.len(), 1);
        assert!(woken[0].run.starts_with("awww clear-cache"));
    }
}
