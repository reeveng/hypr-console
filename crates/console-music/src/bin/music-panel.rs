//! The music, drawn.
//!
//! Two tabs: what is playing, and what there is to play. The player itself is
//! kew, running headless behind this, and every button here is one MPRIS call.
//! Nothing about a song is worked out in this program: the title, the artist
//! and the cover are what the player says they are.
//!
//! The Playing tab is one card about one song, stacked up the middle of it:
//! the sleeve, the title, whose it is, the bar, and the row of buttons. The
//! sleeve is written in characters off kew's ramp, which is the same picture
//! the player draws in a terminal and the one alphabet the rest of the screen
//! is in. Its square is held whether there is a cover for it yet or not,
//! because the player says what the song is a moment before it says where its
//! picture is. The bar says where in the song you are: a tap lands on the
//! moment, and the d-pad moves it a few seconds a press. Under it are the five buttons,
//! said five times so each is its own press for the d-pad: shuffle on the
//! left, repeat on the right, and the three that move between songs in the
//! middle. The card opens with the thumb on play, which is what a hand came
//! to it for.
//!
//! Two of those five rows are things to read rather than things to choose --
//! the sleeve and the two lines of words -- so the d-pad walks past them and
//! nothing on the card draws as though it could be pressed and then cannot.
//!
//! Y is the files panel, standing on the song: renaming a song, copying it to
//! a stick and throwing it away all live there already, behind the same
//! button, and none of it is worth teaching this panel twice. It is offered on
//! the song playing now as well, so the one you are listening to does not have
//! to be found in a list of nine hundred first -- from any row of the card a
//! thumb can stand on, because the whole card is about the one song.
//!
//! The Music tab is the folder, and the line at the top of it is not a filter
//! on the folder. It looks at every song under the music folder and at what
//! each of them says about itself, which is what `music-index` reads and writes
//! down; the ordering of what it finds is `looking`'s.


use console_number::{Float, fitted, whole_usize};
use std::path::Path;
use std::sync::Arc;

use gtk4::glib;
use console_music::ascii;
use console_music::library::{self, Kind, Thing};
use console_music::looking::{self, Song};
use console_music::player::{self, Order, Over, Playing};
use console_music::library::folder;
use console_panel::actor::{self, Addr, Answer};
use console_panel::page::{Bar, Does, Heading, InEffect, Level, Page, Picture, Press, Row, Rows, Showing, Watch};
use console_panel::{chooser, panel, running};

/// How tall the sleeve is drawn, in lines.
///
/// A picture of it took the square `console_panel::strip::SLEEVE` names, which
/// is nine lines of the font a cover is written in. Twelve, because a cover is
/// only as much of a picture as it has cells to be one with: at nine the
/// record was a texture, and at twelve it is a sleeve you can name across the
/// room. That is a third more of the card than the picture took, and the rest
/// of it -- the words, the bar, the presses -- is still above the fold on the
/// screen this is for, which is the whole of what that square was protecting.
///
/// It was six, on a thumbnail at the end of the row naming the song. That is
/// not what this is: the sleeve has a row to itself that the d-pad walks past,
/// and the picture is the point of it.
const TALL: usize = 12;

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

/// Which of the transport's presses is the middle one.
///
/// Play, which is where the highlight starts and where it goes back to when
/// there is nothing to remember. It is a place in the strip rather than a
/// name, because the strip is what is drawn: shuffle, back, play, on, repeat.
const PLAY: usize = 2;

/// How wide the seek bar is drawn, in characters.
///
/// Wide enough that a finger on a touch screen lands on the moment the
/// thumb wanted, rather than on the closest character. The panel knows the
/// real width once it has been laid out; this is what it asks for first.
const BAR_WIDE: usize = 40;

/// One press of left or right on the scrub bar, in microseconds.
///
/// A few seconds, and the same few whatever is playing. It was a twentieth of
/// the song, which is a press that means something different on every one of
/// them: nine seconds through a single and half a minute through a mix, so a
/// thumb that has learned the press on one song overshoots on the next. What a
/// hand has learned on every player it has held is a number of seconds.
///
/// A song of no length is one the player has not told us about, and a seek
/// into it does nothing at all -- the position is asked for as a fraction of
/// something there is none of -- so the step is not worth a case of its own.
const SCRUB: i64 = 5_000_000;

