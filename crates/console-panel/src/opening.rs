//! How long this panel took to appear, stamped as it appears.
//!
//! `console_timings` is where the numbers go and what they mean. This is the
//! one panel's stopwatch, and it is here rather than passed from hand to hand
//! because the moments worth stamping are spread across five places that have
//! nothing else to do with each other: `show` knows when GTK came up, `new`
//! knows when the card was built, `place` knows when the rows went on it, and
//! the frame clock knows when any of it was first drawn. Threading a stopwatch
//! through all four would put a timing argument in signatures that are about
//! drawing.
//!
//! One panel, one process, one loop. It is a thread-local rather than a static
//! because everything here happens on the loop that draws, and a stopwatch that
//! could be reached from the thread the rows are read on would be a stopwatch
//! that needs a lock -- which is more machinery than the thing it measures.
//!
//! Nothing here fails and nothing here waits. A panel that could not write a
//! timing draws exactly as it would have.

use std::cell::RefCell;
use std::time::Duration;

use console_timings::Waiting;

thread_local! {
    /// The opening being timed, until it has been written down.
    static OPENING: RefCell<Option<Waiting>> = const { RefCell::new(None) };
}

/// Start the clock, as far back as this process can see it.
pub fn started(who: &str) {
    OPENING.with(|held| *held.borrow_mut() = Some(Waiting::on(who, "opening")));
}

/// The stretch that ends here.
pub fn mark(doing: &str) {
    with(|waiting| waiting.mark(doing));
}

/// A stretch that had already gone by the time the clock was started.
pub fn taking(doing: &str, took: Duration) {
    with(|waiting| waiting.taking(doing, took));
}

/// How many rows the card came up with.
pub fn counted(name: &str, many: u64) {
    with(|waiting| waiting.counted(name, many));
}

/// Which tab, which door.
pub fn named(name: &str, said: &str) {
    with(|waiting| waiting.named(name, said));
}

/// Whether there is still an opening to write down.
///
/// Asked by the frame clock, which goes on calling for as long as the panel is
/// up and finds nothing to do on every frame after the first.
pub fn running() -> Running {
    match OPENING.with(|held| held.try_borrow().is_ok_and(|held| held.is_some())) {
        true => Running::Yes,
        false => Running::No,
    }
}

/// Whether something is on its way up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Running {
    /// Something was started and has not appeared yet.
    Yes,
    /// Nothing is.
    No,
}

/// It is on the screen. Write it down.
///
/// Called again -- a second frame, a panel drawn twice -- and there is nothing
/// left to write, which is the point: an opening is the first time somebody saw
/// it, and every frame after that is the panel working rather than appearing.
pub fn done() {
    let waiting = OPENING.with(|held| held.borrow_mut().take());

    if let Some(waiting) = waiting {
        waiting.done();
    }
}

fn with(doing: impl FnOnce(&mut Waiting)) {
    OPENING.with(|held| {
        if let Ok(mut held) = held.try_borrow_mut()
            && let Some(waiting) = held.as_mut()
        {
            doing(waiting);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame clock asks this on every frame, so what it says before
    /// anything has begun is what it says after everything has been written.
    #[test]
    fn nothing_is_running_until_something_starts_it() {
        assert_eq!(running(), Running::No);
    }

    /// A panel that was never started is a panel nothing is being timed about,
    /// and every stamp on it is a stamp that does nothing. Which is what the
    /// checks do when they drive a panel that is not being measured.
    #[test]
    fn stamping_an_opening_that_was_never_started_does_nothing() {
        mark("gtk");
        counted("rows", 4);
        named("door", "menu");
        taking("screen", Duration::from_millis(20));
        done();
    }
}
