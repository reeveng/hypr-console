//! The crate's own source, gathered into one file so the stamp can hash it.
//!
//! Gathered here rather than listed in the stamp, because a list of modules
//! written down by hand is a list that a new module is left off.

use std::error::Error;
use std::path::Path;

// A build script says it failed by returning, the same as anything else here.
// Cargo prints the error and stops the build, which is what a panic did, with
// the reason in a type instead of in a message.
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src");
    // Collected through a `Result` rather than filtered with `ok()`: a
    // directory entry that will not read is a build that should stop, not a
    // source file quietly left out of the hash below.
    let mut sources: Vec<_> = std::fs::read_dir("src")?
        .map(|found| found.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|kind| kind == "rs"))
        .collect();
    sources.sort();

    let mut whole = String::new();

    for path in &sources {
        whole.push_str(&std::fs::read_to_string(path)?);
    }

    let out = Path::new(&std::env::var("OUT_DIR")?).join("sources.txt");
    std::fs::write(out, whole)?;
    Ok(())
}