/// What the panel holds between one drawing and the next.
///
/// The rows are read on a thread of the panel's own, which is why this has one
/// owner rather than sitting in a cell.
struct Standing {
    typed: String,
    /// Whether the library has been sent to be read while this panel has been
    /// open. Once is enough: what it reads it writes down.
    reading: bool,
    /// Which of the transport's presses the d-pad is standing on.
    ///
    /// Here rather than on the row, because the rows are built again every
    /// second the playing tab is watched and anything written onto one would
    /// be gone by the next tick. It starts on the middle press -- play, which
    /// is what a hand comes to this tab for.
    press: usize,
}

/// Whether typing changed what was there.
///
/// Named rather than a bare `bool`, so the signature says which way round it
/// reads without a comment under it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Word {
    Same,
    Changed,
}

/// Whether the library had already been sent to be read.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sent {
    NotYet,
    Already,
}

/// Everything that can happen to the standing, and nothing else.
enum Msg {
    /// What has been typed, trimmed.
    Typed(Answer<String>),
    /// Claim the one reading of the library, and say whether it was already
    /// claimed. Both halves in one message, because asking and then claiming
    /// is two crossings with room between them for a second arrival to claim
    /// it as well.
    Read(Answer<Sent>),
    /// Forget what was typed.
    Forget,
    /// Type this, and say whether it changed anything.
    Type { word: String, answer: Answer<Word> },
    /// Which press of the transport the highlight is on.
    Press(Answer<usize>),
    /// Move along the transport by this many presses, and say where that left
    /// it. It stops at either end rather than coming round: a strip of five
    /// that wraps means pressing right at the end of it jumps a thumb back to
    /// the other side of the card.
    Along { by: i32, of: usize, answer: Answer<usize> },
}

impl actor::Machine for Standing {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Typed(answer) => {
                let _ = answer.say(self.typed.trim().to_string());
                self
            },
            Msg::Read(answer) => {
                let _ = answer.say(match self.reading {
                    true => Sent::Already,
                    false => Sent::NotYet,
                });
                Standing { reading: true, ..self }
            },
            Msg::Forget => Standing { typed: String::new(), ..self },
            Msg::Press(answer) => {
                let _ = answer.say(self.press);
                self
            },
            Msg::Along { by, of, answer } => {
                let last = of.saturating_sub(1);
                let step: usize = fitted(by.unsigned_abs());
                let moved = match by < 0 {
                    true => self.press.saturating_sub(step),
                    false => self.press.saturating_add(step).min(last),
                };
                let _ = answer.say(moved);
                Standing { press: moved, ..self }
            },
            Msg::Type { word, answer } => {
                let _ = answer.say(match self.typed == word {
                    true => Word::Same,
                    false => Word::Changed,
                });
                Standing { typed: word, ..self }
            },
        }
    }
}

/// Where the panel reaches it. Cloned into every closure that used to be
/// handed the lock.
type Held = Addr<Msg>;

/// What has been typed, asked of the owner.
///
/// Nothing, if the owner has gone: the panel is on its way out by then, and
/// the folder is what the tab opens on.
fn typed_in(held: &Held) -> String {
    let Ok(typed) = held.ask(Msg::Typed) else { return String::new() };

    typed
}

// ---------------------------------------------------------- what is playing

fn playing_rows(held: &Held) -> Vec<Row> {
    let asked = player::playing();

    match asked.as_ref() {
        Some(playing) if !playing.stopped => playing_card(held, playing),
        _ => vec![Row::nothing("Nothing is playing")],
    }
}

/// Where the transport's highlight is standing, asked of the owner.
///
/// The middle press where the owner has gone, which is play: a panel whose
/// state has been taken away is a panel on its way out, and the answer that
/// matters least is the one it wants then.
fn press_at(held: &Held) -> usize {
    let Ok(press) = held.ask(Msg::Press) else { return PLAY };

    press
}

