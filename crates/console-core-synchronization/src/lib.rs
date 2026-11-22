//! Standard-library locks for callers that manage failures themselves or wait
//! for exclusive access up to a deadline.

#![forbid(unsafe_code)]

pub mod nonpoison;

mod timed_mutex;
pub use timed_mutex::TimedMutex;
