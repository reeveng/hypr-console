use std::sync::mpsc::Receiver;

fn over_and_over(held: u8) -> u8 {
    //~v WARN EXPLICIT055_NO_UNBOUNDED_LOOP
    let one = std::iter::repeat(held);

    one.take(3).sum()
}

fn around_again(held: Vec<u8>) -> u8 {
    //~v WARN EXPLICIT055_NO_UNBOUNDED_LOOP
    let cycle = held.iter().cycle();

    cycle.take(3).sum()
}

fn until_told() -> Option<u8> {
    //~v WARN EXPLICIT055_NO_UNBOUNDED_LOOP
    let made = std::iter::from_fn(|| Some(1u8));

    made.take(3).last()
}

fn walked(from: u8) -> Option<u8> {
    //~v WARN EXPLICIT055_NO_UNBOUNDED_LOOP
    std::iter::successors(Some(from), |n| n.checked_sub(1)).take(3).last()
}

fn served(events: Receiver<u8>) -> u8 {
    let mut heard: u8 = 0;

    //~v WARN EXPLICIT055_NO_UNBOUNDED_LOOP
    for event in events.iter() {
        heard = heard.saturating_add(event);
    }

    heard
}

fn served_whole(events: Receiver<u8>) -> u8 {
    let mut heard: u8 = 0;

    //~v WARN EXPLICIT055_NO_UNBOUNDED_LOOP
    for event in events {
        heard = heard.saturating_add(event);
    }

    heard
}

fn turned_over(events: Receiver<u8>) -> u8 {
    //~v WARN EXPLICIT055_NO_UNBOUNDED_LOOP
    events.into_iter().take(2).sum()
}

fn asked_again() -> Option<u8> {
    //~v WARN EXPLICIT055_NO_UNBOUNDED_LOOP
    std::iter::repeat_with(|| 1u8).take(3).last()
}

fn waiting(events: &Receiver<u8>) -> u8 {
    let mut heard: u8 = 0;

    for event in events.try_iter() {
        heard = heard.saturating_add(event);
    }

    heard
}

fn bounded(held: Vec<u8>) -> u8 {
    let mut sum: u8 = 0;

    for one in held.iter().take(3) {
        sum = sum.saturating_add(*one);
    }

    sum
}

fn main() {
    let (_, events) = std::sync::mpsc::channel();
    let (_, whole) = std::sync::mpsc::channel();
    let (_, waited) = std::sync::mpsc::channel();
    let (_, turned) = std::sync::mpsc::channel();
    let _ = (
        over_and_over(2),
        around_again(vec![1, 2]),
        until_told(),
        walked(3),
        served(events),
        served_whole(whole),
        turned_over(turned),
        asked_again(),
        waiting(&waited),
        bounded(vec![1, 2, 3, 4]),
    );
}
