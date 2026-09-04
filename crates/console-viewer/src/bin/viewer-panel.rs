//! A photograph and a film, drawn.
//!
//!     viewer-panel ~/Pictures/beach.jpg
//!     viewer-panel ~/Pictures
//!     viewer-panel
//!
//! Opened by pressing A on a picture in the files panel, which is xdg-open,
//! which is `console-viewer.desktop`, which is this. Handed a folder instead
//! it opens on the first thing in it that can be shown, and handed nothing at
//! all it opens the pictures folder: the same entry is on the home screen, and
//! a card on the home screen that starts a program which prints a usage line to
//! a stderr nobody can see is a card that does nothing.
//!
//! What is here is the reading of a disk and the drawing of a card. Everything
//! that is a decision -- which things in a folder can be shown, which one is
//! next, what the row under the picture says -- is `console_viewer`, where it
//! is tested without either.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_number::fitted;
use console_panel::page::{Bar, Does, InEffect, Page, Picture, Press, Row, Rows, Stirred, Watch};
use console_panel::{chooser, panel};
use console_viewer::kinds::Kind;
use console_viewer::playing::{Captions, Running};
use console_viewer::reel::{Reel, Stood};
use console_viewer::waking::{self, Awake};
use console_viewer::{playing, saying};
// The pipeline's own prelude and not GTK's. Both re-export glib's, and the two
// names for a gesture's `set_state` and an element's are the same name.
use gstreamer::prelude::*;

/// What kind of thing a file is, as the machine says it.
///
/// Asked of the shared mime database through glib, which is the same answer
/// the files panel and xdg-open get, so a file this refuses is a file nothing
/// else here would have sent it either. By name and not by content: reading
/// the front of every file in a folder of two hundred photographs is two
/// hundred opens before the first one is drawn, and the name is what the rest
/// of this desktop decides by.
fn kind_of(path: &Path) -> String {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return String::new();
    };

    let (kind, _) = gtk4::gio::functions::content_type_guess(Some(name), None::<&[u8]>);

    gtk4::gio::functions::content_type_get_mime_type(&kind)
        .map(|said| said.to_string())
        .unwrap_or_default()
}

/// A folder, read the way a listing reads it.
///
/// Sorted by name, because that is the order the files panel shows and the
/// order a camera's own numbering puts a morning in. Not sorted by date, which
/// would be a second opinion about a folder somebody was just looking at.
fn listing(folder: &Path) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| !entry.path().is_dir())
        .filter_map(|entry| {
            // A name that is not text is a name this cannot work with: the
            // kind is guessed from it, the card says it, and the reel stands
            // on it. Left out of the reel rather than shown as something the
            // panel then cannot open, and said out loud, because a photograph
            // missing from a folder is the kind of thing somebody notices and
            // has nowhere to look.
            let Ok(name) = entry.file_name().into_string() else {
                eprintln!("viewer-panel: {:?}: this name is not text", entry.file_name());

                return None;
            };

            // A dotfile is not shown anywhere else on this desktop.
            match name.starts_with('.') {
                true => None,
                false => Some((name, kind_of(&entry.path()))),
            }
        })
        .collect();

    found.sort_by(|(one, _), (two, _)| one.cmp(two));
    found
}

/// Where the panel is looking: the folder, and which of it is in front.
struct Looking {
    folder: PathBuf,
    reel: Reel,
    /// Where the film has got to, and whether it is running. Left alone for a
    /// picture, which has no transport.
    along: playing::Along,
    running: playing::Running,
    /// Where a press has asked the film to go, until the decoder has been told.
    ///
    /// A press happens where the rows are read, which is not the thread the
    /// decoder may be spoken to on, so it cannot do the seek itself. It writes
    /// down where it wants to be and [`reeling`] does it on the next drawing,
    /// which is the same tick the press asked for anyway.
    ///
    /// Nothing means nobody has asked, which is not the same as asking for the
    /// start: a film paused two minutes in is redrawn every second, and a
    /// standing request to go to nought would drag it back there each time.
    sought: Option<u64>,
    /// How fast the film is asked to run, as a place in [`playing::SPEEDS`].
    speed: usize,
    /// Which of the film's written words are on, if any.
    captions: Captions,
    /// How many the decoder found, which is only knowable once it has read the
    /// file. Nought until then, and a menu of one row -- off -- until it has
    /// said otherwise.
    tracks: usize,
    /// When the card was last pressed, which is what decides whether it is
    /// still showing its controls.
    ///
    /// An instant and not a countdown, because the card is redrawn on a tick it
    /// does not own and a counter would go down once per drawing rather than
    /// once per second. Every press writes this, whatever the press was for.
    stirred: Instant,
}

