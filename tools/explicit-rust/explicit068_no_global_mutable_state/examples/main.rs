use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

static mut COUNTER: u8 = 0;

static THE_ONE: OnceLock<u8> = OnceLock::new();

static GUARDED: Mutex<u8> = Mutex::new(0);

static COUNTED: AtomicU32 = AtomicU32::new(0);

static NAMED: &str = "console";

thread_local! {
    static MINE: u8 = 1;
}

const FROZEN: u8 = 1;

fn main() {
    unsafe {
        COUNTER = 1;
    }

    let _ = (THE_ONE.get(), FROZEN, MINE.with(|held| *held));
    let _ = (GUARDED.try_lock().is_ok(), COUNTED.load(Ordering::SeqCst), NAMED);
}
