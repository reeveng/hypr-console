//! Which panel this is, said rather than worked out.
//!
//! It was `argv[0]`, and that was true for as long as a panel was a process.
//! The program's own name is the layer-shell namespace the compositor reports,
//! and four things read it back: the bar lights an icon by it, the daemon
//! knows a chooser is up by it, the tab a panel was left on is filed under it,
//! and every line in the timing file says which surface it was about. One
//! process drawing several panels in turn has no such name, and the one name
//! it does have is the host's.
//!
//! So it is said at the top of an opening and read wherever the old call was.
//! A thread-local for the reason `opening` is one: everything that asks
//! happens on the loop that draws, and a name reachable from the thread the
//! rows are gathered on is a name that needs a lock.
//!
//! Nothing said is still `argv[0]`, which is what a panel drawn by its own
//! program has always been, and is what every test and every fallback gets
//! without asking for it.

use std::cell::RefCell;
use std::path::Path;

use console_never::Never;

const NOBODY: &str = "console-panel";

thread_local! {
    static WHOSE: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn named(who: &str) -> Result<(), Never> {
    let said = match who.is_empty() {
        true => None,
        false => Some(who.to_string()),
    };

    WHOSE.with(|held| *held.borrow_mut() = said);

    Ok(())
}

pub fn nobody() -> Result<(), Never> {
    WHOSE.with(|held| *held.borrow_mut() = None);

    Ok(())
}

pub fn name() -> Result<String, Never> {
    let said = WHOSE.with(|held| held.borrow().clone());

    match said {
        Some(said) => Ok(said),
        None => argv0(),
    }
}

fn argv0() -> Result<String, Never> {
    Ok(std::env::args()
        .next()
        .and_then(|argv0| {
            Path::new(&argv0).file_name().and_then(|name| name.to_str()).map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| NOBODY.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_was_said_is_what_is_read() {
        let Ok(()) = named("settings-panel");

        assert_eq!(name(), Ok("settings-panel".to_string()));

        let Ok(()) = nobody();
    }

    #[test]
    fn one_opening_does_not_leave_its_name_behind() {
        let Ok(()) = named("launcher");
        let Ok(()) = nobody();
        let Ok(argv0) = argv0();

        assert_eq!(name(), Ok(argv0), "the next opening would draw under the last one's name");
    }

    #[test]
    fn a_name_that_says_nothing_is_not_a_name() {
        let Ok(()) = named("");
        let Ok(argv0) = argv0();

        assert_eq!(name(), Ok(argv0));
    }

    #[test]
    fn the_program_is_what_a_panel_drawing_itself_is_called() {
        let Ok(argv0) = argv0();

        assert!(!argv0.is_empty(), "a surface with no namespace is one nothing can find");
        assert!(!argv0.contains('/'), "the namespace is a name, not a path");
    }
}
