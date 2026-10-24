use std::sync::Mutex;
use std::sync::mpsc::Receiver;

fn watchdog() {}

fn served(events: Receiver<u8>) -> u8 {
    let mut heard: u8 = 0;

    for event in events.iter() {
        heard = heard.saturating_add(event);
    }

    heard
}

fn kicked(events: Receiver<u8>) -> u8 {
    let mut heard: u8 = 0;

    for event in events.iter() {
        watchdog();

        heard = heard.saturating_add(event);
    }

    heard
}

fn served_whole(events: Receiver<u8>) -> u8 {
    let mut heard: u8 = 0;

    for event in events {
        heard = heard.saturating_add(event);
    }

    heard
}

fn beside(locked: &Mutex<u8>, held: [u8; 2], sum: &mut u8) {
    *sum = locked.lock().map_or(0, |inside| *inside);

    for one in held {
        *sum = sum.saturating_add(one);
    }
}

fn walked(held: [u8; 2]) -> u8 {
    let mut sum: u8 = 0;

    for one in held {
        sum = sum.saturating_add(one);
    }

    sum
}

fn main() {
    let (_, events) = std::sync::mpsc::channel();

    let _ = (served(events), walked([1, 2]));

    let (_, events) = std::sync::mpsc::channel();

    let _ = kicked(events);

    let (_, events) = std::sync::mpsc::channel();

    let mut sum: u8 = 0;

    let _ = (served_whole(events), beside(&Mutex::new(1), [1, 2], &mut sum));
}
