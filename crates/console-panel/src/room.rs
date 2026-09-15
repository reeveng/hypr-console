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

use crate::notes::{self, Note};
use console_core_geometry::Size;
use console_core_never::Never;

const ROOM: &str = "room";

const NONE: Size<i32> = Size { wide: 0, tall: 0 };

pub fn last(program: &str) -> Result<Size<i32>, Never> {
    let Ok(held) = notes::read(Note { program, called: ROOM });

    match held {
        Some(held) => read(&held),
        None => Ok(NONE),
    }
}

pub fn keep(program: &str, room: Size<i32>) -> Result<(), Never> {
    let Ok(before) = last(program);

    let room = Size { wide: room.wide.max(before.wide), tall: room.tall.max(before.tall) };

    match room.wide <= 1 || room.tall <= 1 || before == room {
        true => return Ok(()),
        false => {},
    }

    let Ok(said) = said(room);

    let Ok(()) = notes::write(Note { program, called: ROOM }, &said);

    Ok(())
}

fn said(room: Size<i32>) -> Result<String, Never> {
    Ok(format!("{} {}\n", room.wide, room.tall))
}

fn number(word: Option<&str>) -> Result<i32, Never> {
    let word = match word {
        Some(word) => word,
        None => return Ok(0),
    };

    let number = match word.parse::<i32>() {
        Ok(number) => number,
        Err(_fault) => return Ok(0),
    };

    Ok(number)
}

fn read(held: &str) -> Result<Size<i32>, Never> {
    let mut words = held.split_whitespace();
    let Ok(wide) = number(words.next());
    let Ok(tall) = number(words.next());

    Ok(match wide > 1 && tall > 1 {
        true => Size { wide, tall },
        false => NONE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_was_written_is_what_is_read() {
        let Ok(said) = said(Size { wide: 1600, tall: 2400 });

        assert_eq!(read(&said), Ok(Size { wide: 1600, tall: 2400 }));
    }

    #[test]
    fn a_file_saying_anything_else_is_a_panel_that_has_not_been_up() {
        assert_eq!(read(""), Ok(NONE));
        assert_eq!(read("wide tall"), Ok(NONE));
        assert_eq!(read("1600"), Ok(NONE));
        assert_eq!(read("0 0"), Ok(NONE));
        assert_eq!(read("-1600 -2400"), Ok(NONE));
    }

    #[test]
    fn a_room_too_small_to_be_real_is_not_remembered() {
        assert_eq!(read("1 1"), Ok(NONE));
    }

    #[test]
    fn the_largest_room_beats_the_last_one() {
        let short = Size { wide: 1024, tall: 300 };
        let tall = Size { wide: 1024, tall: 602 };

        assert_eq!(largest(short, tall), tall);
        assert_eq!(largest(tall, short), tall);
        assert_eq!(largest(tall, NONE), tall, "nothing remembered yet");
    }

    fn largest(room: Size<i32>, before: Size<i32>) -> Size<i32> {
        Size { wide: room.wide.max(before.wide), tall: room.tall.max(before.tall) }
    }
}
