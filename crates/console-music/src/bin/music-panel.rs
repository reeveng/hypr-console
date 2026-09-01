//! The music, drawn.
//!
//! Two tabs: what is playing, and what there is to play. The player itself is
//! kew, running headless behind this, and every button here is one MPRIS call.
//! Nothing about a song is worked out in this program: the title, the artist
//! and the cover are what the player says they are.
//!
//! The Playing tab is the song, the cover, the bar, and the row of buttons.
//! The cover is on the right of the row that names the song -- the only
//! picture a panel puts on the right is the thing the row is about. Under
//! them is a bar showing where the song is: a tap scrubs it, the d-pad
//! scrubs it a step at a time. Under that are the five buttons, said five
//! times so each is its own press for the d-pad: shuffle on the left,
//! repeat on the right, and the three that move between songs in the middle.
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
use console_panel::page::{Bar, Does, Level, Page, Picture, Row, Rows, Showing, Watch};
use console_panel::{chooser, panel, running};

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

/// How wide the seek bar is drawn, in characters.
///
/// Wide enough that a finger on a touch screen lands on the moment the
/// thumb wanted, rather than on the closest character. The panel knows the
/// real width once it has been laid out; this is what it asks for first.
const BAR_WIDE: usize = 40;

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
    match asked.as_ref() {
        Some(playing) if !playing.stopped => playing_card(playing),
        _ => vec![Row::nothing("Nothing is playing")],
    }
}

/// What the player has to say: the song, where the song is, and the row of
/// buttons the panel offers for it.
///
/// Three parts. The song and its cover are a heading the d-pad walks past --
/// reading it is what the row is for, choosing it is nothing. The bar is one
/// row the d-pad stands on: it scrubs a fraction at a time with left and right
/// and lands wherever a finger lands. The five buttons underneath are the
/// row, said five times: shuffle on the left, repeat on the right, the three
/// that move between songs in the middle.
fn playing_card(playing: &Playing) -> Vec<Row> {
    let cover = playing.art.as_deref().and_then(|art| ascii::read(art, TALL));
    let tail = match &cover {
        Some(cover) => Picture::Written(cover.markup()),
        None => Picture::Space,
    };
    let mut rows = vec![info_row(playing, tail), scrub_row()];
    rows.extend(transport_rows(playing));
    rows
}

/// The song, with its cover on the right.
///
/// A heading rather than a thing to choose: the d-pad walks past it, and a
/// press of A on it does nothing because the cover is the part worth looking
/// at and the words are the part worth reading. The cover goes on the right so
/// the words stack on the left where a hand reading the panel meets them first.
fn info_row(playing: &Playing, tail: Picture) -> Row {
    let mut row = Row::naming(&playing.title, &playing.artist);
    if let Some(path) = playing.path.as_deref() {
        // The song on now is offered Y as well, for the same reason every
        // other song is: the files panel is where renaming, copying and
        // throwing away already live, and a song that is playing is a song
        // somebody might want gone.
        row.more = Some(Arc::new(shown_in_the_files(path)));
    }
    row.tail = Some(tail);
    row
}

/// The bar that says where the song is, and the row the d-pad stands on to
/// move it.
///
/// The position is read from the player as a fraction of the length, both of
/// which the player knows and neither of which the panel has been told yet.
/// A bar with a length of zero is the honest answer to a question nobody has
/// asked -- the dot sits at the start, and the row is still a thing to scrub
/// because the d-pad can take it from there.
fn scrub_row() -> Row {
    let (pos, total) = (player::position(), player::length());
    let at = match total > 0 {
        true => (pos as f64 / total as f64).clamp(0.0, 1.0),
        false => 0.0,
    };
    let wide = BAR_WIDE;
    let char_at = (at * (wide as f64 - 1.0)).round() as usize;

    Row::new("", "", Does::and_stay(|_| {}))
        .picturing(Picture::Bar(Bar { at: char_at, wide }))
        .levelled(scrub_step(pos, total))
        .seeking(|showing, frac| {
            player::seek(frac);
            showing.refresh();
        })
}