impl Looking {
    /// What was asked for, as a folder and a place in it.
    ///
    /// A file opens the folder it is in, standing on it. A folder opens on the
    /// first thing in it that can be shown. Nothing that can be shown at all
    /// is no panel: there is no card worth drawing and saying so on one would
    /// be a card whose only content is that it is empty.
    fn of(asked: &Path) -> Option<Looking> {
        let (folder, opened) = match asked.is_dir() {
            true => (asked.to_path_buf(), String::new()),
            false => (
                asked.parent().unwrap_or(Path::new(".")).to_path_buf(),
                asked.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string(),
            ),
        };

        let reel = Reel::of(&listing(&folder), &opened)?;

        Some(Looking {
            folder,
            reel,
            along: playing::Along::default(),
            running: playing::Running::default(),
            sought: None,
            speed: playing::ordinary(),
            captions: Captions::default(),
            tracks: 0,
            stirred: Instant::now(),
        })
    }

    /// How long the card has been left alone.
    fn since(&self) -> Duration {
        self.stirred.elapsed()
    }

    /// Somebody pressed something. The controls are wanted again.
    fn stir(&mut self) {
        self.stirred = Instant::now();
    }

    /// Where the thing being shown is.
    fn at(&self) -> PathBuf {
        self.folder.join(&self.reel.showing().name)
    }

    /// How big the file is, for the row that says what this is.
    ///
    /// Nought where the disk will not say, which happens when the file has
    /// gone between the folder being read and this row being drawn. It is on
    /// the card for one draw -- [`rows`] notices the same thing and reads the
    /// folder again -- and it is said aloud rather than folded into a default,
    /// because a stat that fails for any other reason is a fault worth seeing.
    fn bytes(&self) -> u64 {
        match std::fs::metadata(self.at()) {
            Ok(held) => held.len(),
            Err(fault) => {
                eprintln!("viewer-panel: {}: {fault}", self.at().display());

                0
            }
        }
    }

    /// How big the picture is, without decoding it.
    ///
    /// `file_info` reads the header and stops, so this costs one open however
    /// large the photograph is. Nothing for a film, whose shape is the
    /// decoder's to say and is not asked for here.
    fn shape(&self) -> (u32, u32) {
        match self.reel.showing().kind {
            Kind::Film => (0, 0),
            Kind::Picture => match gtk4::gdk_pixbuf::Pixbuf::file_info(self.at()) {
                Some((_, wide, tall)) => (wide.unsigned_abs(), tall.unsigned_abs()),
                None => (0, 0),
            },
        }
    }

    /// Walk the folder, and start whatever is landed on from the beginning.
    fn step(&mut self, by: isize) {
        self.reel.step(by);
        self.rewound();
    }

    /// Back to the start, stopped, with nothing asked for.
    ///
    /// What landing on something new means. Written once because the three of
    /// them have to move together: a position left over from the last film is a
    /// card claiming to be two minutes into something it has not started, and a
    /// seek left over is that position being asked for out loud.
    fn rewound(&mut self) {
        self.along = playing::Along::default();
        self.running = playing::Running::default();
        self.sought = None;
        // Which words are on is about the film that had them. Speed is not: it
        // is how fast somebody watches, and carrying it to the next film is the
        // difference between a setting and a thing that has to be set again
        // every time the folder is walked.
        self.captions = Captions::default();
        self.tracks = 0;
    }

    /// Read the folder again, staying on what was being looked at.
    ///
    /// The panel redraws on a timer it does not own, and a folder is a thing
    /// somebody else can change while a card is up: a download lands, a file
    /// is thrown away from the files panel on another tab. Standing on the
    /// name rather than the number means the picture under the thumb is the
    /// picture that was under the thumb.
    fn again(&mut self) {
        let name = self.reel.showing().name.clone();

        let Some(reel) = Reel::of(&listing(&self.folder), &name) else { return };

        self.reel = reel;

        if self.reel.stand_on(&name) == Stood::NotThere {
            self.rewound();
        }
    }
}

type Held = Arc<Mutex<Looking>>;

thread_local! {
    /// The one film this panel has open, and what is reading it.
    ///
    /// A thread local because a surface GTK draws with may not leave the thread
    /// that draws, and one at a time because a card shows one thing: walking to
    /// the next film takes the last one down rather than leaving a decoder
    /// running behind a picture nobody is looking at.
    static REEL: RefCell<Option<Reeling>> = const { RefCell::new(None) };
}