/// What the player has to say: the song, where the song is, and the row of
/// buttons the panel offers for it.
///
/// Three parts. The song, its cover and whose it is are headings the d-pad
/// walks past -- reading them is what they are for, choosing them is nothing.
/// The bar is one row the d-pad stands on: it moves a few seconds a press with
/// left and right and lands wherever a finger lands. The five buttons
/// underneath are the row the card opens on, said five times: shuffle on the
/// left, repeat on the right, the three that move between songs in the middle.
fn playing_card(held: &Held, playing: &Playing) -> Vec<Row> {
    let mut rows = Vec::new();

    // The sleeve, on its own row and up the middle. It used to be a thumbnail
    // at the end of the row that named the song, which is what a picture is
    // for on a list; this card is not a list, it is about one song, and the
    // sleeve is the thing a hand looks at first.
    //
    // Written in characters, off kew's own ramp, rather than drawn as the
    // picture it came from. Everything else on this desktop is text on a
    // plaque and a photograph in the middle of the card reads as something
    // pasted onto one; the ramp is the same sleeve said in the one alphabet
    // the rest of the screen is written in.
    //
    // What was wrong with it before was not the characters. It was six rows of
    // them squeezed onto the end of the row naming the song, and a picture
    // that came out standing up because a character cell is taller than it is
    // wide. Given the sleeve's own row and the correction kew makes for the
    // cell, it is a record again.
    //
    // The row is there whether the player has said a cover or not. The song
    // arrives before the cover does -- the title and the artist are in the
    // metadata and the picture is a file the player has still to write -- so a
    // card that only made room once there was something to put in it grew a
    // sleeve's worth taller a moment after the song changed, under a thumb
    // that had already started reading it.
    let cover = playing.art.as_deref().and_then(|art| ascii::read(art, TALL));
    let drawn = cover.unwrap_or_else(|| ascii::room(TALL));

    rows.push(Row::showing(Picture::Written(drawn.markup())));
    rows.push(info_row(playing));

    // The artist, and the album after it where the player says one. Under the
    // title and quieter than it, which is the order anybody reads them in.
    let under = match playing.album.trim().is_empty() {
        true => playing.artist.clone(),
        false => format!("{} \u{2014} {}", playing.artist, playing.album),
    };

    if !under.trim().is_empty() {
        // A title rather than a row, for the same reason the song's own name
        // is one: nothing happens to an artist. Said with `said`, it was a
        // card the width of the panel that the highlight landed on and A did
        // nothing to, which is the one thing this desktop is not allowed to
        // draw -- what looks like it can be chosen has to be choosable.
        rows.push(Row::naming("", &under).in_the_middle());
    }

    rows.push(scrub_row());
    rows.push(walking(held, transport_row(playing, press_at(held))).chief());
    about_the_song(rows, playing.path.as_deref())
}

/// Y on this card, wherever the thumb is standing on it.
///
/// It was offered on the row that names the song, which is the one row on the
/// card the highlight can never land on: a title is a heading, the d-pad walks
/// past it, and Y is asked of the row being stood on. So the button was on the
/// card and unreachable from every row of it.
///
/// The whole card is about one song, top to bottom, so every row a thumb can
/// stand on offers the same thing -- which is also what Y means everywhere
/// else here: more about the thing under the highlight.
fn about_the_song(rows: Vec<Row>, path: Option<&Path>) -> Vec<Row> {
    let Some(path) = path else { return rows };

    rows.into_iter()
        .map(|row| match row.heading() {
            Heading::Yes => row,
            Heading::No => row.offering(shown_in_the_files(path)),
        })
        .collect()
}

/// The transport, with left and right made to walk along it.
///
/// A level is what left and right already are on a row, so this is the strip
/// borrowing the thing the d-pad was doing anyway. Nothing new had to be
/// taught: up and down leave the row, left and right move within it, and A
/// takes what is under the thumb.
fn walking(held: &Held, row: Row) -> Row {
    let many = row.across.as_ref().map_or(0, |across| across.presses.len());
    let held = held.clone();

    row.levelled(Arc::new(move |by| {
        let _ = held.ask(|answer| Msg::Along { by, of: many, answer });
    }))
}

/// The song's title, under its sleeve and up the middle.
///
/// A heading rather than a thing to choose: the d-pad walks past it, and a
/// press of A on it does nothing because there is nothing to do to a title.
/// The artist is on the row below rather than beside it -- a title and an
/// artist on one line are two things a hand has to tell apart at a glance, and
/// stacked they are what they are.
fn info_row(playing: &Playing) -> Row {
    Row::naming(&playing.title, "").in_the_middle()
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
        true => (pos.float() / total.float()).clamp(0.0, 1.0),
        false => 0.0,
    };
    let wide = BAR_WIDE;
    let char_at = whole_usize(at * (wide.float() - 1.0));

    // How far in on the left and how long altogether on the right, which is
    // where a hand has read them on every player it has held. A bar on its own
    // says where in a song you are and never how much of it is left: a dot a
    // third of the way along could be a minute or ten.
    //
    // How far in is always known and how long is not -- a player that has not
    // said yet, or a stream that has no end, answers nothing. So the left is
    // always there and the right is blank until there is a number, rather than
    // both of them going when one of them is missing.
    let done = clock(pos);
    let whole = match total > 0 {
        true => clock(total),
        false => String::new(),
    };

    Row::new(&done, &whole, Does::and_stay(|_| {}))
        .picturing(Picture::Bar(Bar { at: char_at, wide }))
        .levelled(scrub_step(pos, total))
        .seeking(|showing, frac| {
            player::seek(frac);
            showing.refresh();
        })
}

