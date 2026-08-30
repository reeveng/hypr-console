//! The music, drawn.
//!
//! Two tabs: what is playing, and what there is to play. The player itself is
//! kew, running headless behind this, and every button here is one MPRIS call.
//! Nothing about a song is worked out in this program: the title, the artist
//! and the cover are what the player says they are.
//!
//! Under what is playing are the two modes, which are the player's as well: the
//! order the songs come in, and whether the one on now comes round again.
//!
//! Y is the files panel, standing on the song the row is about: renaming a
//! song, copying it to a stick and throwing it away all live there already,
//! behind the same button, and none of it is worth teaching this panel twice.
//! It is offered on the song playing now as well, so the one you are listening
//! to does not have to be found in a list of nine hundred first.
//!
//! The Music tab is the folder, and the line at the top of it is not a filter
//! on the folder. It looks at every song under the music folder and at what
//! each of them says about itself, which is what `music-index` reads and writes
//! down; the ordering of what it finds is `looking`'s.

use std::path::Path;
use std::sync::{Arc, Mutex};

use gtk4::glib;
use console_music::library::{self, Thing};
use console_music::looking::{self, Song};
use console_music::player::{self, Over, Playing};
use console_music::{ascii, library::folder};
use console_panel::page::{Does, Level, Page, Picture, Row, Rows, Showing};
use console_panel::{chooser, panel, running};

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
///
/// Six rather than twelve. At twelve the sleeve was a hand's width of
/// characters and read as a wall of punctuation rather than as a picture of
/// anything, and the row it sat in was tall enough that the ordinary pink
/// highlight -- the same one every panel on this desktop uses -- covered most
/// of the tab. Half the height is a thumbnail: still the sleeve, no longer the
/// whole card.
const TALL: usize = 6;

/// What the line at the top of the Music tab says while nothing is in it.
const ABOUT: &str = "Type a song, whose it is, or anything it says";

/// The panel's own line stands above the rows, so the first row of the list is
/// the second thing on the screen.
const LINE: usize = 1;

/// How many songs one word is worth putting on the screen.
///
/// A word that answers for four hundred songs is a word worth typing more of,
/// and the ones worth reading are at the top of it either way.
const MANY: usize = 120;

/// What the panel holds between one drawing and the next.
///
/// The rows are read on a thread of the panel's own, which is why there is a
/// lock rather than a cell.
struct Standing {
    typed: String,
    /// Whether the library has been sent to be read while this panel has been
    /// open. Once is enough: what it reads it writes down.
    reading: bool,
}

type Held = Arc<Mutex<Standing>>;

/// The lock, taken and given back before anything is drawn or run.
fn standing<T>(held: &Held, then: impl FnOnce(&mut Standing) -> T) -> T {
    then(&mut held.lock().expect("the standing"))
}

// ---------------------------------------------------------- what is playing

fn playing_rows() -> Vec<Row> {
    let asked = player::playing();
    let mut rows = match asked.as_ref() {
        Some(playing) if !playing.stopped => vec![now(playing)],
        _ => vec![Row::nothing("Nothing is playing")],
    };
    // The two modes, and only where there is a player to be told them. A
    // switch that goes nowhere is worse than no switch at all.
    if asked.is_some() {
        rows.push(order_row(player::shuffling()));
        rows.push(again_row(player::over()));
    }
    rows
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
    let mut row = Row::new(&playing.title, said, stepping(player::play_pause))
        .levelled(along())
        .ended(BACK, ON);

    // And Y is the file it is, so the song on now can be renamed or thrown
    // away without going and finding it in a list of nine hundred. Only where
    // the player says which file it is playing: a row with nothing to offer
    // says nothing, which is better than a guess between two songs of the same
    // name.
    if let Some(path) = playing.path.as_deref() {
        row = row.offering(shown_in_the_files(path));
    }
    let cover = playing.art.as_deref().and_then(|art| ascii::read(art, TALL));

    match cover {
        Some(cover) => row.picturing(Picture::Written(cover.markup())),
        None => row.picturing(Picture::Space),
    }
}

