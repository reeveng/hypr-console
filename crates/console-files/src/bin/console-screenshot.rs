//! A screenshot, into the pictures folder, named for when it was taken.
//!
//! Bound to the bottom right paddle held with L2, and to Super+S for a real
//! keyboard. The paddle used to be enough on its own and was a picture taken
//! by the hand that was only holding the device; the layer is what a person
//! meaning it does. Plain
//! `grim` writes to the working directory, which for a button press is
//! wherever the daemon that read the button happened to start -- which on this
//! machine is `/`, so the picture landed somewhere only root could write and
//! the paddle did nothing.
//!
//! Where the pictures folder is is the machine's answer and not this file's.
//! The shell script this replaces looked at `$XDG_PICTURES_DIR` and fell back
//! to a folder called Pictures, and that variable is set by a login shell
//! rather than by the session: run from a button, it was never there, so the
//! fallback was the answer every time. `console_files::places` reads what the
//! home directory actually says, which is the same answer the files panel's
//! Pictures tab arrives at.

use console_core_external_programs::Program;
use console_files::places::folder;
use console_core_never::Never;

pub fn named(when: &str) -> Result<String, Never> {
    Ok(format!("screenshot-{when}.png"))
}

fn when() -> Result<String, Never> {
    let Ok(mut asking) = Program::Date.command();

    Ok(match asking.arg("+%Y-%m-%d-%H%M%S").output() {
        Ok(said) => String::from_utf8_lossy(&said.stdout).trim().to_string(),

        Err(fault) => {
            eprintln!("console-screenshot: date: naming the picture by when it was taken: {fault}");
            String::new()
        }
    })
}

fn main() -> std::process::ExitCode {
    let Ok(said) = console_core_places::home();

    let home = match said {
        Some(home) => home,

        None => {
            eprintln!("console-screenshot: no HOME, so there is nobody to take a picture for");

            return std::process::ExitCode::FAILURE;
        }
    };

    let Ok(into) = folder(&home, "XDG_PICTURES_DIR", "Pictures");

    match std::fs::create_dir_all(&into) {
        Ok(()) => {},
        Err(why) => {
            eprintln!("console-screenshot: no {}: {why}", into.display());
            return std::process::ExitCode::FAILURE;
        }
    }

    let Ok(when) = when();

    let Ok(named) = named(&when);

    let at = into.join(named);

    let Ok(mut taking) = Program::Grim.command();

    match taking.arg(&at).status() {
        Ok(how) if how.success() => {
            println!("{}", at.display());
            std::process::ExitCode::SUCCESS
        }
        Ok(how) => {
            eprintln!("console-screenshot: grim said {how}");
            std::process::ExitCode::FAILURE
        }
        Err(why) => {
            eprintln!("console-screenshot: no grim to run: {why}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_named_for_the_moment_it_was_taken() {
        let Ok(one) = named("2026-08-31-142309");

        let Ok(other) = named("2026-01-02-000000");

        assert_eq!(one, "screenshot-2026-08-31-142309.png");

        let mut two = [one, other.clone()];

        two.sort();

        assert_eq!(two[0], other);
    }
}
