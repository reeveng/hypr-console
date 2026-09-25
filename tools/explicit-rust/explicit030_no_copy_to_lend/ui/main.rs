// UI test for EXPLICIT030 — a copy allocated so that a borrow of the copy can
// be handed over.

#[derive(Clone)]
pub struct Held {
    pub name: String,
}

fn takes(held: &Held) -> usize {
    held.name.len()
}

fn takes_words(said: &str) -> usize {
    said.len()
}

// BAD EXPLICIT030 — `&held` was already the borrow this allocated a copy for.
fn given(held: &Held) -> usize {
    //~v EXPLICIT030_NO_COPY_TO_LEND
    takes(&held.clone())
}

// BAD EXPLICIT030 — the same, one word along.
fn owned(held: &Held) -> usize {
    //~v EXPLICIT030_NO_COPY_TO_LEND
    takes(&held.to_owned())
}

// BAD EXPLICIT030 — a whole second string, lent out and dropped.
fn worded(said: &String) -> usize {
    //~v EXPLICIT030_NO_COPY_TO_LEND
    takes_words(&said.clone())
}

// GOOD — the borrow of the thing itself.
fn lent(held: &Held) -> usize {
    takes(held)
}

// GOOD — a copy that is kept is a copy someone needs.
fn kept(held: &Held) -> Held {
    held.clone()
}

// GOOD — the types differ, so a coercion is doing work and what to write
// instead depends on what the far side asked for.
fn numbered(at: usize) -> usize {
    takes_words(&at.to_string())
}

fn main() {
    let held = Held { name: String::from("one") };

    let _ = given(&held);
    let _ = owned(&held);
    let _ = worded(&held.name);
    let _ = lent(&held);
    let _ = kept(&held);
    let _ = numbered(2);
}
