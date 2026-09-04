//! The volume rocker on the top edge.
//!
//! One rule here is worth the file. Turning it up also unsilences: the rocker
//! was bound straight at the volume, so on a muted machine pressing it moved a
//! number nobody could hear and the buttons read as broken. Turning it down
//! does not unsilence, because somebody who has just silenced the thing and
//! reaches for down means quieter still, not louder.
//!
//! The other half is where the number went when it left the bar. The bar wears
//! a glyph and no percentage: three speaker marks say quiet, middling and loud,
//! which is what an icon is for. What the number was actually for is this
//! rocker -- pressing it and seeing the figure move is how anybody knows it did
//! anything -- so the figure is said at the moment it changes, where somebody
//! is already looking.

use crate::level::Muted;

/// The sink every one of these is about.
pub const SINK: &str = "@DEFAULT_SINK@";

/// How far one press moves it.
pub const STEP: &str = "5%";

/// One press of the rocker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Press {
    Up,
    Down,
    Mute,
}

impl Press {
    pub fn named(word: &str) -> Option<Self> {
        match word {
            "up" => Some(Press::Up),
            "down" => Some(Press::Down),
            "mute" => Some(Press::Mute),
            _ => None,
        }
    }
}

/// What pactl is asked, in the order it is asked.
///
/// Said as words rather than run, so the one rule this file is about can be
/// asked of it without a sound server: up unsilences and down does not.
pub fn asks(press: Press) -> Vec<Vec<String>> {
    let words = |argv: &[&str]| argv.iter().map(|word| (*word).to_string()).collect();

    match press {
        Press::Up => vec![
            words(&["set-sink-mute", SINK, "0"]),
            words(&["set-sink-volume", SINK, &format!("+{STEP}")]),
        ],
        Press::Down => vec![words(&["set-sink-volume", SINK, &format!("-{STEP}")])],
        Press::Mute => vec![words(&["set-sink-mute", SINK, "toggle"])],
    }
}

/// What the notice says, given what the machine now reports.
///
/// Silence is a word rather than a number, the same way it is on the settings
/// panel: a volume turned down to nothing and a volume silenced at half are
/// different states, and one of them comes back where it was.
pub fn said(level: Option<&str>, muted: Muted) -> String {
    match muted {
        Muted::Yes => "Silent".to_string(),
        Muted::No => format!("Volume {}", level.unwrap_or("?")),
    }
}

/// The first line of `pactl get-sink-volume`, read as the percentage in it.
///
/// pactl prints the reading twice, once per channel, with a good deal else
/// around it. The fifth word of the first line is the one that is a percentage.
pub fn level(said: &str) -> Option<&str> {
    said.lines().next()?.split_whitespace().nth(4)
}

/// Whether `pactl get-sink-mute` said it is silenced.
pub fn muted(said: &str) -> Muted {
    match said.trim().ends_with("yes") {
        true => Muted::Yes,
        false => Muted::No,
    }
}

/// The number a daemon that can draw a bar would want, out of the percentage.
pub fn value(level: Option<&str>) -> Option<i64> {
    let Ok(value) = level?.trim_end_matches('%').parse::<i64>() else { return None };

    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole reason this file exists. On a muted machine, pressing up used
    /// to move a number nobody could hear.
    #[test]
    fn turning_it_up_unsilences_it_first() {
        let asked = asks(Press::Up);
        assert_eq!(asked[0][0], "set-sink-mute");
        assert_eq!(asked[0][2], "0");
        assert_eq!(asked[1][0], "set-sink-volume");
    }

    /// Somebody who has just silenced it and reaches for down means quieter
    /// still, not louder.
    #[test]
    fn turning_it_down_leaves_it_silenced() {
        let asked = asks(Press::Down);
        assert_eq!(asked.len(), 1);
        assert!(!asked.iter().any(|argv| argv[0] == "set-sink-mute"));
    }

    #[test]
    fn mute_is_a_toggle_and_moves_no_number() {
        assert_eq!(asks(Press::Mute), vec![vec!["set-sink-mute", SINK, "toggle"]]);
    }

    #[test]
    fn nothing_but_the_three_words_is_a_press() {
        assert_eq!(Press::named("up"), Some(Press::Up));
        assert_eq!(Press::named("UP"), None);
        assert_eq!(Press::named(""), None);
    }

    #[test]
    fn the_percentage_is_read_out_of_what_pactl_says() {
        let said = "Volume: front-left: 32768 /  50% / -18.06 dB,   front-right: 32768 /  50%\n";
        assert_eq!(level(said), Some("50%"));
        assert_eq!(value(level(said)), Some(50));
    }

    /// pactl answers with nothing on stdout when there is no sound server, and
    /// a reading nobody has is still a sentence somebody can read.
    #[test]
    fn nothing_pactl_says_is_ever_a_reason_to_fail() {
        assert_eq!(level(""), None);
        assert_eq!(value(None), None);
        assert_eq!(said(None, Muted::No), "Volume ?");
    }

    /// A volume turned down to nothing and a volume silenced at half are
    /// different states, and one of them comes back where it was.
    #[test]
    fn silence_is_said_rather_than_shown_as_a_number() {
        assert_eq!(said(Some("50%"), Muted::Yes), "Silent");
        assert_eq!(said(Some("50%"), Muted::No), "Volume 50%");
    }

    #[test]
    fn what_mute_says_is_read_off_the_end_of_the_line() {
        assert_eq!(muted("Mute: yes"), Muted::Yes);
        assert_eq!(muted("Mute: no"), Muted::No);
        assert_eq!(muted(""), Muted::No);
    }
}
