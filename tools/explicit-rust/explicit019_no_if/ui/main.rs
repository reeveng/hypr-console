// edition:2024
// UI test for EXPLICIT019 — decisions use enum matches.

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

// BAD EXPLICIT019 — a guard is an `if` on an arm.
fn guarded_arm(x: Option<u8>) -> u8 {
    match x {
        //~v EXPLICIT019_NO_IF
        Some(n) if n > 2 => n,
        Some(_) | None => 0,
    }
}

// GOOD — the question is asked where its answer has two names.
fn asked(x: Option<u8>) -> u8 {
    match x {
        Some(n) => match (n > 2).then_some(()) {
            Some(()) => n,
            None => 0,
        },
        None => 0,
    }
}

// GOOD — a guard inside a macro's arm is the macro author's.
fn in_a_macro(x: Option<u8>) -> bool {
    matches!(x, Some(n) if n > 2)
}

// BAD EXPLICIT019 — a Boolean scrutinee still needs enum outcomes.
fn named(full: bool) -> u8 {
    //~v EXPLICIT019_NO_IF
    match full {
        true => 0,
        false => 1,
    }
}

const fn constant_case(full: bool) -> u8 {
    //~v EXPLICIT019_NO_IF
    match full { true => 0, false => 1 }
}

fn guarded_boolean(full: bool, wide: bool) -> u8 {
    //~v EXPLICIT019_NO_IF
    match full {
        //~v EXPLICIT019_NO_IF
        true if wide => 1,
        _ => 0,
    }
}

fn conditional_arm(full: bool) -> u8 {
    //~v EXPLICIT019_NO_IF
    match full {
        #[cfg(any())]
        true => 2,
        true => 1,
        false => 0,
    }
}

#[no_implicit_prelude]
mod custom_prelude {
    pub fn needs_core(full: bool) -> u8 {
        //~v EXPLICIT019_NO_IF
        match full { true => 1, false => 0 }
    }
}

// GOOD — a `while` is a loop, not an `if` someone wrote.
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
        guarded_arm(None),
        asked(None),
        u8::from(in_a_macro(None)),
        named(true),
        constant_case(false),
        guarded_boolean(true, true),
        conditional_arm(false),
        custom_prelude::needs_core(false),
        drains(3),
    );
}
