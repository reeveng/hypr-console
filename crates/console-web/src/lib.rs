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

use std::path::{Path, PathBuf};

pub mod pack;
pub mod source;
pub mod stamp;

/// What the add-on calls itself, which is the name the browser knows it by and
/// the name the file has to carry to be found.
pub const ID: &str = "web@console";

/// The profile the browser runs in, under her home.
///
/// Named rather than found, because it is this desktop that named it:
/// `profiles.ini` is shipped saying `console`, so that the colours and now the
/// add-on can be put in a directory that exists before the browser first runs.
pub const PROFILE: &str = ".librewolf/console";

/// The palette file the add-on is dressed out of, under her home.
///
/// The browser's own, which `console-theme` writes and the manifest installs.
/// A palette of ours would be a seventeenth colour on a machine with sixteen.
pub const PALETTE: &str = ".librewolf/console/chrome/palette.css";

/// Where the packed add-on goes: into the profile, beside the browser's own.
///
/// It used to be named in the browser's policy and fetched from /usr/local/lib,
/// and that never once worked. A policy will not install an add-on nobody has
/// signed, whatever `xpinstall.signatures.required` says -- that pref governs a
/// sideload, and this is the sideload it governs. `console_defaults::policies`
/// carries the rest of what was learned.
///
/// The name is the add-on's own id, because that is what a browser looks for
/// when it scans this directory.
pub fn xpi(home: &Path) -> PathBuf {
    home.join(PROFILE).join("extensions").join(format!("{ID}.xpi"))
}

/// Where the note beside it goes, saying what was packed and as what version.
///
/// Beside the profile rather than inside `extensions/`, which the browser reads
/// as a list of add-ons and should hold nothing else.
pub fn stamp(home: &Path) -> PathBuf {
    home.join(PROFILE).join("console-web.stamp")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The browser finds a sideloaded add-on by its id, so the file has to be
    /// called that and has to be in the directory the browser scans.
    #[test]
    fn the_add_on_is_named_where_the_browser_looks_for_it() {
        let home = Path::new("/home/somebody");
        assert_eq!(
            xpi(home),
            PathBuf::from("/home/somebody/.librewolf/console/extensions/web@console.xpi")
        );
    }

    /// `extensions/` is a list of add-ons to the browser. A note of ours in
    /// there is a file it would try to read as one.
    #[test]
    fn the_note_is_not_left_among_the_add_ons() {
        let home = Path::new("/home/somebody");
        let note = stamp(home);
        assert!(!note.starts_with(xpi(home).parent().expect("a directory")), "{}", note.display());
        assert_eq!(note, PathBuf::from("/home/somebody/.librewolf/console/console-web.stamp"));
    }

    /// The palette is read out of the same profile the add-on is written into.
    #[test]
    fn the_palette_and_the_add_on_are_in_one_profile() {
        assert!(PALETTE.starts_with(PROFILE), "{PALETTE} is not under {PROFILE}");
    }
}