/// What order the songs come in.
///
/// The two modes wear the mark of the state they are in, which is the same
/// four marks every music player draws: the crossed arrows and the straight
/// ones, the loop and the loop with a one in it. Everywhere else on this panel
/// a switch is a sentence saying what pressing it does, because a mode that can
/// only be read off which way round the row is written is a mode nobody is
/// sure of -- and a mark that changes says it better than the sentence did, in
/// the width of an icon rather than the width of the card. So here the row is
/// named for the mode, the mark says how it stands, and the words beside it say
/// the same thing for anybody who does not read the mark.
///
/// Out of the icon theme rather than out of a font. Every other picture on this
/// panel comes from there, symbolic so it is drawn in the row's own ink, and
/// the alternative is a private-use codepoint drawn by whichever font
/// fontconfig reaches for -- which is the bug the bar's stylesheet is half
/// written about.
fn order_row(any_order: bool) -> Row {
    let aside = match any_order {
        true => "any order",
        false => "as they are",
    };
    let mark = match any_order {
        true => "media-playlist-shuffle-symbolic",
        false => "media-playlist-consecutive-symbolic",
    };
    Row::new("Order", aside, stepping(move || player::shuffle(!any_order)))
        .picturing(Picture::Named(mark))
}

/// Whether the song on now comes round again when it ends.
fn again_row(over: Over) -> Row {
    let again = over == Over::Again;
    let aside = match again {
        true => "play it again",
        false => "go on",
    };
    let mark = match again {
        true => "media-playlist-repeat-song-symbolic",
        false => "media-playlist-repeat-symbolic",
    };
    Row::new("When it ends", aside, stepping(move || player::repeat(!again)))
        .picturing(Picture::Named(mark))
}

/// A button of the player's, and the card drawn again once it has answered.
fn stepping(press: impl Fn() + Send + Sync + 'static) -> Does {
    Does::and_stay(move |showing| {
        press();
        showing.refresh();
    })
}

// ----------------------------------------------------- what there is to play

fn music_rows(held: &Held) -> Vec<Row> {
    let typed = standing(held, |standing| standing.typed.trim().to_string());
    let folder = folder();

    match typed.is_empty() {
        true => in_the_folder(&folder),
        false => answering(&folder, &typed),
    }
}

/// What is in the music folder, which is what the tab says before a word is
/// typed into it.
fn in_the_folder(folder: &Path) -> Vec<Row> {
    let things = library::things(folder);

    if things.is_empty() {
        return vec![Row::nothing(&format!("Nothing in {}", folder.display()))];
    }
    things.iter().map(chosen).collect()
}

/// What the typed word finds, the closest first.
///
/// The library is walked and the index read on every letter rather than held
/// between them: a walk is a few milliseconds, and what it saves is a list that
/// could be wrong about a song fetched a minute ago.
fn answering(folder: &Path, word: &str) -> Vec<Row> {
    let songs = songs(folder);
    let found = looking::ranked(&songs, word);

    if found.is_empty() {
        return vec![Row::nothing(&format!("Nothing here answers to {word}"))];
    }
    found.iter().take(MANY).map(|song| played(song, folder)).collect()
}

/// Every song under the music folder, carrying whatever has been read about it.
fn songs(folder: &Path) -> Vec<Song> {
    let at = looking::at(&glib::user_cache_dir());
    let known = looking::kept(&std::fs::read_to_string(at).unwrap_or_default());
    looking::songs(folder, &library::things, &known)
}

/// One thing to play. A folder is played whole, in the order it is in.
fn chosen(thing: &Thing) -> Row {
    let said = match thing.folder {
        true => "album",
        false => "",
    };
    Row::new(&thing.name, said, plays(&thing.path, thing.folder))
        .offering(shown_in_the_files(&thing.path))
}

/// One song a word found, said as whose it is or where it is.
fn played(song: &Song, folder: &Path) -> Row {
    Row::new(song.says(), &song.aside(folder), plays(&song.path, false))
        .offering(shown_in_the_files(&song.path))
}

