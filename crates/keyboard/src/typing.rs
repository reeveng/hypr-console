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


use console_number::fitted;
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

/// The keycode a made-up character is sent on.
///
/// Compose, which is a key no arrangement here draws and nothing else sends,
/// so a keymap that binds it to one character binds nothing a person could
/// press by accident.
const SPARE: u32 = 127;

/// What the caller has to do about a press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    /// Nothing changed on the screen.
    Still,
    /// Draw it again: a modifier went down, or the arrangement changed.
    Draw,
}

/// Which way along the walk of arrangements a step goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// On to the next one, which is what the layer key does.
    On,
    /// Back to the one before, which is what it does with shift held.
    Back,
}

/// The language after this one, going round.
///
/// The walk holds both alphabets and shelves -- `landscape`, `thai`,
/// `landscapespecial` -- and only the alphabets are languages: the `?123`
/// shelf has a key of its own and is not somewhere the language button should
/// ever land you. `None` when the machine was given one alphabet, which is
/// when the key has nothing to do and nothing to say.
///
/// `alphabet` is where the *letters* were, not where the keyboard is. Pressed
/// from the symbols shelf the key still means "the language after the one I
/// was typing in", and the shelf is not in this list to be found.
///
/// Free of [`Typist`] because a `Typist` owns a compositor object and this is
/// the half with an opinion: which alphabet comes next is arithmetic, and it
/// is the half that can be wrong in a way somebody notices.
pub fn after(walk: &[Which], alphabet: Which) -> Option<Which> {
    let languages: Vec<Which> = walk.iter().copied().filter(|w| of(*w).primary).collect();

    if languages.len() < 2 {
        return None;
    }

    let here = languages.iter().position(|w| *w == alphabet).unwrap_or(0);
    Some(languages[(here + 1) % languages.len()])
}

/// The shelf of numbers and symbols in this walk, if it was given one.
///
/// The first arrangement in the walk that is not an alphabet. A walk holds
/// both -- `landscape`, `thai`, `landscapespecial` -- and a machine is free to
/// be given more than one shelf, in which case the first is the one the key
/// beside the language key goes to: it is the numbers, and the rest are
/// reached from there.
///
/// Free of [`Typist`] for the reason [`after`] is: this is the half with an
/// opinion, and it can be wrong in a way somebody notices.
pub fn symbols(walk: &[Which]) -> Option<Which> {
    walk.iter().copied().find(|w| !of(*w).primary)
}

/// The keyboard's hands: what it is holding, where it is, and the way out.
pub struct Typist {
    /// The compositor's end of the typing.
    keys: zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
    /// Every alphabet the system has symbols for, composed once at startup.
    /// Composing one costs a few milliseconds and there are ten of them; doing
    /// it on the layer key would be a layer key with a pause in it.
    alphabets: Vec<Keymap>,
    /// Which alphabet the compositor is holding, so it is not sent twice.
    worn: Option<Layer>,
    /// When the keyboard started, for the timestamps the protocol wants.
    since: Instant,
    /// The modifiers being held.
    pub held: u8,
    /// Whether the next press opens the layout its key carries instead of
    /// sending itself.
    pub composing: bool,
    /// The arrangement on the screen.
    pub showing: Which,
    /// The arrangements the layer key walks, in order.
    pub walk: Vec<Which>,
    /// Where in the walk `showing` is, when it is in the walk at all.
    pub step: usize,
    /// The last arrangement somebody was actually typing in, which is where
    /// the way back from a shelf of accents goes.
    pub last_alphabet: Which,
}

impl Typist {
    /// Take the seat's virtual keyboard, and put the first alphabet on it.
    pub fn new(
        manager: &zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
        seat: &wl_seat::WlSeat,
        hand: &QueueHandle<Board>,
        alphabets: Vec<Keymap>,
        walk: Vec<Which>,
    ) -> Typist {
        let keys = manager.create_virtual_keyboard(seat, hand, ());
        let showing = walk.first().copied().unwrap_or(Which::Full);
        let mut typist = Typist {
            keys,
            alphabets,
            worn: None,
            since: Instant::now(),
            held: mods::NONE,
            composing: false,
            showing,
            walk,
            step: 0,
            last_alphabet: showing,
        };
        typist.wear(of(showing).alphabet);
        typist
    }

    /// Milliseconds since the keyboard started, which is all the protocol
    /// wants of a timestamp: it has to rise, and it has to be its own.
    fn when(&self) -> u32 {
        fitted(self.since.elapsed().as_millis())
    }

    /// Hand the compositor the alphabet, if it is not already holding it.
    pub fn wear(&mut self, alphabet: Layer) {
        if self.worn == Some(alphabet) {
            return;
        }

        let Some(keymap) = self.alphabets.iter().find(|k| k.layer == alphabet) else {
            // An alphabet the system has no symbols for. The keys still draw
            // and still send their codes; what they produce is whatever the
            // alphabet already up says they are. Better than a keyboard that
            // will not come up because a language is not installed.
            return;
        };

        let Ok((file, long)) = keymap_file(&keymap.bytes) else { return };

        self.keys.keymap(1, file.as_fd(), fitted(long));
        self.worn = Some(alphabet);
    }

