fn counted_down(mut n: u8) -> u8 {
    //~v EXPLICIT053_NO_WHILE
    while n > 0 {
        n = n.saturating_sub(1);
    }

    n
}

fn drained(mut held: Vec<u8>) -> u8 {
    let mut sum: u8 = 0;

    //~v EXPLICIT053_NO_WHILE
    while let Some(one) = held.pop() {
        sum = sum.saturating_add(one);
    }

    sum
}

fn walked(held: Vec<u8>) -> u8 {
    let mut sum: u8 = 0;

    for one in held {
        sum = sum.saturating_add(one);
    }

    sum
}

fn looped(mut n: u8) -> u8 {
    loop {
        match n > 0 {
            true => n = n.saturating_sub(1),
            false => break n,
        }
    }
}

fn main() {
    let _ = (counted_down(3), drained(vec![1, 2]), walked(vec![1, 2]), looped(3));
}
