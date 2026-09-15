// UI test for EXPLICIT028 — two loops written as one line.

use std::collections::HashSet;

const OURS: [&str; 2] = ["one", "other"];

// BAD EXPLICIT028 — every held name visited for every wanted one.
fn shared(wanted: &[String], held: &Vec<String>) -> usize {
    let mut found = 0usize;

    for one in wanted {
        //~v EXPLICIT028_NO_SEARCH_IN_A_LOOP
        match held.contains(one) {
            true => found = found.saturating_add(1),
            false => {}
        }
    }

    found
}

// BAD EXPLICIT028 — the same walk, spelled as an iterator.
fn shared_either_way(wanted: &[String], held: &[String]) -> usize {
    let mut found = 0usize;

    for one in wanted {
        //~v EXPLICIT028_NO_SEARCH_IN_A_LOOP
        match held.iter().any(|other| other == one) {
            true => found = found.saturating_add(1),
            false => {}
        }
    }

    found
}

// GOOD — the set is made once and asked inside the loop.
fn asked(wanted: &[String], held: &[String]) -> usize {
    let known: HashSet<&String> = held.iter().collect();
    let mut found = 0usize;

    for one in wanted {
        match known.contains(one) {
            true => found = found.saturating_add(1),
            false => {}
        }
    }

    found
}

// GOOD — a table whose length was decided when it was written. The
// multiplication a constant makes is a constant.
const MODIFIERS: &[&str] = &["shift", "ctrl", "alt", "super"];

fn modified(wanted: &[String]) -> usize {
    let mut found = 0usize;

    for one in wanted {
        match MODIFIERS.contains(&one.as_str()) {
            true => found = found.saturating_add(1),
            false => {}
        }
    }

    found
}

// GOOD — an array carries its length in its type, so this is the same thing
// said without a name.
fn cornered(wanted: &[String]) -> usize {
    let corners = ["top", "bottom"];
    let mut found = 0usize;

    for one in wanted {
        match corners.iter().any(|corner| *corner == one.as_str()) {
            true => found = found.saturating_add(1),
            false => {}
        }
    }

    found
}

// GOOD — the loop runs as many times as somebody typed, so the walk is linear
// however long the list is.
fn checked(held: &[String]) -> usize {
    let mut found = 0usize;

    for one in ["one", "two"] {
        match held.iter().any(|other| other.as_str() == one) {
            true => found = found.saturating_add(1),
            false => {}
        }
    }

    found
}

// GOOD — one walk, with nothing standing over it.
fn once(wanted: &str, held: &[String]) -> bool {
    held.iter().any(|one| one.as_str() == wanted)
}

// GOOD — the list is made inside the loop that walks it, so building it and
// walking it are one pass over the same items.
fn narrowed(rows: &[Vec<String>], wanted: &str) -> usize {
    let mut found = 0usize;

    for row in rows {
        let long: Vec<&String> = row.iter().filter(|one| one.len() > 2).collect();

        match long.iter().any(|one| one.as_str() == wanted) {
            true => found = found.saturating_add(1),
            false => {}
        }
    }

    found
}

// BAD EXPLICIT028 — made in the outer loop and walked in the inner one, which
// multiplies exactly as much as one made outside both.
fn spread(rows: &[Vec<String>], wanted: &[String]) -> usize {
    let mut found = 0usize;

    for row in rows {
        let long: Vec<&String> = row.iter().filter(|one| one.len() > 2).collect();

        for one in wanted {
            //~v EXPLICIT028_NO_SEARCH_IN_A_LOOP
            match long.iter().any(|other| *other == one) {
                true => found = found.saturating_add(1),
                false => {}
            }
        }
    }

    found
}

// BAD EXPLICIT028 — the outer walk is spelled `filter` and the inner one is
// still a search, so it is still every held name for every wanted one.
fn shared_through_a_closure(wanted: &[String], held: &[String]) -> usize {
    wanted
        .iter()
        //~v EXPLICIT028_NO_SEARCH_IN_A_LOOP
        .filter(|one| held.contains(one))
        .count()
}

// GOOD — the searched list was fixed when it was written, so it is a handful
// of comparisons per item however long the other list is.
fn ours_among(wanted: &[String]) -> usize {
    wanted.iter().filter(|one| OURS.contains(&one.as_str())).count()
}

// GOOD — the set is made once and the closure asks it.
fn asked_through_a_closure(wanted: &[String], held: &[String]) -> usize {
    let known: HashSet<&String> = held.iter().collect();

    wanted.iter().filter(|one| known.contains(one)).count()
}

fn main() {
    let held = vec![String::from("one"), String::from("two")];

    let _ = shared(&held, &held);
    let _ = shared_either_way(&held, &held);
    let _ = asked(&held, &held);
    let _ = modified(&held);
    let _ = cornered(&held);
    let _ = checked(&held);
    let _ = once("one", &held);

    let rows = vec![held.clone()];

    let _ = narrowed(&rows, "one");
    let _ = spread(&rows, &held);
    let _ = shared_through_a_closure(&held, &held);
    let _ = asked_through_a_closure(&held, &held);
    let _ = ours_among(&held);
}
