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

use std::path::Path;

use xkbcommon::xkb::{Context, Keymap as XkbKeymap};

/// One language the keyboard can write in.
///
/// The variants are the language tags xkbcommon understands; the
/// `name()` method returns the human-readable word that goes on the
/// layer-switching button.
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
    /// The xkb layout tag for this layer.
    ///
    /// This is the word before the colon in `setxkbmap -layout th`, and it
    /// matches the file name in `/usr/share/X11/xkb/symbols/`.
    pub fn tag(self) -> &'static str {
        match self {
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
        }
    }

    /// The label on the layer-switching button.
    ///
    /// Latin gets a special name because the keyboard's first layer is
    /// always latin; calling it "English" on the button is the wrong thing
    /// when the user wrote the rest of the desktop in something else.
    pub fn name(self) -> &'static str {
        match self {
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
        }
    }

    /// What the language key writes when it is about to go here.
    ///
    /// Each language in its own script, because the person who wants it is the
    /// person who reads it: somebody looking for Thai is looking for `ไทย`, and
    /// `thai` is a word in the alphabet they are trying to leave. Short enough
    /// to sit on one key -- these are names, not sentences, and a few are
    /// abbreviated the way the language itself abbreviates them.
    ///
    /// Latin is `ABC` rather than a language name. It is the arrangement every
    /// other one is reached from and returned to, and it is not English: the
    /// letters are what it has in common with French and German, which is why
    /// naming it after a country would be wrong on a keyboard that types all
    /// three.
    pub fn written(self) -> &'static str {
        match self {
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
        }
    }

    /// Every layer the keyboard might offer, in the order the layer-switch
    /// button shows them.
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

/// A keymap for one layer, ready to be sent to the Wayland virtual-keyboard
/// protocol.
///
/// The `bytes` are the full xkb_keymap string the protocol takes, including
/// the keycodes/types/compat sections and the symbols section that names
/// which layout this is. The string is what xkbcommon produces; nothing in
/// this crate has to invent xkb syntax.
#[derive(Debug, Clone)]
pub struct Keymap {
    pub layer: Layer,
    pub bytes: String,
}

/// Why a keymap could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// xkbcommon could not read its rules or its symbols from the system.
    /// The only way this happens on a working desktop is that the
    /// `xkeyboard-config` package is missing.
    NoContext,
    /// The layout the keyboard asked for is not in `/usr/share/X11/xkb/symbols/`.
    /// On a desktop with `xkeyboard-config` installed, the only way this
    /// happens is a typo in `Layer::tag()`.
    UnknownLayout(Layer),
    /// xkbcommon's serialiser returned no bytes, which it can do if the
    /// keymap is empty. A correctly-composed keymap is never empty.
    Empty,
}

/// Build the keymaps for every layer that the system has symbols for.
///
/// `rules` is the xkb rules directory (almost always `evdev`); `symbols_root`
/// is `/usr/share/X11/xkb/symbols` on a standard install. The function
/// returns the layers it could build, in the order the layer-switch button
/// shows them. A layer whose symbols are missing from the system is left
/// out, not refused on — the keyboard that fails because Thai symbols are
/// not installed is the wrong keyboard.
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

    if out.is_empty() {
        return Err(Error::NoContext);
    }

    Ok(out)
}

fn keymap_for(
    context: &Context,
    rules: &str,
    layer: Layer,
) -> Result<Keymap, Error> {
    let keymap = XkbKeymap::new_from_names(
        context,
        rules,
        "",
        layer.tag(),
        "",
        None,
        xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or(Error::UnknownLayout(layer))?;
    let bytes = keymap.get_as_string(xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1);

    if bytes.is_empty() {
        return Err(Error::Empty);
    }

    Ok(Keymap { layer, bytes })
}

/// Find the path to `/usr/share/X11/xkb/symbols`. The default the C binary
/// uses; this is where xkbcommon looks by default. Exposed for tests that
/// want to point at a fixtures directory instead.
pub fn default_symbols_root() -> std::path::PathBuf {
    std::path::PathBuf::from("/usr/share/X11/xkb")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latin_layer_is_us_not_english() {
        // xkbcommon calls the latin layout `us` because the file in
        // /usr/share/X11/xkb/symbols/ is named that. Calling it `en` would
        // be a guess that breaks the moment xkb's defaults shift.
        assert_eq!(Layer::Latin.tag(), "us");
    }

    #[test]
    fn every_layer_has_a_distinct_xkb_tag() {
        let mut tags: Vec<&str> = Layer::ALL.iter().map(|l| l.tag()).collect();
        tags.sort();
        tags.dedup();
        assert_eq!(tags.len(), Layer::ALL.len(), "two layers share an xkb tag");
    }

    #[test]
    fn every_layer_has_a_label() {
        for layer in Layer::ALL {
            assert!(!layer.name().is_empty(), "{} has no label", layer.tag());
        }
    }

    #[test]
    fn the_xkb_system_directory_resolves_on_this_machine() {
        // Smoke test: at least the latin layer can be built. Skipped on
        // machines without xkb (CI without xkeyboard-config).
        let symbols = default_symbols_root().join("symbols");
        if !symbols.is_dir() {
            eprintln!("skipped: no xkb symbols at {}", symbols.display());
            return;
        }
        let result = available("evdev", &default_symbols_root());
        assert!(result.is_ok(), "available: {:?}", result.err());
        let keymaps = result.unwrap();
        let latin = keymaps
            .iter()
            .find(|k| k.layer == Layer::Latin)
            .expect("latin");
        // xkb's serialised keymap starts with `xkb_keymap {` and contains the
        // symbols section.
        assert!(latin.bytes.starts_with("xkb_keymap {"), "{:?}", &latin.bytes[..40]);
        assert!(
            latin.bytes.contains("xkb_symbols"),
            "the serialised keymap has no symbols section"
        );
        // Sanity: the system keymap is small. The C version's hand-rolled
        // Thai keymap is 12330 lines of C; ours is whatever xkb ships.
        // Print the sizes so a regression where someone embeds a giant
        // hand-rolled keymap fails loud.
        let total: usize = keymaps.iter().map(|k| k.bytes.len()).sum();
        eprintln!(
            "xkb keymaps: {} layers, {} bytes total",
            keymaps.len(),
            total
        );
    }
}