/// A film, open.
struct Reeling {
    /// Which one, so the next drawing knows whether this is still it.
    at: PathBuf,
    /// The pipeline reading it, which is what a press is answered by.
    play: gstreamer::Element,
    /// What it paints on, which is what the card draws.
    surface: gtk4::gdk::Paintable,
    /// What it has already been told, so it is not told again on every drawing.
    ///
    /// A card is drawn every second, and a rate is changed by seeking: a
    /// pipeline told its own rate once a second is a film that flushes its
    /// decoder once a second, which on a handheld is a film that stutters
    /// rather than plays.
    rate: f64,
    /// Nothing until the first drawing, which is what makes that drawing tell
    /// the pipeline what the card wants rather than assume the two agree.
    told: Option<Captions>,
}

impl Drop for Reeling {
    /// A pipeline is not finished with when it is dropped.
    ///
    /// It holds the file open and its threads keep running until it is told to
    /// stop, so walking through a folder of films would leave one decoder
    /// behind per press.
    fn drop(&mut self) {
        if let Err(fault) = self.play.set_state(gstreamer::State::Null) {
            eprintln!("viewer-panel: {}: will not stop: {fault}", self.at.display());
        }
    }
}

/// The file of written words beside a film, where there is one.
///
/// The looking. Which names to look under is [`playing::beside`], which is
/// arithmetic on a name and is tested without a disk; this is the disk.
fn words_beside(at: &Path) -> Option<PathBuf> {
    let name = at.file_name().and_then(|name| name.to_str())?;
    let folder = at.parent()?;

    playing::beside(name).into_iter().map(|name| folder.join(name)).find(|beside| beside.is_file())
}

/// A film opened, or nothing where it will not open.
///
/// `playbin` is asked to do the whole of the reading -- which demuxer, which
/// decoder, which of several streams -- because every one of those answers is a
/// package on this machine rather than a decision this panel is in a position
/// to make. What is not left to it is where the picture goes:
/// `gtk4paintablesink` is named outright, because GTK's own way of drawing a
/// film does not work here at all and a pipeline left to choose its own sink
/// would open a window of its own.
///
/// # Why `playbin` and not `playbin3`
///
/// `playbin3` is the newer one and this was written against it first. It has no
/// `current-text` and no `n-text`: choosing a subtitle track on it means
/// listening for a stream collection on the bus and sending a select-streams
/// event back, which is a conversation held over time. `playbin` answers both
/// questions as properties, which is a line each and can be asked on the
/// drawing thread where everything else here happens.
///
/// The cost is that `playbin` is the older element. It is not gone and it is
/// not broken, and what would make it worth the conversation is a film this
/// cannot switch the words on -- which is a thing to find out from a real file
/// rather than from a deprecation note.
///
/// It comes up paused, which is a decision and not an accident. A card that
/// started playing as it was drawn would make noise before anybody had decided
/// to watch anything -- and pausing is also what puts the first frame on the
/// screen, so the card has a picture on it rather than a black rectangle.
fn open(at: &Path) -> Option<Reeling> {
    let uri = match gtk4::glib::filename_to_uri(at, None) {
        Ok(uri) => uri,
        Err(fault) => {
            eprintln!("viewer-panel: {}: {fault}", at.display());

            return None;
        }
    };

    let sink = match gstreamer::ElementFactory::make("gtk4paintablesink").build() {
        Ok(sink) => sink,
        Err(fault) => {
            // The one worth saying the package name in. Everything else here
            // is a broken file; this is a machine that was never given what it
            // takes to draw a film, and the fault is `desktop.conf`'s.
            eprintln!("viewer-panel: no gtk4paintablesink, so no film can be drawn: {fault}");
            eprintln!("viewer-panel: it is gst-plugin-gtk4, which desktop.conf names");

            return None;
        }
    };

    let surface: gtk4::gdk::Paintable = sink.property("paintable");

    let mut building = gstreamer::ElementFactory::make("playbin")
        .property("uri", &uri)
        .property("video-sink", &sink);

    // Words in a file of their own, beside the film, which is how most of them
    // arrive. The decoder finds the ones written into the film on its own and
    // has no reason to look in the folder for the rest, so this is the looking.
    if let Some(beside) = words_beside(at) {
        match gtk4::glib::filename_to_uri(&beside, None) {
            Ok(uri) => building = building.property("suburi", uri),
            Err(fault) => eprintln!("viewer-panel: {}: {fault}", beside.display()),
        }
    }

    let play = match building.build() {
        Ok(play) => play,
        Err(fault) => {
            eprintln!("viewer-panel: {}: {fault}", at.display());

            return None;
        }
    };

    if let Err(fault) = play.set_state(gstreamer::State::Paused) {
        eprintln!("viewer-panel: {}: will not open: {fault}", at.display());

        return None;
    }

    Some(Reeling {
        at: at.to_path_buf(),
        play,
        surface,
        rate: 1.0,
        told: None,
    })
}

