//! How a subscription and a word are spelled on the socket.
//!
//! Lines, because every source this relays is already lines and because a
//! person debugging a desktop should be able to hold `socat` against the
//! socket and read what is going past. There is no framing beyond the newline
//! and no version number: both ends are built by the same `console apply` from
//! the same commit, so a wire that could disagree with itself is a problem
//! this machine cannot have.
//!
//! A topic is one token, which is what makes `publish <topic> <the line>`
//! unambiguous. The only topic with anything in it is a path, and a path may
//! contain a space, so it is escaped -- and that escaping is the whole reason
//! this module has tests rather than being three `format!`s at the call sites.

use std::path::PathBuf;

use console_core_never::Never;
use console_program_contract::{Change, Topic};

const NOTHING_AFTER_IT: &str = "";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Subscribe(Topic),
    Unsubscribe(Topic),
    Publish(Change),
}

pub fn encoded(message: &Message) -> Result<String, Never> {
    Ok(match message {
        Message::Subscribe(topic) => {
            let Ok(token) = token(topic);

            format!("subscribe {token}")
        }
        Message::Unsubscribe(topic) => {
            let Ok(token) = token(topic);

            format!("unsubscribe {token}")
        }
        Message::Publish(change) => {
            let Ok(token) = token(&change.topic);

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
            let requested = match topic(rest) {
                Ok(Some(requested)) => requested,
                Ok(None) | Err(_) => return Ok(None),
            };

            Some(Message::Subscribe(requested))
        }
        "unsubscribe" => {
            let requested = match topic(rest) {
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

            let requested = match topic(spelled) {
                Ok(Some(requested)) => requested,
                Ok(None) | Err(_) => return Ok(None),
            };

            Some(Message::Publish(Change { topic: requested, text: text.to_string() }))
        }
        _ => None,
    })
}

pub fn token(topic: &Topic) -> Result<String, Never> {
    Ok(match topic {
        Topic::Compositor => "compositor".to_string(),
        Topic::Sound => "sound".to_string(),
        Topic::Network => "network".to_string(),
        Topic::Wifi => "wifi".to_string(),
        Topic::Bluetooth => "bluetooth".to_string(),
        Topic::Battery => "battery".to_string(),
        Topic::Notifications => "notifications".to_string(),
        Topic::Units => "units".to_string(),
        Topic::Player => "player".to_string(),
        Topic::Path(at) => {
            let Ok(escaped) = escaped(&at.display().to_string());

            format!("path:{escaped}")
        }
    })
}

pub fn topic(token: &str) -> Result<Option<Topic>, Never> {
    Ok(match token {
        "compositor" => Some(Topic::Compositor),
        "sound" => Some(Topic::Sound),
        "network" => Some(Topic::Network),
        "wifi" => Some(Topic::Wifi),
        "bluetooth" => Some(Topic::Bluetooth),
        "battery" => Some(Topic::Battery),
        "notifications" => Some(Topic::Notifications),
        "units" => Some(Topic::Units),
        "player" => Some(Topic::Player),
        _ => {
            let at = match token.strip_prefix("path:") {
                Some(at) => at,
                None => return Ok(None),
            };

            let Ok(plain) = plain(at);

            Some(Topic::Path(PathBuf::from(plain)))
        }
    })
}

fn escaped(at: &str) -> Result<String, Never> {
    Ok(at.replace('\\', "\\\\").replace(' ', "\\s"))
}

fn plain(at: &str) -> Result<String, Never> {
    let mut plain = String::new();
    let mut said = at.chars();

    while let Some(letter) = said.next() {
        match letter {
            '\\' => match said.next() {
                Some('s') => plain.push(' '),
                Some('\\') => plain.push('\\'),
                Some(other) => {
                    plain.push('\\');
                    plain.push(other);
                }
                None => plain.push('\\'),
            },
            other => plain.push(other),
        }
    }

    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(message: &Message) -> Option<Message> {
        let Ok(spelled) = encoded(message);
        let Ok(read) = decoded(&spelled);

        read
    }

    #[test]
    fn every_topic_survives_the_journey() {
        for topic in [
            Topic::Compositor,
            Topic::Sound,
            Topic::Network,
            Topic::Wifi,
            Topic::Bluetooth,
            Topic::Battery,
            Topic::Notifications,
            Topic::Units,
            Topic::Player,
        ] {
            assert_eq!(round(&Message::Subscribe(topic.clone())), Some(Message::Subscribe(topic)));
        }
    }

    #[test]
    fn a_watched_path_with_a_space_in_it_comes_back_whole() {
        let at = PathBuf::from("/home/someone/My Pictures/a\\b");
        let message = Message::Subscribe(Topic::Path(at.clone()));

        assert_eq!(round(&message), Some(Message::Subscribe(Topic::Path(at))));
    }

    #[test]
    fn what_a_source_said_is_passed_on_exactly() {
        let change = Change {
            topic: Topic::Compositor,
            text: "openwindow>>a1b2,1,alacritty,a terminal".to_string(),
        };

        assert_eq!(round(&Message::Publish(change.clone())), Some(Message::Publish(change)));
    }

    #[test]
    fn a_word_with_nothing_after_it_is_still_a_word() {
        let change = Change { topic: Topic::Sound, text: String::new() };

        assert_eq!(round(&Message::Publish(change.clone())), Some(Message::Publish(change)));
    }

    #[test]
    fn nonsense_on_a_socket_anyone_can_open_is_refused_rather_than_guessed_at() {
        let Ok(nothing) = decoded("");
        let Ok(a_verb_alone) = decoded("subscribe");
        let Ok(an_unknown_topic) = decoded("subscribe nothing-like-that");
        let Ok(an_unknown_verb) = decoded("shout compositor");

        assert_eq!(nothing, None);
        assert_eq!(a_verb_alone, None);
        assert_eq!(an_unknown_topic, None);
        assert_eq!(an_unknown_verb, None);
    }
}
