// UI test for EXPLICIT033 — an absence answered by a value no one wrote down.

// BAD EXPLICIT033 — the reader has to know the type to know what ran.
fn named(of: &[String]) -> String {
    //~v EXPLICIT033_NO_UNNAMED_ABSENCE
    of.first().cloned().unwrap_or_default()
}

// BAD EXPLICIT033 — a number where an absence was, which is a sentinel as well.
fn across(said: Option<u32>) -> u32 {
    //~v EXPLICIT033_NO_UNNAMED_ABSENCE
    said.unwrap_or(0)
}

// BAD EXPLICIT033 — a block at the one place a name was owed.
fn title(of: Option<&str>) -> String {
    //~v EXPLICIT033_NO_UNNAMED_ABSENCE
    of.unwrap_or_else(|| "untitled").to_string()
}

// GOOD — both ways, and the chosen answer written where it is chosen.
fn chosen(said: Option<u32>) -> u32 {
    match said {
        Some(across) => across,
        None => WIDE,
    }
}

const WIDE: u32 = 1024;

// GOOD — the absence is someone else's business and is handed on.
fn handed(of: &[String]) -> Option<String> {
    of.first().cloned()
}

// GOOD — the absence becomes a fault that says what was missing.
fn asked(of: Option<&str>) -> Result<String, String> {
    of.map(str::to_string).ok_or_else(|| String::from("nothing said what it was called"))
}

// GOOD — a `Result`, which is EXPLICIT001's to answer for, not this one.
fn read(at: &str) -> String {
    std::fs::read_to_string(at).unwrap_or_default()
}

fn main() {
    let held = vec![String::from("one")];

    let _ = named(&held);
    let _ = across(Some(3));
    let _ = title(Some("one"));
    let _ = chosen(None);
    let _ = handed(&held);
    let _ = asked(None);
    let _ = read("/dev/null");
}
