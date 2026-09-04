// UI test for EXPLICIT006 — `Option` is for absence, not for errors.

fn fallible() -> Result<i32, std::io::Error> {
    Ok(0)
}

// BAD EXPLICIT006 — the error becomes a `None` that says nothing.
fn launders() -> Option<i32> {
    //~v EXPLICIT006_OPTION_NOT_ERROR
    fallible().ok()
}

// BAD EXPLICIT006 — the value is thrown away and only the failure kept.
fn keeps_only_the_failure() -> Option<std::io::Error> {
    //~v EXPLICIT006_OPTION_NOT_ERROR
    fallible().err()
}

// GOOD — the error keeps its name all the way to the caller.
fn propagates() -> Result<i32, std::io::Error> {
    fallible()
}

// GOOD — `ok` on something that is not a `Result` is somebody's own method.
struct Mine;
impl Mine {
    fn ok(&self) -> bool {
        true
    }
}
fn not_our_business(mine: &Mine) -> bool {
    mine.ok()
}

fn main() {}
