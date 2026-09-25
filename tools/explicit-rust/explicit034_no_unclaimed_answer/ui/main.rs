// UI test for EXPLICIT034 — an answer no one is asked to take.

struct Held {
    named: String,
}

// BAD EXPLICIT034 — a question whose answer can be dropped as a statement.
//~v EXPLICIT034_NO_UNCLAIMED_ANSWER
fn named(of: &[String]) -> String {
    of.first().cloned().unwrap_or_default()
}

// BAD EXPLICIT034 — the same, where the answer is a number.
//~v EXPLICIT034_NO_UNCLAIMED_ANSWER
fn wide(of: u32) -> u32 {
    of.saturating_mul(2)
}

// GOOD — the answer says it is the point of the call.
#[must_use]
fn claimed(of: &[String]) -> String {
    of.first().cloned().unwrap_or_default()
}

// GOOD — a `Result` carries the attribute itself, which is most of this tree.
fn asked(of: &str) -> Result<String, String> {
    Ok(of.to_string())
}

// GOOD — something it was lent can change, so a caller may want only that.
fn stepped(into: &mut Vec<String>, what: &str) -> usize {
    into.push(what.to_string());
    into.len()
}

// GOOD — nothing is handed back at all.
fn does(with: &Held) {
    let _ = &with.named;
}

// GOOD — the signature was someone else's to choose.
impl Default for Held {
    fn default() -> Self {
        Held {
            named: String::new(),
        }
    }
}

fn main() {
    let held = vec![String::from("one")];
    let mut more = held.clone();

    let _ = named(&held);
    let _ = wide(3);
    let _ = claimed(&held);
    let _ = asked("one");
    let _ = stepped(&mut more, "two");
    does(&Held::default());
}
