// UI test for EXPLICIT046 — a jump out of a nested loop says which loop it
// leaves.

// BAD EXPLICIT046 — two loops, and the `break` names neither.
fn leaves_something(rows: &[Vec<u32>]) -> u32 {
    for row in rows {
        for at in row {
            match *at > 10 {
                //~v EXPLICIT046_NO_UNNAMED_JUMP
                true => break,
                false => {}
            }
        }
    }

    0
}

// BAD EXPLICIT046 — the same question asked of `continue`.
fn skips_something(rows: &[Vec<u32>]) {
    for row in rows {
        for at in row {
            match *at > 10 {
                //~v EXPLICIT046_NO_UNNAMED_JUMP
                true => continue,
                false => {}
            }
        }
    }
}

// GOOD — one loop has one answer, so the bare jump says where it goes.
fn leaves_the_only_one(row: &[u32]) {
    for at in row {
        match *at > 10 {
            true => break,
            false => {}
        }
    }
}

// GOOD — the destination is named, so wrapping another loop around it changes
// nothing.
fn says_which(rows: &[Vec<u32>]) {
    'over_rows: for row in rows {
        for at in row {
            match *at > 10 {
                true => break 'over_rows,
                false => {}
            }
        }
    }
}

// GOOD — a closure is its own body, and the loop outside it cannot be reached
// from in here.
fn inside_a_closure(rows: &[Vec<u32>]) {
    for row in rows {
        let _held: Vec<u32> = row
            .iter()
            .map(|at| {
                let mut said = 0;

                for _once in 0..1 {
                    said = *at;
                    break;
                }

                said
            })
            .collect();
    }
}

fn main() {}
