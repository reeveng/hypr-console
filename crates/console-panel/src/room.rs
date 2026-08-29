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

/// Which note this is.
const ROOM: &str = "room";

/// The room this program was granted last time, if it has been up before.
pub fn last(program: &str) -> (i32, i32) {
    notes::read(program, ROOM).map_or((0, 0), |held| read(&held))
}

/// Remember what the compositor granted.
///
/// The largest room it has ever granted, not the last one. The room is the
/// screen less whatever else has taken a piece of it, and the on-screen
/// keyboard takes a large piece for as long as it is up. Remembering the last
/// room meant that opening the keyboard once taught every panel afterwards to
/// open at the height it has with the keyboard up and grow when it finds the
/// keys are gone, which is the shift this is here to spare. The largest is the
/// room with nothing but the bar in it, which is how a panel opens.
///
/// Written only when it changed, so a panel opened and closed all day writes
/// nothing after the first time.
pub fn keep(program: &str, room: (i32, i32)) {
    let before = last(program);
    let room = (room.0.max(before.0), room.1.max(before.1));
    if room.0 <= 1 || room.1 <= 1 || before == room {
        return;
    }
    notes::write(program, ROOM, &said(room));
}

/// The two numbers, as they are written.
fn said(room: (i32, i32)) -> String {
    format!("{} {}\n", room.0, room.1)
}

/// The two numbers, as they are read. Nothing at all if the file says anything
/// else, so a file somebody edited by hand cannot make a panel open at a size
/// no screen has.
fn read(held: &str) -> (i32, i32) {
    let mut words = held.split_whitespace();
    let wide = words.next().and_then(|word| word.parse().ok()).unwrap_or(0);
    let tall = words.next().and_then(|word| word.parse().ok()).unwrap_or(0);
    match wide > 1 && tall > 1 {
        true => (wide, tall),
        false => (0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_was_written_is_what_is_read() {
        assert_eq!(read(&said((1600, 2400))), (1600, 2400));
    }

    #[test]
    fn a_file_saying_anything_else_is_a_panel_that_has_not_been_up() {
        assert_eq!(read(""), (0, 0));
        assert_eq!(read("wide tall"), (0, 0));
        assert_eq!(read("1600"), (0, 0));
        assert_eq!(read("0 0"), (0, 0));
        assert_eq!(read("-1600 -2400"), (0, 0));
    }

    /// The panel is measured against the room it was granted, and a room of
    /// one pixel is the answer given before there is one.
    #[test]
    fn a_room_too_small_to_be_real_is_not_remembered() {
        assert_eq!(read("1 1"), (0, 0));
    }

    /// The keyboard takes the bottom of the screen while it is up, and a panel
    /// opened after it has been up opens with the keys gone.
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
