// UI test for EXPLICIT032 — an iterator walked to its end to answer a yes or a
// no.

// BAD EXPLICIT032 — every element visited, when the first match settled it.
fn holds(held: &[String], wanted: &str) -> bool {
    //~v EXPLICIT032_NO_COUNTING_TO_ASK
    held.iter().filter(|one| one.as_str() == wanted).count() > 0
}

// BAD EXPLICIT032 — the whole walk to find out there was nothing.
fn empty(held: &[String]) -> bool {
    //~v EXPLICIT032_NO_COUNTING_TO_ASK
    held.iter().count() == 0
}

// BAD EXPLICIT032 — the same question written the other way about.
fn holds_one(held: &[String]) -> bool {
    //~v EXPLICIT032_NO_COUNTING_TO_ASK
    1 <= held.iter().count()
}

// GOOD — the count is compared against a real number, so counting is the work.
fn several(held: &[String]) -> bool {
    held.iter().count() > 3
}

// GOOD — stops at the first element that answers.
fn asked(held: &[String], wanted: &str) -> bool {
    held.iter().any(|one| one.as_str() == wanted)
}

// GOOD — a length is a field rather than a walk, and stock clippy has the rest
// of what there is to say about this one.
fn nothing(held: &[String]) -> bool {
    held.is_empty()
}

fn main() {
    let held = vec![String::from("one"), String::from("two")];

    let _ = holds(&held, "one");
    let _ = empty(&held);
    let _ = holds_one(&held);
    let _ = several(&held);
    let _ = asked(&held, "one");
    let _ = nothing(&held);
}