    /// Send one key down and up.
    pub fn tap(&mut self, code: u32) {
        let at = self.when();
        self.keys.key(at, code, 1);
        self.keys.key(self.when(), code, 0);
    }

    /// Tell the compositor what is being held.
    pub fn holding(&mut self, held: u8) {
        self.keys.modifiers(u32::from(held), 0, 0, 0);
    }

    /// Send a character that no alphabet here has a key for.
    ///
    /// The accents are these: nothing in a latin keymap produces `ā`, so a
    /// keymap that does is written, used for one press, and the alphabet put
    /// back. It is a round trip through the compositor per character, which is
    /// why it is for the accent shelves and not for typing.
    pub fn send(&mut self, character: u32) {
        let one = format!(
            "xkb_keymap {{\n\
             xkb_keycodes {{ minimum = 8; maximum = 255; <SPR> = {}; }};\n\
             xkb_types {{ include \"complete\" }};\n\
             xkb_compat {{ include \"complete\" }};\n\
             xkb_symbols {{ key <SPR> {{ [ U{:04X} ] }}; }};\n\
             }};",
            SPARE + 8,
            character
        );

        let Ok((file, long)) = keymap_file(&one) else { return };

        self.keys.keymap(1, file.as_fd(), fitted(long));
        // No modifiers under a keymap that has one key: shift here would
        // produce whatever the shift level of that one key is, which is
        // nothing.
        self.keys.modifiers(0, 0, 0, 0);
        self.tap(SPARE);
        self.worn = None;
        let alphabet = of(self.showing).alphabet;
        self.wear(alphabet);
        let held = self.held;
        self.holding(held);
    }