/// One press of left or right on the scrub bar, in microseconds.
///
/// A step is one twentieth of the song. Long enough to be worth taking, short
/// enough to land within reach of where the dot already is. A bar with no
/// length takes a second instead, which is the same kind of step the panel
/// takes when the player has not told it anything.
fn scrub_step(pos: i64, total: i64) -> Level {
    Arc::new(move |dir| {
        let step = match total > 0 {
            true => total / 20,
            false => 1_000_000,
        };
        let dir = dir as i64;
        let target = (pos + dir * step).clamp(0, total);
        let denom = total.max(1) as f64;
        player::seek(target as f64 / denom);
    })
}

/// The five buttons, said as five rows.
///
/// Each is its own row because that is how the d-pad reaches it. The icons
/// are the four marks every player draws: crossed arrows and straight ones,
/// the loop and the loop with a one in it; the play and pause marks for the
/// middle. The middle one is the bigger, because that is the one a hand comes
/// here for and a thumb is what finds it.
fn transport_rows(playing: &Playing) -> Vec<Row> {
    let shuffle_icon = match player::shuffling() {
        true => "media-playlist-shuffle-symbolic",
        false => "media-playlist-consecutive-symbolic",
    };
    vec![
        shuffle_button(shuffle_icon),
        transport_button("media-skip-backward-symbolic", player::previous),
        play_row(playing),
        transport_button("media-skip-forward-symbolic", player::next),
        repeat_button(),
    ]
}

/// Shuffle: the only button that has to be told the icon it wears, because
/// the icon is the only part of it that changes between presses.
fn shuffle_button(icon: &'static str) -> Row {
    Row::new("", "", Does::and_stay(move |showing| {
        player::shuffle(!player::shuffling());
        showing.refresh();
    }))
    .picturing(Picture::Named(icon))
    .offering(no_more)
    .transport()
}

/// One of the five, as a row with an icon at the front.
fn transport_button(icon: &'static str, press: impl Fn() + Send + Sync + 'static) -> Row {
    Row::new("", "", Does::and_stay(move |showing| {
        press();
        showing.refresh();
    }))
    .picturing(Picture::Named(icon))
    .offering(no_more)
    .transport()
}

/// A row whose icon is the play or the pause, depending on what the player
/// is doing. The middle of the row is bigger than the others, because that is
/// the press a hand reaches for.
fn play_row(playing: &Playing) -> Row {
    let icon = match playing.paused {
        true => "media-playback-start-symbolic",
        false => "media-playback-pause-symbolic",
    };
    Row::new("", "", Does::and_stay(move |showing| {
        player::play_pause();
        showing.refresh();
    }))
    .picturing(Picture::Named(icon))
    .offering(no_more)
    .transport()
}

/// The repeat button, with the mark that says how the player is set up.
///
/// Three states, three marks: nothing on repeat (the loop with no one in it),
/// this one on repeat (the loop with the one), and the whole list on repeat
/// (the loop with the whole bar). The panel offers two and the player keeps
/// the third; the row is read off the player rather than off what the panel
/// last said, because the player is the only one who knows.
fn repeat_button() -> Row {
    let icon = match player::over() {
        Over::Again => "media-playlist-repeat-song-symbolic",
        Over::Round => "media-playlist-repeat-symbolic",
        Over::On => "media-playlist-no-repeat-symbolic",
    };
    Row::new("", "", Does::and_stay(move |showing| {
        player::repeat(player::over() != Over::Again);
        showing.refresh();
    }))
    .picturing(Picture::Named(icon))
    .offering(no_more)
    .transport()
}

/// What Y does on a transport button: nothing. The press is the whole of what
/// the button is for, and a menu on top of it is a menu nobody asked for.
fn no_more(_: &dyn Showing) -> bool {
    false
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
    vec![
        Page::new("Playing", Rows::asked(playing_rows))
            // One second is what an ear hears as "the bar moved", and shorter
            // than that wastes cycles drawing what did not change. The watch
            // is one shell loop ticking the playing tab; the panel reads the
            // player's position on every redraw.
            .watching(Watch::on(
                &["sh", "-c", "while true; do echo tick; sleep 1; done"],
                "tick",
            )),
        music_page(held),
    ]
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
