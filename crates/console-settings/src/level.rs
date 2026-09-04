//! A level, drawn so it can be read across the room.


use console_number::{Float, toward_zero_usize};
/// The cells a level is drawn in, and how many of them a volume gets.
pub const FULL: char = '█';
pub const EMPTY: char = '░';
pub const CELLS: usize = 8;

/// Per press of left or right, in points of a hundred.
pub const STEP: i32 = 5;

/// A level you can read across the room, and the number for exactness.
///
/// Drawn in fewer cells it is a reading rather than a setting, and the number
/// beside it would say more than the thing it is about.
/// Whether the sound is silenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Muted {
    /// Silenced, which is not the same as turned down to nothing: it comes back
    /// where it was.
    Yes,
    /// Not silenced, so the number is what is heard.
    No,
}

pub fn bar(level: i32, muted: Muted, cells: usize) -> String {
    let filled = toward_zero_usize(
        (f64::from(level) / 100.0 * cells.float()).round().clamp(0.0, cells.float()),
    );
    let drawn: String =
        std::iter::repeat_n(FULL, filled).chain(std::iter::repeat_n(EMPTY, cells - filled)).collect();

    match (cells < CELLS, muted) {
        (true, _) => drawn,
        (false, Muted::Yes) => format!("{drawn} silent"),
        (false, Muted::No) => format!("{drawn} {level}%"),
    }
}

/// A volume, drawn as one.
pub fn volume(level: i32, muted: Muted) -> String {
    bar(level, muted, CELLS)
}

/// One step of a level, and never past either end.
pub fn stepped(level: i32, step: i32) -> i32 {
    (level + step * STEP).clamp(0, 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_a_picture_and_a_number() {
        assert_eq!(volume(50, Muted::No), "████░░░░ 50%");
        assert_eq!(volume(100, Muted::No), "████████ 100%");
        assert_eq!(volume(0, Muted::No), "░░░░░░░░ 0%");
    }

    /// Silent is a word rather than an empty bar: a volume turned down to
    /// nothing and a volume silenced at half are different states, and one of
    /// them comes back where it was.
    #[test]
    fn silence_is_said_rather_than_drawn() {
        assert_eq!(volume(50, Muted::Yes), "████░░░░ silent");
    }

    /// How well a network is heard is a reading. The number beside it would be
    /// a number nobody can do anything with.
    #[test]
    fn a_shorter_bar_is_a_reading_with_no_number_on_it() {
        assert_eq!(bar(100, Muted::No, 4), "████");
        assert_eq!(bar(50, Muted::No, 4), "██░░");
    }

    #[test]
    fn a_level_never_steps_past_either_end() {
        assert_eq!(stepped(98, 1), 100);
        assert_eq!(stepped(2, -1), 0);
        assert_eq!(stepped(50, 1), 50 + STEP);
    }
}
