use std::sync::atomic::{AtomicUsize, Ordering};

struct Carried(*const u8);

unsafe impl Send for Carried {}
unsafe impl Sync for Carried {}

fn relaxed(held: &AtomicUsize) -> usize {
    held.fetch_add(1, Ordering::Relaxed)
}

fn settled(held: &AtomicUsize) -> usize {
    held.fetch_add(1, Ordering::SeqCst)
}

fn main() {
    let counted = AtomicUsize::new(0);

    let _ = (relaxed(&counted), settled(&counted));
}