/// What the player calls the track being shown, and how many there are.
const TEXT: &str = "current-text";
const TRACKS: &str = "n-text";

/// Whether the player has a property by that name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Has {
    It,
    Not,
}

/// Whether the player has a property, before it is asked for by name.
///
/// Asking a GObject for a property it does not have is a panic, and every one
/// of these is asked for by a name written down here rather than checked by the
/// compiler. The player is chosen by this panel and does have them, so this is
/// about the day somebody changes which player is used: it should be a film
/// without subtitles, said once on the way past, and not a panel that vanishes.
fn has(play: &gstreamer::Element, named: &str) -> Has {
    match play.find_property(named).is_some() {
        true => Has::It,
        false => {
            eprintln!("viewer-panel: this player has no {named}, so subtitles cannot be chosen");

            Has::Not
        }
    }
}

/// The film the card wants, and the decoder made to agree with what it says.
///
/// Called on every drawing, on the thread that draws, which is the only thread
/// the pipeline is spoken to on. Three things happen here and they happen in
/// this order for a reason: the film is opened or found again, what a press
/// asked for is done, and only then is the card told where the film really got
/// to. Reading first would answer with the position the seek was about to leave.
fn reeling(held: &Held, at: &Path) -> Option<gtk4::gdk::Paintable> {
    REEL.with_borrow_mut(|reel| {
        if reel.as_ref().is_none_or(|open| open.at != at) {
            *reel = open(at);
        }

        let open = reel.as_mut()?;

        let Ok(mut looking) = held.lock() else { return Some(open.surface.clone()) };

        if let Some(to) = looking.sought.take() {
            let flags = gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT;

            if let Err(fault) = open.play.seek_simple(flags, gstreamer::ClockTime::from_seconds(to)) {
                eprintln!("viewer-panel: {}: will not seek: {fault}", at.display());
            }
        }

        // How fast, which is a seek and not a setting: a pipeline is told its
        // rate by being asked to play from here at that rate. Done only where
        // it has changed, because a seek flushes the decoder and a flush every
        // second is a film that stutters rather than plays.
        let (_, rate) = playing::speed(looking.speed);

        if rate != open.rate {
            let at_now = open
                .play
                .query_position::<gstreamer::ClockTime>()
                .unwrap_or(gstreamer::ClockTime::ZERO);
            let flags = gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::ACCURATE;

            match open.play.seek(
                rate,
                flags,
                gstreamer::SeekType::Set,
                at_now,
                gstreamer::SeekType::End,
                gstreamer::ClockTime::ZERO,
            ) {
                Ok(()) => open.rate = rate,
                Err(fault) => {
                    eprintln!("viewer-panel: {}: will not run at {rate}: {fault}", at.display());
                }
            }
        }

        // Which written words are shown, and how many there are to choose
        // from. Both are the player's own properties, and both are asked for by
        // name -- so a player that turns out not to have them says so once and
        // the card goes on drawing the film. A missing property is a panic, and
        // a panel that dies on a device with no terminal is the one outcome
        // worse than a film with no subtitles.
        if has(&open.play, TEXT) == Has::It && has(&open.play, TRACKS) == Has::It {
            if open.told != Some(looking.captions) {
                let told = match looking.captions.track() {
                    // Nought less than nothing is what the player takes as none
                    // of them, so it is a number either way round rather than a
                    // number and a switch.
                    Some(track) => fitted(track),
                    None => -1,
                };

                open.play.set_property(TEXT, told);
                open.told = Some(looking.captions);
            }

            looking.tracks = fitted(open.play.property::<i32>(TRACKS).max(0));
        }

        let wanted = match looking.running {
            Running::Yes => gstreamer::State::Playing,
            Running::Paused => gstreamer::State::Paused,
        };

        if let Err(fault) = open.play.set_state(wanted) {
            eprintln!("viewer-panel: {}: will not {wanted:?}: {fault}", at.display());
        }

        // What the decoder says, and nothing where it has not read far enough
        // to say it. Both are left alone rather than written as nought: the
        // caption leaves out what nothing has told it, and a length that
        // arrived as zero would be a length the card had been told.
        if let Some(now) = open.play.query_position::<gstreamer::ClockTime>() {
            looking.along.at = now.seconds();
        }

        if let Some(whole) = open.play.query_duration::<gstreamer::ClockTime>() {
            looking.along.whole = whole.seconds();
        }

        Some(open.surface.clone())
    })
}


