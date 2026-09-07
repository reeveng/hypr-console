//! How long this panel took to appear, stamped as it appears.
//! `console_response_times` is where the numbers go and what they mean. This is
//! the one panel's stopwatch, and it is here rather than passed from hand to
//! hand because the moments worth stamping are spread across five places that
//! have nothing else to do with each other: `show` knows when GTK came up,
//! `new` knows when the card was built, `place` knows when the rows went on it,
//! and the frame clock knows when any of it was first drawn. Threading a
//! stopwatch through all four would put a timing argument in signatures that
//! are about drawing.  One panel, one process, one loop. It is a thread-local
//! rather than a static because everything here happens on the loop that draws,
//! and a stopwatch that could be reached from the thread the rows are read on
//! would be a stopwatch that needs a lock -- which is more machinery than the
//! thing it measures.  Nothing here fails and nothing here waits. A panel that
//! could not write a timing draws exactly as it would have.  `asked` is the
//! same stopwatch started for a panel that was not exec'd. The stand-in was, so
//! it hands over what its own start cost and what the press behind it was
//! stamped with, and the marks read the way they always did -- minus the one
//! for the toolkit coming up, whose absence is the point.

use std::cell::RefCell;
use std::time::Duration;

use console_core_never::Never;
use console_response_times::Waiting;

thread_local! {
    static OPENING: RefCell<Option<Waiting>> = const { RefCell::new(None) };
}

pub fn started(who: &str) -> Result<(), Never> {
    let Ok(waiting) = Waiting::on(who, "opening");

    OPENING.with(|held| *held.borrow_mut() = Some(waiting));

    Ok(())
}

pub fn asked(
    who: &str,
    pressed: Option<&str>,
    from: &str,
    exec: Duration,
) -> Result<(), Never> {
    let Ok(waiting) = Waiting::asked(who, "opening", pressed, from, exec);

    OPENING.with(|held| *held.borrow_mut() = Some(waiting));

    Ok(())
}

pub fn mark(doing: &str) -> Result<(), Never> {
    with(|waiting| waiting.mark(doing))
}

pub fn taking(doing: &str, took: Duration) -> Result<(), Never> {
    with(|waiting| waiting.taking(doing, took))
}

pub fn counted(name: &str, many: u64) -> Result<(), Never> {
    with(|waiting| waiting.counted(name, many))
}

pub fn named(name: &str, said: &str) -> Result<(), Never> {
    with(|waiting| waiting.named(name, said))
}

pub fn running() -> Result<Running, Never> {
    Ok(match OPENING.with(|held| held.try_borrow().is_ok_and(|held| held.is_some())) {
        true => Running::Yes,
        false => Running::No,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Running {
    Yes,
    No,
}

pub fn done() -> Result<(), Never> {
    let waiting = OPENING.with(|held| held.borrow_mut().take());

    match waiting {
        Some(waiting) => {
            let Ok(()) = waiting.done();
        }
        None => {},
    }

    Ok(())
}

fn with(doing: impl FnOnce(&mut Waiting) -> Result<(), Never>) -> Result<(), Never> {
    OPENING.with(|held| {
        match held.try_borrow_mut() {
            Ok(mut held) => match held.as_mut() {
                Some(waiting) => {
                    let Ok(()) = doing(waiting);
                }
                None => {},
            },
            Err(_it_is_already_being_written) => {},
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_running_until_something_starts_it() {
        assert_eq!(running(), Ok(Running::No));
    }

    #[test]
    fn stamping_an_opening_that_was_never_started_does_nothing() {
        let Ok(()) = mark("gtk");
        let Ok(()) = counted("rows", 4);
        let Ok(()) = named("door", "menu");
        let Ok(()) = taking("screen", Duration::from_millis(20));
        let Ok(()) = done();
    }
}