/// What else can be done with a song, wherever the row is: in the folder, in
/// what a word found, or on the song playing now.
///
/// The files panel opens standing on the file, and renaming it, copying it to a
/// stick and throwing it away are all there already, behind the same Y, said in
/// the same words, asking before it does the one that cannot be taken back.
/// None of that is worth teaching this panel a second time, and a song is a
/// file: what the person wants is nearly always to delete it, and the shortest
/// honest way to that is to put them in front of it.
///
/// The panel goes when it opens, the way a row that runs something does. Two
/// cards over each other with the same song on them is a B nobody can predict.
fn shown_in_the_files(path: &Path) -> impl Fn(&dyn Showing) -> bool + Send + Sync + 'static {
    let argv = vec!["files-panel".to_string(), path.to_string_lossy().to_string()];
    move |_| {
        running::left_running(&argv);
        true
    }
}

/// What choosing a song does.
///
/// Left running rather than waited on. A player is not a command that finishes:
/// handed to the panel's `later`, kew was waited on for the length of the song
/// and stayed a child of the panel, so the music stopped the moment the panel
/// that started it was closed. Started this way it is in a session of its own
/// and outlives the tab it was chosen from, which is what choosing a song
/// means.
fn plays(path: &Path, folder: bool) -> Does {
    let path = path.to_path_buf();
    Does::and_stay(move |showing| {
        let argv = player::opening(&path, folder);
        // The whole of what was asked for, where a song that did not play is
        // diagnosed. kew answers OpenUri only in the fork, so on a machine with
        // the packaged one every play is the second half of this line, and
        // knowing which half ran is the difference between a player that was
        // never asked and a player that was asked and said no.
        eprintln!("playing {}: {}", path.display(), argv.join(" "));
        showing.leave_running(argv);
    })
}

/// Agree with kew about the folder, and send the library to be read if
/// anything in it has not been read yet.
///
/// The folder is settled first because kew stops to ask for it when it has not
/// been told, and asks on a terminal a panel does not have.
///
/// On arriving rather than on every drawing, and once while the panel is open:
/// it is minutes of ffprobe the first time and nothing at all after that, and
/// what it buys is a search that knows whose a song is as well as what it is
/// called.
fn read_the_library(held: &Held, showing: &dyn Showing) {
    let folder = folder();
    library::tell_kew(&folder);
    let unread = looking::unread(&songs(&folder));

    if unread == 0 || standing(held, |standing| std::mem::replace(&mut standing.reading, true)) {
        return;
    }
    showing.note(&how_many(unread));
    showing.later(vec!["music-index".to_string()]);
}

/// What the corner says while that is happening.
fn how_many(unread: usize) -> String {
    match unread {
        1 => "Reading what one more song says about itself".to_string(),
        many => format!("Reading what {many} songs say about themselves"),
    }
}

/// Stop looking, and leave the tab as it was before anything was typed.
///
/// The word is the panel's, so it is the panel that is asked to forget it.
fn stopped(held: &Held, showing: &dyn Showing) {
    showing.forget_typing();
    standing(held, |standing| standing.typed = String::new());
    showing.replace(LINE);
}

// ------------------------------------------------------------------ the tabs

fn pages(held: &Held) -> Vec<Page> {
    vec![Page::new("Playing", Rows::asked(playing_rows)), music_page(held)]
}

fn music_page(held: &Held) -> Page {
    let reading = Arc::clone(held);
    let arriving = Arc::clone(held);
    let backing = Arc::clone(held);
    let typing = Arc::clone(held);

    Page::new("Music", Rows::asked(move || music_rows(&reading)))
        .on_arriving(move |showing| read_the_library(&arriving, showing))
        .on_back(move |showing| {
            // Out of the word first, and out of the panel only once there is
            // no word to come out of.
            if standing(&backing, |standing| standing.typed.trim().is_empty()) {
                return true;
            }
            stopped(&backing, showing);
            false
        })
        .searching(ABOUT, move |showing, word| {
            let changed = standing(&typing, |standing| {
                let changed = standing.typed != word;
                standing.typed = word.to_string();
                changed
            });
            // Standing on the line, which is where the letters go. The rows
            // under it are not the rows that were under it a letter ago.
            if changed {
                showing.replace(0);
            }
        })
}

fn main() {
    if !chooser::alone("music", chooser::Again::Closes) {
        return;
    }
    // Nothing is started here. The panel is on the screen in the time it takes
    // to draw it, and the player is started by the first thing chosen to play.
    let held: Held = Arc::new(Mutex::new(Standing { typed: String::new(), reading: false }));
    panel::show(Arc::new(move || pages(&held)), 0, None);
}
