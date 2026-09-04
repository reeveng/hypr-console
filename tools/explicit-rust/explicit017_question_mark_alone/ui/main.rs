// UI test for EXPLICIT017 — `?` stands alone.

fn settle(x: u8) -> Result<u8, String> {
    Ok(x)
}

fn frame(a: u8, b: u8) -> u8 {
    a.min(b)
}

// BAD EXPLICIT017 — an early return hidden in an argument list.
fn buried(x: u8) -> Result<u8, String> {
    //~v EXPLICIT017_QUESTION_MARK_ALONE
    Ok(frame(settle(x)?, 3))
}

// BAD EXPLICIT017 — hidden mid-chain: the second link can end the function.
fn chained(x: u8) -> Result<u8, String> {
    //~v EXPLICIT017_QUESTION_MARK_ALONE
    let kept = settle(x)?.checked_add(1).ok_or("full")?;
    Ok(kept)
}

// GOOD — the exit sits on the left margin.
fn lifted(x: u8) -> Result<u8, String> {
    let settled = settle(x)?;
    Ok(frame(settled, 3))
}

// GOOD — the whole of a statement.
fn alone(x: u8) -> Result<(), String> {
    settle(x)?;
    Ok(())
}

// GOOD — the tail expression of a block: an exit at the end, in the open.
fn tail(x: u8) -> Result<u8, String> {
    let attempt: Result<Result<u8, String>, String> = Ok(settle(x));
    attempt?
}

fn main() {
    let _ = (buried(0), chained(0), lifted(0), alone(0), tail(0));
}
