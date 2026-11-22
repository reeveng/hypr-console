//! The keymap, composed from the xkb symbols the system already has.
//!
//! The C version ships 12330 lines of hand-written xkb_keymap data — eight
//! layers (latin, cyrillic, arabic, georgian, greek, persian, hebrew, thai)
//! each encoded as a full xkb_keymap string with a custom symbols section
//! for non-latin scripts. That is the wrong place to keep that data:
//!
//! - The symbols are not the keyboard's to know. They live in
//!   `/usr/share/X11/xkb/symbols/` and `console-input-keymaps` reads them.
//! - Thai was added the same way every other language could be added, which
//!   is to say by writing more keymap data into the C source. The list of
//!   languages this keyboard can write closed at compile time, even though
//!   X11's symbol set has been open for thirty years.
//! - A keyboard that needs a French keyboard has to be rebuilt.
//!
//! This module asks for a keymap per language. The keyboard owns the
//! *layout* — which physical key carries which xkb keycode — and xkb owns the
//! *symbols* — which character a keycode produces for a given language. The
//! two are joined at runtime by passing xkb the layout's keymap as its only
//! customisation; everything else is the system default.
//!
//! A language the system has no symbols for still compiles, to a keymap
//! with nothing in it, and the include that could not be found is said as a
//! diagnostic rather than as a failure. That diagnostic is the answer to
//! "does this system know this language", so it is the one asked for.

use console_core_never::Never;
use console_core_words::Words;
use console_input_keymaps::{Context, DEFAULT_INCLUDE_DIR, Diagnostic, DiagnosticKind, Group};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Layer {
  #[words(tag = "us", name = "latin", written = "ABC")]
  Latin,
  #[words(tag = "th", name = "thai", written = "ไทย")]
  Thai,
  #[words(tag = "fr", name = "french", written = "FR")]
  French,
  #[words(tag = "de", name = "german", written = "DE")]
  German,
  #[words(tag = "ru", name = "russian", written = "Рус")]
  Russian,
  #[words(tag = "gr", name = "greek", written = "Ελλ")]
  Greek,
  #[words(tag = "ara", name = "arabic", written = "عربي")]
  Arabic,
  #[words(tag = "il", name = "hebrew", written = "עברית")]
  Hebrew,
  #[words(tag = "ir", name = "persian", written = "فارسی")]
  Persian,
  #[words(tag = "ge", name = "georgian", written = "ქარ")]
  Georgian,
}

impl Layer {
  pub const ALL: &'static [Layer] = &[
    Layer::Latin,
    Layer::French,
    Layer::German,
    Layer::Russian,
    Layer::Greek,
    Layer::Georgian,
    Layer::Arabic,
    Layer::Persian,
    Layer::Hebrew,
    Layer::Thai,
  ];
}

#[derive(Debug, Clone)]
pub struct Keymap {
  pub layer: Layer,
  pub bytes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  NoLayers,
  UnknownLayout(Layer),
}

pub fn available(rules: &str, symbols_root: &Path) -> Result<Vec<Keymap>, Error> {
  let Ok(mut builder) = Context::builder();
  let Ok(()) = builder.prepend_path(symbols_root);
  let Ok(context) = builder.build();
  let mut out = Vec::new();

  for layer in Layer::ALL {
    match keymap_for(&context, rules, *layer) {
      Ok(keymap) => out.push(keymap),
      Err(Error::UnknownLayout(_)) => continue,
      Err(other @ Error::NoLayers) => return Err(other),
    }
  }

  match ::core::primitive::bool::then_some(out.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Err(Error::NoLayers),
    ::core::option::Option::None => {}
  }

  Ok(out)
}

fn keymap_for(context: &Context, rules: &str, layer: Layer) -> Result<Keymap, Error> {
  let Ok(tag) = layer.tag();
  let Ok(compiled) = compiled(
    context,
    rules,
    Group {
      layout: tag,
      variant: "",
    },
  );

  match compiled {
    Some(bytes) => Ok(Keymap { layer, bytes }),
    None => Err(Error::UnknownLayout(layer)),
  }
}

