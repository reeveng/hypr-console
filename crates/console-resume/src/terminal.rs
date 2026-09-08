//! What a terminal window was showing, beyond the terminal itself.
//!
//! A terminal's own command line records where it was told to start, once, and
//! says nothing about the shell inside it. Replaying only that line hands back an
//! empty prompt in the directory the window was opened in, whatever it held by
//! the time the session was saved. The kernel keeps the rest: the shell is a
//! child of the terminal, its `cwd` is where its user actually got to, and the
//! pty's foreground process group is whatever they were running.
//!
//! ## Which terminal, and how it is told
//!
//! [`Spec`] is the table: whether a terminal takes a subcommand first, what its
//! option for a working directory is called and whether the value hangs off it
//! or stands apart, and whether what to run follows an option or is simply the
//! words at the end. A terminal that is not in the table is left exactly as it
//! was found, because a command line this program does not understand is one it
//! cannot improve on.
//!
//! ## A program comes back through the shell that ran it
//!
//! Quitting `nvim` should leave a prompt where it was, not close the window, so
//! what is restarted is the shell with a line to run and an `exec` back into
//! itself. That shell reads its startup files on the way in, which is what keeps
//! a version manager's path in front of the one the compositor was started with.
//!
//! ## And some programs do not come back at all
//!
//! A half finished `sudo pacman -Syu` asking for a password at login is worse
//! than a terminal that comes back empty, and the same goes for anything else
//! that was part way through changing the machine. [`NEVER_RESUMED`] is that
//! list, and a `never-resume` file in this desktop's configuration replaces it
//! rather than adding to it -- so an empty file means everything comes back, and
//! somebody who disagrees with the list does not have to agree with half of it.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::OnceLock;

use console_core_never::Never;

use crate::starting::quote_word;

const SHELLS: &[&str] = &[
    "ash", "bash", "csh", "dash", "elvish", "fish", "ksh", "mksh", "nu", "oksh", "pwsh", "sh",
    "tcsh", "xonsh", "zsh",
];

const NEVER_RESUMED: &[&str] = &[
    "dd", "doas", "fdisk", "mkfs", "pacman", "paru", "parted", "pkexec", "reboot", "rm",
    "shutdown", "su", "sudo", "yay",
];

const NEVER_RESUME_NAME: &str = "never-resume";

