fn narrowed(held: u8) -> u8 {
    let held = held.saturating_add(1);

    held
}

fn named(held: u8) -> u8 {
    let widened = held.saturating_add(1);

    widened
}

fn either(held: Option<u8>) -> u8 {
    match held {
        Some(one) => one,
        None => 0,
    }
}

enum Read {
    Whole(u8),
    Cut(u8),
}

fn halves(read: Read) -> u8 {
    match read {
        Read::Whole(held) | Read::Cut(held) => held,
    }
}

fn walked(rows: [[u8; 2]; 2]) -> u8 {
    let mut sum: u8 = 0;

    for row in rows {
        for one in row {
            sum = sum.saturating_add(one);
        }
    }

    sum
}

fn asked(held: Option<u8>, more: Option<u8>) -> Option<u8> {
    let first = held?;
    let second = more?;

    for iter in [first, second] {
        let _ = iter;
    }

    Some(first.saturating_add(second))
}

fn main() {
    let _ = (
        narrowed(1),
        named(1),
        either(Some(1)),
        halves(Read::Cut(1)),
        halves(Read::Whole(1)),
        walked([[1, 2], [3, 4]]),
        asked(Some(1), Some(2)),
    );
}
