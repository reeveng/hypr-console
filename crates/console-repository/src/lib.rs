//! Where the repository is, from wherever inside it a program was run.
//!
//! Four programs here only make sense run inside this tree: the theme writes
//! the palette into the files that spend it, the wallpaper is drawn out of the
//! same palette, the emulator reads the captured devices, and the publish
//! builds the public copy. Each of them was walking up to find the top on its
//! own, which is one decision -- what marks the top -- kept in four places
//! that could disagree about it.
//!
//! `desktop.conf` is the mark. It is at the top and nowhere else, and a
//! directory that has one is this repository by the same definition the rest
//! of the desktop uses.

pub mod pulling;

use console_never::Never;
use std::path::{Path, PathBuf};

pub const MARK: &str = "desktop.conf";

pub fn above(here: &Path) -> Result<Option<PathBuf>, Never> {
    Ok(here.ancestors().find(|at| at.join(MARK).is_file()).map(Path::to_path_buf))
}

pub fn root() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|fault| format!("no working directory: {fault}"))?;

    let Ok(above) = above(&here);

    above.ok_or_else(|| format!("no {MARK} above {}; run this inside the repository", here.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        from.canonicalize().unwrap_or(from)
    }

    #[test]
    fn the_top_is_found_from_a_crate_inside_it() {
        let Ok(above) = above(Path::new(env!("CARGO_MANIFEST_DIR")));

        assert_eq!(above, Some(root()));
    }

    #[test]
    fn it_is_the_same_answer_from_further_down() {
        let deep = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let Ok(above) = above(&deep);

        assert_eq!(above, Some(root()));
    }

    #[test]
    fn nothing_above_the_filesystem_holds_the_mark() {
        let Ok(above) = above(Path::new("/"));

        assert_eq!(above, None);
    }
}
