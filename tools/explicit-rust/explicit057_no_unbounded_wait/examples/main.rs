use std::io::Read;
use std::sync::mpsc::Receiver;
use std::sync::{Condvar, Mutex};

fn told(events: Receiver<u8>) -> Option<u8> {
    //~v WARN EXPLICIT057_NO_UNBOUNDED_WAIT
    events.recv().ok()
}

fn held(locked: Mutex<u8>) -> Option<u8> {
    //~v WARN EXPLICIT057_NO_UNBOUNDED_WAIT
    locked.lock().ok().map(|held| *held)
}

#[allow(dead_code)]
fn waited(under: Condvar, over: Mutex<u8>) {
    //~v WARN EXPLICIT057_NO_UNBOUNDED_WAIT
    let _unused = under.wait(over.lock().unwrap());
}

fn drained(from: &[u8]) -> Vec<u8> {
    let mut source = from;
    let mut kept = Vec::new();

    //~v WARN EXPLICIT057_NO_UNBOUNDED_WAIT
    let _ = source.read_to_end(&mut kept);

    kept
}

struct Session {
    held: u8,
}

impl Session {
    fn read(&self) -> u8 {
        self.held
    }
}

fn bounded(events: Receiver<u8>) -> Option<u8> {
    events.recv_timeout(std::time::Duration::from_secs(1)).ok()
}

fn taken(locked: Mutex<u8>) -> Option<u8> {
    locked.try_lock().ok().map(|held| *held)
}

fn main() {
    let (_, events) = std::sync::mpsc::channel();
    let _ = told(events);

    let (_, events) = std::sync::mpsc::channel();
    let _ = (held(Mutex::new(1)), bounded(events), taken(Mutex::new(1)));
    let _ = (drained(&[1, 2]), Session { held: 1 }.read());
}
