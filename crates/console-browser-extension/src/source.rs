//! What the add-on is made of.
//!
//! The files under `web/` are carried inside the program that packs them, so
//! the device has one thing to install and nothing to install it from. They
//! are read from this crate at the moment it is compiled, which on this desktop
//! is `console apply` on the machine itself.

use console_core_checksums::sha256::Sha256;
use console_core_never::Never;

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
const PAD_WASM_NAME: &str = "pad.wasm";
const PAD_WASM: &[u8] = include_bytes!("../web/pad.wasm");

pub fn with_host_selector(palette: &str) -> Result<Option<String>, Never> {
  Ok(match ::core::primitive::bool::then_some(palette.contains(":host, :root"), ()) {
    ::core::option::Option::Some(()) => Some(palette.to_string()),
    ::core::option::Option::None => match ::core::primitive::bool::then_some(palette.contains(":root"), ()) {
      ::core::option::Option::Some(()) => Some(palette.replace(":root", ":host, :root")),
      ::core::option::Option::None => None,
    },
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette<'a>(pub &'a str);

pub fn every(version: &str, palette: Palette<'_>) -> Result<Vec<(String, Vec<u8>)>, Never> {
  let palette_2 = palette.0;

  let mut files: Vec<(String, Vec<u8>)> = FILES
    .iter()
    .map(|(name, body)| {
      (
        (*name).to_string(),
        body.replace(VERSION, version).into_bytes(),
      )
    })
    .collect();
  files.push((PAD_WASM_NAME.to_string(), PAD_WASM.to_vec()));
  files.push(("palette.css".to_string(), palette_2.as_bytes().to_vec()));
  Ok(files)
}

pub fn hash(palette: &str) -> Result<String, Never> {
  let mut asked = Sha256::default();

  for (name, body) in FILES {
    let Ok(()) = asked.update(name.as_bytes());
    let Ok(()) = asked.update(body.as_bytes());
  }

  let Ok(()) = asked.update(PAD_WASM_NAME.as_bytes());
  let Ok(()) = asked.update(PAD_WASM);
  let Ok(()) = asked.update(palette.as_bytes());
  let Ok(digest) = asked.finalize();

  Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  const PALETTE: &str = ":root {\n  --pink: #ffb5e2;\n}\n";

  #[test]
  fn the_version_is_filled_in_where_the_mark_is() -> Result<(), Box<dyn Error>> {
    let Ok(files) = every("1.0.9", Palette(PALETTE));
    let (_, manifest) = files
      .iter()
      .find(|(name, _)| name == "manifest.json")
      .ok_or("it")?;
    let said = String::from_utf8(manifest.clone())?;

    assert!(said.contains("\"version\": \"1.0.9\""), "{said}");
    assert!(!said.contains(VERSION));

    Ok(())
  }

  #[test]
  fn the_manifest_leaves_the_version_to_be_filled_in() -> Result<(), Box<dyn Error>> {
    let (_, manifest) = FILES.first().ok_or("the manifest")?;

    assert!(manifest.contains(VERSION));

    Ok(())
  }

  #[test]
  fn the_palette_is_packed_beside_them() -> Result<(), Box<dyn Error>> {
    let Ok(files) = every("1.0.0", Palette(PALETTE));
    let (_, said) = files
      .iter()
      .find(|(name, _)| name == "palette.css")
      .ok_or("the palette")?;

    assert_eq!(said, PALETTE.as_bytes());

    Ok(())
  }

  #[test]
  fn the_pad_module_is_packed_as_webassembly() -> Result<(), Box<dyn Error>> {
    let Ok(files) = every("1.0.0", Palette(PALETTE));
    let (_, wasm) = files
      .iter()
      .find(|(name, _)| name == PAD_WASM_NAME)
      .ok_or("the pad's WebAssembly was left out")?;
    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
    assert_eq!(wasm, PAD_WASM);
    Ok(())
  }

  #[test]
  fn a_palette_a_shadow_root_can_read_says_both_names() -> Result<(), Box<dyn Error>> {
    let Ok(dressed) = with_host_selector(PALETTE);
    let said = dressed.ok_or("a palette")?;

    assert!(said.starts_with(":host, :root {"));
    assert!(said.contains("--pink: #ffb5e2;"));

    Ok(())
  }

  #[test]
  fn a_file_that_names_nothing_to_dress_is_not_a_palette() {
    assert_eq!(with_host_selector("/* nothing here */"), Ok(None));
  }

  #[test]
  fn nothing_here_holds_a_color_of_its_own() {
    let scripts = [
      (
        "browser.ts",
        include_str!("../../../tools/browser-scripts/src/extension/browser.ts"),
      ),
      (
        "pad.ts",
        include_str!("../../../tools/browser-scripts/src/extension/pad.ts"),
      ),
      (
        "around.ts",
        include_str!("../../../tools/browser-scripts/src/extension/around.ts"),
      ),
    ];
    let assets = FILES.into_iter().filter(|(name, _)| !name.ends_with(".js"));

    for (name, body) in assets.chain(scripts) {
      for line in body.lines() {
        let said = match line.split_once("/*") {
          Some((said, _comment)) => said.trim(),
          None => line.trim(),
        };

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
