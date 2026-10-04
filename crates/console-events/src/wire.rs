//! How a subscription and a word are spelled on the socket.
//!
//! Lines, because every source this relays is already lines and because a
//! person debugging a desktop should be able to hold `socat` against the
//! socket and read what is going past. There is no framing beyond the newline
//! and no version number: both ends are built by the same `console apply` from
//! the same commit, so a wire that could disagree with itself is a problem
//! this machine cannot have.
//!
//! An event group is one token, which is what makes `publish <event group> <the
//! line>` unambiguous. The only event group with anything in it is a path, and
//! a path may contain a space, so it is escaped -- and that escaping is the
//! whole reason this module has tests rather than being three `format!`s at the
//! call sites.

use std::path::PathBuf;

use console_core_never::Never;
use console_program_contract::{Change, EventGroup};

const NOTHING_AFTER_IT: &str = "";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Subscribe(EventGroup),
    Unsubscribe(EventGroup),
    Publish(Change),
}

pub fn encoded(message: &Message) -> Result<String, Never> {
    Ok(match message {
        Message::Subscribe(event_group) => {
            let Ok(token) = token(event_group);

            format!("subscribe {token}")
        }
        Message::Unsubscribe(event_group) => {
            let Ok(token) = token(event_group);

            format!("unsubscribe {token}")
        }
        Message::Publish(change) => {
            let Ok(token) = token(&change.event_group);

            format!("publish {token} {}", change.text)
        }
    })
}

pub fn decoded(line: &str) -> Result<Option<Message>, Never> {
    let (verb, rest) = match line.split_once(' ') {
        Some((verb, rest)) => (verb, rest),
        None => return Ok(None),
    };

    Ok(match verb {
        "subscribe" => {
            let requested = match event_group(rest) {
                Ok(Some(requested)) => requested,
                Ok(None) | Err(_) => return Ok(None),
            };

            Some(Message::Subscribe(requested))
        }
        "unsubscribe" => {
            let requested = match event_group(rest) {
                Ok(Some(requested)) => requested,
                Ok(None) | Err(_) => return Ok(None),
            };

            Some(Message::Unsubscribe(requested))
        }
        "publish" => {
            let (spelled, text) = match rest.split_once(' ') {
                Some(both) => both,
                None => (rest, NOTHING_AFTER_IT),
            };

            let requested = match event_group(spelled) {
                Ok(Some(requested)) => requested,
                Ok(None) | Err(_) => return Ok(None),
            };

            Some(Message::Publish(Change { event_group: requested, text: text.to_string() }))
        }
        _ => None,
    })
}

pub fn token(event_group: &EventGroup) -> Result<String, Never> {
    Ok(match event_group {
        EventGroup::Compositor => "compositor".to_string(),
        EventGroup::Sound => "sound".to_string(),
        EventGroup::Network => "network".to_string(),
        EventGroup::Wifi => "wifi".to_string(),
        EventGroup::Bluetooth => "bluetooth".to_string(),
        EventGroup::Battery => "battery".to_string(),
        EventGroup::Notifications => "notifications".to_string(),
        EventGroup::Units => "units".to_string(),
        EventGroup::Player => "player".to_string(),
        EventGroup::Path(at) => {
            let Ok(escaped) = escaped(&at.display().to_string());

            format!("path:{escaped}")
        }
    })
}

pub fn event_group(token: &str) -> Result<Option<EventGroup>, Never> {
    Ok(match token {
        "compositor" => Some(EventGroup::Compositor),
        "sound" => Some(EventGroup::Sound),
        "network" => Some(EventGroup::Network),
        "wifi" => Some(EventGroup::Wifi),
        "bluetooth" => Some(EventGroup::Bluetooth),
        "battery" => Some(EventGroup::Battery),
        "notifications" => Some(EventGroup::Notifications),
        "units" => Some(EventGroup::Units),
        "player" => Some(EventGroup::Player),
        _ => {
            let at = match token.strip_prefix("path:") {
                Some(at) => at,
                None => return Ok(None),
            };

            let Ok(plain) = plain(at);

            Some(EventGroup::Path(PathBuf::from(plain)))
        }
    })
}

fn escaped(at: &str) -> Result<String, Never> {
    Ok(at.replace('\\', "\\\\").replace(' ', "\\s"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Backslash,
    None,
}

fn plain(at: &str) -> Result<String, Never> {
    let (mut plain, pending) = at.chars().fold((String::new(), Pending::None), |(mut plain, pending), letter| {
        match (pending, letter) {
            (Pending::Backslash, 's') => plain.push(' '),
            (Pending::Backslash, '\\') => plain.push('\\'),
            (Pending::Backslash, other) => {
                plain.push('\\');
                plain.push(other);
            }
            (Pending::None, '\\') => return (plain, Pending::Backslash),
            (Pending::None, other) => plain.push(other),
        }

        (plain, Pending::None)
    });

    match pending {
        Pending::Backslash => plain.push('\\'),
        Pending::None => {},
    }

    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(message: &Message) -> Result<Option<Message>, Never> {
        let Ok(spelled) = encoded(message);

        decoded(&spelled)
    }

    #[test]
    fn every_event_group_survives_the_journey() {
        for event_group in [
            EventGroup::Compositor,
            EventGroup::Sound,
            EventGroup::Network,
            EventGroup::Wifi,
            EventGroup::Bluetooth,
            EventGroup::Battery,
            EventGroup::Notifications,
            EventGroup::Units,
            EventGroup::Player,
        ] {
            assert_eq!(round(&Message::Subscribe(event_group.clone())), Ok(Some(Message::Subscribe(event_group))));
        }
    }

    #[test]
    fn a_watched_path_with_a_space_in_it_comes_back_whole() {
        let at = PathBuf::from("/home/someone/My Pictures/a\\b");
        let message = Message::Subscribe(EventGroup::Path(at.clone()));

        assert_eq!(round(&message), Ok(Some(Message::Subscribe(EventGroup::Path(at)))));
    }

    #[test]
    fn what_a_source_said_is_passed_on_exactly() {
        let change = Change {
            event_group: EventGroup::Compositor,
            text: "openwindow>>a1b2,1,alacritty,a terminal".to_string(),
        };

        assert_eq!(round(&Message::Publish(change.clone())), Ok(Some(Message::Publish(change))));
    }

    #[test]
    fn a_word_with_nothing_after_it_is_still_a_word() {
        let change = Change { event_group: EventGroup::Sound, text: String::new() };

        assert_eq!(round(&Message::Publish(change.clone())), Ok(Some(Message::Publish(change))));
    }

    #[test]
    fn nonsense_on_a_socket_anyone_can_open_is_refused_rather_than_guessed_at() {
        let Ok(nothing) = decoded("");
        let Ok(a_verb_alone) = decoded("subscribe");
        let Ok(an_unknown_event_group) = decoded("subscribe nothing-like-that");
        let Ok(an_unknown_verb) = decoded("shout compositor");

        assert_eq!(nothing, None);
        assert_eq!(a_verb_alone, None);
        assert_eq!(an_unknown_event_group, None);
        assert_eq!(an_unknown_verb, None);
    }
}
