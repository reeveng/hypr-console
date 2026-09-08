// UI test for EXPLICIT023 — a decision is a `match`, including the one that
// used to be written as a `let … else`.

// Stands in for `console_core_never::Never`: an enum with no variants, so a
// `Result<T, Never>` has no `Err` to meet and the `let` below it is
// irrefutable rather than a decision.
pub enum Never {}

fn at(home: &str) -> Result<String, Never> {
    Ok(format!("{home}/.config/console/warm"))
}

// BAD EXPLICIT023 — `None` is never written; the case that did not bind is
// spelled `else`.
fn first_word(said: &str) -> String {
    //~v EXPLICIT023_NO_LET_ELSE
    let Some(word) = said.split_whitespace().next() else {
        return String::new();
    };

    word.to_string()
}

// BAD EXPLICIT023 — the same thing with a `Result`, and the fault it met has
// no name either.
fn read(at: &str) -> String {
    //~v EXPLICIT023_NO_LET_ELSE
    let Ok(said) = std::fs::read_to_string(at) else {
        return String::new();
    };

    said
}

// GOOD — the same two decisions, with both cases named and the binding left
// where it was.
fn first_word_said(said: &str) -> String {
    let word = match said.split_whitespace().next() {
        Some(word) => word,
        None => return String::new(),
    };

    word.to_string()
}

fn read_said(at: &str) -> String {
    match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_) => String::new(),
    }
}

// GOOD — an irrefutable `let` over a `Result<T, Never>`. There is no `else`
// because there is no other case, which is EXPLICIT002 rather than a decision.
fn where_it_goes(home: &str) -> String {
    let Ok(at) = at(home);

    at
}

// GOOD — an ordinary binding, and a destructuring one.
fn ordinary() -> usize {
    let (left, right) = (2usize, 3usize);
    let both = left + right;

    both
}

fn main() {}
