//! Where a screen's own answers are kept.
//!
//! The size and the turn were one word each for the whole machine, which is
//! right for exactly as long as there is one screen. Two is enough to break
//! it in both directions: a monitor on a desk wearing the quarter turn a
//! handheld is held at comes up on its side, and a handheld wearing the
//! density someone chose for a 27 inch panel comes up at a size no one can
//! read. Neither of those is a setting gone wrong -- both are one answer being
//! asked of two different questions.
//!
//! So an answer is kept under the connector it is about:
//! `~/.config/console/screens/eDP-1/turn`, beside a `scale` of its own. A
//! screen with nothing under its name wears what the compositor gave it, which
//! is what `console apply` wrote out of the panel's own mode -- so a monitor
//! plugged in for the first time is upright at its own density rather than
//! wearing whatever the last screen was set to.
//!
//! A connector is named by the kernel and is letters, digits and hyphens.
//! Anything else is not a connector, and [`Output::under`] says so rather than
//! joining it onto a path: a name with a separator in it is a file written
//! somewhere no one asked for, and the one thing a directory named after
//! someone else's string must not do is leave the directory.

use std::path::{Path, PathBuf};

use console_core_never::Never;

pub const UNDER: &str = "screens";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output<'a>(pub &'a str);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unnamed {
    NotAConnector(String),
}

impl std::fmt::Display for Unnamed {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unnamed::NotAConnector(said) => {
                write!(to, "{said:?} is not what a kernel calls a connector")
            }
        }
    }
}

impl std::error::Error for Unnamed {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spelled {
    LikeAConnector,
    LikeSomethingElse,
}

fn spelled(named: &str) -> Result<Spelled, Never> {
    let letters = named.chars().all(|one| one.is_ascii_alphanumeric() || one == '-');

    Ok(match named.is_empty() || !letters {
        true => Spelled::LikeSomethingElse,
        false => Spelled::LikeAConnector,
    })
}

impl Output<'_> {
    pub fn under(self, home: &Path) -> Result<PathBuf, Unnamed> {
        let named = self.0.trim();
        let Ok(spelled) = spelled(named);

        match spelled {
            Spelled::LikeAConnector => {},
            Spelled::LikeSomethingElse => return Err(Unnamed::NotAConnector(self.0.to_string())),
        }

        let Ok(ours) = console_core_places::Base::Configuration.ours_under(home);

        Ok(ours.join(UNDER).join(named))
    }

    pub fn keeping(self, home: &Path, called: &str) -> Result<PathBuf, Unnamed> {
        let under = self.under(home)?;

        Ok(under.join(called))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(named: &str, called: &str) -> Result<PathBuf, Unnamed> {
        Output(named).keeping(Path::new("/home/ada"), called)
    }

    #[test]
    fn a_screens_answers_are_kept_in_a_directory_named_after_it() {
        assert_eq!(
            at("eDP-1", "turn"),
            Ok(PathBuf::from("/home/ada/.config/console/screens/eDP-1/turn"))
        );
        assert_eq!(
            at("DP-3", "scale"),
            Ok(PathBuf::from("/home/ada/.config/console/screens/DP-3/scale"))
        );
    }

    #[test]
    fn two_screens_do_not_share_an_answer() {
        assert_ne!(at("eDP-1", "turn"), at("HDMI-A-1", "turn"));
    }

    #[test]
    fn a_name_that_could_leave_the_directory_is_not_a_connector() {
        assert_eq!(at("../../..", "turn"), Err(Unnamed::NotAConnector("../../..".to_string())));
        assert_eq!(
            at("eDP-1/../../x", "turn"),
            Err(Unnamed::NotAConnector("eDP-1/../../x".to_string()))
        );
        assert_eq!(at("", "turn"), Err(Unnamed::NotAConnector(String::new())));
    }
}
