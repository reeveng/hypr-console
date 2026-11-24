//! Platform-independent program and command execution.
//!
//! A platform supplies time, arrivals, effects and command operations. The
//! execution loop owns initialization, queued replies, timer subscriptions and
//! stopping rules. It has no default host implementation or platform feature.
//! Desktop integrations live in their own crates. Use [`provide`] to bind a
//! platform before running the program. Simulation runs the same loop.
//!
//! Implement [`Platform`] for an event loop, [`Clock`] for the runtime's
//! elapsed time, and [`commands::Platform`] for command operations. The command
//! adapter chooses its own executable representation:
//! a capability handle is as valid as a path. Neither interface selects an OS
//! implementation or a service implicitly.
//!
//! [`execute_with`] accepts a [`TransitionExecutor`] for a program's decisions.
//! The loop sends one request at a time and commits the resulting effects in
//! order. Executors can choose where to run a transition while the platform
//! continues to own effect execution and arrivals.
//! Use [`Seeded`] to choose between two executors with a reproducible sequence.

pub mod commands;
pub mod execution;
mod jobs;
mod placement;
pub mod simulation;

pub use execution::{
  Clock, Completed, ExecutionError, Inline, Limit, Platform, Provided, TransitionExecutor, Wake,
  execute, execute_with, provide,
};
pub use jobs::{Invocation, ReplyStatus};
pub use placement::{PlacementError, Seeded};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subscribed {
  Yes,
  No,
}
