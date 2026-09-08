//! What a press comes to, and how it leaves the keyboard.
//!
//! Two things live here because they are the same thing from either end. The
//! outward half is `zwp_virtual_keyboard_v1`: a keymap handed to the
//! compositor once, and key and modifier events sent against it afterwards.
//! The inward half is what a key press means -- shift held until the next
//! letter, a layer key that walks, a long press that opens the accents, a
//! character that is in no keymap at all.
//!
//! ## Why a keymap of our own rather than the system's
//!
//! A uinput keyboard emits a keycode and the compositor decides what it
//! produces, from whatever layout the compositor was configured with. This
//! device is configured `us`, so a uinput keyboard here can type what a US
//! keyboard types and nothing else, and the Thai layer would be forty keys
//! that all produce latin letters.
//!
//! The virtual-keyboard protocol is the one that lets a client say what its
//! own keys mean. So the alphabet is uploaded with the layer: switching to
//! Thai uploads the Thai keymap, and the key under the thumb marked ก sends
//! the keycode that is ก *in that keymap*. `keymap` composes them out of the
//! system's own xkb symbols, so what is typed is what `/usr/share/X11/xkb`
//! says the alphabet is, and adding a language installs no data here.


use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::os::fd::AsFd;
use std::time::Instant;

use wayland_client::QueueHandle;
use wayland_client::protocol::wl_seat;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1,
};

use crate::keymap::{Keymap, Layer};
use crate::layout::{Drops, Kind, Layout, Which, mods, of};
use crate::shared_memory::keymap_file;
use crate::surface::Board;

const SPARE: u32 = 127;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    Still,
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    On,
    Back,
}

pub fn after(walk: &[Which], alphabet: Which) -> Result<Option<Which>, Never> {
    let languages: Vec<Which> = walk
        .iter()
        .copied()
        .filter(|which| {
            let Ok(of) = of(*which);

            of.primary
        })
        .collect();

    match languages.len() < 2 {
        true => return Ok(None),
        false => {},
    }

    let here = languages.iter().position(|w| *w == alphabet).unwrap_or(0);
    Ok(languages.get(here.saturating_add(1).checked_rem(languages.len()).unwrap_or(0)).copied())
}

pub fn symbols(walk: &[Which]) -> Result<Option<Which>, Never> {
    Ok(walk.iter().copied().find(|which| {
        let Ok(of) = of(*which);

        !of.primary
    }))
}

pub struct Typist {
    keys: zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
    alphabets: Vec<Keymap>,
    worn: Option<Layer>,
    since: Instant,
    pub held: u8,
    pub composing: bool,
    pub showing: Which,
    pub walk: Vec<Which>,
    pub step: usize,
    pub last_alphabet: Which,
}

impl Typist {
    pub fn new(
        manager: &zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
        seat: &wl_seat::WlSeat,
        hand: &QueueHandle<Board>,
        alphabets: Vec<Keymap>,
        walk: Vec<Which>,
        opening: Option<Which>,
    ) -> Result<Typist, Never> {
        let keys = manager.create_virtual_keyboard(seat, hand, ());
        let showing = opening
            .filter(|which| walk.contains(which))
            .or_else(|| walk.first().copied())
            .unwrap_or(Which::Full);
        let step = walk.iter().position(|which| *which == showing).unwrap_or(0);
        let mut typist = Typist {
            keys,
            alphabets,
            worn: None,
            since: Instant::now(),
            held: mods::NONE,
            composing: false,
            showing,
            walk,
            step,
            last_alphabet: showing,
        };
        let Ok(of) = of(showing);
        let Ok(()) = typist.wear(of.alphabet);
        Ok(typist)
    }

    fn when(&self) -> Result<u32, Never> {
        fitted(self.since.elapsed().as_millis())
    }

    pub fn wear(&mut self, alphabet: Layer) -> Result<(), Never> {
        match self.worn == Some(alphabet) {
            true => return Ok(()),
            false => {},
        }

        let keymap = match self.alphabets.iter().find(|k| k.layer == alphabet) {
            Some(keymap) => keymap,
            None => return Ok(()),
        };

        let (file, long) = match keymap_file(&keymap.bytes) {
            Ok((file, long)) => (file, long),
            Err(_fault) => return Ok(()),
        };

        let Ok(long) = fitted(long);

        self.keys.keymap(1, file.as_fd(), long);
        self.worn = Some(alphabet);

        Ok(())
    }

    pub fn tap(&mut self, code: u32) -> Result<(), Never> {
        let Ok(at) = self.when();

        self.keys.key(at, code, 1);

        let Ok(then) = self.when();

        self.keys.key(then, code, 0);

        Ok(())
    }

    pub fn holding(&mut self, held: u8) -> Result<(), Never> {
        self.keys.modifiers(u32::from(held), 0, 0, 0);

        Ok(())
    }

