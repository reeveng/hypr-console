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

use console_never::Never;

use crate::level::Muted;

pub const SINK: &str = "@DEFAULT_SINK@";

pub const STEP: &str = "5%";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Press {
    Up,
    Down,
    Mute,
}

impl Press {
    pub fn named(word: &str) -> Result<Option<Self>, Never> {
        match word {
            "up" => Ok(Some(Press::Up)),
            "down" => Ok(Some(Press::Down)),
            "mute" => Ok(Some(Press::Mute)),
            _ => Ok(None),
        }
    }
}

pub fn asks(press: Press) -> Result<Vec<Vec<String>>, Never> {
    let words = |argv: &[&str]| argv.iter().map(|word| (*word).to_string()).collect();

    match press {
        Press::Up => Ok(vec![
            words(&["set-sink-mute", SINK, "0"]),
            words(&["set-sink-volume", SINK, &format!("+{STEP}")]),
        ]),
        Press::Down => Ok(vec![words(&["set-sink-volume", SINK, &format!("-{STEP}")])]),
        Press::Mute => Ok(vec![words(&["set-sink-mute", SINK, "toggle"])]),
    }
}

pub fn said(level: Option<&str>, muted: Muted) -> Result<String, Never> {
    match muted {
        Muted::Yes => Ok("Silent".to_string()),
        Muted::No => Ok(format!("Volume {}", level.unwrap_or("?"))),
    }
}

pub fn level(said: &str) -> Result<Option<&str>, Never> {
    let Some(first) = said.lines().next() else {
        return Ok(None);
    };

    Ok(first.split_whitespace().nth(4))
}

pub fn muted(said: &str) -> Result<Muted, Never> {
    match said.trim().ends_with("yes") {
        true => Ok(Muted::Yes),
        false => Ok(Muted::No),
    }
}

pub fn value(level: Option<&str>) -> Result<Option<i64>, Never> {
    let Some(said) = level else {
        return Ok(None);
    };

    let Ok(value) = said.trim_end_matches('%').parse::<i64>() else { return Ok(None) };

    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turning_it_up_unsilences_it_first() {
        let asked = asks(Press::Up).expect("what it asks");
        assert_eq!(asked[0][0], "set-sink-mute");
        assert_eq!(asked[0][2], "0");
        assert_eq!(asked[1][0], "set-sink-volume");
    }

    #[test]
    fn turning_it_down_leaves_it_silenced() {
        let asked = asks(Press::Down).expect("what it asks");
        assert_eq!(asked.len(), 1);
        assert!(!asked.iter().any(|argv| argv[0] == "set-sink-mute"));
    }

    #[test]
    fn mute_is_a_toggle_and_moves_no_number() {
        assert_eq!(
            asks(Press::Mute),
            Ok(vec![vec!["set-sink-mute".to_string(), SINK.to_string(), "toggle".to_string()]])
        );
    }

    #[test]
    fn nothing_but_the_three_words_is_a_press() {
        assert_eq!(Press::named("up"), Ok(Some(Press::Up)));
        assert_eq!(Press::named("UP"), Ok(None));
        assert_eq!(Press::named(""), Ok(None));
    }

    #[test]
    fn the_percentage_is_read_out_of_what_pactl_says() {
        let said = "Volume: front-left: 32768 /  50% / -18.06 dB,   front-right: 32768 /  50%\n";
        assert_eq!(level(said), Ok(Some("50%")));

        let found = level(said).expect("the level");

        assert_eq!(value(found), Ok(Some(50)));
    }

    #[test]
    fn nothing_pactl_says_is_ever_a_reason_to_fail() {
        assert_eq!(level(""), Ok(None));
        assert_eq!(value(None), Ok(None));
        assert_eq!(said(None, Muted::No), Ok("Volume ?".to_string()));
    }

    #[test]
    fn silence_is_said_rather_than_shown_as_a_number() {
        assert_eq!(said(Some("50%"), Muted::Yes), Ok("Silent".to_string()));
        assert_eq!(said(Some("50%"), Muted::No), Ok("Volume 50%".to_string()));
    }

    #[test]
    fn what_mute_says_is_read_off_the_end_of_the_line() {
        assert_eq!(muted("Mute: yes"), Ok(Muted::Yes));
        assert_eq!(muted("Mute: no"), Ok(Muted::No));
        assert_eq!(muted(""), Ok(Muted::No));
    }
}
