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
//! for the toolkit coming up, whose absence is the point. A host starts it on
//! the thread that took the request and draws on another, so the stopwatch is
//! handed across with `handing` and `handed`: a thread-local nobody hands on is
//! an opening that is never written down, which is what every hosted panel was
//! from the day the toolkit went until the first frame was timed again.

use std::cell::Cell;
use std::time::Duration;

use console_core_never::Never;
use console_response_times::{Note, Wait, Waiting};

thread_local! {
    static OPENING: Cell<Option<Waiting>> = const { Cell::new(None) };
}

pub struct Provided(Option<Waiting>);

pub fn handing() -> Result<Provided, Never> {
  Ok(Provided(OPENING.with(|held| held.take())))
}

pub fn handed(handed: Provided) -> Result<(), Never> {
  match handed.0 {
    Some(waiting) => OPENING.with(|held| held.set(Some(waiting))),
    None => {}
  }

  Ok(())
}

pub fn started(who: &str) -> Result<(), Never> {
  let Ok(waiting) = Waiting::on(Wait {
    who,
    what: "opening",
  });

  OPENING.with(|held| held.set(Some(waiting)));

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Came<'a>(pub &'a str);

pub fn start(
  who: &str,
  pressed: Option<&str>,
  from: Came<'_>,
  exec: Duration,
) -> Result<(), Never> {
  let wait = Wait {
    who,
    what: "opening",
  };
  let Ok(waiting) = Waiting::start(wait, pressed, from.0, exec);

  OPENING.with(|held| held.set(Some(waiting)));

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

pub fn add_note(note: Note<'_>) -> Result<(), Never> {
  with(|waiting| waiting.add_note(note))
}

pub fn running() -> Result<Running, Never> {
  OPENING.with(|held| {
    let waiting = held.take();
    let mode = match waiting.as_ref() {
      Some(_) => Running::Yes,
      None => Running::No,
    };
    held.set(waiting);

    Ok(mode)
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Running {
  Yes,
  No,
}

pub fn finish() -> Result<(), Never> {
  let waiting = OPENING.with(|held| held.take());

  match waiting {
    Some(waiting_2) => {
      let Ok(()) = waiting_2.finish();
    }
    None => {}
  }

  Ok(())
}

fn with(doing: impl FnOnce(&mut Waiting) -> Result<(), Never>) -> Result<(), Never> {
  OPENING.with(|held| {
    match held.take() {
      Some(mut waiting) => {
        let Ok(()) = doing(&mut waiting);
        held.set(Some(waiting));
      }
      None => {}
    }

    Ok(())
  })
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
    let Ok(()) = mark("surface");
    let Ok(()) = counted("rows", 4);
    let Ok(()) = add_note(Note {
      name: "door",
      said: "menu",
    });
    let Ok(()) = taking("screen", Duration::from_millis(20));
    let Ok(()) = finish();
  }
}
