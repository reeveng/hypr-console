//! A level, drawn so it can be read across the room.


use console_core_never::Never;
use console_core_number_conversion::{index, toward_zero_u32};
pub const FULL: char = '█';
pub const EMPTY: char = '░';
pub const CELLS: u32 = 8;

pub const STEP: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Muted {
    Yes,
    No,
}

pub fn bar(level: i32, muted: Muted, cells: u32) -> Result<String, Never> {
    let many = f64::from(cells);
    let Ok(filled) =
        toward_zero_u32((f64::from(level) / 100.0 * many).round().clamp(0.0, many));
    let Ok(full) = index(filled);
    let Ok(empty) = index(cells.saturating_sub(filled));
    let drawn: String = std::iter::repeat_n(FULL, full)
        .chain(std::iter::repeat_n(EMPTY, empty))
        .collect();

    match (cells < CELLS, muted) {
        (true, _) => Ok(drawn),
        (false, Muted::Yes) => Ok(format!("{drawn} silent")),
        (false, Muted::No) => Ok(format!("{drawn} {level}%")),
    }
}

pub fn volume(level: i32, muted: Muted) -> Result<String, Never> {
    bar(level, muted, CELLS)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step(pub i32);

pub fn stepped(level: i32, step: Step) -> Result<i32, Never> {
    Ok(level.saturating_add(step.0.saturating_mul(STEP)).clamp(0, 100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_a_picture_and_a_number() {
        assert_eq!(volume(50, Muted::No), Ok("████░░░░ 50%".to_string()));
        assert_eq!(volume(100, Muted::No), Ok("████████ 100%".to_string()));
        assert_eq!(volume(0, Muted::No), Ok("░░░░░░░░ 0%".to_string()));
    }

    #[test]
    fn silence_is_said_rather_than_drawn() {
        assert_eq!(volume(50, Muted::Yes), Ok("████░░░░ silent".to_string()));
    }

    #[test]
    fn a_shorter_bar_is_a_reading_with_no_number_on_it() {
        assert_eq!(bar(100, Muted::No, 4), Ok("████".to_string()));
        assert_eq!(bar(50, Muted::No, 4), Ok("██░░".to_string()));
    }

    #[test]
    fn a_level_never_steps_past_either_end() {
        assert_eq!(stepped(98, Step(1)), Ok(100));
        assert_eq!(stepped(2, Step(-1)), Ok(0));
        assert_eq!(stepped(50, Step(1)), Ok(50 + STEP));
    }
}
