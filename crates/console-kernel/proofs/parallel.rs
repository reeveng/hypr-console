//! `cargo kani -p console-kernel --lib --harness parallel` checks these models.
//! The model steps are the serialization points for scheduler and GPU events.

#[path = "gpu_budget.rs"]
mod budget;
#[path = "gpu_frames.rs"]
mod frames;
#[path = "parallel_tasks.rs"]
mod tasks;