/// The card: the picture, what it is, and how to walk the folder.
///
/// Three rows and no more. A viewer is a surface somebody came to look at
/// something on, so everything that is not the thing being looked at is a
/// caption -- which is why the picture is the first row and the row that opens
/// under the highlight, and why what the folder is called is not on the card
/// at all.
fn rows(held: &Held) -> Vec<Row> {
    let Ok(mut looking) = held.lock() else { return Vec::new() };

    // A folder is a thing somebody else can change while a card is up: a file
    // thrown away from the files panel on another tab, a download landing.
    // Asked only when what is being shown has gone, which is one stat rather
    // than a folder read on every redraw -- and it is the only case where what
    // the card is drawing has stopped being true.
    if !looking.at().exists() {
        looking.again();
    }

    let shot = looking.reel.showing();
    let at = looking.at();
    let (wide, tall) = looking.shape();
    let shown = at.exists().then(|| at.clone());

    // A photograph is opened, drawn and forgotten; a film is a decoder the
    // drawing has to keep hold of. Which of the two is the whole of the
    // difference between the cards, and it is decided here rather than in the
    // framework, which knows how big a film is drawn and nothing about how one
    // is read.
    let mut every = vec![
        match shot.kind {
            Kind::Picture => Row::showing(Picture::Showing(shown.clone())),
            Kind::Film => Row::showing(Picture::Playing(shown.clone())),
        }
        // A on the picture fills the screen with it, which is the press
        // somebody came here to make. It is the row the card opens standing on
        // for the same reason: the first press of A does the obvious thing
        // without a walk down the card first.
        .choosing(Does::and_stay(|showing| showing.open_out()))
        .chief(),
    ];

    // Left alone for a while, that is the whole card.
    //
    // The rows below are the controls, and controls are what a hand is holding
    // rather than what a film is watched through. They go together and they
    // come back together, on the first press of anything -- which the panel
    // spends on bringing them back rather than on what it looked like, because
    // the row a thumb was reaching for is not where it was.
    //
    // Every point they were taking is the picture's while they are gone: the
    // panel counts what a tab wrote under the picture and gives the picture the
    // rest, so this is a bigger photograph and a bigger film and not a card
    // with a hole in the bottom of it.
    if waking::awake(looking.since()) == Awake::No {
        return every;
    }

    // What this is, under it. A file that will not open says so here rather
    // than leaving the card with a hole in it and nothing to read.
    let under = match shown.is_some() {
        true => saying::under(
            shot.kind,
            console_viewer::fitting::Size::new(wide, tall),
            looking.bytes(),
            looking.along,
        ),
        false => saying::wont_open(&shot.name),
    };

    let said = Row::said(&shot.name, &under).in_the_middle();

    every.push(match shot.kind {
        // The name wears the walk: the file before this one and the file
        // after it, either side of which file this is, which is where a
        // finger looks for them on anything that pages. A film's walk stays
        // on the transport instead, where a music player's hands already
        // know it.
        Kind::Picture => {
            let walking = Arc::clone(held);
            said.levelled(Arc::new(move |by| walk(&walking, fitted(by)))).ended("‹", "›")
        }
        Kind::Film => {
            let asking = Arc::clone(held);
            let name = shot.name.clone();
            let tracks = looking.tracks;

            // The card stays up: the questions are about the film in front,
            // and the answer worth standing on afterwards is it.
            said.offering(move |showing| {
                what_else(&asking, showing, &name, tracks);

                false
            })
        }
    });

    // And the bar, which is the music card's bar.
    //
    // It says the same two things in the same two places -- how far in on the
    // left, how long altogether on the right -- and it is scrubbed the same
    // two ways: left and right for a step, a finger anywhere along it for the
    // place it landed. A person who has used the music panel has used this,
    // which is the whole argument for it being the same row.
    if shot.kind == Kind::Film {
        every.push(bar_row(held, looking.along));
    }

    // And the third row, which is a different row for each kind and is the
    // last one the card has room for. A photograph's is where in the folder it
    // is; a film's is the transport, because a film that cannot be started is
    // not being shown at all.
    match shot.kind {
        Kind::Film => every.push(transport(held, &looking)),
        Kind::Picture => {
            // Which of how many, and the two presses that walk them. Left off a
            // folder holding one thing, where it would be a row saying "1 of 1"
            // beside two presses that do nothing.
            if looking.reel.many() > 1 {
                let stepping = Arc::clone(held);
                let onward = Arc::clone(held);
                every.push(
                    Row::new(
                        &format!("{} of {}", looking.reel.which(), looking.reel.many()),
                        "",
                        Does::and_stay(move |showing| {
                            walk(&onward, 1);
                            showing.refresh();
                        }),
                    )
                    // Left and right walk the folder from here too. Nothing
                    // here asks for a redraw: a level press is answered by the
                    // panel asking for the rows again, which is the same rule
                    // the music card scrubs under. The two presses a finger
                    // aims at are the name's, so this row draws no ends of its
                    // own -- a second pair under the first was two answers to
                    // one question.
                    .levelled(Arc::new(move |by| walk(&stepping, fitted(by))))
                    .ended("", ""),
                );
            }
        }
    }

    every
}

