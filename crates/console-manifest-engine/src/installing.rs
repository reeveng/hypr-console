//! What pacman says while it installs, read for how far it has got.
//!
//! The packages stretch is nothing on almost every apply and minutes on the one
//! after somebody adds a package, and for those minutes the line under it did
//! not move: pacman was told to be quiet, its output went to the terminal
//! unread, and nothing here knew whether it was fetching, unpacking or hung.
//!
//! Pacman is read instead of guessed at. Two of the lines it prints say where
//! it is and both are stable enough to lean on -- ` name downloading...` while
//! it fetches, which is the shape it takes when nobody is watching it from a
//! terminal, and `(2/3) installing name` while it writes, which carries its own
//! count and its own total. Everything else it says is passed through to the
//! screen and counted as nothing: the keyring, the integrity check and the file
//! conflicts each print counts of their own, restarting at one, and a bar that
//! believed them would fill four times over.
//!
//! Fetching is given the first half and installing the second. It is a
//! division, not a measurement: a package that is already in the cache is not
//! fetched at all and the first half goes by at once, and one that is fifty
//! megabytes over a phone is most of the wait. Half apiece is the estimate that
//! is wrong in both directions rather than badly wrong in one.

use console_core_never::Never;

use console_how_far as how_far;

pub const FETCHING: f64 = 0.5;

const VERBS: [&str; 5] =
    ["installing", "upgrading", "reinstalling", "downgrading", "removing"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    Fetching(String),
    Doing { done: usize, many: usize, name: String },
    Nothing,
}

pub fn said(line: &str) -> Result<Said, Never> {
    let Ok(plain) = how_far::plain(line);
    let said = plain.trim();

    let Some(fetching) = said.strip_suffix("downloading...") else {
        return counted(said);
    };

    Ok(Said::Fetching(fetching.trim().to_string()))
}

fn counted(said: &str) -> Result<Said, Never> {
    let Some(rest) = said.strip_prefix('(') else {
        return Ok(Said::Nothing);
    };

    let Some((count, doing)) = rest.split_once(')') else {
        return Ok(Said::Nothing);
    };

    let Some((done, many)) = count.split_once('/') else {
        return Ok(Said::Nothing);
    };

    let Ok(done) = done.trim().parse::<usize>() else {
        return Ok(Said::Nothing);
    };

    let Ok(many) = many.trim().parse::<usize>() else {
        return Ok(Said::Nothing);
    };

    let doing = doing.trim();

    let Some(verb) = VERBS.iter().find(|verb| doing.starts_with(*verb)) else {
        return Ok(Said::Nothing);
    };

    Ok(Said::Doing {
        done,
        many,
        name: doing.trim_start_matches(*verb).trim().to_string(),
    })
}

pub fn fetched(done: usize, many: usize) -> Result<f64, Never> {
    let Ok(part) = how_far::fraction(done, many);

    Ok(part * FETCHING)
}

pub fn done(done: usize, many: usize) -> Result<f64, Never> {
    let Ok(part) = how_far::fraction(done, many);

    Ok(FETCHING + part * (1.0 - FETCHING))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(line: &str) -> Said {
        let Ok(said) = said(line);

        said
    }

    #[test]
    fn the_line_that_names_a_package_being_written_carries_its_own_count() {
        assert_eq!(
            read("(2/3) installing console-fonts"),
            Said::Doing { done: 2, many: 3, name: "console-fonts".to_string() }
        );
        assert_eq!(
            read("( 7/12) upgrading linux-firmware"),
            Said::Doing { done: 7, many: 12, name: "linux-firmware".to_string() }
        );
    }

    #[test]
    fn a_count_that_is_not_about_a_package_moves_nothing() {
        for line in [
            "(3/3) checking keys in keyring",
            "(1/3) checking package integrity",
            "(2/3) loading package files",
            "(3/3) checking for file conflicts",
            "(1/1) checking available disk space",
        ] {
            assert_eq!(read(line), Said::Nothing, "{line} was counted");
        }
    }

    #[test]
    fn the_line_pacman_prints_when_nobody_is_watching_it_names_a_download() {
        assert_eq!(
            read(" linux-firmware-20240909.1-1-any downloading..."),
            Said::Fetching("linux-firmware-20240909.1-1-any".to_string())
        );
    }

    #[test]
    fn what_it_says_in_colour_is_read_the_same_as_what_it_says_plain() {
        assert_eq!(
            read("\u{1b}[0;1m(2/3)\u{1b}[0m installing console-fonts"),
            Said::Doing { done: 2, many: 3, name: "console-fonts".to_string() }
        );
    }

    #[test]
    fn everything_else_it_says_is_passed_through_and_counted_as_nothing() {
        for line in [
            ":: Retrieving packages...",
            ":: Processing package changes...",
            "warning: console-fonts-1.0 is up to date -- reinstalling",
            "",
            "(",
            "(x/3) installing nothing",
            "(1/) installing nothing",
        ] {
            assert_eq!(read(line), Said::Nothing, "{line:?} was read as progress");
        }
    }

    #[test]
    fn fetching_has_the_first_half_and_writing_the_second() {
        assert_eq!(fetched(0, 4), Ok(0.0));
        assert_eq!(fetched(4, 4), Ok(FETCHING));
        assert_eq!(done(0, 4), Ok(FETCHING));
        assert_eq!(done(4, 4), Ok(1.0));
    }

    #[test]
    fn a_stretch_with_no_packages_in_it_divides_by_nothing_and_says_nothing() {
        assert_eq!(fetched(0, 0), Ok(0.0));
        assert_eq!(done(0, 0), Ok(FETCHING));
    }
}
