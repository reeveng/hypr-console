//! Child process trees owned by systemd user scopes.

#[cfg(not(target_os = "linux"))]
compile_error!("console-program-scopes-systemd requires Linux");

use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;
use std::process::Stdio;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrapped {
    InAScope,
    AsItWasHandedIn,
}

pub fn in_a_scope_of_its_own(
    named: Option<&str>,
    arguments: &[String],
) -> Result<(Wrapped, Vec<String>), Never> {
    let Ok(scopes) = scopes();

    match scopes {
        Scopes::Unavailable => return Ok((Wrapped::AsItWasHandedIn, arguments.to_vec())),
        Scopes::Available => {}
    }

    let unit: Vec<String> = named
        .into_iter()
        .map(|named| format!("--unit={named}"))
        .collect();

    let Ok(said) = Program::SystemdRun.words(
        [
            "--user".to_string(),
            "--scope".to_string(),
            "--quiet".to_string(),
        ]
        .into_iter()
        .chain(unit)
        .chain(std::iter::once("--".to_string()))
        .chain(arguments.iter().cloned())
        .collect(),
    );

    Ok((Wrapped::InAScope, said))
}

pub fn nothing_left_in(named: &str) -> Result<(), Never> {
    let Ok(scopes) = scopes();

    match scopes {
        Scopes::Unavailable => return Ok(()),
        Scopes::Available => {}
    }

    let Ok(mut asking) = Program::Systemctl.command();

    let _ended = asking
        .args(["--user", "stop", "--no-block", &format!("{named}.scope")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scopes {
    Available,
    Unavailable,
}

pub fn scopes() -> Result<Scopes, Never> {
    let Ok(systemd_run) = Program::SystemdRun.name();
    let Ok(found) = installed(systemd_run);

    match found {
        Installed::No => return Ok(Scopes::Unavailable),
        Installed::Yes => {}
    }

    let Ok(mut asking) = Program::Systemctl.command();
    let answered = asking
        .args(["--user", "show", "-p", "Version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(match answered {
        Ok(how) => match how.success() {
            true => Scopes::Available,
            false => Scopes::Unavailable,
        },
        Err(_no_systemctl) => Scopes::Unavailable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrap_is_in_front_of_everything_else() {
        let arguments = vec![
            "firefox".to_string(),
            "--new-window".to_string(),
            "https://example.com".to_string(),
        ];
        let Ok((wrapped, made)) = in_a_scope_of_its_own(None, &arguments);

        match wrapped {
            Wrapped::AsItWasHandedIn => {
                eprintln!("skipped: no systemd-run on PATH; scopes cannot be made");

                return;
            }
            Wrapped::InAScope => {}
        }

        assert_eq!(
            made,
            vec![
                "systemd-run".to_string(),
                "--user".to_string(),
                "--scope".to_string(),
                "--quiet".to_string(),
                "--".to_string(),
                "firefox".to_string(),
                "--new-window".to_string(),
                "https://example.com".to_string(),
            ]
        );
    }

    #[test]
    fn a_program_named_like_a_flag_is_kept_a_program() {
        let arguments = vec!["--something".to_string()];
        let Ok((wrapped, made)) = in_a_scope_of_its_own(None, &arguments);

        match wrapped {
            Wrapped::AsItWasHandedIn => return,
            Wrapped::InAScope => {}
        }

        assert_eq!(
            made.get(4).map(String::as_str),
            Some("--"),
            "the terminator is what keeps the program a program"
        );
        assert!(
            made.iter().take(4).all(|word| word != "--"),
            "the terminator is what keeps the program a program"
        );
        assert_eq!(made.last().map(String::as_str), Some("--something"));
    }

    #[test]
    fn a_scope_that_is_named_can_be_stopped_by_that_name() {
        let arguments = vec!["sleep".to_string()];
        let Ok((wrapped, made)) = in_a_scope_of_its_own(Some("console-run-1"), &arguments);

        match wrapped {
            Wrapped::AsItWasHandedIn => return,
            Wrapped::InAScope => {}
        }

        assert!(
            made.contains(&"--unit=console-run-1".to_string()),
            "a scope nothing named is a scope nothing can stop: {made:?}"
        );
    }
}
