// UI test for EXPLICIT021 — a program waits for a thing, not for a number of
// seconds.

use std::sync::mpsc::Receiver;
use std::time::Duration;

// BAD EXPLICIT021 — the thread stops on the clock and nothing can wake it.
fn waits_for_a_number() {
    //~v EXPLICIT021_NO_SLEEPING
    std::thread::sleep(Duration::from_millis(50));
}

// BAD EXPLICIT021 — a `use` of the same function is the same wait.
fn imported() {
    use std::thread::sleep;

    //~v EXPLICIT021_NO_SLEEPING
    sleep(Duration::from_millis(50));
}

// GOOD — a bounded wait on a real event: it comes back when the message
// arrives, and the duration is only how long it is prepared to be patient.
fn waits_for_a_message(heard: &Receiver<u8>) -> Option<u8> {
    match heard.recv_timeout(Duration::from_millis(50)) {
        Ok(byte) => Some(byte),
        Err(_) => None,
    }
}

// GOOD — the elapsing is the thing being asked for, and the site says so.
fn holds_it_down() {
    #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
            explicit021_no_sleeping,
            reason = "the press is a duration: released sooner and the window it is aimed at never sees it"
        )
    )]
    std::thread::sleep(Duration::from_millis(50));
}

fn main() {}
