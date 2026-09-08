//! Say something went wrong, where somebody who is not in a terminal sees it.
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

use console_notifications::saying::{Kept, fault, for_the_journal, journal, raise};

fn main() -> std::process::ExitCode {
    let said: Vec<String> = std::env::args().skip(1).collect();

    let (kind, summary, rest) = match said.as_slice() {
        [kind, summary, rest @ ..] => (kind, summary, rest),
        _not_enough_words => {
            eprintln!("usage: console-say KIND SUMMARY [BODY]");
            return std::process::ExitCode::from(2);
        }
    };

    match kind.is_empty() || summary.is_empty() {
        true => {
            eprintln!("usage: console-say KIND SUMMARY [BODY]");
            return std::process::ExitCode::from(2);
        }
        false => {}
    }

    let body = rest.first().map(String::as_str).unwrap_or_default();

    let Ok(said) = for_the_journal(kind, summary, body);
    let Ok(()) = journal(&said);
    let Ok(counting) = Kept::counting(kind);
    let Ok(again) = counting.again();
    let Ok(fault) = fault(summary, body, again);

    match fault {
        Some(notice) => {
            let Ok(_) = raise(&notice);
        }
        None => {}
    }

    std::process::ExitCode::SUCCESS
}
