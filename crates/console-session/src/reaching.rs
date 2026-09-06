//! Getting a word into the desktop's session from outside it.
//!
//! Everything a person sees is drawn by programs belonging to whoever the
//! desktop is for, in a session with a Wayland socket and a compositor
//! signature in its environment. A shell arriving over ssh has none of that:
//! it is root, it is on no seat, and `hyprctl` from it prints nothing at all.
//!
//! So a command that has to land on the screen is wrapped twice -- once to
//! become the right person, and once to be handed the four names their session
//! was started with. `console-test-stages` wrote that wrapper for the checks
//! and `tools/console-migrate` wrote it again in shell, which is one decision
//! about what a session is kept in two places that could disagree about it.
//! This is the one place.
//!
//! `runuser` rather than `machinectl shell`, in the half of this that carries
//! an answer back: `machinectl` reports its own status and not the command's,
//! and a question whose no cannot be told from a machine that could not be
//! reached is not a question.

use console_external_programs::Program;
use console_never::Never;

pub const OWNER: &str = "set -- $(ls -1 /home 2>/dev/null); \
                         if [ $# -eq 1 ]; then echo \"$1\"; else id -nu 1000; fi";

pub fn environment(whom: &str) -> Result<String, Never> {
    Ok(format!(
        "export XDG_RUNTIME_DIR=/run/user/$(id -u {whom}); \
         export HYPRLAND_INSTANCE_SIGNATURE=$(ls -1t \"$XDG_RUNTIME_DIR/hypr\" 2>/dev/null \
         | head -1); \
         export WAYLAND_DISPLAY=$(ls -1t \"$XDG_RUNTIME_DIR\" 2>/dev/null \
         | grep -E '^wayland-[0-9]+$' | head -1); \
         [ -n \"$HYPRLAND_INSTANCE_SIGNATURE\" ] || \
         {{ echo 'nothing on the device is in a Hyprland session' >&2; exit 1; }}"
    ))
}

pub fn in_session(whom: &str, command: &str) -> Result<String, Never> {
    let Ok(environment) = environment(whom);

    let inside = format!("{environment}; exec {command}");

    let Ok(quoted) = quoted(&inside);
    let Ok(runuser) = Program::Runuser.name();
    let Ok(shell) = Program::Sh.name();

    Ok(format!("{runuser} -u {whom} -- {shell} -c {quoted}"))
}

pub fn as_them(whom: &str, command: &str) -> Result<String, Never> {
    let Ok(quoted) = quoted(command);
    let Ok(runuser) = Program::Runuser.name();
    let Ok(shell) = Program::Sh.name();

    Ok(format!("{runuser} -u {whom} -- {shell} -c {quoted}"))
}

pub fn quoted(word: &str) -> Result<String, Never> {
    Ok(format!("'{}'", word.replace('\'', "'\\''")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quote_inside_a_command_does_not_end_the_command() {
        let Ok(quoted) = quoted("say 'hello'");

        assert_eq!(quoted, "'say '\\''hello'\\'''");
    }

    #[test]
    fn reaching_the_session_becomes_the_person_and_hands_over_their_socket() {
        let Ok(asked) = in_session("someone", "console-confirm 'send it?'");

        assert!(asked.starts_with("runuser -u someone -- sh -c "));
        assert!(asked.contains("WAYLAND_DISPLAY"));
        assert!(asked.contains("HYPRLAND_INSTANCE_SIGNATURE"));
    }

    #[test]
    fn being_the_person_is_not_the_same_as_being_in_their_session() {
        let Ok(asked) = as_them("someone", "ls");

        assert!(!asked.contains("WAYLAND_DISPLAY"));
    }
}
