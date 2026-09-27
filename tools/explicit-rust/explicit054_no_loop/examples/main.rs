use std::sync::mpsc::Receiver;

fn counted_down(mut n: u8) -> u8 {
    //~v EXPLICIT054_NO_LOOP
    loop {
        match n > 0 {
            true => n = n.saturating_sub(1),
            false => break n,
        }
    }
}

fn walked(held: Vec<u8>) -> u8 {
    let mut sum: u8 = 0;

    for one in held {
        sum = sum.saturating_add(one);
    }

    sum
}

fn stepped(from: u8) -> Option<u8> {
    std::iter::successors(Some(from), |n| n.checked_sub(1)).last()
}

fn served(events: Receiver<u8>) -> u8 {
    let mut heard: u8 = 0;

    for event in events.iter() {
        heard = heard.saturating_add(event);
    }

    heard
}

fn main() {
    let (_, events) = std::sync::mpsc::channel();
    let _ = (counted_down(3), walked(vec![1, 2]), stepped(3), served(events));
}
