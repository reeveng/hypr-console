// UI test for EXPLICIT018 — an `allow` carries its reason.

// BAD EXPLICIT018 — a rule waived in silence.
//~v EXPLICIT018_ALLOW_WITH_REASON
#[allow(dead_code)]
fn quiet() {}

// GOOD — the allow says which harm is absent and why.
#[allow(
    dead_code,
    reason = "a ui fixture is compiled, never called; there is no caller to lose"
)]
fn spoken_for() {}

// GOOD — `warn` hides nothing, so it has nothing to explain.
#[warn(unused_variables)]
fn warned() {}

fn main() {
    quiet();
    spoken_for();
    warned();
}
