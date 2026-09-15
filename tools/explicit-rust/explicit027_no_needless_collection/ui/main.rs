// UI test for EXPLICIT027 — a list allocated to do what the iterator that made
// it was already doing.

// BAD EXPLICIT027 — the two words cancel.
fn widened(held: &[String]) -> Vec<usize> {
    //~v EXPLICIT027_NO_NEEDLESS_COLLECTION
    held.iter()
        .map(|one| one.len())
        .collect::<Vec<usize>>()
        .into_iter()
        .map(|one| one.saturating_add(1))
        .collect()
}

// BAD EXPLICIT027 — a whole list allocated to be asked how long it is.
fn how_many(held: &[String]) -> usize {
    //~v EXPLICIT027_NO_NEEDLESS_COLLECTION
    held.iter().map(|one| one.len()).collect::<Vec<usize>>().len()
}

// BAD EXPLICIT027 — the same waste with a name on it: made, walked once,
// dropped.
fn doubled(held: &[String]) -> Vec<usize> {
    //~v EXPLICIT027_NO_NEEDLESS_COLLECTION
    let lengths: Vec<usize> = held.iter().map(|one| one.len()).collect();

    lengths.into_iter().map(|one| one.saturating_mul(2)).collect()
}

// GOOD — walked twice, so the list is what saves the second walk.
fn both_ways(held: &[String]) -> usize {
    let lengths: Vec<usize> = held.iter().map(|one| one.len()).collect();
    let longest = lengths.iter().max().copied().unwrap_or_default();
    let total: usize = lengths.iter().sum();

    total.saturating_add(longest)
}

// GOOD — walked inside a loop, so the list is what stops the work repeating.
fn over_and_over(held: &[String], rounds: usize) -> usize {
    let lengths: Vec<usize> = held.iter().map(|one| one.len()).collect();
    let mut total = 0usize;

    for _ in 0..rounds {
        total = total.saturating_add(lengths.iter().sum::<usize>());
    }

    total
}

// GOOD — one use, and a walk, and the list still has to exist: every `&str` in
// the answer is a borrow of it.
fn borrowed(said: &str) -> usize {
    let words: Vec<String> = said.split_whitespace().map(str::to_string).collect();
    let held: Vec<&str> = words.iter().map(String::as_str).collect();

    held.len()
}

// GOOD — the list is the answer, so making it is the work.
fn listed(held: &[String]) -> Vec<usize> {
    held.iter().map(|one| one.len()).collect()
}

fn main() {
    let held = vec![String::from("one"), String::from("two")];

    let _ = widened(&held);
    let _ = how_many(&held);
    let _ = doubled(&held);
    let _ = both_ways(&held);
    let _ = over_and_over(&held, 2);
    let _ = listed(&held);
    let _ = borrowed("one two");
}