const SEARCH_LIMIT: usize = 64;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Occupant {
    pub directory: Option<String>,
    pub program: Option<Vec<String>>,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resumable {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Directory {
    Told,
    NotTold,
}

enum Value {
    Attached,
    Separate,
}

enum Command {
    Flag(&'static str),
    Trailing,
}

struct Spec {
    subcommand: Option<&'static str>,
    directory: Option<(&'static str, Value)>,
    command: Command,
}

fn spec(binary: &str) -> Result<Option<Spec>, Never> {
    let attached = |flag| Some((flag, Value::Attached));
    let separate = |flag| Some((flag, Value::Separate));

    Ok(Some(match binary {
        "alacritty" => Spec {
            subcommand: None,
            directory: separate("--working-directory"),
            command: Command::Flag("-e"),
        },
        "foot" => Spec {
            subcommand: None,
            directory: attached("--working-directory"),
            command: Command::Flag("-e"),
        },
        "footclient" => Spec {
            subcommand: None,
            directory: attached("--working-directory"),
            command: Command::Trailing,
        },
        "ghostty" => Spec {
            subcommand: None,
            directory: attached("--working-directory"),
            command: Command::Flag("-e"),
        },
        "gnome-terminal" => Spec {
            subcommand: None,
            directory: attached("--working-directory"),
            command: Command::Flag("--"),
        },
        "kitty" => Spec {
            subcommand: None,
            directory: separate("--directory"),
            command: Command::Trailing,
        },
        "konsole" => Spec {
            subcommand: None,
            directory: separate("--workdir"),
            command: Command::Flag("-e"),
        },
        "rio" => Spec {
            subcommand: None,
            directory: separate("--working-dir"),
            command: Command::Flag("-e"),
        },
        "st" | "xterm" => {
            Spec { subcommand: None, directory: None, command: Command::Flag("-e") }
        },
        "terminator" | "xfce4-terminal" => Spec {
            subcommand: None,
            directory: attached("--working-directory"),
            command: Command::Flag("-x"),
        },
        "tilix" => Spec {
            subcommand: None,
            directory: attached("--working-directory"),
            command: Command::Flag("-e"),
        },
        "rxvt" | "urxvt" | "urxvtc" => Spec {
            subcommand: None,
            directory: separate("-cd"),
            command: Command::Flag("-e"),
        },
        "wezterm" => Spec {
            subcommand: Some("start"),
            directory: separate("--cwd"),
            command: Command::Flag("--"),
        },
        _something_this_does_not_know => return Ok(None),
    }))
}

pub fn restored(argv: &[String], pid: i32) -> Result<Option<Vec<String>>, Never> {
    let occupant = inspect(pid)?;

    restore(argv, &occupant)
}

pub fn inspect(pid: i32) -> Result<Occupant, Never> {
    let Ok(within) = shell_within(pid);

    let shell = match within {
        Some(shell) => shell,
        None => return Ok(Occupant::default()),
    };

    let Ok(said) = stat(shell);

    let stat = match said {
        Some(stat) => stat,
        None => return Ok(Occupant::default()),
    };

    let foreground = match stat.foreground > 0 && stat.foreground != stat.group {
        true => Some(stat.foreground),
        false => None,
    };

    let program = match foreground {
        Some(foreground) => {
            let Ok(running) = running(foreground);

            running
        },
        None => None,
    };

    let Ok(under_the_shell) = directory(shell);

    let directory = match (&program, foreground) {
        (Some(_it_was_running_something), Some(foreground)) => {
            let Ok(where_it_got_to) = directory(foreground);

            match where_it_got_to {
                Some(directory) => Some(directory),
                None => under_the_shell,
            }
        },
        (Some(_), None) | (None, _) => under_the_shell,
    };

    let Ok(words) = argv_of(shell);

    let named = match words.as_ref().and_then(|words| words.first()) {
        Some(word) => {
            let Ok(path) = binary_path(word);

            Some(path.to_string())
        },
        None => None,
    };

    Ok(Occupant { directory, program, shell: named })
}

fn running(pid: i32) -> Result<Option<Vec<String>>, Never> {
    let Ok(said) = argv_of(pid);

    let words = match said {
        Some(words) => words,
        None => return Ok(None),
    };

    let named = match words.first() {
        Some(word) => word,
        None => return Ok(None),
    };

    let Ok(binary) = binary(named);
    let Ok(resumable) = resumable(binary);

    Ok(match resumable {
        Resumable::Yes => Some(words),
        Resumable::No => None,
    })
}

fn restore(argv: &[String], occupant: &Occupant) -> Result<Option<Vec<String>>, Never> {
    let first = match argv.first() {
        Some(first) => first,
        None => return Ok(None),
    };

    let Ok(named) = binary(first);

    let Ok(known) = spec(named);

    let spec = match known {
        Some(spec) => spec,
        None => return Ok(None),
    };

    let mut line = vec![first.clone()];

    match spec.subcommand {
        Some(subcommand) => line.push(subcommand.to_string()),
        None => {},
    }

    let rest = match argv.split_first() {
        Some((_the_terminal_itself, rest)) => rest,
        None => &[],
    };

    let Ok(kept) = options(rest, &spec);

    line.extend(kept);

    let told = match (&occupant.directory, &spec.directory) {
        (Some(directory), Some((flag, value))) => {
            match value {
                Value::Attached => line.push(format!("{flag}={directory}")),
                Value::Separate => {
                    line.push((*flag).to_string());
                    line.push(directory.clone());
                },
            }

            Directory::Told
        },
        (Some(_), None) | (None, _) => Directory::NotTold,
    };

    let inside = inside(occupant, told)?;

    match &inside {
        Some(inside) => {
            match spec.command {
                Command::Flag(flag) => line.push(flag.to_string()),
                Command::Trailing => {},
            }

            line.extend(inside.clone());
        },
        None => {},
    }

    Ok(match (told, &inside) {
        (Directory::NotTold, None) => None,
        (Directory::Told, _) | (_, Some(_)) => Some(line),
    })
}

fn inside(occupant: &Occupant, told: Directory) -> Result<Option<Vec<String>>, Never> {
    let step_in = match (told, &occupant.directory) {
        (Directory::NotTold, Some(directory)) => {
            let Ok(quoted) = quote_word(directory);

            format!("cd {quoted}; ")
        },
        (Directory::NotTold, None) | (Directory::Told, _) => String::new(),
    };

    Ok(match (&occupant.program, &occupant.shell) {
        (Some(program), Some(shell)) => {
            let words: Vec<String> = program
                .iter()
                .map(|word| {
                    let Ok(quoted) = quote_word(word);

                    quoted
                })
                .collect();

            let Ok(call) =
                shell_call(shell, &format!("{step_in}{}; exec {shell} -i", words.join(" ")));

            Some(call)
        },
        (Some(program), None) => Some(program.clone()),
        (None, Some(shell)) => match step_in.is_empty() {
            true => None,
            false => {
                let Ok(call) = shell_call(shell, &format!("{step_in}exec {shell} -i"));

                Some(call)
            },
        },
        (None, None) => None,
    })
}

fn shell_call(shell: &str, line: &str) -> Result<Vec<String>, Never> {
    Ok(vec![shell.to_string(), "-i".to_string(), "-c".to_string(), line.to_string()])
}

fn options(words: &[String], spec: &Spec) -> Result<Vec<String>, Never> {
    let mut kept = Vec::new();
    let mut skip_value = false;

    for word in words {
        match skip_value {
            true => {
                skip_value = false;

                continue;
            },
            false => {},
        }

        match Some(word.as_str()) == spec.subcommand {
            true => continue,
            false => {},
        }

        let ours = match &spec.directory {
            Some((flag, value)) => match word == flag {
                true => {
                    skip_value = matches!(value, Value::Separate);

                    true
                },
                false => word.starts_with(&format!("{flag}=")),
            },
            None => false,
        };

        match ours {
            true => continue,
            false => {},
        }

        let stop = match spec.command {
            Command::Flag(flag) => word == flag,
            Command::Trailing => !word.starts_with('-'),
        };

        match stop {
            true => break,
            false => kept.push(word.clone()),
        }
    }

    Ok(kept)
}

fn resumable(program: &str) -> Result<Resumable, Never> {
    let Ok(denied) = never_resumed();

    let refused = SHELLS.contains(&program) || denied.iter().any(|one| one == program);

    Ok(match refused {
        true => Resumable::No,
        false => Resumable::Yes,
    })
}

fn never_resumed() -> Result<&'static Vec<String>, Never> {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();

    Ok(LIST.get_or_init(|| {
        let Ok(at) = never_resume_path();

        let said = match at {
            Some(at) => std::fs::read_to_string(at),
            None => return NEVER_RESUMED.iter().map(|name| (*name).to_string()).collect(),
        };

        match said {
            Ok(listed) => listed
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_string)
                .collect(),
            Err(_nobody_has_written_one) => {
                NEVER_RESUMED.iter().map(|name| (*name).to_string()).collect()
            },
        }
    }))
}

fn never_resume_path() -> Result<Option<PathBuf>, Never> {
    let ours = console_core_places::Base::Config.ours()?;

    Ok(ours.map(|at| at.join(crate::OURS).join(NEVER_RESUME_NAME)))
}

fn shell_within(pid: i32) -> Result<Option<i32>, Never> {
    let Ok(first) = children(pid);

    let mut queue = VecDeque::from(first);
    let mut examined: usize = 0;

    while let Some(candidate) = queue.pop_front() {
        examined = examined.saturating_add(1);

        match examined > SEARCH_LIMIT {
            true => return Ok(None),
            false => {},
        }

        let Ok(words) = argv_of(candidate);

        let is_shell = match words.as_ref().and_then(|words| words.first()) {
            Some(word) => {
                let Ok(named) = binary(word);

                SHELLS.contains(&named)
            },
            None => false,
        };

        match is_shell {
            true => return Ok(Some(candidate)),
            false => {},
        }

        let Ok(more) = children(candidate);

        queue.extend(more);
    }

    Ok(None)
}

fn numbered(word: &str) -> Result<Option<i32>, Never> {
    Ok(match word.parse::<i32>() {
        Ok(number) => Some(number),
        Err(_that_is_not_a_process) => None,
    })
}

fn children(pid: i32) -> Result<Vec<i32>, Never> {
    let mut found = Vec::new();

    let tasks = match std::fs::read_dir(format!("/proc/{pid}/task")) {
        Ok(tasks) => tasks,
        Err(_it_is_gone) => return Ok(found),
    };

    for task in tasks.flatten() {
        match std::fs::read_to_string(task.path().join("children")) {
            Ok(listed) => {
                found.extend(listed.split_whitespace().filter_map(|word| {
                    let Ok(numbered) = numbered(word);

                    numbered
                }));
            },
            Err(_that_thread_has_none) => {},
        }
    }

    Ok(found)
}

struct Stat {
    group: i32,
    foreground: i32,
}

fn stat(pid: i32) -> Result<Option<Stat>, Never> {
    let raw = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(raw) => raw,
        Err(_it_is_gone) => return Ok(None),
    };

    let after = match raw.rsplit_once(')') {
        Some((_its_own_name_in_brackets, after)) => after,
        None => return Ok(None),
    };

    let fields: Vec<&str> = after.split_whitespace().collect();

    let at = |which: usize| match fields.get(which) {
        Some(said) => {
            let Ok(numbered) = numbered(said);

            numbered
        },
        None => None,
    };

    Ok(match (at(2), at(5)) {
        (Some(group), Some(foreground)) => Some(Stat { group, foreground }),
        (None, _) | (_, None) => None,
    })
}

fn argv_of(pid: i32) -> Result<Option<Vec<String>>, Never> {
    let raw = match std::fs::read_to_string(format!("/proc/{pid}/cmdline")) {
        Ok(raw) => raw,
        Err(_it_is_gone) => return Ok(None),
    };

    let words: Vec<String> =
        raw.split('\0').filter(|word| !word.is_empty()).map(str::to_string).collect();

    Ok(match words.is_empty() {
        true => None,
        false => Some(words),
    })
}

fn directory(pid: i32) -> Result<Option<String>, Never> {
    let at = match std::fs::read_link(format!("/proc/{pid}/cwd")) {
        Ok(at) => at,
        Err(_it_is_gone) => return Ok(None),
    };

    Ok(at.to_str().map(str::to_string))
}

fn binary_path(word: &str) -> Result<&str, Never> {
    Ok(word.strip_prefix('-').unwrap_or(word))
}

fn binary(word: &str) -> Result<&str, Never> {
    let Ok(path) = binary_path(word);

    Ok(path.rsplit('/').next().unwrap_or(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    fn occupant(directory: &str, program: &[&str]) -> Occupant {
        Occupant {
            directory: Some(directory.to_string()),
            program: match program.is_empty() {
                true => None,
                false => Some(argv(program)),
            },
            shell: Some("/usr/bin/zsh".to_string()),
        }
    }

    fn restore(words: &[&str], occupant: &Occupant) -> Option<Vec<String>> {
        let Ok(line) = super::restore(&argv(words), occupant);

        line
    }

    #[test]
    fn brings_back_the_directory_the_shell_was_in() {
        assert_eq!(
            restore(
                &["foot", "--working-directory=/home/ada"],
                &occupant("/home/ada/Documents/projects/website", &[])
            ),
            Some(argv(&["foot", "--working-directory=/home/ada/Documents/projects/website"]))
        );
    }

    #[test]
    fn brings_back_the_program_through_the_shell_that_ran_it() {
        assert_eq!(
            restore(&["foot"], &occupant("/home/ada/notes", &["nvim", "today.md"])),
            Some(argv(&[
                "foot",
                "--working-directory=/home/ada/notes",
                "-e",
                "/usr/bin/zsh",
                "-i",
                "-c",
                "nvim today.md; exec /usr/bin/zsh -i",
            ]))
        );
    }

    #[test]
    fn quotes_a_program_argument_that_would_otherwise_be_shell_syntax() {
        let line = restore(&["foot"], &occupant("/tmp", &["nvim", "two words"]));

        assert_eq!(
            line.as_ref().and_then(|line| line.last()).map(String::as_str),
            Some("nvim 'two words'; exec /usr/bin/zsh -i")
        );
    }

    #[test]
    fn steps_into_the_directory_when_the_terminal_has_no_option_for_it() {
        assert_eq!(
            restore(&["xterm"], &occupant("/home/ada/notes", &["nvim"])),
            Some(argv(&[
                "xterm",
                "-e",
                "/usr/bin/zsh",
                "-i",
                "-c",
                "cd /home/ada/notes; nvim; exec /usr/bin/zsh -i",
            ]))
        );
    }

    #[test]
    fn replaces_the_options_it_sets_and_keeps_the_rest() {
        assert_eq!(
            restore(
                &[
                    "foot",
                    "--font=Mono:size=12",
                    "--working-directory=/home/ada",
                    "-e",
                    "nvim",
                    "old.md"
                ],
                &occupant("/tmp", &["nvim", "new.md"])
            ),
            Some(argv(&[
                "foot",
                "--font=Mono:size=12",
                "--working-directory=/tmp",
                "-e",
                "/usr/bin/zsh",
                "-i",
                "-c",
                "nvim new.md; exec /usr/bin/zsh -i",
            ]))
        );
    }

    #[test]
    fn reads_a_directory_option_that_stands_apart_from_its_value() {
        assert_eq!(
            restore(
                &["alacritty", "--working-directory", "/home/ada", "--title", "x"],
                &occupant("/tmp", &[])
            ),
            Some(argv(&["alacritty", "--title", "x", "--working-directory", "/tmp"]))
        );
    }

    #[test]
    fn puts_a_subcommand_first_and_a_command_last() {
        assert_eq!(
            restore(&["wezterm", "start", "--cwd", "/home/ada"], &occupant("/tmp", &["btop"])),
            Some(argv(&[
                "wezterm",
                "start",
                "--cwd",
                "/tmp",
                "--",
                "/usr/bin/zsh",
                "-i",
                "-c",
                "btop; exec /usr/bin/zsh -i",
            ]))
        );
    }

    #[test]
    fn drops_the_command_a_terminal_carries_without_an_option() {
        assert_eq!(
            restore(
                &["kitty", "--directory", "/home/ada", "nvim", "old.md"],
                &occupant("/tmp", &["nvim", "new.md"])
            ),
            Some(argv(&[
                "kitty",
                "--directory",
                "/tmp",
                "/usr/bin/zsh",
                "-i",
                "-c",
                "nvim new.md; exec /usr/bin/zsh -i",
            ]))
        );
    }

    #[test]
    fn leaves_alone_a_window_that_is_not_a_terminal_it_knows() {
        assert_eq!(restore(&["librewolf"], &occupant("/home/ada", &["nvim"])), None);
    }

    #[test]
    fn leaves_alone_a_terminal_whose_tree_said_nothing() {
        assert_eq!(restore(&["foot"], &Occupant::default()), None);
    }

    #[test]
    fn refuses_to_start_a_shell_or_a_half_finished_upgrade_again() {
        assert_eq!(resumable("nvim"), Ok(Resumable::Yes));
        assert_eq!(resumable("claude"), Ok(Resumable::Yes));
        assert_eq!(resumable("zsh"), Ok(Resumable::No));
        assert_eq!(resumable("sudo"), Ok(Resumable::No));
        assert_eq!(resumable("pacman"), Ok(Resumable::No));
    }

    #[test]
    fn names_a_program_without_its_path_or_a_login_shell_s_dash() {
        assert_eq!(binary("/usr/bin/zsh"), Ok("zsh"));
        assert_eq!(binary("-zsh"), Ok("zsh"));
        assert_eq!(binary("-/usr/bin/zsh"), Ok("zsh"));
        assert_eq!(binary_path("-/usr/bin/zsh"), Ok("/usr/bin/zsh"));
    }

    #[test]
    fn says_nothing_about_a_process_tree_with_no_shell_in_it() {
        let Ok(mine) = console_core_number_conversion::fitted::<u32, i32>(std::process::id());

        assert_eq!(inspect(mine), Ok(Occupant::default()));
    }
}
