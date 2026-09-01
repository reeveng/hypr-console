//! What the add-on is made of.
//!
//! The files under `web/` are carried inside the program that packs them, so
//! the device has one thing to install and nothing to install it from. They
//! are read from this crate at the moment it is compiled, which on this desktop
//! is `console apply` on the machine itself.

use sha2::{Digest, Sha256};

/// Every file of the add-on, in the order they are packed.
///
/// The palette is not among them. It is the browser's own, read off the
/// machine as this is packed, so that the labels a page is dressed in are the
/// same pink as the highlight on every panel of this desktop.
pub const FILES: [(&str, &str); 5] = [
    ("manifest.json", include_str!("../web/manifest.json")),
    ("browser.js", include_str!("../web/browser.js")),
    ("pad.js", include_str!("../web/pad.js")),
    ("pad.css", include_str!("../web/pad.css")),
    ("new.html", include_str!("../web/new.html")),
];

/// Where the version goes in the manifest.
///
/// The same mark the rest of this repository fills a name in with. A version
/// written in the file would be a number somebody has to remember to change,
/// and a browser only reinstalls an add-on whose number has gone up.
pub const VERSION: &str = "@version@";

/// The palette, as a stylesheet inside a shadow root can reach it.
///
/// Everything this add-on draws is drawn in a shadow root, which is the only
/// way to put something on somebody else's page and be sure their stylesheet
/// cannot dress it. `:root` names nothing in there -- the root of a shadow
/// tree is its host -- so the one selector in the file is made to say both.
///
/// Nothing else about the file is touched. It is the browser's own copy, and
/// the day it is written differently this says so rather than quietly packing
/// an add-on with no colours in it.
pub fn hosted(palette: &str) -> Option<String> {
    match palette.contains(":root") {
        true => Some(palette.replace(":root", ":host, :root")),
        false => None,
    }
}

/// Everything that goes into the archive, with the version filled in.
pub fn every(version: &str, palette: &str) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = FILES
        .iter()
        .map(|(name, body)| ((*name).to_string(), body.replace(VERSION, version).into_bytes()))
        .collect();
    files.push(("palette.css".to_string(), palette.as_bytes().to_vec()));
    files
}

/// What is being packed, said in one line.
///
/// The version is left out of it on purpose: it is a number that goes up when
/// the rest of this changes, so hashing it would mean every apply found a
/// difference and the browser reinstalled an add-on nobody had touched.
pub fn hash(palette: &str) -> String {
    let mut asked = Sha256::new();
    for (name, body) in FILES {
        asked.update(name.as_bytes());
        asked.update(body.as_bytes());
    }
    asked.update(palette.as_bytes());
    format!("{:x}", asked.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALETTE: &str = ":root {\n  --pink: #ffb5e2;\n}\n";

    #[test]
    fn the_version_is_filled_in_where_the_mark_is() {
        let files = every("1.0.9", PALETTE);
        let (_, manifest) = files.iter().find(|(name, _)| name == "manifest.json").expect("it");
        let said = String::from_utf8(manifest.clone()).expect("json");
        assert!(said.contains("\"version\": \"1.0.9\""), "{said}");
        assert!(!said.contains(VERSION));
    }

    /// The mark has to be in the file for the line above to have anything to
    /// do. A manifest with a version written into it is one the browser stops
    /// reinstalling the day somebody forgets to raise it by hand.
    #[test]
    fn the_manifest_leaves_the_version_to_be_filled_in() {
        let (_, manifest) = FILES[0];
        assert!(manifest.contains(VERSION));
    }

    #[test]
    fn the_palette_is_packed_beside_them() {
        let files = every("1.0.0", PALETTE);
        let (_, said) = files.iter().find(|(name, _)| name == "palette.css").expect("the palette");
        assert_eq!(said, PALETTE.as_bytes());
    }

    #[test]
    fn a_palette_a_shadow_root_can_read_says_both_names() {
        let said = hosted(PALETTE).expect("a palette");
        assert!(said.starts_with(":host, :root {"));
        assert!(said.contains("--pink: #ffb5e2;"));
    }

    /// A file that is not the palette any more is said so about, rather than
    /// packed as an add-on with no colour in it.
    #[test]
    fn a_file_that_names_nothing_to_dress_is_not_a_palette() {
        assert_eq!(hosted("/* nothing here */"), None);
    }

    /// There is no colour in this add-on. Every one it draws is a name out of
    /// the palette, which is the rule the whole desktop is held to and the
    /// reason a colour can be changed in one file.
    #[test]
    fn nothing_here_holds_a_colour_of_its_own() {
        for (name, body) in FILES {
            for line in body.lines() {
                let said = line.split("/*").next().unwrap_or("").trim();
                assert!(!said.contains('#') || !said.contains(';'), "{name}: {line}");
                assert!(!said.contains("rgb("), "{name}: {line}");
            }
        }
    }

    /// The same files packed twice are the same archive, and a file changed by
    /// a letter is not. That is the whole of how an apply knows whether the
    /// browser has anything new to install.
    #[test]
    fn what_is_packed_is_what_the_hash_is_of() {
        assert_eq!(hash(PALETTE), hash(PALETTE));
        assert_ne!(hash(PALETTE), hash(":root { --pink: #000000; }"));
    }
}