/// Y on a film: what can be done with one that is not a press on the card.
///
/// Everything a thumb does often is a press on the card -- start it, stop it,
/// move along it, walk the folder. What is left is the pair of settings a
/// person changes once and then forgets -- how fast it runs, and whether the
/// words are on -- and the way to the whole screen, which A on the picture
/// already is but which nothing on the card says out loud. Those go behind Y,
/// which is where everything else on this desktop puts what else can be done
/// with the thing in front of you.
///
/// Questions in turn and not one list, because each is answered by pressing
/// one of a few buttons side by side and a row holding every speed and every
/// language at once is a row nobody can read. The first asks which of them,
/// the second asks the thing itself.
fn what_else(held: &Held, showing: &dyn console_panel::page::Showing, name: &str, tracks: usize) {
    let speeding = Arc::clone(held);
    let wording = Arc::clone(held);
    let about = name.to_string();

    showing.sure(
        "About this film",
        name,
        &["Speed", "Subtitles", "Full screen"],
        Arc::new(move |showing, which| match which {
            0 => how_fast(&speeding, showing, &about),
            1 => which_words(&wording, showing, &about, tracks),
            _ => showing.open_out(),
        }),
    );
}

/// How fast it runs.
fn how_fast(held: &Held, showing: &dyn console_panel::page::Showing, name: &str) {
    let setting = Arc::clone(held);
    let says: Vec<&str> = playing::SPEEDS.iter().map(|(says, _)| *says).collect();

    showing.sure(
        "How fast",
        name,
        &says,
        Arc::new(move |_, which| {
            if let Ok(mut looking) = setting.lock() {
                looking.speed = which;
            }
        }),
    );
}

/// Which of the written words are shown, or none of them.
fn which_words(held: &Held, showing: &dyn console_panel::page::Showing, name: &str, tracks: usize) {
    let setting = Arc::clone(held);
    let said = playing::captions(tracks);
    let says: Vec<&str> = said.iter().map(String::as_str).collect();

    showing.sure(
        "Subtitles",
        name,
        &says,
        Arc::new(move |_, which| {
            if let Ok(mut looking) = setting.lock() {
                looking.captions = playing::Captions::chosen(which);
            }
        }),
    );
}

/// The transport under a film: start it, stop it, and walk the folder.
///
/// The shape of a music player's, because that is the shape a hand already
/// knows: the middle press is the one it makes without looking, and the two
/// beside it are the ones it aims at. Where this parts company is what the
/// outer two do -- previous and next file rather than previous and next track,
/// because there is no queue here, only what is in the folder.
///
/// They are left off a folder holding one film, where they would be two marks
/// either side of the play button that do nothing when pressed.
fn transport(held: &Held, looking: &Looking) -> Row {
    let running = looking.running;
    let alone = looking.reel.many() <= 1;
    let (back, on, forward) = (Arc::clone(held), Arc::clone(held), Arc::clone(held));

    let mut presses = Vec::new();

    if !alone {
        presses.push(Press::new("media-skip-backward-symbolic", InEffect::No, move |showing| {
            walk(&back, -1);
            showing.refresh();
        }));
    }

    // The mark for what the press will do and not for what is happening, which
    // is `Running::icon`'s whole argument and is tested there.
    presses.push(
        Press::new(running.icon(), InEffect::No, move |showing| {
            if let Ok(mut looking) = on.lock() {
                looking.running = looking.running.other();
            }

            showing.refresh();
        })
        .chief(),
    );

    if !alone {
        presses.push(Press::new("media-skip-forward-symbolic", InEffect::No, move |showing| {
            walk(&forward, 1);
            showing.refresh();
        }));
    }

    // Opening on the play button rather than on the first of them. It is the
    // press somebody came to make, and standing on it means a film is watched
    // with one press of A rather than with a walk along a row first.
    let at = match alone {
        true => 0,
        false => 1,
    };

    Row::pressing(presses, at)
}

