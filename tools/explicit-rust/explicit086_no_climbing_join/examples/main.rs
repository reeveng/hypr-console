use std::path::{Path, PathBuf};

const SHELF: &str = "books";

struct Beneath(String);

impl Beneath {
    fn checked(named: &str) -> Option<Self> {
        let climbs = named.is_empty() || named.starts_with('/') || named.split('/').any(|part| part == "..");

        match climbs {
            true => None,
            false => Some(Self(named.to_string())),
        }
    }
}

impl AsRef<Path> for Beneath {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

fn unpacked(base: &Path, entry: &str) -> PathBuf {
    //~v WARN EXPLICIT086_NO_CLIMBING_JOIN
    base.join(entry)
}

fn pushed(base: &Path, entry: String) -> PathBuf {
    let mut at = base.to_path_buf();

    //~v WARN EXPLICIT086_NO_CLIMBING_JOIN
    at.push(entry);

    at
}

fn built(base: &Path, named: &str) -> PathBuf {
    //~v WARN EXPLICIT086_NO_CLIMBING_JOIN
    base.join(format!("{named}.png"))
}

fn replaced(base: &Path) -> PathBuf {
    //~v WARN EXPLICIT086_NO_CLIMBING_JOIN
    base.join("/etc/passwd")
}

fn walked_out(base: &Path) -> PathBuf {
    //~v WARN EXPLICIT086_NO_CLIMBING_JOIN
    base.join("../elsewhere")
}

fn written(base: &Path) -> PathBuf {
    base.join("pages/first.png")
}

fn named_once(base: &Path) -> PathBuf {
    base.join(SHELF)
}

fn checked(base: &Path, entry: &Beneath) -> PathBuf {
    base.join(entry)
}

fn words(listed: &[&str]) -> String {
    listed.join(", ")
}

fn main() {
    let base = Path::new("somewhere");
    let _ = unpacked(base, "entry");
    let _ = pushed(base, "entry".to_string());
    let _ = built(base, "entry");
    let _ = replaced(base);
    let _ = walked_out(base);
    let _ = written(base);
    let _ = named_once(base);
    let _ = Beneath::checked("entry").map(|entry| checked(base, &entry));
    let _ = words(&["one", "two"]);
}
