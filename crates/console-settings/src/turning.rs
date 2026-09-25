//! Which way up the screen stands.
//!
//! A panel is mounted the way the machine it is screwed into wanted it, and
//! this desktop's is mounted sideways: the transform in the compositor's file
//! is the quarter turn that puts a portrait panel the right way up for
//! someone holding it. So a turn here is a quarter either side of *that*
//! rather than a number of degrees from nothing, which is what lets the same
//! three words mean the same three things on a panel that was mounted upright.
//!
//! Nothing drawn has to be told. Every surface in this repository takes
//! fractions of the screen it is given, and the density is a rung divided into
//! the panel's own width -- so the same eval that turns the screen hands the
//! scale for the width it will have once turned, and the desktop comes back at
//! the size it was asked for in the shape it was asked for.
//!
//! The one thing that is told is the finger. A touch panel reports in its own
//! orientation and the compositor reads it through a quarter of its own, which
//! was written down once for the one way up this device stood -- so the first
//! turn left every press being read at the mounting while the picture stood
//! somewhere else, which is a desktop where a thumb on the bar opens whatever
//! is a quarter turn away from what it is on. [`crate::size::lua`] says both in
//! one eval, because they are one answer.
//!
//! Which way is left is the one thing here that cannot be settled without the
//! device in someone's hands: [`Turn::Left`] is one quarter on from the
//! mounting and [`Turn::Right`] is three, and if that reads backwards in the
//! hand the two arms swap.
//!
//! [`Turn::Over`] is the half turn, and it is offered because the four a panel
//! can stand at are four and not three. On a handheld it is the way round that
//! puts the sticks where a stand does not foul them; on a panel screwed in
//! upside down it is the only way up that reads at all.

use console_core_never::Never;
use console_core_words::Words;
use console_screen::{Mounted, Screen};

use crate::screens::{Output, Unnamed};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum Turn {
    #[words(written = "left")]
    Left,
    #[words(written = "upright")]
    Upright,
    #[words(written = "right")]
    Right,
    #[words(written = "over")]
    Over,
}

pub const EVERY: [Turn; 4] = [Turn::Left, Turn::Upright, Turn::Right, Turn::Over];

pub const QUARTERS: u32 = 4;

impl Turn {
    pub fn quarters(self) -> Result<u32, Never> {
        Ok(match self {
            Turn::Left => 1,
            Turn::Upright => 0,
            Turn::Right => 3,
            Turn::Over => 2,
        })
    }

    pub fn transform(self, mode: console_core_geometry::Size<u32>) -> Result<u32, Never> {
        let Ok(mounted) = Mounted::of(mode);
        let Ok(mounting) = mounted.transform();
        let Ok(quarters) = self.quarters();

        let round = mounting.saturating_add(quarters).checked_rem(QUARTERS);

        Ok(match round {
            Some(transform) => transform,
            None => mounting,
        })
    }

    pub fn of(said: &str) -> Result<Option<Self>, Never> {
        Ok(EVERY.into_iter().find(|turn| {
            let Ok(written) = turn.written();

            written == said.trim()
        }))
    }
}

pub fn standing(screen: &Screen) -> Result<Option<Turn>, Never> {
    Ok(EVERY.into_iter().find(|turn| {
        let Ok(transform) = turn.transform(screen.mode);

        transform == screen.transform
    }))
}

pub fn turned(screen: &Screen, turn: Turn) -> Result<Screen, Never> {
    let Ok(transform) = turn.transform(screen.mode);

    Ok(Screen { transform, ..*screen })
}

pub const NAMED: &str = "turn";

pub fn at(
    home: &std::path::Path,
    panel: Output<'_>,
) -> Result<std::path::PathBuf, Unnamed> {
    panel.keeping(home, NAMED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_geometry::Size;

    fn panel() -> Screen {
        Screen { mode: Size { width: 1600, height: 2560 }, refresh: 144, scale: 2.5, transform: 1 }
    }

    fn laptop() -> Screen {
        Screen { mode: Size { width: 1920, height: 1200 }, refresh: 60, scale: 1.0, transform: 0 }
    }

    #[test]
    fn upright_is_the_way_the_panel_is_mounted_and_not_a_transform_of_nothing() {
        let Ok(handheld) = Turn::Upright.transform(panel().mode);
        let Ok(desk) = Turn::Upright.transform(laptop().mode);

        assert_eq!(handheld, 1);
        assert_eq!(desk, 0);
    }

    #[test]
    fn a_quarter_either_way_is_a_quarter_either_side_of_the_mounting() {
        let Ok(left) = Turn::Left.transform(panel().mode);
        let Ok(right) = Turn::Right.transform(panel().mode);

        assert_eq!((left, right), (2, 0));
    }

    #[test]
    fn the_four_a_panel_can_stand_at_are_four_transforms_and_no_two_the_same() {
        let every: Vec<u32> = EVERY
            .into_iter()
            .map(|turn| {
                let Ok(transform) = turn.transform(panel().mode);

                transform
            })
            .collect();

        assert_eq!(every, vec![2, 1, 0, 3]);

        let Ok(over) = Turn::Over.transform(laptop().mode);

        assert_eq!(over, 2, "a half turn is a half turn whichever way a panel is mounted");
    }

    #[test]
    fn what_a_screen_is_standing_at_is_the_turn_that_would_put_it_there() {
        for turn in EVERY {
            let Ok(screen) = turned(&panel(), turn);
            let Ok(standing) = standing(&screen);

            assert_eq!(standing, Some(turn));
        }
    }

    #[test]
    fn turning_the_panel_a_quarter_makes_the_rung_narrower_than_the_panel_it_was() {
        let Ok(sideways) = turned(&panel(), Turn::Upright);
        let Ok(portrait) = turned(&panel(), Turn::Left);
        let Ok(was) = crate::size::Size::Normal.scale_on(&sideways);
        let Ok(now) = crate::size::Size::Normal.scale_on(&portrait);

        assert!(now < was, "a turned panel is narrower, so its rung is a smaller density");
    }
}
