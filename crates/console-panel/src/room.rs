//! The room a panel was given last time it was up.
//!
//! A panel opens before the compositor has granted it anything, so the only
//! answer available then is the size of the screen, and the screen is far
//! wider than the share a card takes. The card is therefore drawn once at the
//! wrong size and again at the right one, which is a shift on the screen every
//! time a menu is opened.
//!
//! What was granted last time is a much better guess than the monitor, and on
//! a machine where every panel comes up in the same place under the same bar
//! it is usually the answer exactly. The first fit still corrects it, so a
//! remembered size that has gone stale costs the shift it used to cost every
//! time.
//!
//! None of this is required to work. A file that cannot be read or written is
//! a panel that opens the way it did before there was one.

use crate::notes;
use console_core_never::Never;

const ROOM: &str = "room";

pub fn last(program: &str) -> Result<(i32, i32), Never> {
    let Ok(held) = notes::read(program, ROOM);

    match held {
        Some(held) => read(&held),
        None => Ok((0, 0)),
    }
}

pub fn keep(program: &str, room: (i32, i32)) -> Result<(), Never> {
    let Ok(before) = last(program);

    let room = (room.0.max(before.0), room.1.max(before.1));

    match room.0 <= 1 || room.1 <= 1 || before == room {
        true => return Ok(()),
        false => {},
    }

    let Ok(said) = said(room);

    let Ok(()) = notes::write(program, ROOM, &said);

    Ok(())
}

fn said(room: (i32, i32)) -> Result<String, Never> {
    Ok(format!("{} {}\n", room.0, room.1))
}

fn number(word: Option<&str>) -> Result<i32, Never> {
    let Some(word) = word else { return Ok(0) };

    let Ok(number) = word.parse::<i32>() else { return Ok(0) };

    Ok(number)
}

fn read(held: &str) -> Result<(i32, i32), Never> {
    let mut words = held.split_whitespace();
    let Ok(wide) = number(words.next());
    let Ok(tall) = number(words.next());

    Ok(match wide > 1 && tall > 1 {
        true => (wide, tall),
        false => (0, 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_was_written_is_what_is_read() {
        let Ok(said) = said((1600, 2400));

        assert_eq!(read(&said), Ok((1600, 2400)));
    }

    #[test]
    fn a_file_saying_anything_else_is_a_panel_that_has_not_been_up() {
        assert_eq!(read(""), Ok((0, 0)));
        assert_eq!(read("wide tall"), Ok((0, 0)));
        assert_eq!(read("1600"), Ok((0, 0)));
        assert_eq!(read("0 0"), Ok((0, 0)));
        assert_eq!(read("-1600 -2400"), Ok((0, 0)));
    }

    #[test]
    fn a_room_too_small_to_be_real_is_not_remembered() {
        assert_eq!(read("1 1"), Ok((0, 0)));
    }

    #[test]
    fn the_largest_room_beats_the_last_one() {
        assert_eq!(largest((1024, 300), (1024, 602)), (1024, 602));
        assert_eq!(largest((1024, 602), (1024, 300)), (1024, 602));
        assert_eq!(largest((1024, 602), (0, 0)), (1024, 602), "nothing remembered yet");
    }

    fn largest(room: (i32, i32), before: (i32, i32)) -> (i32, i32) {
        (room.0.max(before.0), room.1.max(before.1))
    }
}
