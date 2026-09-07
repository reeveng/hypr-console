//! What the add-on is made of.
//!
//! The files under `web/` are carried inside the program that packs them, so
//! the device has one thing to install and nothing to install it from. They
//! are read from this crate at the moment it is compiled, which on this desktop
//! is `console apply` on the machine itself.

use console_core_never::Never;
use sha2::{Digest, Sha256};

pub const FILES: [(&str, &str); 7] = [
    ("manifest.json", include_str!("../web/manifest.json")),
    ("browser.js", include_str!("../web/browser.js")),
    ("pad.js", include_str!("../web/pad.js")),
    ("pad.css", include_str!("../web/pad.css")),
    ("new.html", include_str!("../web/new.html")),
    ("around.js", include_str!("../web/around.js")),
    ("around.json", include_str!("../web/around.json")),
];

pub const VERSION: &str = "@version@";

pub fn hosted(palette: &str) -> Result<Option<String>, Never> {
    Ok(match palette.contains(":root") {
        true => Some(palette.replace(":root", ":host, :root")),
        false => None,
    })
}

pub fn every(version: &str, palette: &str) -> Result<Vec<(String, Vec<u8>)>, Never> {
    let mut files: Vec<(String, Vec<u8>)> = FILES
        .iter()
        .map(|(name, body)| ((*name).to_string(), body.replace(VERSION, version).into_bytes()))
        .collect();
    files.push(("palette.css".to_string(), palette.as_bytes().to_vec()));
    Ok(files)
}

pub fn hash(palette: &str) -> Result<String, Never> {
    let mut asked = Sha256::new();

    for (name, body) in FILES {
        asked.update(name.as_bytes());
        asked.update(body.as_bytes());
    }

    asked.update(palette.as_bytes());
    Ok(format!("{:x}", asked.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALETTE: &str = ":root {\n  --pink: #ffb5e2;\n}\n";

    fn hosted(palette: &str) -> Option<String> {
        let Ok(said) = super::hosted(palette);

        said
    }

    fn every(version: &str, palette: &str) -> Vec<(String, Vec<u8>)> {
        let Ok(files) = super::every(version, palette);

        files
    }

    fn hash(palette: &str) -> String {
        let Ok(said) = super::hash(palette);

        said
    }

    #[test]
    fn the_version_is_filled_in_where_the_mark_is() {
        let files = every("1.0.9", PALETTE);
        let (_, manifest) = files.iter().find(|(name, _)| name == "manifest.json").expect("it");
        let said = String::from_utf8(manifest.clone()).expect("json");
        assert!(said.contains("\"version\": \"1.0.9\""), "{said}");
        assert!(!said.contains(VERSION));
    }

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

    #[test]
    fn a_file_that_names_nothing_to_dress_is_not_a_palette() {
        assert_eq!(hosted("/* nothing here */"), None);
    }

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

    #[test]
    fn what_is_packed_is_what_the_hash_is_of() {
        assert_eq!(hash(PALETTE), hash(PALETTE));
        assert_ne!(hash(PALETTE), hash(":root { --pink: #000000; }"));
    }
}
