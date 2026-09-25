//! The same question as the rest of this crate, asked about a thread.
//!
//! A `std::process::Child` and a `std::thread::JoinHandle` are the same three
//! lines at the call site and the same silence afterwards: a thing was started,
//! and nothing says how long it is meant to run. [`BoundToParent`](crate::BoundToParent)
//! and [`Detached`](crate::Detached) make a caller say which it meant about a child,
//! and threads were left out of that for no better reason than that a handle is
//! cheap to drop. Eleven of them were dropped on the line that made them.
//!
//! Only half the answer carries over. A child can be ended from outside, so
//! `BoundToParent` does something as well as saying something; a thread cannot, so
//! the only two endings a thread has are the one it reaches itself and the one
//! the process reaches. Where the caller is waiting on the thread it holds the
//! handle and joins it, which is a word already. Where it is not -- a listener
//! that will run until the program does -- there is nothing to hold and nothing
//! to wait for, and [`let_go`] is that said out loud rather than spelled
//! `let _ =`, which is the spelling for throwing away anything at all.
//!
//! It takes a thread that answers nothing. A `JoinHandle<T>` let go is an answer
//! no one will ever read, and a thread computing something for a caller that is
//! not going to collect it is a different fault with a better fix.

use console_core_never::Never;
use std::thread::JoinHandle;

pub fn let_go(thread: JoinHandle<()>) -> Result<(), Never> {
    drop(thread);

    Ok(())
}
