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

use console_notices::saying::{Kept, fault, for_the_journal, journal, raise};

fn main() -> std::process::ExitCode {
    let said: Vec<String> = std::env::args().skip(1).collect();
    let [kind, summary, rest @ ..] = said.as_slice() else {
        eprintln!("usage: console-say KIND SUMMARY [BODY]");
        return std::process::ExitCode::from(2);
    };
    if kind.is_empty() || summary.is_empty() {
        eprintln!("usage: console-say KIND SUMMARY [BODY]");
        return std::process::ExitCode::from(2);
    }
    let body = rest.first().map(String::as_str).unwrap_or_default();

    journal(&for_the_journal(kind, summary, body));

    // Counted before it is decided, so the count is what has happened rather
    // than what was shown: a kind that has gone quiet goes on counting, and
    // nothing starts showing again because the screen stopped.
    if let Some(notice) = fault(summary, body, Kept::counting(kind).again()) {
        raise(&notice);
    }
    std::process::ExitCode::SUCCESS
}