    /// The arrangement on the screen.
    pub fn layout(&self) -> &'static Layout {
        of(self.showing)
    }

    /// Go to an arrangement, uploading its alphabet if it is a different one.
    pub fn go(&mut self, which: Which) {
        self.showing = which;

        if of(which).primary {
            self.last_alphabet = which;

            if let Some(step) = self.walk.iter().position(|w| *w == which) {
                self.step = step;
            }
        }

        let alphabet = of(which).alphabet;
        self.wear(alphabet);
    }

    /// The next arrangement in the walk, which is what the layer key does.
    ///
    /// Backwards with shift held, and back to the first with a modifier that
    /// has no business being on a layer key at all -- which is the C
    /// keyboard's way out of a layer somebody landed on and cannot read.
    pub fn next(&mut self, way: Way) {
        if self.walk.is_empty() {
            return;
        }

        if self.held & (mods::CTRL | mods::ALT | mods::ALTGR) != 0 || self.composing {
            self.held = mods::NONE;
            self.composing = false;
            self.step = 0;
        } else if way == Way::Back || self.held & (mods::SHIFT | mods::CAPS) != 0 {
            self.step = match self.step {
                0 => self.walk.len() - 1,
                step => step - 1,
            };
        } else {
            self.step = (self.step + 1) % self.walk.len();
        }

        let going = self.walk[self.step];
        self.go(going);
    }

    /// A key was pressed. Do what it says.
    pub fn pressed(&mut self, kind: Kind, force: u8, reset: Drops) -> After {
        // A key pressed while compose is armed opens what it carries instead
        // of sending itself, which is the second way to an accent: the first
        // is holding the key down.
        if self.composing {
            self.composing = false;

            if let Kind::Code { held: Some(shelf), .. } = kind {
                self.go(shelf);
                return After::Draw;
            }
        }

        match kind {
            Kind::Code { code, .. } => {
                let held = self.held | force;
                self.holding(held);
                self.tap(code);

                match reset == Drops::Held || self.held != mods::NONE {
                    // Shift is held for one letter, the way a thumb expects.
                    // Caps lock is the one that stays, and it is not dropped
                    // here: it is a key that was pressed twice.
                    true => {
                        self.held &= mods::CAPS;
                        self.holding(self.held);
                        After::Draw
                    },
                    false => {
                        self.holding(self.held);
                        After::Still
                    },
                }
            },
            Kind::Copy { code, shifted } => {
                let which = match self.held & (mods::SHIFT | mods::CAPS) != 0 {
                    true => shifted,
                    false => code,
                };
                self.send(which);
                // An accent shelf is somewhere you went for one character.
                let back = self.last_alphabet;
                self.go(back);
                After::Draw
            },
            Kind::Mod(bit) => {
                self.held ^= bit;
                let held = self.held;
                self.holding(held);
                After::Draw
            },
            Kind::Layout(which) => {
                self.go(which);
                After::Draw
            },
            Kind::Language => {
                if let Some(next) = self.next_language() {
                    self.go(next);
                }

                After::Draw
            },
            Kind::Back => {
                let back = self.last_alphabet;
                self.go(back);
                After::Draw
            },
            Kind::Next => {
                self.next(Way::On);
                After::Draw
            },
            Kind::Symbols => {
                match symbols(&self.walk) {
                    Some(shelf) => {
                        self.go(shelf);
                        After::Draw
                    },
                    // A walk of alphabets alone has nowhere to send this, and
                    // a keyboard redrawn to look exactly as it did is a key
                    // that looks broken twice.
                    None => After::Still,
                }
            },
            Kind::Compose => {
                self.composing = !self.composing;
                After::Draw
            },
            Kind::Pad | Kind::EndRow => After::Still,
        }
    }

    /// A key was held down rather than tapped: open what it carries.
    /// The language the language key is about to go to, if there is another
    /// one.
    pub fn next_language(&self) -> Option<Which> {
        after(&self.walk, self.last_alphabet)
    }

    pub fn held_down(&mut self, kind: Kind) -> After {
        match kind {
            Kind::Code { held: Some(shelf), .. } => {
                self.go(shelf);
                After::Draw
            },
            _ => After::Still,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::{Kind, Which, mods, named, of};

    /// The walk goes round, and the way back from an accent shelf is to the
    /// alphabet somebody was typing in rather than to wherever they came from
    /// -- a shelf reached from a shelf would otherwise strand them.
    ///
    /// The protocol half cannot be tested without a compositor, so what is
    /// tested is the half that decides where the keyboard goes, which is the
    /// half that has an opinion.
    #[test]
    fn an_accent_shelf_goes_back_to_the_alphabet_not_to_the_shelf() {
        let a = named("composea").expect("the a shelf");
        assert!(!of(a).primary, "an accent shelf is not somewhere you type");
        let full = named("full").expect("full");
        assert!(of(full).primary, "the letters are");
    }

    /// The language key steps through the alphabets and leaves the symbols
    /// alone, and it comes back round rather than stopping at the end.
    #[test]
    fn the_language_key_walks_the_alphabets_and_skips_the_shelf() {
        let landscape = named("landscape").expect("landscape");
        let thai = named("thai").expect("thai");
        let shelf = named("landscapespecial").expect("landscapespecial");
        let walk = [landscape, thai, shelf];
        assert_eq!(super::after(&walk, landscape), Some(thai), "latin goes to Thai");
        assert_eq!(super::after(&walk, thai), Some(landscape), "and Thai comes back round");
        assert!(!of(shelf).primary, "the ?123 shelf is not a language");
    }

    /// The numbers key goes to this walk's shelf, which is the whole reason it
    /// is not a shelf named in the table: Thai is in both walks, and either
    /// name would have been the wrong one half the time.
    #[test]
    fn the_numbers_key_finds_the_shelf_of_whichever_walk_it_is_in() {
        let thai = named("thai").expect("thai");
        let landscape = named("landscape").expect("landscape");
        let wide_shelf = named("landscapespecial").expect("landscapespecial");
        let full = named("full").expect("full");
        let tall_shelf = named("special").expect("special");

        assert_eq!(super::symbols(&[landscape, thai, wide_shelf]), Some(wide_shelf));
        assert_eq!(super::symbols(&[full, thai, tall_shelf]), Some(tall_shelf));
    }

    /// And a walk of alphabets alone has nowhere to send it. The key is drawn
    /// -- the table is the same table -- and pressing it leaves the keyboard
    /// where it is rather than redrawing it to look exactly as it did.
    #[test]
    fn a_walk_with_no_shelf_has_no_numbers_to_go_to() {
        let full = named("full").expect("full");
        let thai = named("thai").expect("thai");
        assert_eq!(super::symbols(&[full, thai]), None);
    }

    /// A machine given one alphabet has a language key with nowhere to go, and
    /// it says so rather than drawing a key that goes back to where it is.
    #[test]
    fn one_alphabet_leaves_the_language_key_with_nothing_to_say() {
        let landscape = named("landscape").expect("landscape");
        let shelf = named("landscapespecial").expect("landscapespecial");
        assert_eq!(super::after(&[landscape, shelf], landscape), None);
    }

    /// A third language is a word in the walk and nothing else -- no new key,
    /// no new table, which is the whole point of one key rather than three.
    #[test]
    fn a_third_language_needs_no_new_key() {
        let landscape = named("landscape").expect("landscape");
        let thai = named("thai").expect("thai");
        let russian = named("cyrillic").expect("cyrillic");
        let walk = [landscape, thai, russian];
        assert_eq!(super::after(&walk, landscape), Some(thai));
        assert_eq!(super::after(&walk, thai), Some(russian));
        assert_eq!(super::after(&walk, russian), Some(landscape));
    }

    /// Every arrangement the walk is made of has to be one somebody can type
    /// in, or the layer key is a key that goes somewhere and stops.
    #[test]
    fn the_walk_this_desktop_uses_is_alphabets_and_a_shelf_of_symbols() {
        for name in ["full", "thai"] {
            let which = named(name).expect(name);
            assert!(of(which).primary, "{name} is not an alphabet");
        }
    }

    /// A long press opens what the key carries, and most keys carry nothing.
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

    /// Shift is one letter and caps lock is not, which is the difference a
    /// thumb notices.
    #[test]
    fn shift_and_caps_lock_are_different_bits() {
        assert_ne!(mods::SHIFT, mods::CAPS);
        assert_eq!(mods::SHIFT & mods::CAPS, 0);
    }
}
