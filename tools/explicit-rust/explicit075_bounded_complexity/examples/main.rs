fn one(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8, h: u8, i: u8) -> u8 {
    a.saturating_add(b)
}

#[allow(dead_code)]
fn stepped(held: Option<u8>) -> u8 {
    match held {
        Some(one) => match one {
            0 => match one.checked_sub(1) {
                Some(two) => match two.checked_sub(1) {
                    Some(three) => match three.checked_sub(1) {
                        Some(four) => match four.checked_sub(1) {
                            Some(five) => five,
                            None => 0,
                        },
                        None => 0,
                    },
                    None => 0,
                },
                None => 0,
            },
            _ => 0,
        },
        None => 0,
    }
}

fn held(held: u8) -> u8 {
    match held {
        0 => 1,
        _ => held,
    }
}

fn walked(rows: [Option<u8>; 3]) -> Option<u8> {
    let mut sum: u8 = 0;

    for row in rows {
        match row {
            Some(one) => match one.checked_sub(1) {
                Some(two) => {
                    sum = sum.saturating_add(two);
                }
                None => {}
            },
            None => {}
        }
    }

    Some(sum)
}

fn asked(held: Option<u8>) -> Option<u8> {
    match held {
        Some(one) => match one.checked_sub(1) {
            Some(two) => match two.checked_sub(1) {
                Some(three) => Some(three.checked_sub(1)?.checked_sub(1)?.checked_sub(1)?),
                None => None,
            },
            None => None,
        },
        None => None,
    }
}

fn main() {
    let _ = (
        one(1, 2, 3, 4, 5, 6, 7, 8, 9),
        stepped(Some(2)),
        held(1),
        walked([Some(1), None, Some(3)]),
        asked(Some(9)),
    );
}