    pub fn send(&mut self, character: u32) -> Result<(), Never> {
        let one = format!(
            "xkb_keymap {{\n\
             xkb_keycodes {{ minimum = 8; maximum = 255; <SPR> = {}; }};\n\
             xkb_types {{ include \"complete\" }};\n\
             xkb_compat {{ include \"complete\" }};\n\
             xkb_symbols {{ key <SPR> {{ [ U{:04X} ] }}; }};\n\
             }};",
            SPARE.saturating_add(8),
            character
        );

        let (file, long) = match keymap_file(&one) {
            Ok((file, long)) => (file, long),
            Err(_fault) => return Ok(()),
        };

        let Ok(long) = fitted(long);

        self.keys.keymap(1, file.as_fd(), long);
        self.keys.modifiers(0, 0, 0, 0);
        let Ok(()) = self.tap(SPARE);
        self.worn = None;
        let Ok(of) = of(self.showing);
        let Ok(()) = self.wear(of.alphabet);
        let held = self.held;
        let Ok(()) = self.holding(held);

        Ok(())
    }

    pub fn layout(&self) -> Result<&'static Layout, Never> {
        of(self.showing)
    }

    pub fn go(&mut self, which: Which) -> Result<(), Never> {
        self.showing = which;

        let Ok(of) = of(which);

        match of.primary {
            true => {
                self.last_alphabet = which;

                match self.walk.iter().position(|w| *w == which) {
                    Some(step) => self.step = step,
                    None => {},
                }
            }
            false => {},
        }

        let Ok(()) = self.wear(of.alphabet);

        Ok(())
    }

    pub fn next(&mut self, way: Way) -> Result<(), Never> {
        match self.walk.is_empty() {
            true => return Ok(()),
            false => {},
        }

        let held_down = self.held & (mods::CTRL | mods::ALT | mods::ALTGR) != 0 || self.composing;
        let backward = way == Way::Back || self.held & (mods::SHIFT | mods::CAPS) != 0;

        match (held_down, backward) {
            (true, _) => {
                self.held = mods::NONE;
                self.composing = false;
                self.step = 0;
            }
            (false, true) => {
                self.step = match self.step {
                    0 => self.walk.len().saturating_sub(1),
                    step => step.saturating_sub(1),
                };
            }
            (false, false) => {
                self.step = self.step.saturating_add(1).checked_rem(self.walk.len()).unwrap_or(0);
            }
        }

        let going = match self.walk.get(self.step).copied() {
            Some(going) => going,
            None => return Ok(()),
        };

        let Ok(()) = self.go(going);

        Ok(())
    }

    pub fn pressed(&mut self, kind: Kind, force: u8, reset: Drops) -> Result<After, Never> {
        match self.composing {
            true => {
                self.composing = false;

                match kind {
                    Kind::Code { held: Some(shelf), .. } => {
                        let Ok(()) = self.go(shelf);
                        return Ok(After::Draw);
                    }
                    Kind::Code { held: None, .. }
                    | Kind::Pad
                    | Kind::EndRow
                    | Kind::Mod(_)
                    | Kind::Copy { .. }
                    | Kind::Layout(_)
                    | Kind::Back
                    | Kind::Next
                    | Kind::Language
                    | Kind::Symbols
                    | Kind::Compose => {},
                }
            }
            false => {},
        }

        Ok(match kind {
            Kind::Code { code, .. } => {
                let held = self.held | force;
                let Ok(()) = self.holding(held);
                let Ok(()) = self.tap(code);

                match reset == Drops::Held || self.held != mods::NONE {
                    true => {
                        self.held &= mods::CAPS;
                        let Ok(()) = self.holding(self.held);
                        After::Draw
                    },
                    false => {
                        let Ok(()) = self.holding(self.held);
                        After::Still
                    },
                }
            },
            Kind::Copy { code, shifted } => {
                let which = match self.held & (mods::SHIFT | mods::CAPS) != 0 {
                    true => shifted,
                    false => code,
                };
                let Ok(()) = self.send(which);
                let back = self.last_alphabet;
                let Ok(()) = self.go(back);
                After::Draw
            },
            Kind::Mod(bit) => {
                self.held ^= bit;
                let held = self.held;
                let Ok(()) = self.holding(held);
                After::Draw
            },
            Kind::Layout(which) => {
                let Ok(()) = self.go(which);
                After::Draw
            },
            Kind::Language => {
                let Ok(next) = self.next_language();

                match next {
                    Some(next) => {
                        let Ok(()) = self.go(next);
                    },
                    None => {},
                }

                After::Draw
            },
            Kind::Back => {
                let back = self.last_alphabet;
                let Ok(()) = self.go(back);
                After::Draw
            },
            Kind::Next => {
                let Ok(()) = self.next(Way::On);
                After::Draw
            },
            Kind::Symbols => {
                let Ok(shelf) = symbols(&self.walk);

                match shelf {
                    Some(shelf) => {
                        let Ok(()) = self.go(shelf);
                        After::Draw
                    },
                    None => After::Still,
                }
            },
            Kind::Compose => {
                self.composing = !self.composing;
                After::Draw
            },
            Kind::Pad | Kind::EndRow => After::Still,
        })
    }

    pub fn next_language(&self) -> Result<Option<Which>, Never> {
        after(&self.walk, self.last_alphabet)
    }

    pub fn held_down(&mut self, kind: Kind) -> Result<After, Never> {
        Ok(match kind {
            Kind::Code { held: Some(shelf), .. } => {
                let Ok(()) = self.go(shelf);
                After::Draw
            },
            Kind::Code { held: None, .. }
            | Kind::Pad
            | Kind::Mod(_)
            | Kind::Copy { .. }
            | Kind::Layout(_)
            | Kind::Back
            | Kind::Next
            | Kind::Language
            | Kind::Symbols
            | Kind::Compose
            | Kind::EndRow => After::Still,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::{Kind, Layout, Which, mods};

    fn named(name: &str) -> Option<Which> {
        let Ok(named) = crate::layout::named(name);

        named
    }

    fn of(which: Which) -> &'static Layout {
        let Ok(of) = crate::layout::of(which);

        of
    }

    #[test]
    fn an_accent_shelf_goes_back_to_the_alphabet_not_to_the_shelf() {
        let a = named("composea").expect("the a shelf");
        assert!(!of(a).primary, "an accent shelf is not somewhere you type");
        let full = named("full").expect("full");
        assert!(of(full).primary, "the letters are");
    }

    #[test]
    fn the_language_key_walks_the_alphabets_and_skips_the_shelf() {
        let landscape = named("landscape").expect("landscape");
        let thai = named("thai").expect("thai");
        let shelf = named("landscapespecial").expect("landscapespecial");
        let walk = [landscape, thai, shelf];
        assert_eq!(super::after(&walk, landscape), Ok(Some(thai)), "latin goes to Thai");
        assert_eq!(super::after(&walk, thai), Ok(Some(landscape)), "and Thai comes back round");
        assert!(!of(shelf).primary, "the ?123 shelf is not a language");
    }

    #[test]
    fn the_numbers_key_finds_the_shelf_of_whichever_walk_it_is_in() {
        let thai = named("thai").expect("thai");
        let landscape = named("landscape").expect("landscape");
        let wide_shelf = named("landscapespecial").expect("landscapespecial");
        let full = named("full").expect("full");
        let tall_shelf = named("special").expect("special");

        assert_eq!(super::symbols(&[landscape, thai, wide_shelf]), Ok(Some(wide_shelf)));
        assert_eq!(super::symbols(&[full, thai, tall_shelf]), Ok(Some(tall_shelf)));
    }

    #[test]
    fn a_walk_with_no_shelf_has_no_numbers_to_go_to() {
        let full = named("full").expect("full");
        let thai = named("thai").expect("thai");
        assert_eq!(super::symbols(&[full, thai]), Ok(None));
    }

    #[test]
    fn one_alphabet_leaves_the_language_key_with_nothing_to_say() {
        let landscape = named("landscape").expect("landscape");
        let shelf = named("landscapespecial").expect("landscapespecial");
        assert_eq!(super::after(&[landscape, shelf], landscape), Ok(None));
    }

    #[test]
    fn a_third_language_needs_no_new_key() {
        let landscape = named("landscape").expect("landscape");
        let thai = named("thai").expect("thai");
        let russian = named("cyrillic").expect("cyrillic");
        let walk = [landscape, thai, russian];
        assert_eq!(super::after(&walk, landscape), Ok(Some(thai)));
        assert_eq!(super::after(&walk, thai), Ok(Some(russian)));
        assert_eq!(super::after(&walk, russian), Ok(Some(landscape)));
    }

    #[test]
    fn the_walk_this_desktop_uses_is_alphabets_and_a_shelf_of_symbols() {
        for name in ["full", "thai"] {
            let which = named(name).expect(name);
            assert!(of(which).primary, "{name} is not an alphabet");
        }
    }

    #[test]
    fn the_letters_that_have_accents_carry_them_and_the_rest_do_not() {
        let full = of(named("full").expect("full"));
        let a = full
            .keys
            .iter()
            .find(|key| key.label == "a")
            .expect("the a key");
        assert!(
            matches!(a.kind, Kind::Code { held: Some(Which::ComposeA), .. }),
            "a long press on a does not reach the accents"
        );
        let space = full
            .keys
            .iter()
            .find(|key| key.label == "space" || key.width > 3.0)
            .expect("the space bar");
        assert!(matches!(space.kind, Kind::Code { held: None, .. }), "the space bar carries a shelf");
    }

    #[test]
    fn shift_and_caps_lock_are_different_bits() {
        assert_ne!(mods::SHIFT, mods::CAPS);
        assert_eq!(mods::SHIFT & mods::CAPS, 0);
    }
}
