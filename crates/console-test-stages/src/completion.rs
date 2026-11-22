//! Completion checks ask first and wait only when the answer is not ready.
//!
//! The device's old wait slept before its first question and counted only
//! gaps, so every already-completed action cost a pause and slow queries did
//! not spend its deadline. This is the common wait over an injected clock:
//! dry runs and tests advance virtual time, live checks spend actual elapsed
//! time, and an interruption ends the wait without asking the device again.

use std::time::Duration;

use console_core_never::Never;
use console_waiting::clock::Clock;
use console_waiting::{Outcome, Ready, Schedule};

use crate::stopping::Stop;

pub(crate) fn wait<C: Clock, M, E>(
  clock: &mut C,
  handed: &mut M,
  patience: Duration,
  mut question: impl FnMut(&mut M) -> Result<Ready, E>,
  interrupt: impl Fn() -> Result<Stop, Never>,
) -> Result<Outcome, E> {
  let Ok(schedule) = Schedule::of(patience);
  let mut handed_2 = (handed, &mut question);
  let found = console_waiting::until_some_handed_on_result(
    clock,
    schedule,
    &mut handed_2,
    |(handed_3, question_2)| {
      let Ok(stop) = interrupt();

      match stop {
        Stop::Requested => return Ok(Some(Outcome::RanOut)),
        Stop::No => {}
      }

      let ready = question_2(handed_3)?;

      Ok(match ready {
        Ready::Yes => Some(Outcome::Happened),
        Ready::NotYet => None,
      })
    },
  )?;

  Ok(match found {
    Some(outcome) => outcome,
    None => Outcome::RanOut,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_waiting::clock::TestClock;

  #[test]
  fn completion_already_present_has_no_pause_even_with_no_patience() {
    let mut clock = TestClock::default();
    let result = wait(
      &mut clock,
      &mut (),
      Duration::ZERO,
      |()| Ok::<_, Never>(Ready::Yes),
      || Ok(Stop::No),
    );

    assert_eq!(result, Ok(Outcome::Happened));
    assert!(clock.paused.is_empty());
  }

  #[test]
  fn query_failure_is_returned_without_waiting_or_turning_into_a_timeout() {
    let mut clock = TestClock::default();
    let result = wait(
      &mut clock,
      &mut (),
      Duration::from_secs(10),
      |()| Err::<Ready, _>(17_u32),
      || Ok(Stop::No),
    );

    assert_eq!(result, Err(17));
    assert!(clock.paused.is_empty());
  }

  #[test]
  fn pending_completion_is_asked_again_and_stops_as_soon_as_it_is_ready() {
    let mut clock = TestClock::default();
    let mut queries = 0_u32;
    let result = wait(
      &mut clock,
      &mut queries,
      Duration::from_secs(10),
      |queries_2| {
        *queries_2 = queries_2.saturating_add(1);

        Ok::<_, Never>(match *queries_2 {
          1 => Ready::NotYet,
          0 | 2.. => Ready::Yes,
        })
      },
      || Ok(Stop::No),
    );

    assert_eq!(result, Ok(Outcome::Happened));
    assert_eq!(queries, 2);
    assert_eq!(clock.paused, vec![Duration::from_millis(50)]);
  }

  #[test]
  fn missing_completion_runs_out_at_the_deadline() {
    let mut clock = TestClock::default();
    let result = wait(
      &mut clock,
      &mut (),
      Duration::from_millis(75),
      |()| Ok::<_, Never>(Ready::NotYet),
      || Ok(Stop::No),
    );

    assert_eq!(result, Ok(Outcome::RanOut));
    assert_eq!(clock.elapsed, Duration::from_millis(75));
  }

  #[test]
  fn interruption_does_not_query_or_wait() {
    let mut clock = TestClock::default();
    let mut queries = 0_u32;
    let result = wait(
      &mut clock,
      &mut queries,
      Duration::from_secs(10),
      |queries_2| {
        *queries_2 = queries_2.saturating_add(1);

        Ok::<_, Never>(Ready::NotYet)
      },
      || Ok(Stop::Requested),
    );

    assert_eq!(result, Ok(Outcome::RanOut));
    assert_eq!(queries, 0);
    assert!(clock.paused.is_empty());
  }
}
