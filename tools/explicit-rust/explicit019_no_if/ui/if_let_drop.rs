// edition:2024

use std::sync::atomic::{AtomicBool, Ordering};

static DROPPED: AtomicBool = AtomicBool::new(false);

struct Observe;

impl Drop for Observe {
    fn drop(&mut self) {
        DROPPED.store(true, Ordering::Relaxed);
    }
}

const FAILED: Result<bool, Observe> = Err(Observe);

fn main() {
    let saw_drop = if let Ok(answer) = FAILED {
        answer
    } else {
        DROPPED.load(Ordering::Relaxed)
    };
    assert!(saw_drop);
}
