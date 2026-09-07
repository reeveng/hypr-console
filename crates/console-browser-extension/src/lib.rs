//! The add-on this desktop puts in its browser, and the packing of it.
//!
//! A page is the one place on this device where the promise the buttons make
//! was not kept. Everywhere else the d-pad moves between things and A takes the
//! one it is standing on; in a browser the stick pushed a pointer at a link and
//! A clicked wherever the pointer had got to. `web/` is the add-on that keeps
//! the promise inside a page, and this crate is what turns those files into
//! something a browser will install.
//!
//! The packed file goes into the profile's own `extensions/` directory, which
//! is the one way a browser will take an add-on nobody has signed. It was named
//! in the browser's policy first, and that never worked once: a policy checks
//! the signature whatever `xpinstall.signatures.required` says.
//!
//! The palette it is dressed in is the copy already in the profile, read rather
//! than written down a second time, because a second copy of an answer is a
//! copy that is wrong the first day somebody changes the other one.

use console_core_never::Never;
use std::path::{Path, PathBuf};

pub mod pack;
pub mod source;
pub mod stamp;

pub const ID: &str = "web@console";

pub const PROFILE: &str = ".librewolf/console";

pub const PALETTE: &str = ".librewolf/console/chrome/palette.css";

pub fn xpi(home: &Path) -> Result<PathBuf, Never> {
    Ok(home.join(PROFILE).join("extensions").join(format!("{ID}.xpi")))
}

pub fn stamp(home: &Path) -> Result<PathBuf, Never> {
    Ok(home.join(PROFILE).join("console-web.stamp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xpi(home: &Path) -> PathBuf {
        let Ok(named) = super::xpi(home);

        named
    }

    fn stamp(home: &Path) -> PathBuf {
        let Ok(named) = super::stamp(home);

        named
    }

    #[test]
    fn the_add_on_is_named_where_the_browser_looks_for_it() {
        let home = Path::new("/home/somebody");
        assert_eq!(
            xpi(home),
            PathBuf::from("/home/somebody/.librewolf/console/extensions/web@console.xpi")
        );
    }

    #[test]
    fn the_note_is_not_left_among_the_add_ons() {
        let home = Path::new("/home/somebody");
        let note = stamp(home);
        assert!(!note.starts_with(xpi(home).parent().expect("a directory")), "{}", note.display());
        assert_eq!(note, PathBuf::from("/home/somebody/.librewolf/console/console-web.stamp"));
    }

    #[test]
    fn the_palette_and_the_add_on_are_in_one_profile() {
        assert!(PALETTE.starts_with(PROFILE), "{PALETTE} is not under {PROFILE}");
    }
}
