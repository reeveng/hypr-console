// UI test for EXPLICIT036 — a program named by a string nothing answers for.

use std::process::Command;

enum Program {
    Hyprctl,
}

impl Program {
    fn named(&self) -> &'static str {
        match self {
            Program::Hyprctl => "hyprctl",
        }
    }
}

// BAD EXPLICIT036 — a name spelled at the call.
fn asked() {
    //~v EXPLICIT036_NO_PROGRAM_BY_NAME
    let _ = Command::new("hyprctl").arg("monitors").status();
}

// BAD EXPLICIT036 — one of this tree's own, which the manifest has to name.
fn said() {
    //~v EXPLICIT036_NO_PROGRAM_BY_NAME
    let _ = std::process::Command::new("console-say").arg("hello").status();
}

// GOOD — the variant answers for the name.
fn off_the_list() {
    let _ = Command::new(Program::Hyprctl.named()).arg("monitors").status();
}

// GOOD — a name handed in is a name someone else answered for.
fn handed(named: &str) {
    let _ = Command::new(named).status();
}

fn main() {
    asked();
    said();
    off_the_list();
    handed("hyprctl");
}