/// The bar under a film, which is the music card's bar.
///
/// How far in on the left and how long altogether on the right, a dot on a
/// strip between them, and the two ways a thumb moves it: left and right for a
/// step, a finger anywhere along it for the place it landed. Nothing about the
/// arithmetic is here -- where the dot sits is [`playing::dot`], where a tap
/// lands is [`playing::Along::sought`] -- so what a bar does at either end of a
/// film is a question with an answer on a laptop.
///
/// A film the decoder has not measured yet says nothing on the right rather
/// than a made-up total. `0:12` alone is true; `0:12 of 0:00` is a card
/// claiming to know something it does not.
fn bar_row(held: &Held, along: playing::Along) -> Row {
    let stepping = Arc::clone(held);
    let tapped = Arc::clone(held);

    let whole = match along.whole > 0 {
        true => playing::clock(along.whole),
        false => String::new(),
    };

    Row::new(&playing::clock(along.at), &whole, Does::and_stay(|_| {}))
        .picturing(Picture::Bar(Bar { at: playing::dot(along, BAR_WIDE), wide: BAR_WIDE }))
        .levelled(Arc::new(move |by| scrub(&stepping, by)))
        .seeking(move |showing, fraction| {
            seek_to(&tapped, fraction);
            showing.refresh();
        })
}

/// How many characters the bar is drawn across.
///
/// The music card's number, so that the two bars are the same length on the
/// same screen and a thumb that has learned one has learned the other.
const BAR_WIDE: usize = 40;

/// A finger landing somewhere along the bar.
///
/// Written down rather than done, for the same reason as [`scrub`]: this runs
/// where the rows are read and the decoder may only be spoken to on the thread
/// that draws. A tap is also a press, so it wakes the card like any other.
fn seek_to(held: &Held, fraction: f64) {
    if let Ok(mut looking) = held.lock() {
        looking.stir();
        looking.along = looking.along.sought(fraction);
        looking.sought = Some(looking.along.at);
    }
}

/// One press of left or right on a film: five seconds along it.
///
/// Written down rather than done, because this runs where the rows are read and
/// the decoder may only be spoken to on the thread that draws. [`reeling`] does
/// it on the drawing this press already asked for.
fn scrub(held: &Held, by: i32) {
    if let Ok(mut looking) = held.lock() {
        looking.stir();

        let step = i64::from(by).saturating_mul(fitted(playing::STEP));

        looking.along = looking.along.moved(step);
        looking.sought = Some(looking.along.at);
    }
}

/// One press of left or right: the next thing in the folder.
fn walk(held: &Held, by: isize) {
    if let Ok(mut looking) = held.lock() {
        looking.step(by);
    }
}

/// Where the card is, in the tabs this panel has.
///
/// Named rather than written as a number in two places: the folder list turns
/// back to the card when something is chosen on it, and a number that is right
/// in one of those places and stale in the other is a press that lands on the
/// wrong tab.
const CARD: usize = 0;

/// Everything in the folder that this panel can open, as a list.
///
/// The same shape as the music panel's second tab and there for the same
/// reason. Walking a folder with left and right is right for the next one and
/// the one before; it is not a way to get to the fortieth photograph of an
/// afternoon, and it says nothing about what is in the folder that you have not
/// walked to yet.
///
/// Both kinds together rather than one list of films and another of
/// photographs. It is one folder and this panel opens both, and each row says
/// which it is -- a still one carries a picture of itself, a film carries the
/// mark films wear everywhere else here. Splitting them would be two tabs that
/// are empty most of the time, because a folder is usually all of one thing.
fn folder_rows(held: &Held) -> Vec<Row> {
    let Ok(looking) = held.lock() else { return Vec::new() };

    let here = looking.reel.which();

    looking
        .reel
        .every()
        .iter()
        .enumerate()
        .map(|(at, shot)| {
            let going = Arc::clone(held);
            let name = shot.name.clone();
            let picture = match shot.kind {
                Kind::Picture => Picture::At(looking.folder.join(&shot.name)),
                Kind::Film => Picture::Named("video-x-generic-symbolic"),
            };

            // The one being shown wears the word every other list here says
            // "already true" with.
            let aside = match at + 1 == here {
                true => console_panel::page::NOW,
                false => "",
            };

            Row::new(
                &shot.name,
                aside,
                Does::and_stay(move |showing| {
                    stand_on(&going, &name);
                    // Back to the card, because choosing here has done its work
                    // there. Leaving somebody on the list they have just
                    // finished with is a press they have to guess at.
                    showing.turn_to(CARD);
                }),
            )
            .picturing(picture)
        })
        .collect()
}

