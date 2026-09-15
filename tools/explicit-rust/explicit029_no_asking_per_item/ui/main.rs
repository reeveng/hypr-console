// UI test for EXPLICIT029 — the machine asked once per item.

use std::process::Command;

// GOOD — a syscall is microseconds and a file read in a loop is nearly always
// a read that has to be per item. The rule asked about these once and the tree
// said it should not.
fn read_each(names: &[String]) -> usize {
    let mut held = 0usize;

    for one in names {
        match std::fs::read_to_string(one) {
            Ok(said) => held = held.saturating_add(said.len()),
            Err(_) => {}
        }
    }

    held
}

// BAD EXPLICIT029 — a whole process per name, twice over: one to make it and
// one to run it.
fn run_each(names: &[String]) -> usize {
    let mut held = 0usize;

    for one in names {
        //~v EXPLICIT029_NO_ASKING_PER_ITEM
        match Command::new("hyprctl").arg(one).status() {
            Ok(_) => held = held.saturating_add(1),
            Err(_) => {}
        }
    }

    held
}

// GOOD — two attempts at one thing, and a loop that runs as many times as
// somebody typed. There is no fan-out here and nothing to hoist.
fn twice(name: &str) -> usize {
    let mut held = 0usize;

    for how in ["--first", "--second"] {
        match Command::new("hyprctl").args([how, name]).status() {
            Ok(_) => held = held.saturating_add(1),
            Err(_) => {}
        }
    }

    held
}

// BAD EXPLICIT029 — a fixed loop inside one that is not fixed is still once
// per item.
fn twice_each(names: &[String]) -> usize {
    let mut held = 0usize;

    for one in names {
        for how in ["--first", "--second"] {
            //~v EXPLICIT029_NO_ASKING_PER_ITEM
            match Command::new("hyprctl").args([how, one.as_str()]).status() {
                Ok(_) => held = held.saturating_add(1),
                Err(_) => {}
            }
        }
    }

    held
}

// GOOD — asked once, and its answer walked.
fn read_once(at: &str, names: &[String]) -> usize {
    let said = std::fs::read_to_string(at).unwrap_or_default();
    let mut held = 0usize;

    for one in names {
        match said.contains(one.as_str()) {
            true => held = held.saturating_add(1),
            false => {}
        }
    }

    held
}

// BAD EXPLICIT029 — the loop is spelled `map`, and a fork and an exec once
// per name costs exactly what it costs written the other way.
fn run_each_mapped(names: &[String]) -> Vec<std::process::Command> {
    //~v EXPLICIT029_NO_ASKING_PER_ITEM
    names.iter().map(|one| std::process::Command::new(one)).collect()
}

// GOOD — `map` on an `Option` is the same word over one thing, and multiplies
// by nothing.
fn run_maybe(named: Option<&String>) -> Option<std::process::Command> {
    named.map(|one| std::process::Command::new(one))
}

// GOOD — a list fixed when it was written: two attempts at one thing, and
// there is nothing to hoist out of it.
fn run_both() -> Vec<std::process::Command> {
    ["one", "other"].iter().map(std::process::Command::new).collect()
}

fn main() {
    let names = vec![String::from("one")];

    let _ = read_each(&names);
    let _ = run_each(&names);
    let _ = read_once("/dev/null", &names);
    let _ = twice("one");
    let _ = twice_each(&names);
    let _ = run_each_mapped(&names);
    let _ = run_maybe(names.first());
    let _ = run_both();
}
