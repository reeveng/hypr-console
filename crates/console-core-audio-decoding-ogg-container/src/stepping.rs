//! A walk over the stream that ends in a value or a fault, and the one place the fault of a
//! walk that never ended is spelled.
//!
//! `console_core_iteration::iterate` says a walk can run out of steps without any of them
//! halting, which for a demuxer is a fault of the stream rather than of the caller.

use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;

pub(crate) type Certain<T> = std::result::Result<T, Never>;

pub(crate) type Stepped<State, Found> = std::result::Result<Step<State, Result<Found>>, Never>;

pub(crate) fn walk<State, Found>(
  state: State,
  step: impl Fn(State) -> Stepped<State, Found>,
) -> Result<Found> {
  match iterate(state, step) {
    Ok(ended) => ended,
    Err(Endless) => decode_error("ogg: a walk over the stream never ended"),
  }
}