fn compiled(context: &Context, rules: &str, group: Group<'_>) -> Result<Option<String>, Never> {
  let mut diagnostics: Vec<Diagnostic> = Vec::new();
  let Ok(keymap) = context.keymap_from_names(&mut diagnostics, rules, &[group]);
  let unknown = diagnostics.iter().any(|diagnostic| {
    let Ok(kind) = diagnostic.kind();

    kind == DiagnosticKind::FileNotFound
  });

  match ::core::primitive::bool::then_some(unknown, ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  let Ok(text) = keymap.format();

  Ok(Some(text.to_string()))
}

pub fn default_symbols_root() -> Result<std::path::PathBuf, Never> {
  Ok(std::path::PathBuf::from(DEFAULT_INCLUDE_DIR))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_latin_layer_is_us_not_english() {
    assert_eq!(Layer::Latin.tag(), Ok("us"));
  }

  #[test]
  fn every_layer_has_a_distinct_xkb_tag() {
    let mut tags: Vec<&str> = Layer::ALL
      .iter()
      .map(|layer| {
        let Ok(tag) = layer.tag();

        tag
      })
      .collect();

    tags.sort();
    tags.dedup();

    assert_eq!(tags.len(), Layer::ALL.len(), "two layers share an xkb tag");
  }

  #[test]
  fn every_layer_has_a_label() {
    for layer in Layer::ALL {
      let Ok(name) = layer.name();
      let Ok(tag) = layer.tag();

      assert!(!name.is_empty(), "{tag} has no label");
    }
  }

  #[test]
  fn the_xkb_system_directory_resolves_on_this_machine() -> Result<(), Error> {
    let Ok(root) = default_symbols_root();
    let symbols = root.join("symbols");

    match std::fs::read_dir(&symbols) {
      Ok(_installed_symbols) => {}
      Err(_no_installed_symbols) => {
        eprintln!("skipped: no xkb symbols at {}", symbols.display());

        return Ok(());
      }
    }

    let keymaps = available("evdev", &root)?;
    let latin = keymaps
      .iter()
      .find(|keymap| keymap.layer == Layer::Latin)
      .ok_or(Error::UnknownLayout(Layer::Latin))?;

    assert!(
      latin.bytes.starts_with("xkb_keymap {"),
      "{:?}",
      latin.bytes.get(..40)
    );
    assert!(
      latin.bytes.contains("xkb_symbols"),
      "the serialised keymap has no symbols section"
    );

    Ok(())
  }

  #[test]
  fn a_language_the_system_has_no_symbols_for_is_not_offered() -> Result<(), Error> {
    let Ok(root) = default_symbols_root();

    let thai_symbols = std::fs::File::open(root.join("symbols/th"))
      .and_then(|file| file.metadata())
      .map(|metadata| metadata.is_file());

    match thai_symbols {
      Ok(true) => {}
      Ok(false) => {
        eprintln!("skipped: no thai symbols under {}", root.display());

        return Ok(());
      }
      Err(fault) => {
        eprintln!(
          "skipped: could not open thai symbols under {}: {fault}",
          root.display()
        );

        return Ok(());
      }
    }

    let Ok(mut builder) = Context::builder();
    let Ok(()) = builder.prepend_path(&root);
    let Ok(context) = builder.build();
    let thai = keymap_for(&context, "evdev", Layer::Thai)?;

    assert!(
      thai.bytes.contains("Thai_kokai"),
      "the thai keymap has no thai in it"
    );
    assert_eq!(
      compiled(
        &context,
        "evdev",
        Group {
          layout: "zz",
          variant: ""
        }
      ),
      Ok(None)
    );

    Ok(())
  }
}
