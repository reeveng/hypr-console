//! A list you step round, and what the step means at the ends of it.
//!
//! Seven crates had written this, and all seven had written it the same wrong
//! way: `position(...).unwrap_or(0)` to find where you are, then
//! `checked_rem(len).unwrap_or(0)` to take the step. Both halves hide the one
//! thing a ring has to answer for. The first says "not in the list" and "at the
//! start of the list" with the same number, so a name that has gone missing
//! reads as the first alphabet, the first wallpaper, the first speed. The second
//! is there only because the list might be empty, and a divisor of nothing is
//! the one way `%` fails -- so it asks, gets `None`, and answers `0`, which is a
//! position in a list that has no positions.
//!
//! Both are the same mistake: an empty list is not a list with something at the
//! front of it. So [`Ring::of`] is where that is decided, once, and it hands
//! back nothing for an empty list. What comes back holds a [`NonZeroUsize`],
//! which is the proof `%` wants, so every step after it is a plain remainder
//! that cannot fail -- EXPLICIT015 knows that divisor as a policy for this
//! reason.
//!
//! `where_it_is` is the other half and is kept separate on purpose. A caller
//! that cannot find what it was standing on has a decision to make -- begin
//! again at the front, stay where it was, say it is gone -- and the three are
//! not the same answer. This crate will not pick one.

use std::num::NonZeroUsize;

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ring(pub NonZeroUsize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Forward,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Between {
    pub from: usize,
    pub to: usize,
}

impl Ring {
    pub fn round(many: usize) -> Result<Option<Ring>, Never> {
        Ok(NonZeroUsize::new(many).map(Ring))
    }

    pub fn of<T>(every: &[T]) -> Result<Option<Ring>, Never> {
        Ring::round(every.len())
    }

    pub fn many(&self) -> Result<usize, Never> {
        Ok(self.0.get())
    }

    pub fn last(&self) -> Result<usize, Never> {
        Ok(self.0.get().saturating_sub(1))
    }

    pub fn at(&self, which: usize) -> Result<usize, Never> {
        Ok(which % self.0)
    }

    pub fn stepped(&self, from: usize, step: Step) -> Result<usize, Never> {
        let Ok(here) = self.at(from);

        Ok(match step {
            Step::Forward => {
                let Ok(round) = self.at(here.saturating_add(1));

                round
            }
            Step::Back => match here.checked_sub(1) {
                Some(before) => before,
                None => {
                    let Ok(last) = self.last();

                    last
                }
            },
        })
    }

    pub fn walked(&self, from: usize, by: isize) -> Result<usize, Never> {
        let Ok(away) = self.at(by.unsigned_abs());

        let forward = match by < 0 {
            true => self.0.get().saturating_sub(away),
            false => away,
        };

        self.at(from.saturating_add(forward))
    }

    pub fn steps(&self, between: Between) -> Result<usize, Never> {
        let Between { from, to } = between;

        let Ok(from) = self.at(from);
        let Ok(to) = self.at(to);

        self.at(self.0.get().saturating_add(to).saturating_sub(from))
    }
}

pub fn where_it_is<T: PartialEq>(every: &[T], wanted: &T) -> Result<Option<usize>, Never> {
    Ok(every.iter().position(|one| one == wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(many: usize) -> Ring {
        let Ok(round) = Ring::round(many);

        match round {
            Some(ring) => ring,
            None => Ring(NonZeroUsize::MIN),
        }
    }

    #[test]
    fn an_empty_list_is_not_a_ring_at_all() {
        let nothing: [u8; 0] = [];
        let Ok(round) = Ring::of(&nothing);

        assert_eq!(round, None);
    }

    #[test]
    fn a_step_forward_off_the_end_comes_back_to_the_front() {
        let Ok(went) = ring(3).stepped(2, Step::Forward);

        assert_eq!(went, 0);
    }

    #[test]
    fn a_step_back_off_the_front_comes_round_to_the_end() {
        let Ok(went) = ring(3).stepped(0, Step::Back);

        assert_eq!(went, 2);
    }

    #[test]
    fn a_position_past_the_end_is_wrapped_rather_than_refused() {
        let Ok(at) = ring(3).at(7);

        assert_eq!(at, 1);
    }

    #[test]
    fn walking_backwards_is_walking_the_long_way_round() {
        let Ok(went) = ring(5).walked(1, -3);

        assert_eq!(went, 3);
    }

    #[test]
    fn walking_further_than_the_ring_is_long_lands_where_the_remainder_does() {
        let Ok(went) = ring(4).walked(0, 9);

        assert_eq!(went, 1);
    }

    #[test]
    fn the_steps_between_two_places_are_counted_the_way_round_they_go() {
        let Ok(forward) = ring(4).steps(Between { from: 3, to: 1 });

        assert_eq!(forward, 2);
    }

    #[test]
    fn a_single_place_ring_never_goes_anywhere() {
        let Ok(forward) = ring(1).stepped(0, Step::Forward);
        let Ok(back) = ring(1).stepped(0, Step::Back);

        assert_eq!((forward, back), (0, 0));
    }

    #[test]
    fn what_is_not_in_the_list_is_nowhere_rather_than_at_the_front() {
        let held = ["one", "two"];
        let Ok(found) = where_it_is(&held, &"three");

        assert_eq!(found, None);
    }
}
