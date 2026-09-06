//! What a panel crate hands over, so that something else can draw it.
//!
//! Every panel's `main` was the same five lines with a different crate in the
//! middle: take the screen, build the state, hand a closure to `panel::show`,
//! and shut the state down when the loop ends. That shape is why one program
//! can hold them all -- what a panel is, is a door it comes out of and a card
//! to draw, and neither of those needs to be a process.
//!
//! The two are asked for separately because they cost differently. A door is a
//! name and a rule about opening it twice, worked out from the arguments and
//! nothing else, and it is wanted before anything is drawn, by whoever is about
//! to take the screen. A card reads the machine -- the applications, the songs,
//! the folder somebody asked for -- and it is wanted only where the drawing
//! happens. Asking for both in one call would make the program that only holds
//! the screen do the work of the program that draws.
//!
//! `done` is what the old `main` ran after the loop ended, which was always
//! shutting down the actor that held the state. It is here rather than left to
//! a `Drop` because a panel that is closed has to be a panel whose thread has
//! joined before the next one opens, and a value dropped at the end of a
//! process is a thing that never had to be said out loud.

use console_never::Never;

use crate::chooser::Again;
use crate::panel::Build;

pub type Done = Box<dyn FnOnce() -> Result<(), Never>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Door {
    pub name: String,
    pub again: Again,
}

impl Door {
    pub fn new(name: &str, again: Again) -> Result<Self, Never> {
        Ok(Door { name: name.to_string(), again })
    }

    pub fn closing(name: &str) -> Result<Self, Never> {
        Door::new(name, Again::Closes)
    }
}

pub struct Card {
    pub build: Build,
    pub column: i32,
    pub start: Option<String>,
    pub done: Done,
}

impl Card {
    pub fn new(build: Build) -> Result<Self, Never> {
        Ok(Card { build, column: 0, start: None, done: Box::new(|| Ok(())) })
    }

    pub fn under(mut self, column: i32) -> Result<Self, Never> {
        self.column = column;

        Ok(self)
    }

    pub fn opening_at(mut self, tab: Option<&str>) -> Result<Self, Never> {
        self.start = tab.map(str::to_string);

        Ok(self)
    }

    pub fn shutting(mut self, done: Done) -> Result<Self, Never> {
        self.done = done;

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_door_closes_unless_it_was_asked_not_to() {
        let Ok(door) = Door::closing("menu");

        assert_eq!(door.again, Again::Closes, "a bar icon tapped twice has to put its panel away");
    }

    #[test]
    fn a_card_says_nothing_about_a_tab_until_it_is_told_one() {
        let Ok(card) = Card::new(std::sync::Arc::new(Vec::new));

        assert_eq!(card.start, None, "opened with nothing named, a panel opens where it was left");
        assert_eq!(card.column, 0);

        let Ok(card) = card.opening_at(Some("Sound"));

        assert_eq!(card.start.as_deref(), Some("Sound"));
    }
}