/// Stand the card on something the folder list was pressed on.
fn stand_on(held: &Held, name: &str) {
    if let Ok(mut looking) = held.lock() {
        // Whether it was found or not, the card is about something else now:
        // a position and a running film belong to what was being shown a moment
        // ago, and a name that has gone from the folder leaves the reel
        // somewhere else again.
        looking.reel.stand_on(name);
        looking.rewound();
        // Chosen on the other tab, which is a press. The card it turns back to
        // is a card somebody has just asked for, so it comes up with its
        // controls rather than having to be woken.
        looking.stir();
    }
}

/// Every press on the card, before the panel acts on it.
///
/// Two things at once, and they have to be in this order. What the press means
/// is decided against the card as it was -- quiet, so the press is spent waking
/// it -- and then the card is marked as pressed, so the rows the next reading
/// builds are the full set.
///
/// The Folder tab has none of this. It is a list and a list has nothing to get
/// out of the way of.
fn stir(held: &Held) -> Stirred {
    let Ok(mut looking) = held.lock() else { return Stirred::Awake };

    let was = waking::awake(looking.since());
    looking.stir();

    match was {
        Awake::Yes => Stirred::Awake,
        Awake::No => Stirred::Woke,
    }
}

fn pages(held: &Held) -> Vec<Page> {
    let drawing = Arc::clone(held);
    let stirring = Arc::clone(held);
    let listing = Arc::clone(held);

    vec![
        Page::new("Looking", Rows::asked(move || rows(&drawing)))
            // A second, which is what an eye reads as the clock moving and is
            // the same tick the music card's bar runs on. It ticks over a
            // photograph too, and costs nothing there: the rows come back
            // saying exactly what they said before, and a card whose rows have
            // not changed is left alone rather than built again.
            //
            // It is also what moves a film at all. A press writes down where it
            // wants the film to be and the drawing is what tells the decoder,
            // so a card that was only drawn when something was pressed would be
            // a film that only advanced when something was pressed.
            .watching(Watch::on(&["sh", "-c", "while true; do echo tick; sleep 1; done"], "tick"))
            // And what that tick is measured against. The rows under the
            // picture go away when nobody has pressed anything for a while,
            // and the press that brings them back is spent doing only that.
            .stirring(move || stir(&stirring)),
        Page::new("Folder", Rows::asked(move || folder_rows(&listing))),
    ]
}

/// What to open when nothing was named.
///
/// The pictures folder, asked of glib rather than joined onto the home
/// directory, because that is the same answer every other program on this
/// machine gets and it is hers to move. Nothing where there is no such folder,
/// which is a machine that has never had a photograph on it and is a usage line
/// rather than a card saying so.
fn by_default() -> Option<PathBuf> {
    gtk4::glib::user_special_dir(gtk4::glib::UserDirectory::Pictures)
        .filter(|folder| folder.is_dir())
}

fn main() {
    let asked = match std::env::args().nth(1) {
        Some(asked) if !asked.is_empty() => PathBuf::from(asked),

        // Pressed on the home screen rather than opened onto a file. There is
        // one entry and it opens what a person keeps pictures in; a film is
        // reached from the files panel, or by walking the folder it is in.
        _ => {
            let Some(folder) = by_default() else {
                eprintln!("usage: viewer-panel FILE-OR-FOLDER");
                return;
            };

            folder
        }
    };

    let Some(looking) = Looking::of(&asked) else {
        eprintln!("viewer-panel: {}: nothing here is a picture or a film", asked.display());
        return;
    };

    // Before anything is drawn, because the first card may be a film. A
    // photograph does not need it and is not stopped by it failing: what a
    // machine with no GStreamer on it can still do is open pictures, and a
    // panel that refused to start would take that away as well.
    if let Err(fault) = gstreamer::init() {
        eprintln!("viewer-panel: no film can be read: {fault}");
    }

    // One card per thing being looked at, so opening a second photograph from
    // the files panel replaces the first rather than stacking two viewers over
    // each other.
    if chooser::alone("viewer", chooser::Again::Closes) == chooser::Alone::No {
        return;
    }

    let held: Held = Arc::new(Mutex::new(looking));
    let drawing = Arc::clone(&held);
    let reading = Arc::clone(&held);

    // What the framework does about a film, which is to ask this. It draws the
    // surface and knows nothing about what fills it, so the day this desktop
    // reads a film some other way it is this line and the two functions above
    // it that change.
    panel::films(move |at| reeling(&reading, at));

    panel::show(Arc::new(move || pages(&drawing)), 0, None);
}
