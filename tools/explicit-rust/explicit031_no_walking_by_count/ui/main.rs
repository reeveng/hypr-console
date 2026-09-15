// UI test for EXPLICIT031 — a loop about a number standing in for a loop about
// a list.

// BAD EXPLICIT031 — the bound is written by hand and the `None` cannot happen.
fn walked(held: &[String]) -> usize {
    let mut total = 0usize;

    //~v EXPLICIT031_NO_WALKING_BY_COUNT
    for at in 0..held.len() {
        match held.get(at) {
            Some(one) => total = total.saturating_add(one.len()),
            None => {}
        }
    }

    total
}

// BAD EXPLICIT031 — and the spelling that is off the end as well.
fn walked_further(held: &[String]) -> usize {
    let mut total = 0usize;

    //~v EXPLICIT031_NO_WALKING_BY_COUNT
    for at in 0..=held.len() {
        match held.get(at) {
            Some(one) => total = total.saturating_add(one.len()),
            None => {}
        }
    }

    total
}

// GOOD — the item is the subject and there is no bound to get wrong.
fn said(held: &[String]) -> usize {
    let mut total = 0usize;

    for one in held.iter() {
        total = total.saturating_add(one.len());
    }

    total
}

// GOOD — the position wanted alongside the item.
fn numbered(held: &[String]) -> usize {
    let mut total = 0usize;

    for (at, one) in held.iter().enumerate() {
        total = total.saturating_add(at.saturating_mul(one.len()));
    }

    total
}

// GOOD — a range whose numbers really are the subject.
fn rounds(times: usize) -> usize {
    let mut total = 0usize;

    for _ in 0..times {
        total = total.saturating_add(1);
    }

    total
}

fn main() {
    let held = vec![String::from("one"), String::from("two")];

    let _ = walked(&held);
    let _ = walked_further(&held);
    let _ = said(&held);
    let _ = numbered(&held);
    let _ = rounds(2);
}
