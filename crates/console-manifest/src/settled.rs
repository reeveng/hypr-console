//! Whether one thing the manifest names is the way the manifest says.
//!
//! One word for the answer, shared by everything the report has a line for: a
//! package, a program that has to be built, a file that has to be in place. It
//! is the same question in all three, and the report reads it the same way.

/// Whether something matches what the manifest asks of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settled {
    /// It is as the manifest says, and an apply would leave it alone.
    Yes,
    /// It has drifted, and an apply is what puts it back.
    No,
}
