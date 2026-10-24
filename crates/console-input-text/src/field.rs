//! A line being typed, and where in it the next letter goes.
//!
//! The line used to be a string that only grew and shrank at its end, so the
//! arrows the keyboard draws reached a question and did nothing: a pair typed
//! as `()` could not be stepped back into, and a word misspelt at the front
//! was every letter after it typed again. The caret is counted in letters
//! rather than bytes, because a step is one letter whatever script it is in,
//! and the bytes are worked out only where the string is cut.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_draw_surface::Keysym;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
    text: String,
    caret: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    Edited,
    Moved,
    None,
}

impl TextField {
    pub fn holding(text: &str) -> Result<Self, Never> {
        let Ok(caret) = fitted(text.chars().count());

        Ok(TextField { text: text.to_string(), caret })
    }

    pub fn text(&self) -> Result<&str, Never> {
        Ok(&self.text)
    }

    pub fn caret(&self) -> Result<u32, Never> {
        Ok(self.caret)
    }

    pub fn letters(&self) -> Result<u32, Never> {
        fitted(self.text.chars().count())
    }

    pub fn before_caret(&self) -> Result<&str, Never> {
        let Ok(at) = self.byte(self.caret);
        let Ok(at) = index(at);

        Ok(match self.text.get(..at) {
            Some(before) => before,
            None => &self.text,
        })
    }

    pub fn typed(&mut self, key: Keysym) -> Result<Typed, Never> {
        let Ok(last) = self.letters();

        match key {
            Keysym::BackSpace => {
                return Ok(match self.caret.checked_sub(1) {
                    Some(before) => {
                        let Ok(at) = self.byte(before);
                        let Ok(at) = index(at);
                        let _gone = self.text.remove(at);

                        self.caret = before;

                        Typed::Edited
                    }
                    None => Typed::None,
                });
            }
            Keysym::Delete => {
                return Ok(match self.caret < last {
                    true => {
                        let Ok(at) = self.byte(self.caret);
                        let Ok(at) = index(at);
                        let _gone = self.text.remove(at);

                        Typed::Edited
                    }
                    false => Typed::None,
                });
            }
            Keysym::Left => return self.moved_to(self.caret.saturating_sub(1)),
            Keysym::Right => return self.moved_to(self.caret.saturating_add(1).min(last)),
            Keysym::Home => return self.moved_to(0),
            Keysym::End => return self.moved_to(last),
            _ => {},
        }

        let letter = match key.key_char() {
            Some(letter) => letter,
            None => return Ok(Typed::None),
        };

        match letter.is_control() {
            true => Ok(Typed::None),
            false => self.insert(letter),
        }
    }

    pub fn insert(&mut self, letter: char) -> Result<Typed, Never> {
        let Ok(at) = self.byte(self.caret);
        let Ok(at) = index(at);

        self.text.insert(at, letter);
        self.caret = self.caret.saturating_add(1);

        Ok(Typed::Edited)
    }

    pub fn moved_to(&mut self, caret: u32) -> Result<Typed, Never> {
        let Ok(last) = self.letters();
        let caret = caret.min(last);

        Ok(match caret == self.caret {
            true => Typed::None,
            false => {
                self.caret = caret;

                Typed::Moved
            }
        })
    }

    fn byte(&self, letters: u32) -> Result<u32, Never> {
        let Ok(letters) = index(letters);

        match self.text.char_indices().nth(letters) {
            Some((at, _letter)) => fitted(at),
            None => fitted(self.text.len()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(field: &mut TextField, keys: &[Keysym]) -> Result<Vec<Typed>, Never> {
        let mut told = Vec::new();

        for key in keys {
            let Ok(typed) = field.typed(*key);

            told.push(typed);
        }

        Ok(told)
    }

    #[test]
    fn a_letter_goes_into_the_line_and_a_backspace_takes_it_out_again() {
        let mut field = TextField::default();

        assert_eq!(typed(&mut field, &[Keysym::a, Keysym::b]), Ok(vec![Typed::Edited, Typed::Edited]));
        assert_eq!(field.text(), Ok("ab"));

        assert_eq!(field.typed(Keysym::BackSpace), Ok(Typed::Edited));
        assert_eq!(field.text(), Ok("a"));

        assert_eq!(typed(&mut field, &[Keysym::Return, Keysym::Down]), Ok(vec![Typed::None, Typed::None]));
        assert_eq!(field.text(), Ok("a"), "a key that means something to the panel is not a letter");

        assert_eq!(TextField::default().typed(Keysym::BackSpace), Ok(Typed::None), "nothing to take out of an empty line");
    }

    #[test]
    fn a_pair_is_typed_and_then_stepped_back_into() {
        let mut field = TextField::default();
        let Ok(_typed) = typed(&mut field, &[Keysym::parenleft, Keysym::parenright, Keysym::Left, Keysym::x]);

        assert_eq!(field.text(), Ok("(x)"));
        assert_eq!(field.before_caret(), Ok("(x"));
    }

    #[test]
    fn the_caret_stops_at_either_end_of_the_line() {
        let Ok(mut field) = TextField::holding("ab");

        assert_eq!(field.typed(Keysym::Right), Ok(Typed::None), "a line held starts with the caret after it");
        assert_eq!(field.typed(Keysym::Home), Ok(Typed::Moved));
        assert_eq!(field.typed(Keysym::Left), Ok(Typed::None));
        assert_eq!(field.typed(Keysym::BackSpace), Ok(Typed::None), "nothing is before the front");
        assert_eq!(field.typed(Keysym::Delete), Ok(Typed::Edited));
        assert_eq!(field.text(), Ok("b"));
        assert_eq!(field.typed(Keysym::End), Ok(Typed::Moved));
        assert_eq!(field.typed(Keysym::Delete), Ok(Typed::None), "nothing is after the end");
    }

    #[test]
    fn a_step_is_a_letter_and_not_a_byte() {
        let Ok(mut field) = TextField::holding("café");
        let Ok(_typed) = typed(&mut field, &[Keysym::Left, Keysym::BackSpace]);

        assert_eq!(field.text(), Ok("caé"));
        assert_eq!(field.before_caret(), Ok("ca"));
    }
}