/// A length in microseconds, as minutes and seconds.
///
/// Hours where a recording has them, because a set or a mix is one file and
/// `73:20` is a number somebody has to work out. Nothing here is ever longer
/// than a day, so there is no place above hours.
fn clock(micros: i64) -> String {
    let seconds = micros.max(0) / 1_000_000;
    let (hours, minutes, seconds) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);

    match hours > 0 {
        true => format!("{hours}:{minutes:02}:{seconds:02}"),
        false => format!("{minutes}:{seconds:02}"),
    }
}

/// One press of left or right on the bar, handed to the player.
fn scrub_step(pos: i64, total: i64) -> Level {
    Arc::new(move |dir| player::seek(stepped(pos, total, dir)))
}

/// Where that press lands, as a fraction of the song.
///
/// Its own function so that what a press comes to can be tried without a
/// player on the bus: everything below is one call to something that answers
/// nothing back.
///
/// It stops at both ends rather than running past them. A seek before the
/// start is a number the player has no answer for, and one past the end is the
/// next song asked for in the one way that skips whatever is between here and
/// it.
fn stepped(pos: i64, total: i64, dir: i32) -> f64 {
    let target = (pos + i64::from(dir) * SCRUB).clamp(0, total);

    target.float() / total.max(1).float()
}

/// The transport: one row, with the presses side by side across it.
///
/// It was five rows -- shuffle above previous above play above next above
/// repeat, straight down the middle of the card. That is five presses of the
/// d-pad from one end of it to the other, it is not what a music player looks
/// like anywhere anybody has held one, and it made the tallest tab on the
/// desktop out of five things a hand does in one gesture.
///
/// One row now. Left and right walk between the presses, A takes the one being
/// stood on, and each of them can still be tapped on its own. The icons are
/// the marks every player draws: the crossed arrows for the order, the loop
/// for going round, and the play or the pause in the middle where a hand looks
/// for it. What is switched on is said in mint rather than in a mark of its
/// own, which is what a row in effect wears everywhere else here.
fn transport_row(playing: &Playing, at: usize) -> Row {

    let shuffling = player::shuffling();
    let over = player::over();

    let presses = vec![
        // The one mark, lit or not. It used to be two -- crossed arrows for
        // shuffling and straight ones for the order they are in -- which says
        // the same thing twice: mint is what this desktop says "already true"
        // in, on a row in effect and on a press alike. Said twice it read as
        // two different buttons in the same place, and a hand that had learned
        // the crossed arrows had to learn the straight ones as well to find
        // out they were the same press.
        Press::new(
            "media-playlist-shuffle-symbolic",
            match shuffling {
                Order::Any => InEffect::Yes,
                Order::AsListed => InEffect::No,
            },
            move |showing| {
                player::shuffle(match player::shuffling() {
                    Order::Any => Order::AsListed,
                    Order::AsListed => Order::Any,
                });
                showing.refresh();
            },
        ),
        Press::new("media-skip-backward-symbolic", InEffect::No, |showing| {
            player::previous();
            showing.refresh();
        }),
        Press::new(
            match playing.paused {
                true => "media-playback-start-symbolic",
                false => "media-playback-pause-symbolic",
            },
            InEffect::No,
            |showing| {
                player::play_pause();
                showing.refresh();
            },
        )
        .chief(),
        Press::new("media-skip-forward-symbolic", InEffect::No, |showing| {
            player::next();
            showing.refresh();
        }),
        // Two marks and the light between them. Repeating one song and
        // repeating the whole list are two different jobs and wear two
        // different marks; not repeating at all is the same button unlit,
        // which is how everything else on this desktop says a thing is off.
        //
        // It was a third mark, `media-playlist-no-repeat-symbolic`, which is
        // not a name the icon theme this desktop is dressed in is bound to
        // have -- it is in neither Adwaita nor breeze on the machine this was
        // written on. A GTK icon nobody has draws as the broken square, and
        // the state it would have drawn for is the one the strip is in nearly
        // all the time.
        //
        // The panel offers two of the three and the player keeps the third,
        // which is where its own keyboard can leave it, so what is drawn is
        // read off the player rather than off what the panel last said.
        Press::new(
            match over {
                Over::Again => "media-playlist-repeat-song-symbolic",
                Over::On | Over::Round => "media-playlist-repeat-symbolic",
            },
            match over {
                Over::On => InEffect::No,
                Over::Again | Over::Round => InEffect::Yes,
            },
            move |showing| {
                player::repeat(match player::over() {
                    Over::Again => Over::On,
                    _ => Over::Again,
                });
                showing.refresh();
            },
        ),
    ];

    Row::pressing(presses, at)
}

