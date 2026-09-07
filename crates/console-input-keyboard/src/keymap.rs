//! The keymap, composed from what xkbcommon already knows.
//!
//! The C version ships 12330 lines of hand-written xkb_keymap data — eight
//! layers (latin, cyrillic, arabic, georgian, greek, persian, hebrew, thai)
//! each encoded as a full xkb_keymap string with a custom symbols section
//! for non-latin scripts. That is the wrong place to keep that data:
//!
//! - The symbols are not the keyboard's to know. They live in
//!   `/usr/share/X11/xkb/symbols/` and xkbcommon reads them.
//! - Thai was added the same way every other language could be added, which
//!   is to say by writing more keymap data into the C source. The list of
//!   languages this keyboard can write closed at compile time, even though
//!   X11's symbol set has been open for thirty years.
//! - A keyboard that needs a French keyboard has to be rebuilt.
//!
//! This module asks xkbcommon for a keymap per language. The keyboard owns
//! the *layout* — which physical key carries which xkb keycode — and xkb
//! owns the *symbols* — which character a keycode produces for a given
//! language. The two are joined at runtime by passing xkb the layout's
//! keymap as its only customisation; everything else is the system default.

use console_core_never::Never;
use std::path::Path;

use xkbcommon::xkb::{Context, Keymap as XkbKeymap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Latin,
    Thai,
    French,
    German,
    Russian,
    Greek,
    Arabic,
    Hebrew,
    Persian,
    Georgian,
}

impl Layer {
    pub fn tag(self) -> Result<&'static str, Never> {
        Ok(match self {
            Layer::Latin => "us",
            Layer::Thai => "th",
            Layer::French => "fr",
            Layer::German => "de",
            Layer::Russian => "ru",
            Layer::Greek => "gr",
            Layer::Arabic => "ara",
            Layer::Hebrew => "il",
            Layer::Persian => "ir",
            Layer::Georgian => "ge",
        })
    }

    pub fn name(self) -> Result<&'static str, Never> {
        Ok(match self {
            Layer::Latin => "latin",
            Layer::Thai => "thai",
            Layer::French => "french",
            Layer::German => "german",
            Layer::Russian => "russian",
            Layer::Greek => "greek",
            Layer::Arabic => "arabic",
            Layer::Hebrew => "hebrew",
            Layer::Persian => "persian",
            Layer::Georgian => "georgian",
        })
    }

    pub fn written(self) -> Result<&'static str, Never> {
        Ok(match self {
            Layer::Latin => "ABC",
            Layer::Thai => "ไทย",
            Layer::French => "FR",
            Layer::German => "DE",
            Layer::Russian => "Рус",
            Layer::Greek => "Ελλ",
            Layer::Arabic => "عربي",
            Layer::Hebrew => "עברית",
            Layer::Persian => "فارسی",
            Layer::Georgian => "ქარ",
        })
    }

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
    NoContext,
    UnknownLayout(Layer),
    Empty,
}

pub fn available(rules: &str, symbols_root: &Path) -> Result<Vec<Keymap>, Error> {
    let mut context = Context::new(xkbcommon::xkb::CONTEXT_NO_DEFAULT_INCLUDES);
    context.include_path_append(symbols_root);
    context.include_path_append_default();
    let mut out = Vec::new();

    for layer in Layer::ALL {
        let keymap = match keymap_for(&context, rules, *layer) {
            Ok(keymap) => keymap,
            Err(Error::UnknownLayout(_)) => continue,
            Err(other) => return Err(other),
        };
        out.push(keymap);
    }

    match out.is_empty() {
        true => return Err(Error::NoContext),
        false => {},
    }

    Ok(out)
}

fn keymap_for(
    context: &Context,
    rules: &str,
    layer: Layer,
) -> Result<Keymap, Error> {
    let Ok(tag) = layer.tag();
    let keymap = XkbKeymap::new_from_names(
        context,
        rules,
        "",
        tag,
        "",
        None,
        xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or(Error::UnknownLayout(layer))?;
    let bytes = keymap.get_as_string(xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1);

    match bytes.is_empty() {
        true => return Err(Error::Empty),
        false => {},
    }

    Ok(Keymap { layer, bytes })
}

pub fn default_symbols_root() -> Result<std::path::PathBuf, Never> {
    Ok(std::path::PathBuf::from("/usr/share/X11/xkb"))
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
    fn the_xkb_system_directory_resolves_on_this_machine() {
        let Ok(root) = default_symbols_root();
        let symbols = root.join("symbols");
        if !symbols.is_dir() {
            eprintln!("skipped: no xkb symbols at {}", symbols.display());
            return;
        }
        let result = available("evdev", &root);
        assert!(result.is_ok(), "available: {:?}", result.err());
        let keymaps = result.unwrap();
        let latin = keymaps
            .iter()
            .find(|k| k.layer == Layer::Latin)
            .expect("latin");
        assert!(latin.bytes.starts_with("xkb_keymap {"), "{:?}", &latin.bytes[..40]);
        assert!(
            latin.bytes.contains("xkb_symbols"),
            "the serialised keymap has no symbols section"
        );
        let total: usize = keymaps.iter().map(|k| k.bytes.len()).sum();
        eprintln!(
            "xkb keymaps: {} layers, {} bytes total",
            keymaps.len(),
            total
        );
    }
}
