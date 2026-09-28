//! The tasks this tree runs on itself: `cargo x ready`, `cargo x test`.
//!
//! What they decide is here with no machine in it -- which crates a change
//! reaches, what `ready` asks and in what order, when a pass is written down
//! -- so it is pressed by `cargo test` like anything else. The program is the
//! edges: git, `cargo metadata`, and each step run in a control group of its
//! own. These were bash in the justfile, where none of the rules the rest of
//! the tree keeps could reach them.

pub mod ready;
pub mod reached;
pub mod task;