// ----------------------------------------------------- what there is to play

fn music_rows(held: &Held) -> Vec<Row> {
    let typed = typed_in(held);
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
    // Read on every draw, so nothing is said about a file that is not there
    // or will not open: `looking::songs` walks the folder either way and the
    // index is only what saves it re-reading every tag.
    let said = std::fs::read_to_string(at);

    let known = match said {
        Ok(said) => looking::kept(&said),
        Err(_) => Vec::new(),
    };
    looking::songs(folder, &library::things, &known)
}

/// One thing to play. A folder is played whole, in the order it is in.
fn chosen(thing: &Thing) -> Row {
    let said = match thing.folder {
        true => "album",
        false => "",
    };
    let kind = match thing.folder {
        true => Kind::AFolder,
        false => Kind::ASong,
    };
    Row::new(&thing.name, said, plays(&thing.path, kind))
        .offering(shown_in_the_files(&thing.path))
}

/// One song a word found, said as whose it is or where it is.
fn played(song: &Song, folder: &Path) -> Row {
    Row::new(song.says(), &song.aside(folder), plays(&song.path, Kind::ASong))
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
fn plays(path: &Path, folder: Kind) -> Does {
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
        // And then keep going: the whole library, in any order, round for
        // ever, starting on this. What one press on a song means is *play*,
        // not *play this one thing and then stop*, and a handheld being
        // carried about is the last place anybody wants to come back to a
        // panel to hear a second song.
        //
        // The song goes with it because the line above may have arrived before
        // there was a player to hear it: on the press that starts one, this is
        // what tells it what to play, once it is there to be told.
        //
        // Sent rather than done here, because it has to wait for the player to
        // take its name on the bus and this is the drawing thread. `later`
        // draws the tab again when it comes back, which is when the two marks
        // on the transport light up.
        showing.later(player::onward_for(&path));
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

    if unread == 0 || !matches!(held.ask(Msg::Read), Ok(Sent::NotYet)) {
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
    let _ = held.tell(Msg::Forget);
    showing.replace(LINE);
}

// ------------------------------------------------------------------ the tabs

fn pages(held: &Held) -> Vec<Page> {
    let showing = held.clone();

    vec![
        Page::new("Playing", Rows::asked(move || playing_rows(&showing)))
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
    let reading = held.clone();
    let arriving = held.clone();
    let backing = held.clone();
    let typing = held.clone();

    Page::new("Music", Rows::asked(move || music_rows(&reading)))
        .on_arriving(move |showing| read_the_library(&arriving, showing))
        .on_back(move |showing| {
            // Out of the word first, and out of the panel only once there is
            // no word to come out of.
            if typed_in(&backing).is_empty() {
                return true;
            }

            stopped(&backing, showing);
            false
        })
        .searching(ABOUT, move |showing, word| {
            let changed = typing.ask(|answer| Msg::Type { word: word.to_string(), answer });

            // Standing on the line, which is where the letters go. The rows
            // under it are not the rows that were under it a letter ago.
            if matches!(changed, Ok(Word::Changed)) {
                showing.replace(0);
            }
        })
}

fn main() {
    if chooser::alone("music", chooser::Again::Closes) == chooser::Alone::No {
        return;
    }

    // Nothing is started here. The panel is on the screen in the time it takes
    // to draw it, and the player is started by the first thing chosen to play.
    let standing = actor::supervise(|| Standing { typed: String::new(), reading: false, press: PLAY });
    let held = standing.addr.clone();
    panel::show(Arc::new(move || pages(&held)), 0, None);
    // The panel is down and nothing is going to ask again. Waited for rather
    // than dropped, so a message already in the mailbox is finished with.
    standing.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_panel::page::Same;

    /// The two rows of the card a thumb can stand on: the bar and the strip of
    /// presses. Written as what they are rather than built by the card, which
    /// asks the player where the song is and how long it is.
    fn card() -> Vec<Row> {
        vec![
            Row::showing(Picture::Written(ascii::room(TALL).markup())),
            Row::naming("Blue Monday", "").in_the_middle(),
            Row::naming("", "New Order \u{2014} Power, Corruption & Lies").in_the_middle(),
            scrub_row(),
            Row::pressing(
                vec![Press::new("media-playback-start-symbolic", InEffect::No, |_| ())],
                0,
            ),
        ]
    }

    /// Y was offered on the row that names the song, and that row is a
    /// heading: the highlight walks past it, Y is asked of the row being stood
    /// on, and so the one row carrying it was the one row it could never be
    /// pressed from. It is on every row a thumb can stand on now, because the
    /// card is about one song from top to bottom.
    #[test]
    fn y_reaches_the_song_from_every_row_the_d_pad_can_stand_on() {
        let rows = about_the_song(card(), Some(Path::new("/music/blue-monday.flac")));
        let standing: Vec<bool> = rows
            .iter()
            .filter(|row| row.heading() == Heading::No)
            .map(|row| row.more.is_some())
            .collect();

        assert_eq!(standing, [true, true], "a row a thumb stands on with nothing behind Y");
    }

    /// The kew in the repositories does not say which file it is playing, so
    /// there is nothing to open in the files panel and the button says nothing
    /// rather than opening a folder on a guess.
    #[test]
    fn a_player_that_will_not_say_which_file_it_is_offers_nothing() {
        let rows = about_the_song(card(), None);
        assert!(rows.iter().all(|row| row.more.is_none()));
    }

    /// The words on the card are read, not chosen. Said with `said`, the
    /// artist was a card the width of the panel that the highlight landed on
    /// and A did nothing to.
    #[test]
    fn nothing_on_the_card_that_cannot_be_pressed_is_stood_on() {
        let readable = [0, 1, 2];

        for at in readable {
            assert_eq!(card()[at].heading(), Heading::Yes, "row {at} is stood on for nothing");
        }
    }

    /// The sleeve's square is held from the moment the song changes. The
    /// player says the title before it says where the cover is, so a row that
    /// waited for the picture grew the card a sleeve's worth taller under a
    /// thumb already reading it.
    ///
    /// The room is the grid, and the grid is what the crate is asked for
    /// whether there is a picture to put in it or not: the same rows, the same
    /// columns, and nothing written on them yet.
    #[test]
    fn the_sleeve_keeps_its_room_before_there_is_a_cover_for_it() {
        let room = ascii::room(TALL);
        let empty = Row::showing(Picture::Written(room.markup()));

        assert_eq!(room.plain().lines().count(), TALL);
        assert!(room.plain().lines().all(|line| line.chars().count() == room.cols));
        assert_eq!(empty.looks_like(&Row::showing(Picture::None)), Same::No);
    }

    /// Five seconds a press, whatever is playing. A twentieth of the song is a
    /// press that means nine seconds on one song and half a minute on the
    /// next, which is a step no thumb can learn.
    #[test]
    fn the_bar_steps_by_the_same_few_seconds_whatever_the_song_is() {
        let single = 3 * 60 * 1_000_000;
        let mix = 73 * 60 * 1_000_000;
        // What the press moved, in seconds, out of the fraction it asked the
        // player for.
        let moved = |total: i64| {
            (stepped(30_000_000, total, 1) * total.float() - 30_000_000.0) / 1_000_000.0
        };

        assert!((moved(single) - 5.0).abs() < 0.001, "a single moved {}", moved(single));
        assert!((moved(mix) - 5.0).abs() < 0.001, "a mix moved {}", moved(mix));
    }

    /// And stops at both ends. Before the start is a number the player has no
    /// answer for; past the end is the next song, asked for in the one way
    /// that skips whatever is between here and it.
    #[test]
    fn the_bar_stops_at_both_ends_of_the_song() {
        let song = 3 * 60 * 1_000_000;
        assert!(stepped(1_000_000, song, -1) < f64::EPSILON);
        assert!((stepped(song - 1_000_000, song, 1) - 1.0).abs() < f64::EPSILON);
    }
}
