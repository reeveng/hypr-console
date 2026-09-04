// UI test for EXPLICIT016 — a `match` over an enum may not have a wildcard arm.

enum Stage {
    Drawing,
    Holding,
    Leaving,
}

// BAD EXPLICIT016 — Leaving is decided by omission.
fn wildcard(stage: &Stage) -> u8 {
    match stage {
        Stage::Drawing => 1,
        Stage::Holding => 2,
        //~v EXPLICIT016_NO_WILDCARD_ARM
        _ => 0,
    }
}

// BAD EXPLICIT016 — a bare binding is the same omission with a name on it.
fn binding(stage: Stage) -> u8 {
    match stage {
        Stage::Drawing => 1,
        //~v EXPLICIT016_NO_WILDCARD_ARM
        other => drop_it(other),
    }
}

fn drop_it(_stage: Stage) -> u8 {
    0
}

// GOOD — every variant is named; a fourth variant is a build error here.
fn named(stage: &Stage) -> u8 {
    match stage {
        Stage::Drawing => 1,
        Stage::Holding => 2,
        Stage::Leaving => 0,
    }
}

// GOOD — a wildcard over an integer is not an enum losing a variant.
fn numbers(n: u8) -> u8 {
    match n {
        0 => 1,
        _ => 0,
    }
}

fn main() {
    let _ = (
        wildcard(&Stage::Drawing),
        binding(Stage::Drawing),
        named(&Stage::Leaving),
        numbers(0),
    );
}
