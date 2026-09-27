//! The volume rocker on the top edge.
//!
//! One rule here is worth the file. Turning it up also unsilences: the rocker
//! was bound straight at the volume, so on a muted machine pressing it moved a
//! number no one could hear and the buttons read as broken. Turning it down
//! does not unsilence, because someone who has just silenced the thing and
//! reaches for down means quieter still, not louder.
//!
//! The other half is where the number went when it left the bar. The bar wears
//! a glyph and no percentage: three speaker marks say quiet, middling and loud,
//! which is what an icon is for. What the number was actually for is this
//! rocker -- pressing it and seeing the figure move is how anyone knows it did
//! anything -- so the figure is said at the moment it changes, where someone
//! is already looking.

use console_core_never::Never;

use crate::level::Muted;

const NOTHING_HEARD: &str = "?";


pub const SINK: &str = "@DEFAULT_SINK@";

pub const STEP: &str = "5%";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonPress {
    Up,
    Down,
    Mute,
}

impl ButtonPress {
    pub fn parse(word: &str) -> Result<Option<Self>, Never> {
        match word {
            "up" => Ok(Some(ButtonPress::Up)),
            "down" => Ok(Some(ButtonPress::Down)),
            "mute" => Ok(Some(ButtonPress::Mute)),
            _ => Ok(None),
        }
    }
}

pub fn asks(press: ButtonPress) -> Result<Vec<Vec<String>>, Never> {
    let words = |arguments: &[&str]| arguments.iter().map(|word| (*word).to_string()).collect();

    match press {
        ButtonPress::Up => Ok(vec![
            words(&["set-sink-mute", SINK, "0"]),
            words(&["set-sink-volume", SINK, &format!("+{STEP}")]),
        ]),
        ButtonPress::Down => Ok(vec![words(&["set-sink-volume", SINK, &format!("-{STEP}")])]),
        ButtonPress::Mute => Ok(vec![words(&["set-sink-mute", SINK, "toggle"])]),
    }
}

pub fn volume_label(level: Option<&str>, muted: Muted) -> Result<String, Never> {
    match muted {
        Muted::Yes => Ok("Silent".to_string()),
        Muted::No => Ok(format!("Volume {}", match level {
            Some(level) => level,
            None => NOTHING_HEARD,
        })),
    }
}

pub fn level(said: &str) -> Result<Option<&str>, Never> {
    let first = match said.lines().next() {
        Some(first) => first,
        None => return Ok(None),
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
    let said = match level {
        Some(said) => said,
        None => return Ok(None),
    };

    let value = match said.trim_end_matches('%').parse::<i64>() {
        Ok(value) => value,
        Err(_not_a_number) => return Ok(None),
    };

    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    #[test]
    fn turning_it_up_unsilences_it_first() -> Result<(), Failure> {
        let Ok(asked) = asks(ButtonPress::Up);
        let unmuting = asked.first().ok_or("nothing asked")?;
        let raising = asked.get(1).ok_or("one thing asked")?;

        assert_eq!(unmuting.first().map(String::as_str), Some("set-sink-mute"));
        assert_eq!(unmuting.get(2).map(String::as_str), Some("0"));
        assert_eq!(raising.first().map(String::as_str), Some("set-sink-volume"));

        Ok(())
    }

    #[test]
    fn turning_it_down_leaves_it_silenced() {
        let Ok(asked) = asks(ButtonPress::Down);

        assert_eq!(asked.len(), 1);
        assert!(!asked.iter().any(|arguments| arguments.first().is_some_and(|verb| verb == "set-sink-mute")));
    }

    #[test]
    fn mute_is_a_toggle_and_moves_no_number() {
        assert_eq!(
            asks(ButtonPress::Mute),
            Ok(vec![vec!["set-sink-mute".to_string(), SINK.to_string(), "toggle".to_string()]])
        );
    }

    #[test]
    fn nothing_but_the_three_words_is_a_press() {
        assert_eq!(ButtonPress::parse("up"), Ok(Some(ButtonPress::Up)));
        assert_eq!(ButtonPress::parse("UP"), Ok(None));
        assert_eq!(ButtonPress::parse(""), Ok(None));
    }

    #[test]
    fn the_percentage_is_read_out_of_what_pactl_says() {
        let said = "Volume: front-left: 32768 /  50% / -18.06 dB,   front-right: 32768 /  50%\n";
        let Ok(found) = level(said);

        assert_eq!(found, Some("50%"));
        assert_eq!(value(found), Ok(Some(50)));
    }

    #[test]
    fn nothing_pactl_says_is_ever_a_reason_to_fail() {
        assert_eq!(level(""), Ok(None));
        assert_eq!(value(None), Ok(None));
        assert_eq!(volume_label(None, Muted::No), Ok("Volume ?".to_string()));
    }

    #[test]
    fn silence_is_said_rather_than_shown_as_a_number() {
        assert_eq!(volume_label(Some("50%"), Muted::Yes), Ok("Silent".to_string()));
        assert_eq!(volume_label(Some("50%"), Muted::No), Ok("Volume 50%".to_string()));
    }

    #[test]
    fn what_mute_says_is_read_off_the_end_of_the_line() {
        assert_eq!(muted("Mute: yes"), Ok(Muted::Yes));
        assert_eq!(muted("Mute: no"), Ok(Muted::No));
        assert_eq!(muted(""), Ok(Muted::No));
    }
}
