// UI test for EXPLICIT019 — `if` is forbidden; a decision is a `match`.

// BAD EXPLICIT019 — the false path is decided by omission.
fn guarded(full: bool) -> u8 {
    //~v EXPLICIT019_NO_IF
    if full {
        return 0;
    }

    1
}

// BAD EXPLICIT019 — even with an `else`, the decision hides its scrutinee.
fn either(wide: bool) -> u8 {
    //~v EXPLICIT019_NO_IF
    let width = if wide { 3 } else { 1 };
    width
}

// BAD EXPLICIT019 — `if let` names one case and waves at the rest.
fn one_case(x: Option<u8>) -> u8 {
    //~v EXPLICIT019_NO_IF
    if let Some(n) = x {
        return n;
    }

    0
}

// GOOD — both outcomes on the screen, each with a name.
fn named(full: bool) -> u8 {
    match full {
        true => 0,
        false => 1,
    }
}

// GOOD — a `while` is a loop, not an `if` somebody wrote.
fn drains(mut n: u8) -> u8 {
    while n > 0 {
        n = n.saturating_sub(1);
    }

    n
}

fn main() {
    let _ = (
        guarded(true),
        either(false),
        one_case(None),
        named(true),
        drains(3),
    );
}
