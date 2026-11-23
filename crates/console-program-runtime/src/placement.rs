//! Seeded placement between two transition executors.
//!
//! Construct one [`Seeded`] per machine run and pass it to `execute_with`.
//! Initialization and each handled request consume one choice. The seed and
//! the position in that sequence determine placement, independently of the
//! platform clock and of work done by other machines. The selected executor
//! owns the call and its error. Each executor must implement the same machine
//! semantics for replay to produce the same state and effects.
//!
//! The sequence uses [SplitMix64](https://prng.di.unimi.it/splitmix.c) and takes
//! one bit of its output for each choice.

use std::fmt;

use console_core_never::Never;
use console_core_state_machine::{Machine, Transition};

use crate::TransitionExecutor;

const SEQUENCE_INCREMENT: u64 = 0x9e37_79b9_7f4a_7c15;

pub struct Seeded<A, B> {
  sequence: u64,
  first: A,
  second: B,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementError<A, B> {
  First(A),
  Second(B),
}

enum Placement {
  First,
  Second,
}

impl<A, B> Seeded<A, B> {
  pub fn new(seed: u64, first: A, second: B) -> Result<Self, Never> {
    Ok(Seeded {
      sequence: seed,
      first,
      second,
    })
  }

  fn advance(&mut self) -> Result<u64, Never> {
    self.sequence = self.sequence.wrapping_add(SEQUENCE_INCREMENT);

    Ok(self.sequence)
  }

  fn place(&mut self) -> Result<Placement, Never> {
    let Ok(word) = self.advance();
    let Ok(bits) = mix(word);

    Ok(match bits & 1 {
      0 => Placement::First,
      _odd => Placement::Second,
    })
  }
}

const fn mix(mut bits: u64) -> Result<u64, Never> {
  bits = (bits ^ bits.wrapping_shr(30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
  bits = (bits ^ bits.wrapping_shr(27)).wrapping_mul(0x94d0_49bb_1331_11eb);

  Ok(bits ^ bits.wrapping_shr(31))
}

impl<M, A, B> TransitionExecutor<M> for Seeded<A, B>
where
  M: Machine,
  A: TransitionExecutor<M>,
  B: TransitionExecutor<M>,
{
  type Error = PlacementError<A::Error, B::Error>;

  fn initialize(
    &mut self,
    input: &M::Input,
  ) -> Result<Transition<M::State, M::Effect>, Self::Error> {
    let Ok(placement) = self.place();

    match placement {
      Placement::First => self.first.initialize(input).map_err(PlacementError::First),
      Placement::Second => self
        .second
        .initialize(input)
        .map_err(PlacementError::Second),
    }
  }

  fn transition(
    &mut self,
    state: M::State,
    request: M::Request,
  ) -> Result<Transition<M::State, M::Effect>, Self::Error> {
    let Ok(placement) = self.place();

    match placement {
      Placement::First => self
        .first
        .transition(state, request)
        .map_err(PlacementError::First),
      Placement::Second => self
        .second
        .transition(state, request)
        .map_err(PlacementError::Second),
    }
  }
}

impl<A: fmt::Display, B: fmt::Display> fmt::Display for PlacementError<A, B> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PlacementError::First(error) => write!(to, "first executor failed ({error})"),
      PlacementError::Second(error) => write!(to, "second executor failed ({error})"),
    }
  }
}

impl<A: std::error::Error + 'static, B: std::error::Error + 'static> std::error::Error
  for PlacementError<A, B>
{
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      PlacementError::First(error) => Some(error),
      PlacementError::Second(error) => Some(error),
    }
  }
}

#[cfg(kani)]
#[path = "../proofs/placement.rs"]
mod proofs;
