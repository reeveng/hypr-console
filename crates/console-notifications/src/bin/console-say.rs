//! Say something went wrong, where someone who is not in a terminal sees it.
//!
//!     console-say KIND SUMMARY [BODY]
//!
//! The journal always gets it. The screen gets it a few times per kind per
//! session, because everything this is called from is a loop of one sort or
//! another: a service that restarts, a daemon that comes round every five
//! minutes, an apply that walks a list.
//!
//! KIND is what is counted, so it names the fault and not the moment: two
//! pictures that will not delete are one kind, and the picture and the
//! compositor are two.

use console_core_arguments::{Command, Operands, Reason, ValidationError, read};
use console_notifications::saying::{StatePath, Content, fault, for_the_journal, journal, raise};

const NO_BODY: &str = "";


const COMMAND: Command = Command {
    name: "console-say",
    about: "say something went wrong, where someone who is not in a terminal sees it: a KIND that is counted, a SUMMARY, and a BODY if there is one",
    flags: &[],
    operands: Operands::Verbatim("KIND SUMMARY BODY"),
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Message {
    kind: String,
    summary: String,
    body: String,
}

fn message(words: &[String]) -> Result<Message, ValidationError> {
    let read = read(&COMMAND, words);
    let line = read?;
    let Ok(operands) = line.operands();

    let (kind, summary, body) = match operands {
        [kind, summary] => (kind, summary, NO_BODY),
        [kind, summary, body] => (kind, summary, body.as_str()),
        [_kind, _summary, _body, extra, ..] => {
            let Ok(refusal) = line.refusal(Reason::ExtraArgument(extra.clone()));

            return Err(refusal);
        }
        [] | [_] => {
            let missing = ["KIND", "SUMMARY"].into_iter().skip(operands.len()).collect();
            let Ok(refusal) = line.refusal(Reason::MissingOperands(missing));

            return Err(refusal);
        }
    };

    match kind.is_empty() || summary.is_empty() {
        true => {
            let Ok(refusal) = line.refusal(Reason::MissingOperands(vec!["KIND", "SUMMARY"]));

            Err(refusal)
        }
        false => Ok(Message { kind: kind.clone(), summary: summary.clone(), body: body.to_string() }),
    }
}

fn main() -> std::process::ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let Message { kind, summary, body } = match message(&words) {
        Ok(message) => message,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return std::process::ExitCode::from(code);
        }
    };
    let (kind, summary, body) = (kind.as_str(), summary.as_str(), body.as_str());

    let Ok(said) = for_the_journal(kind, Content { summary, body });
    let Ok(()) = journal(&said);
    let Ok(counting) = StatePath::counting(kind);
    let Ok(again) = counting.again();
    let Ok(fault) = fault(Content { summary, body }, again);

    match fault {
        Some(notification) => {
            let Ok(_) = raise(&notification);
        }
        None => {}
    }

    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_never::Never;

    fn words(said: &[&str]) -> Result<Vec<String>, Never> {
        Ok(said.iter().map(|word| (*word).to_string()).collect())
    }

    #[test]
    fn a_body_goes_on_as_it_is_even_when_it_starts_with_dashes() {
        let Ok(dashed) = words(&["apply", "--- stderr ---", "--locked: no such flag"]);
        let Ok(bare) = words(&["apply", "It stopped"]);

        assert_eq!(
            message(&dashed),
            Ok(Message { kind: "apply".to_string(), summary: "--- stderr ---".to_string(), body: "--locked: no such flag".to_string() })
        );
        assert_eq!(message(&bare), Ok(Message { kind: "apply".to_string(), summary: "It stopped".to_string(), body: String::new() }));
    }

    #[test]
    fn it_refuses_a_kind_with_nothing_to_say_and_a_fourth_word() {
        let Ok(alone) = words(&["apply"]);
        let Ok(empty) = words(&["", "It stopped"]);
        let Ok(four) = words(&["apply", "It stopped", "body", "stray"]);

        assert_eq!(message(&alone).map_err(|refusal| refusal.reason), Err(Reason::MissingOperands(vec!["SUMMARY"])));
        assert_eq!(message(&empty).map_err(|refusal| refusal.reason), Err(Reason::MissingOperands(vec!["KIND", "SUMMARY"])));
        assert_eq!(message(&four).map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("stray".to_string())));
    }
}
