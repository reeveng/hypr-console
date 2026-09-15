// UI test for EXPLICIT038 — a fault said in prose.

use std::fmt;

#[derive(Debug)]
enum Torn {
    Short,
    Ended,
}

impl fmt::Display for Torn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Torn::Short => write!(f, "there was less here than the head said"),
            Torn::Ended => write!(f, "nothing else is coming"),
        }
    }
}

// BAD EXPLICIT038 — every way this can fail arrives as one type.
//~v EXPLICIT038_NO_PROSE_FAULT
fn read(at: &str) -> Result<String, String> {
    std::fs::read_to_string(at).map_err(|fault| format!("{at}: {fault}"))
}

// BAD EXPLICIT038 — the same thing with a lifetime on it.
//~v EXPLICIT038_NO_PROSE_FAULT
fn held(of: &[u8]) -> Result<u8, &'static str> {
    match of.first() {
        Some(byte) => Ok(*byte),
        None => Err("nothing was said"),
    }
}

// BAD EXPLICIT038 — a trait is asked where it is written.
trait Asking {
    //~v EXPLICIT038_NO_PROSE_FAULT
    fn asked(&self) -> Result<u8, String>;
}

// GOOD — the ways it fails are named, and `Display` says them once.
fn walked(of: &[u8]) -> Result<u8, Torn> {
    match of.first() {
        Some(byte) => Ok(*byte),
        None => Err(Torn::Short),
    }
}

// GOOD — an impl did not choose the signature.
struct Held;

impl Asking for Held {
    fn asked(&self) -> Result<u8, String> {
        Ok(0)
    }
}

// GOOD — the entry point has no caller left to decide anything.
fn main() -> Result<(), String> {
    let _ = read("/dev/null");
    let _ = held(&[1]);
    let _ = walked(&[1]);
    let _ = Held.asked();
    let _ = Torn::Ended;

    Ok(())
}
