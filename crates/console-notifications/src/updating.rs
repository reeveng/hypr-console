//! How far the long thing has got, written where the bar can read it.
//!
//! `console apply` runs as root and the bar runs as whoever the desktop
//! belongs to, so the two cannot share anything but a file. This is that file
//! and both ends of it: the engine writes, `bar-updating` reads, and the
//! format is in one place rather than agreed twice.
//!
//! An apply is not the only long thing. A run of the checks against the device
//! is minutes of somebody's handheld pressing its own buttons, and it is driven
//! from a laptop over ssh, so the person holding the device is the one party
//! with no idea how much longer. It writes the same file the same way, from the
//! other end of the ssh, and the strip does not care which of them is filling
//! it: what it says is that the machine is busy with something long, and how
//! much of it is left.
//!
//! Under `/run`, because it is about this apply and nothing else. A number
//! left behind by a machine that lost power is a bar stuck at 62% until
//! somebody notices, and `/run` is emptied at boot.
//!
//! # Where it is when the desktop is a copy of itself
//!
//! The nested desktop reads this device's own files out of a staged tree, and
//! every path written inside one of those files is rewritten on the way in to
//! point back into the stage. This path cannot be: it is spelled in a program
//! rather than in a file, and `/run/console` on somebody's laptop is a
//! directory the person running the checks is not allowed to make. So it is
//! asked rather than assumed, and `CONSOLE_UPDATING_PATH` is how a staged
//! session answers -- the same move the stage already makes for a home
//! directory and the four XDG directories, and the only way a check can look at
//! the strip filling rather than at the JSON behind it. The engine runs as root
//! outside anybody's session and is never told, so the machine's own answer is
//! the one below.
//!
//! # Why the bar is told rather than asked
//!
//! Nothing polls this. The engine signals waybar when the number changes and
//! waybar runs `bar-updating` again, so an idle desktop -- which is almost all
//! of them, almost all the time -- does no work at all for a bar that has
//! nothing to say. A read every fifth of a second for the life of a session is
//! a wake-up a battery pays for, on a machine that spends most of its life in
//! somebody's hands doing something else.

use std::path::Path;
use std::path::PathBuf;

use console_core_external_programs::Program;
use console_core_never::Never;

pub const WHERE: &str = "CONSOLE_UPDATING_PATH";

pub fn under(told: Option<&str>) -> Result<PathBuf, Never> {
    Ok(match told.map(str::trim).filter(|said| !said.is_empty()) {
        Some(said) => PathBuf::from(said),
        None => Path::new("/run/console").join("updating"),
    })
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "CONSOLE_UPDATING_PATH belongs to this crate, and the const beside it is the only spelling of the name"
    )
)]
pub fn at() -> Result<PathBuf, Never> {
    let told = match std::env::var(WHERE) {
        Ok(said) => Some(said),
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("console: {WHERE}: {fault}");

            None
        }
    };

    under(told.as_deref())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Far {
    pub percent: u16,
    pub doing: String,
}

pub fn written(far: &Far) -> Result<String, Never> {
    Ok(format!("{} {}\n", far.percent, far.doing))
}

pub fn reading(held: &str) -> Result<Option<Far>, Never> {
    let first = match held.lines().next() {
        Some(first) => first,
        None => return Ok(None),
    };

    let line = first.trim();

    let (percent, doing) = match line.split_once(' ') {
        Some((percent, doing)) => (percent, doing),
        None => return Ok(None),
    };

    let percent = match percent.parse::<u16>() {
        Ok(percent) => percent,
        Err(_fault) => return Ok(None),
    };

    Ok(match percent <= 100 && !doing.trim().is_empty() {
        true => Some(Far { percent, doing: doing.trim().to_string() }),
        false => None,
    })
}

pub fn wrote(far: &Far) -> Result<(), Never> {
    let Ok(at) = at();

    let holding = match at.parent() {
        Some(holding) => holding,
        None => return Ok(()),
    };

    match std::fs::create_dir_all(holding) {
        Ok(()) => {}
        Err(fault) => {
            eprintln!("console: {}: keeping how far along an apply is: {fault}", holding.display());

            return Ok(());
        }
    }

    let Ok(written) = written(far);

    match console_core_atomic_writes::whole(&at, written.as_bytes()) {
        Ok(()) => {}
        Err(fault) => {
            eprintln!("console: {}: writing how far along an apply is: {fault}", at.display());

            return Ok(());
        }
    }

    Ok(())
}

pub fn done() -> Result<(), Never> {
    let Ok(at) = at();

    let _ = std::fs::remove_file(at);

    Ok(())
}

pub const WAKING: &str = "-RTMIN+4";

pub fn wake() -> Result<(), Never> {
    let Ok(mut waking) = Program::Pkill.command();

    let _ = waking
        .arg(WAKING)
        .args(["-x", "waybar"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    Ok(())
}

pub fn far() -> Result<Option<Far>, Never> {
    let Ok(at) = at();

    let said = match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_fault) => return Ok(None),
    };

    reading(&said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_engine_writes_is_what_the_bar_reads() {
        for far in [
            Far { percent: 0, doing: "reading packages".to_string() },
            Far { percent: 62, doing: "building".to_string() },
            Far { percent: 100, doing: "done".to_string() },
        ] {
            let Ok(written) = written(&far);

            assert_eq!(reading(&written), Ok(Some(far)));
        }
    }

    #[test]
    fn a_name_with_spaces_in_it_survives_the_round_trip() {
        let far = Far { percent: 8, doing: "writing files".to_string() };
        let Ok(written) = written(&far);

        assert_eq!(reading(&written), Ok(Some(far)));
    }

    #[test]
    fn nothing_is_read_out_of_something_we_did_not_write() {
        for held in ["", "\n", "building", "62", "62 ", "  ", "-1 building", "x building"] {
            assert_eq!(reading(held), Ok(None), "{held:?} was read as a number");
        }
    }

    #[test]
    fn a_number_past_the_end_is_not_ours() {
        assert_eq!(reading("101 building"), Ok(None));
        assert_eq!(reading("999999 building"), Ok(None));
    }

    #[test]
    fn it_is_under_run_when_nothing_says_otherwise() {
        assert_eq!(under(None), Ok(PathBuf::from("/run/console/updating")));
        assert_eq!(under(Some("")), Ok(PathBuf::from("/run/console/updating")));
        assert_eq!(under(Some("   ")), Ok(PathBuf::from("/run/console/updating")));
    }

    #[test]
    fn a_staged_session_is_read_where_it_says_rather_than_under_run() {
        assert_eq!(
            under(Some("/somewhere/.stage/session-1/run/console/updating")),
            Ok(PathBuf::from("/somewhere/.stage/session-1/run/console/updating"))
        );
    }
}
