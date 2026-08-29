//! The crate's own source, gathered into one file so the stamp can hash it.
//!
//! Gathered here rather than listed in the stamp, because a list of modules
//! written down by hand is a list that a new module is left off.

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=src");
    let mut sources: Vec<_> = std::fs::read_dir("src")
        .expect("the crate has a source directory")
        .filter_map(|found| found.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|kind| kind == "rs"))
        .collect();
    sources.sort();

    let whole: String = sources
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("a source file reads"))
        .collect();
    let out = Path::new(&std::env::var("OUT_DIR").expect("cargo says where")).join("sources.txt");
    std::fs::write(out, whole).expect("the source is gathered");
}
