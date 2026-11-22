//! A walk over a stream that ends when a step says so.
//!
//! Hunting for a frame, for the end of a metadata run or for the packet a timestamp falls in
//! is a read, a decision and another read, and none of it knows how many reads it will take.
//! `console_core_iteration::iterate` is where that is written; what it adds here is the
//! answer it can give that these walks cannot, which is that the repetition ended without a
//! step saying so. It is a fault of the stream rather than of the caller, so it is said as one.

use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_byte_streams::ReadBytes;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::fitted;

pub type Infallible<Value> = std::result::Result<Value, Never>;

pub fn read_count<B: ReadBytes>(reader: &mut B, buf: &mut [u8]) -> std::io::Result<u32> {
  match reader.read_buf(buf).map(fitted::<_, u32>) {
    Ok(Ok(count)) => Ok(count),
    Ok(Err(never)) => match never {},
    Err(fault) => Err(fault),
  }
}

pub fn run<State, Found>(
  state: State,
  step: impl Fn(State) -> std::result::Result<Step<State, Result<Found>>, Never>,
) -> Result<Found> {
  match iterate(state, step) {
    Ok(finished) => finished,
    Err(Endless) => decode_error("flac: the stream gave no end to a search that needed one"),
  }
}

pub fn halted<State, Found>(
  fault: Error,
) -> std::result::Result<Step<State, Result<Found>>, Never> {
  Ok(Step::Halt(Err(fault)))
}
