// UI test for EXPLICIT024 — a quantity that shares a representation with
// another quantity is a call site waiting to be written backwards.

pub struct Pixels(pub u32);
pub struct Rows(pub u32);

// An alias is a nickname rather than a type, which is the whole reason the
// middle signature is read instead of the written one.
pub type Milliseconds = u32;

// BAD EXPLICIT024 — a height goes where a width belongs and nothing says so.
//~v EXPLICIT024_NO_SWAPPABLE_PARAMS
fn draw(across: u32, down: u32) -> u32 {
    across.saturating_mul(down)
}

// BAD EXPLICIT024 — two nicknames for one type are still one type.
//~v EXPLICIT024_NO_SWAPPABLE_PARAMS
fn wait(patience: Milliseconds, step: Milliseconds) -> Milliseconds {
    patience.saturating_sub(step)
}

// BAD EXPLICIT024 — references peel, so these are two of the same words.
//~v EXPLICIT024_NO_SWAPPABLE_PARAMS
fn under(folder: &str, name: &str) -> String {
    format!("{folder}/{name}")
}

// GOOD — one of each, and each says what it is.
fn place(across: Pixels, down: Rows) -> u32 {
    across.0.saturating_mul(down.0)
}

// GOOD — a repeated named type is a pair someone has already decided about.
fn between(from: Pixels, to: Pixels) -> u32 {
    to.0.saturating_sub(from.0)
}

pub struct Screen {
    pub across: u32,
}

impl Screen {
    // GOOD — a receiver is not a parameter anyone passes.
    fn wider(&self, across: u32) -> u32 {
        self.across.saturating_add(across)
    }
}

// GOOD — a closure has no call site to protect.
fn measured() -> u32 {
    let both = |across: u32, down: u32| across.saturating_add(down);

    both(2, 3)
}

fn main() {
    let _ = draw(2, 3);
    let _ = wait(2, 3);
    let _ = under("a", "b");
    let _ = place(Pixels(2), Rows(3));
    let _ = between(Pixels(2), Pixels(3));
    let _ = Screen { across: 2 }.wider(3);
    let _ = measured();
}
