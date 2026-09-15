// UI test for EXPLICIT035 — a thread let go without the word being said.

use std::thread::JoinHandle;

// BAD EXPLICIT035 — the handle is dropped on the line that made it.
fn loose() {
    //~v EXPLICIT035_NO_LOOSE_THREAD
    let _ = std::thread::spawn(move || {
        let _ = 1_u32.saturating_add(1);
    });
}

fn named() -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new().name(String::from("asking")).spawn(move || {})
}

// BAD EXPLICIT035 — the same decision, with the handle inside a `Result`.
fn named_and_loose() {
    //~v EXPLICIT035_NO_LOOSE_THREAD
    let _ = named();
}

// GOOD — held and joined where the thread ends.
fn joined() {
    let thread = std::thread::spawn(move || {});

    let _ = thread.join();
}

// GOOD — handed back, so whoever wanted it decides.
fn handed() -> JoinHandle<()> {
    std::thread::spawn(move || {})
}

// GOOD — handed on to something that says what happens to it.
fn let_go(_thread: JoinHandle<()>) {}

fn said() {
    let_go(std::thread::spawn(move || {}));
}

fn main() {
    loose();
    named_and_loose();
    joined();

    let thread = handed();

    let _ = thread.join();

    said();
}
