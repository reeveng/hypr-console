//! What the greeter and the login window say to each other, a line each.
//!
//! greetd's greeters speak JSON over a socket because greetd has to serve
//! greeters it has never met. This one has met its greeter: a line is a word
//! and what follows it, and there are five lines in all. A pattern is letters
//! and nothing else, so a line that says `login` with anything else after it
//! is not a login, and nothing a module says can end a line early because a
//! newline in it is written as a space.

use console_core_never::Never;

#[derive(Clone, PartialEq, Eq)]
pub enum FromGreeter {
    Login(String),
    Terminal,
}

impl std::fmt::Debug for FromGreeter {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FromGreeter::Login(_) => write!(to, "Login(..)"),
            FromGreeter::Terminal => write!(to, "Terminal"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToGreeter {
    Failed(String),
    Message(String),
    Welcome,
}

const LOGIN: &str = "login";
const TERMINAL: &str = "terminal";
const FAILED: &str = "failed";
const MESSAGE: &str = "message";
const WELCOME: &str = "welcome";

pub fn from_greeter(line: &str) -> Result<Option<FromGreeter>, Never> {
    let line = line.trim_end_matches('\n');
    let (word, rest) = match line.split_once(' ') {
        Some((word, rest)) => (word, Some(rest)),
        None => (line, None),
    };

    Ok(match (word, rest) {
        (LOGIN, Some(secret)) => {
            let letters = !secret.is_empty() && secret.chars().all(|letter| letter.is_ascii_alphanumeric());

            match letters {
                true => Some(FromGreeter::Login(secret.to_string())),
                false => None,
            }
        }
        (TERMINAL, None) => Some(FromGreeter::Terminal),
        _ => None,
    })
}

pub fn to_greeter(line: &str) -> Result<Option<ToGreeter>, Never> {
    let line = line.trim_end_matches('\n');
    let (word, rest) = match line.split_once(' ') {
        Some((word, rest)) => (word, rest),
        None => (line, ""),
    };

    Ok(match word {
        FAILED => Some(ToGreeter::Failed(rest.to_string())),
        MESSAGE => Some(ToGreeter::Message(rest.to_string())),
        WELCOME => Some(ToGreeter::Welcome),
        _ => None,
    })
}

pub fn line_from_greeter(message: &FromGreeter) -> Result<String, Never> {
    Ok(match message {
        FromGreeter::Login(secret) => format!("{LOGIN} {secret}\n"),
        FromGreeter::Terminal => format!("{TERMINAL}\n"),
    })
}

pub fn line_to_greeter(message: &ToGreeter) -> Result<String, Never> {
    let flat = |text: &str| text.replace('\n', " ");

    Ok(match message {
        ToGreeter::Failed(why) => format!("{FAILED} {}\n", flat(why)),
        ToGreeter::Message(text) => format!("{MESSAGE} {}\n", flat(text)),
        ToGreeter::Welcome => format!("{WELCOME}\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_reads_back_as_what_was_written() {
        let from = [FromGreeter::Login("tat".to_string()), FromGreeter::Terminal];
        let to = [ToGreeter::Failed("that is not the pattern".to_string()), ToGreeter::Message("hello".to_string()), ToGreeter::Welcome];

        for message in from {
            let Ok(line) = line_from_greeter(&message);
            let Ok(read) = from_greeter(&line);

            assert_eq!(read, Some(message));
        }

        for message in to {
            let Ok(line) = line_to_greeter(&message);
            let Ok(read) = to_greeter(&line);

            assert_eq!(read, Some(message));
        }
    }

    #[test]
    fn a_login_that_is_not_letters_is_not_a_login() {
        for line in ["login", "login ", "login tat tat", "login t;t", "login tat\ttat"] {
            let Ok(read) = from_greeter(line);

            assert_eq!(read, None, "{line:?}");
        }
    }

    #[test]
    fn a_module_cannot_end_a_line_early() {
        let Ok(line) = line_to_greeter(&ToGreeter::Message("one\nwelcome".to_string()));

        assert_eq!(line.matches('\n').count(), 1);
    }

    #[test]
    fn a_login_never_prints_its_pattern() {
        assert_eq!(format!("{:?}", FromGreeter::Login("tat".to_string())), "Login(..)");
    }
}
