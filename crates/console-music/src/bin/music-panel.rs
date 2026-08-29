//! The music, drawn.
//!
//! Two tabs: what is playing, and what there is to play. The player itself is
//! kew, running headless behind this, and every button here is one MPRIS call.
//! Nothing about a song is worked out in this program: the title, the artist
//! and the cover are what the player says they are.

use std::sync::Arc;

use console_music::library::{self, Thing};
use console_music::player::{self, Playing};
use console_music::{ascii, library::folder};
use console_panel::page::{Does, Level, Page, Picture, Row, Rows};
use console_panel::{chooser, panel};

/// Geometry rather than the media characters, which every font on this machine
/// draws as an orange emoji.
const BACK: &str = "\u{25c2}\u{25c2}";
const ON: &str = "\u{25b8}\u{25b8}";
const PLAYING: &str = "\u{25b8} playing";
const PAUSED: &str = "\u{2016} paused";

/// How tall a cover is drawn, in characters.
///
/// The panel gives every row the height of the tallest, so a cover twice this
/// size is a card with one row on it and the way to the next song off the
/// bottom of the screen.
const TALL: usize = 12;

fn playing_rows() -> Vec<Row> {
    let playing = player::playing().unwrap_or_default();

    match playing.stopped {
        true => vec![Row::said("Nothing is playing", "")],
        false => vec![now(&playing)],
    }
}

/// The song before this one, and the song after it.
fn along() -> Level {
    Arc::new(|step| match step > 0 {
        true => player::next(),
        false => player::previous(),
    })
}

/// The song, with its cover. A stops and starts it, left is the song before and
/// right is the song after.
fn now(playing: &Playing) -> Row {
    let said = match playing.paused {
        true => PAUSED,
        false => PLAYING,
    };
    let row = Row::new(&playing.title, said, stepping(player::play_pause))
        .levelled(along())
        .ended(BACK, ON);
    let cover = playing.art.as_deref().and_then(|art| ascii::read(art, TALL));

    match cover {
        Some(cover) => row.picturing(Picture::Written(cover.markup())),
        None => row.picturing(Picture::Space),
    }
}

/// A button of the player's, and the card drawn again once it has answered.
fn stepping(press: fn()) -> Does {
    Does::and_stay(move |showing| {
        press();
        showing.refresh();
    })
}

fn music_rows() -> Vec<Row> {
    let things = library::things(&folder());

    if things.is_empty() {
        return vec![Row::said(&format!("Nothing in {}", folder().display()), "")];
    }
    things.iter().map(chosen).collect()
}

/// One thing to play. A folder is played whole, in the order it is in.
fn chosen(thing: &Thing) -> Row {
    let path = thing.path.clone();
    let said = match thing.folder {
        true => "album",
        false => "",
    };
    Row::new(
        &thing.name,
        said,
        // Off the drawing thread: starting a player takes a second or two, and
        // a card that has stopped answering the buttons reads as a machine
        // that has crashed.
        Does::and_stay(move |showing| showing.later(player::opening(&path))),
    )
}

fn main() {
    if !chooser::alone("music", chooser::Again::Closes) {
        return;
    }
    // Nothing is started here. The panel is on the screen in the time it takes
    // to draw it, and the player is started by the first thing chosen to play.
    panel::show(
        Arc::new(|| {
            vec![
                Page::new("Playing", Rows::asked(playing_rows)),
                Page::new("Music", Rows::asked(music_rows)),
            ]
        }),
        0,
        None,
    );
}
