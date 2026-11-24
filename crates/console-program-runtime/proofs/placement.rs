//! Run with `cargo kani -p console-program-runtime --lib --harness placement`.
//!
//! Replay follows by induction on the sequence counter. Equal counters feed
//! the same word to the pure placement mixer at each step.

use super::*;
use console_core_state_machine::{EffectDependencies, Queue, Service};

struct Accumulator;

#[derive(Debug, PartialEq, Eq)]
struct Sample(u8);

impl EffectDependencies for Sample {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[])
  }
}

impl Machine for Accumulator {
  type Input = u8;
  type State = u8;
  type Request = u8;
  type Effect = Sample;

  fn initialize(
    input: &u8,
    _previous: Option<u8>,
    effects: &mut Queue<Sample>,
  ) -> Result<u8, Never> {
    let Ok(()) = effects.offer(Sample(*input));

    Ok(*input)
  }

  fn handle(state: u8, request: u8, effects: &mut Queue<Sample>) -> Result<u8, Never> {
    let Ok(()) = effects.offer_all([Sample(state), Sample(request)]);

    Ok(state.wrapping_add(request))
  }
}

struct Recorder {
  calls: u8,
  failure: Option<u8>,
}

impl Recorder {
  fn new(failure: Option<u8>) -> Self {
    Self { calls: 0, failure }
  }

  fn record(&mut self) -> Result<(), u8> {
    self.calls += 1;
    match self.failure {
      Some(code) => Err(code),
      None => Ok(()),
    }
  }
}

impl TransitionExecutor<Accumulator> for Recorder {
  type Error = u8;

  fn initialize(&mut self, input: &u8) -> Result<Transition<u8, Sample>, u8> {
    self.record()?;
    let Ok(decision) = Accumulator::initial_transition(input, None);

    Ok(decision)
  }

  fn transition(&mut self, state: u8, request: u8) -> Result<Transition<u8, Sample>, u8> {
    self.record()?;
    let Ok(decision) = Accumulator::transition(state, request);

    Ok(decision)
  }
}

#[kani::proof]
fn seeded_placement_replays_independently_of_other_machines() {
  let seed: u64 = kani::any();
  let Ok(mut placed) = Seeded::new(seed, (), ());
  let Ok(mut replay) = Seeded::new(seed, (), ());
  let Ok(mut unrelated) = Seeded::new(kani::any(), (), ());

  let Ok(word) = placed.advance();
  let Ok(_unrelated_placement) = unrelated.place();
  let Ok(repeated) = replay.advance();

  assert_eq!(word, repeated);
  assert_eq!(placed.sequence, seed.wrapping_add(SEQUENCE_INCREMENT));
  assert_eq!(placed.sequence, replay.sequence);
}

#[kani::proof]
#[kani::unwind(4)]
fn seeded_placement_preserves_state_and_ordered_effects_for_each_step() {
  let seed: u64 = kani::any();
  let previous: Option<u8> = kani::any();
  let request: u8 = kani::any();
  let Ok(mut placed) = Seeded::new(seed, Recorder::new(None), Recorder::new(None));

  match previous {
    None => {
      let decision = placed.initialize(&request).unwrap();
      assert_eq!(decision.state, request);
      assert_eq!(decision.effects, [Sample(request)]);
    }
    Some(state) => {
      let decision = placed.transition(state, request).unwrap();
      assert_eq!(decision.state, state.wrapping_add(request));
      assert_eq!(decision.effects, [Sample(state), Sample(request)]);
    }
  }

  assert_eq!(placed.first.calls + placed.second.calls, 1);
  assert_eq!(placed.sequence, seed.wrapping_add(SEQUENCE_INCREMENT));
}

#[kani::proof]
fn a_placement_failure_is_returned_by_the_only_executor_called() {
  let first: u8 = kani::any();
  let second: u8 = kani::any();
  let Ok(mut placed) = Seeded::new(
    kani::any(),
    Recorder::new(Some(first)),
    Recorder::new(Some(second)),
  );

  let failure = if kani::any() {
    placed.initialize(&kani::any())
  } else {
    placed.transition(kani::any(), kani::any())
  };

  match (placed.first.calls, placed.second.calls) {
    (1, 0) => assert_eq!(failure, Err(PlacementError::First(first))),
    (0, 1) => assert_eq!(failure, Err(PlacementError::Second(second))),
    _ => panic!("the request must reach exactly one executor"),
  }

  kani::cover!(placed.first.calls == 1);
  kani::cover!(placed.second.calls == 1);
}
