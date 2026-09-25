//! What the panel draws that is not a row, and what each part is called.
//!
//! The names are how a part is styled, so they are written once and read by
//! both the drawing and the stylesheet. A part named in one and not the other
//! is a part that is there and cannot be seen, or one that is styled and does
//! not exist.
//!
//! `HIDDEN` is what a letter of a password looks like, which the toolkit used
//! to decide for itself inside an entry that was told to keep its text: a
//! surface of our own draws the text it was given, so what to draw instead of
//! the letters is said here with the rest of the marks.
//!
//! `CANCEL` is the one word here that a person reads rather than a stylesheet.
//! It used to be a pair, `Yes` and `No`, and they said nothing: a card asking
//! whether to bring a machine to what it had been sent drew the same two words
//! as one asking whether to throw a photograph away, so the button under the
//! highlight could not be read on its own. Every other answer names what it
//! does, and the one that undoes the asking is called what it is.

pub const SHUT: &str = "\u{d7}";

pub const BEFORE: &str = "\u{2039}";
pub const AFTER: &str = "\u{203a}";

pub const INTO: &str = AFTER;

pub const LESS: &str = "\u{2212}";
pub const MORE: &str = "+";

pub const ELSE: &str = "\u{2026}";

pub const HIDDEN: &str = "\u{25cf}";

pub const CANCEL: &str = "Cancel";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_marks_are_the_same_mark() {
        let mut every = [BEFORE, AFTER, SHUT, LESS, MORE, ELSE, HIDDEN].to_vec();
        every.sort_unstable();
        let mut once = every.clone();
        once.dedup();

        assert_eq!(once, every, "two things a hand is offered drawn with one glyph");
    }
}
