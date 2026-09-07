//! How a subscription and a word are spelled on the socket.
//!
//! Lines, because every source this relays is already lines and because a
//! person debugging a desktop should be able to hold `socat` against the
//! socket and read what is going past. There is no framing beyond the newline
//! and no version number: both ends are built by the same `console apply` from
//! the same commit, so a wire that could disagree with itself is a problem
//! this machine cannot have.
//!
//! A topic is one token, which is what makes `said <topic> <the line>`
//! unambiguous. The only topic with anything in it is a path, and a path may
//! contain a space, so it is escaped -- and that escaping is the whole reason
//! this module has tests rather than being three `format!`s at the call sites.

use std::path::PathBuf;

use console_core_never::Never;
use console_program_contract::{Changed, Topic};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Says {
    Listen(Topic),
    Deafen(Topic),
    Said(Changed),
}

pub fn spelt(says: &Says) -> Result<String, Never> {
    Ok(match says {
        Says::Listen(topic) => {
            let Ok(token) = token(topic);

            format!("listen {token}")
        }
        Says::Deafen(topic) => {
            let Ok(token) = token(topic);

            format!("deafen {token}")
        }
        Says::Said(changed) => {
            let Ok(token) = token(&changed.about);

            format!("said {token} {}", changed.said)
        }
    })
}

pub fn read(line: &str) -> Result<Option<Says>, Never> {
    let Some((verb, rest)) = line.split_once(' ') else { return Ok(None) };

    Ok(match verb {
        "listen" => {
            let Ok(Some(about)) = topic(rest) else { return Ok(None) };

            Some(Says::Listen(about))
        }
        "deafen" => {
            let Ok(Some(about)) = topic(rest) else { return Ok(None) };

            Some(Says::Deafen(about))
        }
        "said" => {
            let (spelt, said) = rest.split_once(' ').unwrap_or((rest, ""));

            let Ok(Some(about)) = topic(spelt) else { return Ok(None) };

            Some(Says::Said(Changed { about, said: said.to_string() }))
        }
        _ => None,
    })
}

pub fn token(topic: &Topic) -> Result<String, Never> {
    Ok(match topic {
        Topic::Compositor => "compositor".to_string(),
        Topic::Sound => "sound".to_string(),
        Topic::Network => "network".to_string(),
        Topic::Notices => "notices".to_string(),
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
        "notices" => Some(Topic::Notices),
        "units" => Some(Topic::Units),
        "player" => Some(Topic::Player),
        _ => {
            let Some(at) = token.strip_prefix("path:") else { return Ok(None) };

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

    fn round(says: &Says) -> Option<Says> {
        let Ok(spelt) = spelt(says);
        let Ok(read) = read(&spelt);

        read
    }

    #[test]
    fn every_topic_survives_the_journey() {
        for topic in [
            Topic::Compositor,
            Topic::Sound,
            Topic::Network,
            Topic::Notices,
            Topic::Units,
            Topic::Player,
        ] {
            assert_eq!(round(&Says::Listen(topic.clone())), Some(Says::Listen(topic)));
        }
    }

    #[test]
    fn a_watched_path_with_a_space_in_it_comes_back_whole() {
        let at = PathBuf::from("/home/someone/My Pictures/a\\b");
        let says = Says::Listen(Topic::Path(at.clone()));

        assert_eq!(round(&says), Some(Says::Listen(Topic::Path(at))));
    }

    #[test]
    fn what_a_source_said_is_passed_on_exactly() {
        let said = Changed {
            about: Topic::Compositor,
            said: "openwindow>>a1b2,1,alacritty,a terminal".to_string(),
        };

        assert_eq!(round(&Says::Said(said.clone())), Some(Says::Said(said)));
    }

    #[test]
    fn a_word_with_nothing_after_it_is_still_a_word() {
        let said = Changed { about: Topic::Sound, said: String::new() };

        assert_eq!(round(&Says::Said(said.clone())), Some(Says::Said(said)));
    }

    #[test]
    fn nonsense_on_a_socket_anybody_can_open_is_refused_rather_than_guessed_at() {
        let Ok(nothing) = read("");
        let Ok(a_verb_alone) = read("listen");
        let Ok(an_unknown_topic) = read("listen nothing-like-that");
        let Ok(an_unknown_verb) = read("shout compositor");

        assert_eq!(nothing, None);
        assert_eq!(a_verb_alone, None);
        assert_eq!(an_unknown_topic, None);
        assert_eq!(an_unknown_verb, None);
    }
}
