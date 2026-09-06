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

use console_external_programs::Program;
use console_never::Never;

pub fn at() -> Result<PathBuf, Never> {
    Ok(Path::new("/run/console").join("updating"))
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
    let Some(first) = held.lines().next() else {
        return Ok(None);
    };

    let line = first.trim();

    let Some((percent, doing)) = line.split_once(' ') else {
        return Ok(None);
    };

    let Ok(percent) = percent.parse::<u16>() else {
        return Ok(None);
    };

    Ok(match percent <= 100 && !doing.trim().is_empty() {
        true => Some(Far { percent, doing: doing.trim().to_string() }),
        false => None,
    })
}

pub fn wrote(far: &Far) -> Result<(), Never> {
    let Ok(at) = at();

    let Some(holding) = at.parent() else {
        return Ok(());
    };

    match std::fs::create_dir_all(holding) {
        Ok(()) => {}
        Err(fault) => {
            eprintln!("console: {}: keeping how far along an apply is: {fault}", holding.display());

            return Ok(());
        }
    }

    let beside = holding.join("updating.writing");

    let Ok(written) = written(far);

    match std::fs::write(&beside, written) {
        Ok(()) => {}
        Err(fault) => {
            eprintln!("console: {}: writing how far along an apply is: {fault}", beside.display());

            return Ok(());
        }
    }

    let _ = std::fs::rename(&beside, &at);

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

    let Ok(said) = std::fs::read_to_string(at) else {
        return Ok(None);
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
    fn it_is_under_run() {
        assert_eq!(at(), Ok(PathBuf::from("/run/console/updating")));
    }
}
