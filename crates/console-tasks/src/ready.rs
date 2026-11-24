//! What must hold before a deploy, in the order it is asked.
//!
//! A deployment checks vocabulary, isolated tests, clippy and Explicit Rust.
//! A test run uses the same effect and reply path without recording a deploy pass.
//!
//! A pass is written down only for a tree that was committed and still: a pass
//! over files that were being edited is a pass over something that no longer
//! exists. And a tree that already passed is not asked again.
//!
//! The CLI supplies observations and carries out effects. A check must answer
//! before the machine asks for the next one, and its reply names the check.

use std::collections::VecDeque;
use std::path::PathBuf;

use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service, Transition};
use console_program_contract::{Effect, Event, Exit};

use crate::reached::Scope;
use crate::task::Suite;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
  pub tree_id: String,
  pub tree: Tree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Before {
  pub checkout: Checkout,
  pub pass_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyEffect {
  Inspect,
  FindScope(Option<String>),
  Check(Step),
  RecheckTree,
  RecordPass { path: PathBuf, tree_id: String },
}

impl EffectDependencies for ReadyEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Inspect => &[Service::Process, Service::FileSystem],
      Self::FindScope(_) | Self::RecheckTree => &[Service::Process],
      Self::Check(_) => &[Service::Process],
      Self::RecordPass { .. } => &[Service::FileSystem],
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyEvent {
  Inspected {
    before: Before,
    last_pass: Option<String>,
  },
  ScopeFound(Scope),
  Checked {
    step: Step,
    result: Result<(), String>,
  },
  TreeRechecked(Checkout),
  PassRecorded,
  Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
  Inspecting,
  FindingScope(Before),
  Checking {
    completion: Completion,
    pending: Step,
    remaining: VecDeque<Step>,
  },
  Rechecking(Before),
  Recording,
  Done,
}

pub struct Ready;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completion {
  Finish,
  Recheck(Before),
}

pub enum Run {
  Deploy,
  Tests(Vec<Suite>),
  PanelTest(String),
}

impl Machine for Ready {
  type Input = Run;
  type State = State;
  type Request = Event<ReadyEvent>;
  type Effect = Effect<ReadyEffect>;

  fn initialize(
    input: &Run,
    _previous: Option<State>,
    effects: &mut Queue<Self::Effect>,
  ) -> Result<State, Never> {
    let Ok(turn) = match input {
      Run::Deploy => Transition::new(
        State::Inspecting,
        vec![Effect::Custom(ReadyEffect::Inspect)],
      ),
      Run::Tests(suites) => next_check(
        Completion::Finish,
        suites.iter().copied().map(Step::Test).collect(),
      ),
      Run::PanelTest(name) => next_check(
        Completion::Finish,
        VecDeque::from([Step::PanelTest(name.clone())]),
      ),
    };

    turn.offered(effects)
  }

  fn handle(
    state: State,
    event: Self::Request,
    effects: &mut Queue<Self::Effect>,
  ) -> Result<State, Never> {
    let Ok(turn) = decide(state, event);

    turn.offered(effects)
  }
}

type Turn = Result<Transition<State, Effect<ReadyEffect>>, Never>;

fn decide(state: State, event: Event<ReadyEvent>) -> Turn {
  match (state, event) {
    (State::Done, _) => Transition::without_effects(State::Done),
    (_, Event::Stopping) => stop(Exit::Failure(
      "ready stopped before it finished".to_string(),
    )),
    (_, Event::Custom(ReadyEvent::Failed(fault))) => stop(Exit::Failure(fault)),
    (State::Inspecting, Event::Custom(ReadyEvent::Inspected { before, last_pass })) => {
      inspected(before, last_pass.as_deref())
    }
    (State::FindingScope(before), Event::Custom(ReadyEvent::ScopeFound(scope))) => {
      let Ok(flags) = scope.flags();
      let Ok(checks) = steps(&scope);
      let notice = Effect::Print(format!(
        "clippy and the rules are asked of: {}",
        flags.join(" ")
      ));
      let Ok(mut turn) = next_check(Completion::Recheck(before), VecDeque::from(checks));

      turn.effects.insert(0, notice);

      Ok(turn)
    }
    (
      State::Checking {
        completion,
        pending,
        remaining,
      },
      Event::Custom(ReadyEvent::Checked { step, result }),
    ) => {
      match (
        ::core::primitive::bool::then_some(step == pending, ()),
        result,
      ) {
        (Some(()), Ok(())) => next_check(completion, remaining),
        (Some(()), Err(fault)) => stop(Exit::Failure(fault)),
        (None, _) => Transition::without_effects(State::Checking {
          completion,
          pending,
          remaining,
        }),
      }
    }
    (State::Rechecking(before), Event::Custom(ReadyEvent::TreeRechecked(after))) => {
      reviewed(before, after)
    }
    (State::Recording, Event::Custom(ReadyEvent::PassRecorded)) => stop(Exit::Success),
    (unchanged, _) => Transition::without_effects(unchanged),
  }
}

fn inspected(before: Before, last_pass: Option<&str>) -> Turn {
  let Checkout { tree_id, tree } = &before.checkout;
  let Ok(earlier) = earlier(*tree, tree_id, last_pass);

  match earlier {
    Earlier::Passed => Transition::new(
      State::Done,
      vec![
        Effect::Print(format!(
          "ready already passed on this tree ({tree_id}), nothing to ask again"
        )),
        Effect::Stop(Exit::Success),
      ],
    ),
    Earlier::Ask { since } => Transition::new(
      State::FindingScope(before),
      vec![Effect::Custom(ReadyEffect::FindScope(since))],
    ),
  }
}

fn next_check(completion: Completion, mut remaining: VecDeque<Step>) -> Turn {
  match remaining.pop_front() {
    Some(step) => Transition::new(
      State::Checking {
        completion,
        pending: step.clone(),
        remaining,
      },
      vec![Effect::Custom(ReadyEffect::Check(step))],
    ),
    None => match completion {
      Completion::Finish => stop(Exit::Success),
      Completion::Recheck(before) => Transition::new(
        State::Rechecking(before),
        vec![Effect::Custom(ReadyEffect::RecheckTree)],
      ),
    },
  }
}

fn reviewed(before: Before, after: Checkout) -> Turn {
  let same =
    match ::core::primitive::bool::then_some(before.checkout.tree_id == after.tree_id, ()) {
      Some(()) => Same::Yes,
      None => Same::No,
    };
  let Ok(record) = record(before.checkout.tree, after.tree, same);

  match record {
    Record::Write => Transition::new(
      State::Recording,
      vec![Effect::Custom(ReadyEffect::RecordPass {
        path: before.pass_file,
        tree_id: after.tree_id,
      })],
    ),
    Record::Leave => Transition::new(
      State::Done,
      vec![
        Effect::Print(
          "ready passed, but the tree was not committed and still, so the pass is not written down"
            .to_string(),
        ),
        Effect::Stop(Exit::Success),
      ],
    ),
  }
}

fn stop(exit: Exit) -> Turn {
  Transition::new(State::Done, vec![Effect::Stop(exit)])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
  RetiredWords,
  Test(Suite),
  PanelTest(String),
  Clippy(Scope),
  Rules(Scope),
}

pub fn steps(scope: &Scope) -> Result<Vec<Step>, Never> {
  Ok(vec![
    Step::RetiredWords,
    Step::Test(Suite::Unit),
    Step::Test(Suite::Integration),
    Step::Test(Suite::Panels),
    Step::Test(Suite::Live),
    Step::Clippy(scope.clone()),
    Step::Rules(scope.clone()),
  ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tree {
  Committed,
  Loose,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Earlier {
  Passed,
  Ask { since: Option<String> },
}

pub fn earlier(tree: Tree, now: &str, recorded: Option<&str>) -> Result<Earlier, Never> {
  let since = recorded
    .map(|tree_2| tree_2.trim().to_string())
    .filter(|tree_3| !tree_3.is_empty());

  Ok(match (tree, since) {
    (Tree::Committed, Some(since_2)) => {
      match ::core::primitive::bool::then_some(since_2 == now, ()) {
        ::core::option::Option::Some(()) => Earlier::Passed,
        ::core::option::Option::None => Earlier::Ask {
          since: Some(since_2),
        },
      }
    }
    (Tree::Committed, None) | (Tree::Loose, None) => Earlier::Ask { since: None },
    (Tree::Loose, Some(since_3)) => Earlier::Ask {
      since: Some(since_3),
    },
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
  Write,
  Leave,
}

pub fn record(before: Tree, after: Tree, same_tree: Same) -> Result<Record, Never> {
  Ok(match (before, after, same_tree) {
    (Tree::Committed, Tree::Committed, Same::Yes) => Record::Write,
    (Tree::Committed, Tree::Committed, Same::No)
    | (Tree::Committed | Tree::Loose, Tree::Loose, Same::Yes | Same::No)
    | (Tree::Loose, Tree::Committed, Same::Yes | Same::No) => Record::Leave,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Same {
  Yes,
  No,
}

#[cfg(test)]
mod tests {
  use console_core_state_machine::run;

  use super::*;

  fn inspection(last_pass: Option<&str>) -> Result<Event<ReadyEvent>, Never> {
    Ok(Event::Custom(ReadyEvent::Inspected {
      before: Before {
        checkout: Checkout {
          tree_id: "tree-before".to_string(),
          tree: Tree::Committed,
        },
        pass_file: PathBuf::from(".git/console-ready"),
      },
      last_pass: last_pass.map(str::to_owned),
    }))
  }

  #[test]
  fn a_cached_pass_stops_before_asking_for_any_checks() {
    let Ok(inspected) = inspection(Some("tree-before\n"));
    let Ok(trace) = run::<Ready>(&Run::Deploy, &[inspected]);
    let Ok(effects) = trace.effects();

    assert_eq!(trace.initialized, [Effect::Custom(ReadyEffect::Inspect)]);
    assert!(matches!(
      effects.as_slice(),
      [Effect::Print(_), Effect::Stop(Exit::Success)]
    ));
    assert_eq!(trace.state, State::Done);
  }

  #[test]
  fn a_pass_is_recorded_after_the_last_check_and_a_fresh_tree_observation() {
    let scope = Scope::Workspace;
    let Ok(checks) = steps(&scope);
    let Ok(inspected) = inspection(None);
    let mut replies = vec![inspected, Event::Custom(ReadyEvent::ScopeFound(scope))];
    replies.extend(checks.iter().cloned().map(|step| {
      Event::Custom(ReadyEvent::Checked {
        step,
        result: Ok(()),
      })
    }));
    let Ok(checked) = run::<Ready>(&Run::Deploy, &replies);
    let Ok(asked) = checked.effects();

    assert_eq!(
      asked.first(),
      Some(&Effect::Custom(ReadyEffect::FindScope(None)))
    );
    assert_eq!(
      asked.last(),
      Some(&Effect::Custom(ReadyEffect::RecheckTree))
    );
    assert_eq!(
      asked
        .iter()
        .filter_map(|effect| match effect {
          Effect::Custom(ReadyEffect::Check(step)) => Some(step),
          Effect::Custom(
            ReadyEffect::Inspect
            | ReadyEffect::FindScope(_)
            | ReadyEffect::RecheckTree
            | ReadyEffect::RecordPass { .. },
          )
          | Effect::Run(_)
          | Effect::Stream(_)
          | Effect::Fork(_)
          | Effect::Interrupt(_)
          | Effect::Prompt(_)
          | Effect::Spawn(_)
          | Effect::Subscribe(_)
          | Effect::Unsubscribe(_)
          | Effect::Write(_)
          | Effect::Notify(_)
          | Effect::Print(_)
          | Effect::Stop(_) => None,
        })
        .collect::<Vec<_>>(),
      checks.iter().collect::<Vec<_>>(),
    );
    assert!(matches!(checked.state, State::Rechecking(_)));

    let Ok(drifted) = Ready::transition(
      checked.state.clone(),
      Event::Custom(ReadyEvent::TreeRechecked(Checkout {
        tree_id: "tree-after".to_string(),
        tree: Tree::Committed,
      })),
    );
    assert!(matches!(
      drifted.effects.as_slice(),
      [Effect::Print(_), Effect::Stop(Exit::Success)]
    ));

    let Ok(writing) = Ready::transition(
      checked.state,
      Event::Custom(ReadyEvent::TreeRechecked(Checkout {
        tree_id: "tree-before".to_string(),
        tree: Tree::Committed,
      })),
    );
    assert_eq!(
      writing.effects,
      [Effect::Custom(ReadyEffect::RecordPass {
        path: PathBuf::from(".git/console-ready"),
        tree_id: "tree-before".to_string(),
      })]
    );
    let Ok(finished) = Ready::transition(writing.state, Event::Custom(ReadyEvent::PassRecorded));
    assert_eq!(finished.effects, [Effect::Stop(Exit::Success)]);
  }

  #[test]
  fn only_a_reply_to_the_pending_check_can_advance_or_stop_the_gate() {
    let Ok(inspected) = inspection(None);
    let Ok(trace) = run::<Ready>(
      &Run::Deploy,
      &[
        inspected,
        Event::Custom(ReadyEvent::ScopeFound(Scope::Workspace)),
        Event::Custom(ReadyEvent::Checked {
          step: Step::Test(Suite::Unit),
          result: Ok(()),
        }),
        Event::Custom(ReadyEvent::Checked {
          step: Step::RetiredWords,
          result: Err("words failed".to_string()),
        }),
      ],
    );
    let Ok(unrelated) = trace.on(2);
    let Ok(failed) = trace.on(3);

    assert_eq!(unrelated, Some([].as_slice()));
    assert_eq!(
      failed,
      Some([Effect::Stop(Exit::Failure("words failed".to_string()))].as_slice())
    );
    assert_eq!(trace.state, State::Done);
  }

  #[test]
  fn a_committed_tree_that_passed_is_not_asked_again() {
    let Ok(answer) = earlier(Tree::Committed, "abc", Some("abc\n"));

    assert_eq!(answer, Earlier::Passed);
  }

  #[test]
  fn a_loose_tree_is_asked_even_when_its_commit_passed() {
    let Ok(answer) = earlier(Tree::Loose, "abc", Some("abc"));

    assert_eq!(
      answer,
      Earlier::Ask {
        since: Some("abc".to_string())
      }
    );
  }

  #[test]
  fn with_no_pass_written_down_there_is_nothing_to_measure_from() {
    let Ok(answer) = earlier(Tree::Committed, "abc", Some("  \n"));

    assert_eq!(answer, Earlier::Ask { since: None });
  }

  #[test]
  fn a_pass_is_written_only_for_a_tree_that_stayed_committed_and_still() {
    let Ok(written) = record(Tree::Committed, Tree::Committed, Same::Yes);
    let Ok(moved) = record(Tree::Committed, Tree::Committed, Same::No);
    let Ok(edited) = record(Tree::Committed, Tree::Loose, Same::Yes);
    let Ok(started_loose) = record(Tree::Loose, Tree::Committed, Same::Yes);

    assert_eq!(
      (written, moved, edited, started_loose),
      (Record::Write, Record::Leave, Record::Leave, Record::Leave)
    );
  }

  #[test]
  fn deployment_checks_vocabulary_before_the_guest() {
    let Ok(asked) = steps(&Scope::Workspace);

    assert_eq!(asked.first(), Some(&Step::RetiredWords));
    assert_eq!(asked.get(1), Some(&Step::Test(Suite::Unit)));
    assert!(asked.contains(&Step::Test(Suite::Panels)));
    assert_eq!(asked.last(), Some(&Step::Rules(Scope::Workspace)));
  }

  #[test]
  fn testing_requests_each_suite_without_inspecting_or_recording_the_tree() {
    let Ok(trace) = run::<Ready>(
      &Run::Tests(vec![Suite::Unit, Suite::Integration]),
      &[
        Event::Custom(ReadyEvent::Checked {
          step: Step::Test(Suite::Unit),
          result: Ok(()),
        }),
        Event::Custom(ReadyEvent::Checked {
          step: Step::Test(Suite::Integration),
          result: Ok(()),
        }),
      ],
    );
    let Ok(asked) = trace.effects();

    assert_eq!(
      trace.initialized,
      [Effect::Custom(ReadyEffect::Check(Step::Test(Suite::Unit)))]
    );
    assert_eq!(
      asked,
      [
        Effect::Custom(ReadyEffect::Check(Step::Test(Suite::Integration))),
        Effect::Stop(Exit::Success)
      ]
    );
  }
}
