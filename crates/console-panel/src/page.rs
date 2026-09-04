//! What a panel is asked to draw.
//!
//! A page is what the tab says and the rows under it. A row is what it says,
//! what is written beside it, what it does, and, where there is one, what left
//! and right do to it. A row that does nothing is read rather than chosen,
//! which is what a guide is made of.

use std::path::PathBuf;
use std::sync::Arc;

/// The panel, offered to a row that wants to change what is on it.
pub trait Showing {
    /// Ask for the rows again, in case they say something else now.
    fn refresh(&self);

    /// Ask for them again as another list, and stand on a given row of it.
    ///
    /// `refresh` is for a reading that has moved. The rows are the same rows,
    /// so it leaves you standing where you were and draws what the tab had
    /// while it asks. A folder that has been walked into is not that: the list
    /// is another list, the row you were on is gone, and what the tab
    /// remembered would be on the screen for a moment under the new folder's
    /// name.
    fn replace(&self, standing_on: usize);

    /// Rub out what has been typed into the line the list is narrowed with.
    ///
    /// The word is the panel's: it lives in the line, and a tab that has
    /// finished with it cannot reach in and take it back. Walking into a folder
    /// a search found is exactly that, and without this the line goes on saying
    /// a word over a list that is no longer about it.
    fn forget_typing(&self);

    /// Take a line of text, and hand it on.
    fn ask(&self, question: &str, then: Answer);

    /// Ask a question that is answered by taking one of a few answers.
    ///
    /// `about` is the thing it is about, said beside it. `does` are the answers
    /// that do something; no is drawn first and is where it opens.
    fn sure(&self, question: &str, about: &str, does: &[&str], then: Taken);

    /// The same, with what is being typed shown.
    ///
    /// `ask` hides it, because until now the only thing anything here has had
    /// to type was a network's password. A name is not that. Typed blind on an
    /// on-screen keyboard, a filename is a guess about whether the last key
    /// registered, and the first anybody would know of a wrong one is a row
    /// that has quietly become something else.
    fn ask_aloud(&self, question: &str, then: Answer);

    /// Draw this card on the whole screen, with the picture it is about
    /// filling what the rows under it leave.
    ///
    /// For the one card that is about looking at something rather than about
    /// choosing something. A settings tab opened out is a short list with a
    /// great deal of nothing under it; a photograph opened out is the reason
    /// somebody pressed A.
    ///
    /// There is no way back through here, and that is on purpose. B is the way
    /// back from anything on this desktop, and a card that had to offer its own
    /// press to be got out of would be the one surface that did not answer the
    /// press every other one answers.
    fn open_out(&self);

    /// Turn to another tab, as a shoulder would.
    ///
    /// For a page that is a way of choosing what another page is about: a list
    /// of what is in a folder, over a card showing one of them. Choosing a row
    /// there has done its work on the card, and leaving somebody on the list
    /// they have just finished with is a press they have to guess at.
    ///
    /// Past the last tab is the last tab. A page asking for one that is not
    /// there is a page that has miscounted, and the answer to that is the panel
    /// staying where it is rather than closing itself.
    fn turn_to(&self, tab: usize);

    /// Say something in the corner of the screen, for a moment.
    ///
    /// What goes with `later`. A press that hands its work to `later` is a
    /// press that leaves the panel looking exactly as it did, and a picture
    /// that arrives a minute after it was chosen is a press that did nothing
    /// twice. This is what says otherwise: one line, off the card, gone on its
    /// own, and never in the way of the next press.
    ///
    /// Not the notification daemon. A panel is a layer over everything on this
    /// screen, so a notification raised from one is drawn behind the panel it
    /// was raised from.
    fn note(&self, said: &str);

    /// Run something slow without the panel going deaf while it happens, and
    /// draw it again once that is done.
    ///
    /// Connecting to a network takes seconds. Waiting for it where the drawing
    /// happens stops the panel answering the buttons, which reads as a machine
    /// that has crashed rather than one that is working.
    fn later(&self, argv: Vec<String>);

    /// Start something and leave it running, and draw again once it has had
    /// long enough to say that it has.
    ///
    /// The other one is for a command that finishes. A player does not: it is
    /// still running when the song ends and still running when the panel that
    /// started it has gone, which is the whole point of it. Waiting on one
    /// holds a thread for the length of the music and keeps the player a child
    /// of the panel, so closing the panel took the music with it.
    fn leave_running(&self, argv: Vec<String>);
}

/// What a line of text is handed to, once it has been typed.
///
/// The panel is handed back with it, because what is done with an answer is
/// usually slow and the panel is what knows how to do a slow thing quietly.
pub type Answer = Arc<dyn Fn(&dyn Showing, &str) + Send + Sync>;

/// What a question does once one of its answers is taken, said as the place of
/// that answer in the ones the caller gave.
pub type Taken = Arc<dyn Fn(&dyn Showing, usize) + Send + Sync>;

/// What choosing a row does.
pub type Act = Arc<dyn Fn(&dyn Showing) -> bool + Send + Sync>;

/// What left and right do to a row that carries one.
pub type Level = Arc<dyn Fn(i32) + Send + Sync>;

/// A command to run and leave on, or something to call and stay.
#[derive(Clone)]
pub enum Does {
    Call(Act),
    Run(Vec<String>),
}

impl Does {
    /// Something to run, from words.
    pub fn run(argv: &[&str]) -> Self {
        Does::Run(argv.iter().map(|word| (*word).to_string()).collect())
    }

    /// Something to call, which stays unless it says to go.
    pub fn call(act: impl Fn(&dyn Showing) -> bool + Send + Sync + 'static) -> Self {
        Does::Call(Arc::new(act))
    }

    /// Something to call that always stays.
    pub fn and_stay(act: impl Fn(&dyn Showing) + Send + Sync + 'static) -> Self {
        Does::call(move |showing| {
            act(showing);
            false
        })
    }
}

/// The word a row says beside itself when it is the one in effect.
///
/// Whether a row can be chosen has nothing to do with whether it is the one in
/// effect. Asking for both marked the current power profile and left the joined
/// network saying the same word in a different colour, so the two tabs had to
/// be read differently.
pub const NOW: &str = "now";

/// What a reading says while the machine has not answered about it yet.
///
/// A tab is drawn twice: once out of what is known without asking anything, and
/// again when the machine has answered. The first drawing is the whole list, in
/// the right order and under the right names, and the one thing missing from it
/// is the number. So the one thing that says it is missing stands where the
/// number is going to be, and the wait is one value on one row rather than a
/// card that is not there yet.
pub const YET: &str = "\u{2026}";

/// Room at the front of a row, and what is in it.
///
/// Room is kept for a row with nothing to put in it so that the names line up.
/// A column that starts in two places is harder to read down than one that
/// starts in one, and a listing whose folders begin at the edge while its
/// photographs begin an inch in reads as a mistake.
///
/// Kept while a picture is still being made, too. They arrive together and the
/// list is drawn again when they do, so a row that only made room once it had
/// something to show would slide every name sideways a moment after the folder
/// opened.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Picture {
    #[default]
    None,
    /// Room kept, and nothing to put in it.
    Space,
    /// An icon by name, out of the theme the desktop is dressed in.
    ///
    /// What the row is rather than what it holds. A folder has no picture of
    /// itself worth making and every folder is the same shape anyway, so the
    /// thing worth drawing in front of one is the mark that says it is a folder
    /// and opens like one. Symbolic, so it is drawn in the ink the row is
    /// written in rather than in colours of its own.
    Named(&'static str),
    At(PathBuf),
    /// A picture drawn large, for a card that is about the one thing it is a
    /// picture of.
    ///
    /// `At` is a thumbnail at the front of a row in a list, which is the right
    /// size for a list and far too small to be what a hand looks at first.
    /// This is the picture itself, as the file holds it. The music card draws
    /// its sleeve with [`Picture::Written`] instead -- the same square, said in
    /// characters -- so nothing on the desktop asks for this today.
    ///
    /// Nothing, where there is no picture yet or none at all: the square is
    /// drawn either way and is empty until there is something to put in it.
    /// The player answers about the song a moment before it answers about the
    /// cover, so a card that kept no room until the cover arrived was a card
    /// that grew a sleeve's worth taller under a thumb that had already
    /// started reading it -- which is the same argument [`Picture::Space`]
    /// settles for a row in a list, said about the one picture a card is
    /// about.
    Sleeve(Option<PathBuf>),
    /// A picture drawn as large as the card will let it be, whatever shape it
    /// is.
    ///
    /// What the viewer panel is about. [`Picture::Sleeve`] is the nearest
    /// thing and will not do: it is a fixed square and it centre-crops, which
    /// is right for a record and takes both ends off a landscape photograph.
    /// This one keeps the shape it came in and takes the room that is left.
    ///
    /// Nothing, where the file will not open. The card says so in words on the
    /// row under it rather than leaving a hole -- a person holding a device
    /// with no terminal has to be able to tell a broken file from a broken
    /// panel.
    Showing(Option<PathBuf>),
    /// A film, drawn in the room [`Picture::Showing`] draws a still one in.
    ///
    /// A path, like every other picture here, and for a reason that took a
    /// wrong turn to find. What a film is really drawn from is a running
    /// decoder with a position in it, and the obvious thing -- hand the panel
    /// the surface the decoder paints -- cannot be done: rows are read off the
    /// main thread, and nothing GTK draws with may cross one. So this says
    /// which film, and [`crate::panel::films`] is where a panel says what to
    /// do about it, on the thread that may.
    ///
    /// That split is worth more than the tidiness it cost. The framework knows
    /// how big a film is drawn and where on the card it sits, and knows nothing
    /// about what reads one -- so the day the decoder is swapped out, or this
    /// desktop grows one of its own, it is a change in the panel that shows
    /// films and not in the framework every panel draws through.
    ///
    /// Nothing, where there is no film yet or none that will open. The room is
    /// kept either way and the row under it says what happened, the same as for
    /// a still one.
    Playing(Option<PathBuf>),
    /// A picture written in characters, as Pango markup.
    ///
    /// The cover of what is playing, drawn the way a terminal draws one. It is
    /// a picture and not a row of its own because it stands where every other
    /// picture on this machine stands, at the front of the row that says what
    /// it is a picture of.
    Written(String),
    /// A bar showing where the song is, with a dot at its current position.
    ///
    /// Tapped or scrubbed with the d-pad. A bar of one colour with one dot is
    /// the only thing on it that moves, so a finger on a touch screen lands
    /// where the song is going and the player jumps to it.
    Bar(Bar),
}

/// A progress bar drawn across the full width of its row.
///
/// `at` is the position of the dot in characters from the left; the rest of
/// the row is the bar itself. A bar that has not been told how long the song is
/// has the dot at the start: empty is the honest answer to a question the panel
/// has not asked yet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Bar {
    /// Where the dot is, in characters from the left edge of the row.
    pub at: usize,
    /// How many characters the whole bar is drawn across.
    pub wide: usize,
}

/// A seek callback, given the panel and a fraction of the song from the start.
///
/// Called when the bar is tapped and the touch lands at that fraction. The
/// row's `level` is what the d-pad moves, which is a step rather than a
/// position; the fraction is what a tap wants to land at. The panel is the
/// other thing it has to know about, because the dot has to redraw once the
/// song has been told to move.
pub type Seek = Arc<dyn Fn(&dyn Showing, f64) + Send + Sync>;

/// One row.
#[derive(Clone, Default)]
pub struct Row {
    pub says: String,
    pub aside: String,
    pub does: Option<Does>,
    pub level: Option<Level>,
    /// What the two ends of that level are drawn as, where minus and plus are
    /// not what they mean.
    pub ends: Option<(String, String)>,
    /// What else can be done with this one, where there is more than the one
    /// thing choosing it does.
    ///
    /// A row does one thing on A because one thing is what a thumb should have
    /// to know. Everything else it could be done to is behind Y, on the row
    /// itself rather than on a menu somewhere, so what the options are about is
    /// the thing you are standing on and never a guess about what was last
    /// selected.
    pub more: Option<Act>,
    pub picture: Picture,
    /// A picture at the other end of the row, where one is wanted.
    ///
    /// The usual picture is on the left because most rows have their picture of
    /// themselves. A few carry one on the right because the row is about what
    /// sits beside it -- the song on now, with its cover drawn where the hand
    /// reads it as the album rather than as another icon on the left.
    pub tail: Option<Picture>,
    /// Whether choosing this row opens another list rather than doing something
    /// where it stands.
    ///
    /// A list that goes deeper looks exactly like one that does not, and the
    /// only way to find out used to be to press A and see where you ended up.
    /// The mark says which rows are a way in, so a setting three presses down
    /// can be found by reading rather than by trying.
    pub opens: bool,
    /// Whether this row is the line the list is narrowed with.
    ///
    /// The panel puts one at the top of a page that seeks and nothing else
    /// makes one. It is a row rather than a line above the list because a line
    /// above the list is a line only a pointer can reach, and the hands on this
    /// machine are on a controller: the d-pad walks onto it, the letters go
    /// into it while it is stood on, and walking off it hands the pad back to
    /// the list.
    pub typing: bool,
    /// Whether this row is the name of what the list under it is about.
    ///
    /// A question's first row used to be the thing it was about, said with
    /// `said` and drawn like every other row on the page: same card, same ink,
    /// same shape a thumb is aiming at. So a list of six things that can be
    /// done read as seven, the d-pad walked onto the one that was not an
    /// answer, and the row telling you what you were deciding about looked like
    /// one of the decisions.
    ///
    /// Said here instead, so the panel can draw it as a title and walk past it.
    /// A heading by the plain rule is one nothing happens to and nothing is
    /// written beside; a title is often both of those and is a title anyway,
    /// because a file's name has its size beside it.
    pub naming: bool,
    /// Whether this row is the panel saying the list is empty.
    ///
    /// Nearly every tab here has a list that can come up with nothing in it:
    /// no notification waiting, no song in the folder, nothing that answers to
    /// the word typed. Each said so in a row, and a row is a card the width of
    /// the panel in the ink an option is written in, shaped like the thing a
    /// thumb is aiming at. So a tab holding one thing you cannot do read as a
    /// tab holding one thing you can, and the only way to learn otherwise was
    /// to press A at it and watch nothing happen.
    ///
    /// Said here instead, so the panel can draw it as what it is: no card, no
    /// mark, quiet, and across the middle rather than down the left where the
    /// options line up. It declares nothing because there is nothing to
    /// declare, and now it looks like nothing as well.
    pub nothing: bool,
    /// A seek callback for a row holding a bar.
    ///
    /// `level` is what the d-pad moves -- a step. A tap on the bar wants a
    /// fraction rather than a step, and this is where it goes: handed the
    /// fraction of the way through, asked to land there.
    pub seek: Option<Seek>,
    /// Whether what is on this row sits in the middle of the card.
    ///
    /// Rows line up down the left edge because a list is read down its left
    /// edge. A now-playing card is not a list: it is a sleeve, a title and an
    /// artist, and every player anybody has held stacks those three up the
    /// middle. So the rule is asked for by the row rather than assumed, and
    /// only the card that is about one thing asks for it.
    pub middle: bool,
    /// Several presses laid side by side on this one row.
    ///
    /// A transport bar is a row of presses, not a list of choices, and it was
    /// five rows: shuffle above previous above play above next above repeat,
    /// down the middle of the card, which is not what a music player looks
    /// like anywhere and is five presses of the d-pad from one end to the
    /// other. This is one row with the presses across it, which is the shape a
    /// hand already knows -- and it costs the d-pad nothing, because left and
    /// right on a row were already free.
    pub across: Option<Across>,
    /// Whether this is the row the card opens standing on.
    ///
    /// The highlight lands on the first row something happens to, which is
    /// the right answer for a list: the first thing on it is the first thing a
    /// thumb wants. A now-playing card is not a list. Walking down it, the
    /// first row anything happens to is the bar the song is scrubbed with, and
    /// the press a hand opened the card to make is play, one row below it -- so
    /// every opening of the tab began with a press of down.
    ///
    /// One row at most, and it is the row's own claim rather than a number the
    /// panel is given: the rows are built again on every drawing and what is
    /// third on the card depends on whether the player has said a cover and an
    /// album yet. It is asked only while the highlight has not been put
    /// anywhere yet, so a thumb that walked off it is left where it walked to.
    pub chief: bool,
}

/// One press in a strip of them.
#[derive(Clone)]
pub struct Press {
    /// The icon drawn on it, out of the theme.
    pub icon: &'static str,
    /// Whether what it turns on is on now -- shuffling, repeating. Drawn lit,
    /// the way the row that is in effect is drawn in mint.
    pub now: bool,
    /// Whether this is the press the whole strip is for.
    ///
    /// One at most, and on a music player it is play. Drawn as a filled circle
    /// rather than as a mark like the others, because it is the press a hand
    /// makes without looking and the four around it are the ones it aims at.
    pub chief: bool,
    /// What pressing it does.
    pub does: Act,
}

impl Press {
    pub fn new(icon: &'static str, now: InEffect, does: impl Fn(&dyn Showing) + Send + Sync + 'static) -> Press {
        Press {
            icon,
            now: now == InEffect::Yes,
            chief: false,
            does: Arc::new(move |showing| {
                does(showing);
                false
            }),
        }
    }

    /// The press the strip is for, drawn as a filled circle.
    pub fn chief(mut self) -> Press {
        self.chief = true;
        self
    }
}

/// A strip of presses on one row, and which of them is under the highlight.
///
/// `at` is where the d-pad is standing within the strip. It belongs to whoever
/// built the row rather than to the panel, for the same reason the seek
/// position does: the rows are made again every time the page is drawn, so
/// anything the panel wrote onto one would be gone by the next reading.
#[derive(Clone)]
pub struct Across {
    pub presses: Vec<Press>,
    pub at: usize,
}

impl Row {
    /// A row that is read rather than chosen.
    pub fn said(says: &str, aside: &str) -> Self {
        Row { says: says.to_string(), aside: aside.to_string(), ..Row::default() }
    }

    /// The panel saying the list is empty, and why.
    ///
    /// Not an option, so it is not shaped like one: it is drawn without a card,
    /// quietly, across the width of the list, and the highlight never lands on
    /// it. For the one line a tab puts up in place of the rows it has none of.
    /// Anything a thumb could act on afterwards — clearing the folder, looking
    /// again — is still a row of its own under it.
    pub fn nothing(says: &str) -> Self {
        Row { nothing: true, ..Row::said(says, "") }
    }

    /// The name of what the rows under it are about.
    ///
    /// Drawn as a title rather than as a row: no card of its own, quieter and
    /// smaller, and the highlight never lands on it. Not an option, so it is
    /// not shaped like one.
    pub fn naming(says: &str, aside: &str) -> Self {
        Row { naming: true, ..Row::said(says, aside) }
    }

    /// A row that does something.
    pub fn new(says: &str, aside: &str, does: Does) -> Self {
        Row { does: Some(does), ..Row::said(says, aside) }
    }

    /// Row nought: the way back out of wherever this is.
    ///
    /// Every surface here has one, because B has no answer for a finger and
    /// the panel's own way out closes the whole card. It is made here rather
    /// than by each panel so that the way back says the same thing and wears
    /// the same mark whichever list it is at the top of.
    pub fn back(says: &str, then: impl Fn(&dyn Showing) + Send + Sync + 'static) -> Self {
        Row::new(&format!("{} {says}", crate::marks::BEFORE), "", Does::and_stay(then))
    }

    /// The line the list is narrowed with, which only the panel makes.
    pub(crate) fn line_to_type_in() -> Self {
        Row { typing: true, ..Row::default() }
    }

    /// The two ends of this row's level, said in marks of its own.
    pub fn ended(mut self, less: &str, more: &str) -> Self {
        self.ends = Some((less.to_string(), more.to_string()));
        self
    }

    /// The same, carrying a level that left and right move.
    pub fn levelled(mut self, level: Level) -> Self {
        self.level = Some(level);
        self
    }

    /// The same, keeping room at the front for a picture.
    pub fn picturing(mut self, picture: Picture) -> Self {
        self.picture = picture;
        self
    }

    /// The same, with a picture at the other end of the row.
    ///
    /// Where the row is about the thing beside it. The info a player reads off
    /// the file comes with a cover; the cover is part of what the row says,
    /// not a part of the row's icon.
    pub fn tailing(mut self, tail: Picture) -> Self {
        self.tail = Some(tail);
        self
    }

    /// The same, saying that it opens onto another list.
    pub fn opening(mut self) -> Self {
        self.opens = true;
        self
    }

    /// The same, with more behind Y than the one thing A does.
    pub fn offering(mut self, more: impl Fn(&dyn Showing) -> bool + Send + Sync + 'static) -> Self {
        self.more = Some(Arc::new(more));
        self
    }

    /// The same, with a tap on the bar going to a fraction of the song.
    ///
    /// The row's `level` is what the d-pad scrubs by -- a step. A finger on
    /// the bar wants a fraction rather than a step, and this is what tells the
    /// player where the finger said to go. The panel is handed in too, because
    /// the dot has to redraw once the song has been told to move.
    pub fn seeking(mut self, seek: impl Fn(&dyn Showing, f64) + Send + Sync + 'static) -> Self {
        self.seek = Some(Arc::new(seek));
        self
    }

    /// A row that is one picture and nothing else, up the middle of the card.
    ///
    /// The sleeve on a now-playing card. Read rather than chosen: nothing
    /// happens to a picture of what is already playing, so the d-pad walks
    /// past it the way it walks past a heading.
    pub fn showing(picture: Picture) -> Self {
        Row { picture, naming: true, middle: true, ..Row::default() }
    }

    /// The same, and the row this card opens standing on.
    ///
    /// The press a hand came for, said as the row it is on. What the strip of
    /// presses already says about itself with [`Press::chief`], said one level
    /// up: the card opens with the thumb on the row, and the row opens with it
    /// on the press.
    pub fn chief(mut self) -> Self {
        self.chief = true;
        self
    }

    /// The same, and choosing it does something.
    ///
    /// A picture drawn large is read and not chosen -- there is nothing to do
    /// to it, so the thumb walks past it to the rows under it that say what it
    /// is. A picture that opens out on A is not that: it is the press somebody
    /// came for, and a press the highlight cannot land on is not a press.
    pub fn choosing(mut self, does: Does) -> Self {
        self.does = Some(does);
        self.naming = false;
        self
    }

    /// Draw what is on this row in the middle of the card rather than down its
    /// left edge.
    pub fn in_the_middle(mut self) -> Self {
        self.middle = true;
        self
    }

    /// A row of presses, and which of them the highlight is on.
    ///
    /// A is the one being stood on, left and right move between them, and each
    /// of them can be tapped on its own -- which is the answer every button on
    /// this desktop owes a hand holding nothing.
    pub fn pressing(presses: Vec<Press>, at: usize) -> Self {
        let at = at.min(presses.len().saturating_sub(1));
        let taken = presses.clone();
        let standing = at;

        Row {
            does: Some(Does::call(move |showing| match taken.get(standing) {
                Some(press) => (press.does)(showing),
                None => false,
            })),
            across: Some(Across { presses, at }),
            ..Row::default()
        }
    }

    /// Whether this row is the one in effect.
    pub fn now(&self) -> InEffect {
        match self.aside == NOW {
            true => InEffect::Yes,
            false => InEffect::No,
        }
    }

    /// Whether anything happens to this row when it is stood on.
    ///
    /// Not the same as having something to do. The screen and the speakers are
    /// held at a level and chosen for nothing, so a row with no `does` on it is
    /// as often the one thing on its tab anybody touches as it is a heading.
    pub fn acts(&self) -> Acts {
        match self.does.is_some() || self.level.is_some() || self.seek.is_some() {
            true => Acts::Yes,
            false => Acts::Nothing,
        }
    }

    /// Whether the row is a heading: a word over the rows under it.
    ///
    /// Nothing happens to it and there is nothing beside it either, which is
    /// what separates a heading from a row that is there to be read. The guide
    /// is a panel of rows to be read, a button on one side and what it does on
    /// the other, and a highlight that walked past everything nothing happens
    /// to walked past most of the page to land in the middle of it.
    pub fn heading(&self) -> Heading {
        let over = self.naming
            || self.nothing
            || (self.acts() == Acts::Nothing && self.aside.is_empty() && !self.typing);

        match over {
            true => Heading::Yes,
            false => Heading::No,
        }
    }

    /// Whether this row would be drawn exactly as that one was.
    ///
    /// A tab is drawn twice on every opening: once as it was last time, and
    /// again when the machine has answered. Nearly always those are the same
    /// list -- the applications installed have not changed since the menu was
    /// last opened -- and the second drawing was taking every row off the card
    /// and building it again to arrive at what was already there.
    ///
    /// The callbacks are compared by whether there is one rather than by what
    /// it does, because two closures cannot be asked whether they agree and
    /// because it is not what a row looks like: what having one changes is the
    /// shape drawn, and that is the part this is about. The row that replaces
    /// this one is still kept, so A on it runs the new answer's work.
    ///
    /// Written out field by field, and taken apart by name on purpose: a row
    /// that grows a new thing to draw will not compile until it is said here,
    /// which is the only guard against this quietly leaving something stale on
    /// the screen.
    pub fn looks_like(&self, other: &Row) -> Same {
        let Row {
            says,
            aside,
            does,
            level,
            ends,
            more,
            picture,
            tail,
            opens,
            typing,
            naming,
            nothing,
            seek,
            middle,
            across,
            chief,
        } = self;
        let alike = says == &other.says
            && aside == &other.aside
            && does.is_some() == other.does.is_some()
            && level.is_some() == other.level.is_some()
            && ends == &other.ends
            && more.is_some() == other.more.is_some()
            && picture == &other.picture
            && tail == &other.tail
            && opens == &other.opens
            && typing == &other.typing
            && naming == &other.naming
            && nothing == &other.nothing
            && seek.is_some() == other.seek.is_some()
            && middle == &other.middle
            // Which press the highlight is on is part of what is drawn, so a
            // strip that has been walked along is a different row and the card
            // is built again. Compared by what it looks like rather than by
            // what it does, which is the rule every other field here follows.
            && across.as_ref().map(Across::looks) == other.across.as_ref().map(Across::looks)
            // Not drawn, and here anyway: where a card opens is only asked on
            // the drawing that builds it, so a card that has moved which row
            // that is has to be one of the drawings that builds it.
            && chief == &other.chief;

        match alike {
            true => Same::Yes,
            false => Same::No,
        }
    }
}

impl Across {
    /// What this strip looks like: which icons, which are lit, and which one
    /// the highlight is on.
    fn looks(&self) -> (Vec<(&'static str, bool, bool)>, usize) {
        (self.presses.iter().map(|press| (press.icon, press.now, press.chief)).collect(), self.at)
    }
}

/// Whether a row is the one in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InEffect {
    /// It is what the machine is doing now, and wears the mark that says so.
    Yes,
    /// It is one of the others.
    No,
}

/// Whether anything happens to a row when it is stood on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acts {
    /// Something does: it runs, or it is held at a level, or it is typed into.
    Yes,
    /// Nothing does.
    Nothing,
}

/// Whether a row is a heading: a word over the rows under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heading {
    /// It is, so the highlight walks past it.
    Yes,
    /// It is a row a thumb can land on.
    No,
}

/// Whether one row would be drawn exactly as another was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Same {
    /// It would, so what is on the screen can be left alone.
    Yes,
    /// It would not, and the list has to be drawn again.
    No,
}

/// The rows of one tab: a fixed list, or a question asked at the moment it is
/// drawn.
///
/// A tab that names a function is not computed until you are looking at it.
/// Everything on the settings is read off the machine, and reading all of it to
/// show one tab meant scanning for networks to open the sound.
#[derive(Clone)]
pub enum Rows {
    Asked(Arc<dyn Fn() -> Vec<Row> + Send + Sync>),
    Fixed(Vec<Row>),
}

impl Rows {
    pub fn asked(of: impl Fn() -> Vec<Row> + Send + Sync + 'static) -> Self {
        Rows::Asked(Arc::new(of))
    }

    pub fn read(&self) -> Vec<Row> {
        match self {
            Rows::Asked(of) => of(),
            Rows::Fixed(rows) => rows.clone(),
        }
    }
}

/// What to redraw a tab for.
///
/// The volume rocker on the top edge moves the same number the Sound tab shows,
/// and a panel that goes on showing the old one is worse than one showing
/// nothing: it is a reading, and it is wrong.
#[derive(Clone)]
pub struct Watch {
    pub argv: Vec<String>,
    /// Only lines carrying this count. What these commands report is everything
    /// the machine is doing, most of it caused by this panel reading the
    /// machine, and answering all of it means every redraw asks for another.
    pub about: String,
}

impl Watch {
    pub fn on(argv: &[&str], about: &str) -> Self {
        Watch {
            argv: argv.iter().map(|word| (*word).to_string()).collect(),
            about: about.to_string(),
        }
    }
}

/// A line to type into, drawn as the first row of the list.
///
/// Not a question. A question replaces the rows and waits for an answer; this
/// is the top of a list that is still a list, and narrows it as the letters
/// arrive. Standing on it, the letters are its and the d-pad walks off it;
/// standing anywhere else, the pad is the list's and the line is a row you can
/// see. That is what makes a menu of two hundred applications reachable both by
/// thumb and by name.
#[derive(Clone)]
pub struct Sought {
    /// What the empty line says it is for.
    pub about: String,
    /// Handed the whole line, every time it changes.
    pub then: Answer,
}

/// What one press was spent on.
///
/// A tab whose rows come and go has to be able to say that the press which
/// brought them back was only that. Without it, the first press after a card
/// has gone quiet acts on rows nobody could see: the thumb reaching for pause
/// finds the picture where the transport used to be, and the film fills the
/// screen instead of stopping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stirred {
    /// Spent on waking the tab. The panel draws it again and does nothing else
    /// with the press.
    Woke,
    /// The tab was already awake, and the press means what it looks like.
    Awake,
}

/// Told about every press the panel has a meaning for, before it acts on one.
///
/// Asked and not merely told, because the answer decides whether the press
/// goes any further. A tab that never sleeps does not set one of these and
/// every press is its own.
pub type Stirring = Arc<dyn Fn() -> Stirred + Send + Sync>;

/// One tab: what it says, what fills it, what to do on arriving, and what to
/// listen to.
#[derive(Clone)]
pub struct Page {
    pub title: String,
    pub rows: Rows,
    /// A line to type into above the rows, where there is one.
    pub sought: Option<Sought>,
    /// What going back means here, when it means anything other than shutting.
    ///
    /// A panel is one list and B closes it. A tab that is somewhere has a way
    /// out that is not the panel's: inside a folder, back is the folder above
    /// it, and only at the top of the tree does back mean the panel. Saying
    /// true is saying there was nowhere left to go.
    pub back: Option<Act>,
    /// What a tab wants done when you arrive on it, if anything.
    ///
    /// Drawing a tab shows what is already known; this is for going and finding
    /// out. The two are separate on purpose, so the panel appears at once and
    /// fills in, rather than waiting on a radio before it draws.
    pub entered: Option<Act>,
    /// The tab as it can be drawn before the machine has been asked anything.
    ///
    /// Reading a tab means asking the machine, and asking the machine takes
    /// long enough to see: a panel opened on a tab it has not shown before came
    /// up with nothing on it and filled in. Most of a settings tab is known
    /// before anything is asked, though. The three power profiles are the same
    /// three whatever the answer turns out to be, and all the answer decides is
    /// which of them is marked and how bright the screen is.
    ///
    /// So this is the list as it stands without an answer, wearing `YET` where
    /// a reading is going to be, and the answer arrives into a card that is
    /// already on the screen.
    pub meanwhile: Option<Arc<dyn Fn() -> Vec<Row> + Send + Sync>>,
    pub watch: Option<Watch>,
    /// Handed every press before the panel acts on it, where this tab's rows
    /// are a thing that goes away when it is left alone.
    pub stirs: Option<Stirring>,
}

impl Page {
    pub fn new(title: &str, rows: Rows) -> Self {
        Page {
            title: title.to_string(),
            rows,
            sought: None,
            back: None,
            entered: None,
            meanwhile: None,
            watch: None,
            stirs: None,
        }
    }

    /// Draw a line to type into at the top of the rows, and hand it what is
    /// typed as it is typed.
    ///
    /// What to do with it is the caller's: this crate draws the line and reads
    /// the keys, and which rows a word leaves standing is a question only the
    /// thing that built them can answer.
    pub fn searching(
        mut self,
        about: &str,
        act: impl Fn(&dyn Showing, &str) + Send + Sync + 'static,
    ) -> Self {
        self.sought = Some(Sought { about: about.to_string(), then: Arc::new(act) });
        self
    }

    /// What B does here before it does what B always does.
    pub fn on_back(mut self, act: impl Fn(&dyn Showing) -> bool + Send + Sync + 'static) -> Self {
        self.back = Some(Arc::new(act));
        self
    }

    pub fn on_arriving(mut self, act: impl Fn(&dyn Showing) + Send + Sync + 'static) -> Self {
        self.entered = Some(Arc::new(move |showing| {
            act(showing);
            false
        }));
        self
    }

    /// What to put up while the machine is being asked.
    pub fn meanwhile(mut self, rows: impl Fn() -> Vec<Row> + Send + Sync + 'static) -> Self {
        self.meanwhile = Some(Arc::new(rows));
        self
    }

    pub fn watching(mut self, watch: Watch) -> Self {
        self.watch = Some(watch);
        self
    }

    /// Hear every press first, and say whether it was spent waking this tab up.
    ///
    /// For a tab that puts its rows away when nobody is pressing anything. It
    /// is asked before the press is acted on and before the rows are read, so
    /// what it writes down is what the next reading draws.
    pub fn stirring(mut self, act: impl Fn() -> Stirred + Send + Sync + 'static) -> Self {
        self.stirs = Some(Arc::new(act));
        self
    }
}

/// Which tab to open on, by the word on it.
///
/// Something on the bar that stands for one of these opens the panel at that
/// one, so tapping the battery and pressing Legion right arrive in the same
/// place by different roads. A name nothing answers to opens the first tab
/// rather than nothing at all.
pub fn find(pages: &[Page], name: Option<&str>) -> usize {
    let Some(wanted) = name.map(|name| name.trim().to_lowercase()) else { return 0 };

    pages
        .iter()
        .position(|page| page.title.to_lowercase() == wanted)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reading that lands after an opening builds new closures for rows
    /// that are otherwise word for word what is already on the card. Comparing
    /// those by what they do would say every list had changed, and the drawing
    /// this saves would never be saved.
    #[test]
    fn two_rows_that_do_different_things_and_read_the_same_look_the_same() {
        let one = Row::new("Firefox", "", Does::run(&["firefox"]));
        let two = Row::new("Firefox", "", Does::call(|_| true));
        assert_eq!(one.looks_like(&two), Same::Yes);
    }

    /// And everything a hand can see is a difference, because what this decides
    /// is whether to leave what is on the screen alone.
    #[test]
    fn anything_that_is_drawn_differently_is_a_row_that_must_be_drawn_again() {
        let row = Row::new("Firefox", "", Does::run(&["firefox"]));
        let named = |says| Row::new(says, "", Does::run(&["firefox"]));
        assert_eq!(row.looks_like(&named("Chromium")), Same::No);
        assert_eq!(row.looks_like(&Row::new("Firefox", "now", Does::run(&["firefox"]))), Same::No);
        assert_eq!(row.looks_like(&Row::said("Firefox", "")), Same::No);
        assert_eq!(row.looks_like(&row.clone().opening()), Same::No);
        assert_eq!(row.looks_like(&row.clone().picturing(Picture::Space)), Same::No);
        assert_eq!(row.looks_like(&Row::naming("Firefox", "")), Same::No);
        // Not drawn, and a difference anyway: where a card opens is only asked
        // on the drawing that builds it.
        assert_eq!(row.looks_like(&row.clone().chief()), Same::No);
        assert_eq!(row.looks_like(&Row::nothing("Firefox")), Same::No);
    }

    /// A reading that has moved is the case this must never step over: the
    /// level's own value is written beside it, so a volume that has changed is
    /// a row that reads differently.
    #[test]
    fn a_level_that_says_a_new_reading_is_drawn_again() {
        let quiet = Row::said("Volume", "40%").levelled(Arc::new(|_| ()));
        let loud = Row::said("Volume", "60%").levelled(Arc::new(|_| ()));
        assert_eq!(quiet.looks_like(&loud), Same::No);
        assert_eq!(quiet.looks_like(&quiet.clone()), Same::Yes);
    }

    fn strip() -> Vec<Press> {
        vec![
            Press::new("media-playlist-shuffle-symbolic", InEffect::No, |_| ()),
            Press::new("media-playback-pause-symbolic", InEffect::No, |_| ()),
            Press::new("media-playlist-repeat-symbolic", InEffect::Yes, |_| ()),
        ]
    }

    /// A is the press being stood on and not the first of them, which is the
    /// whole of what a strip of presses is: the row is one row and the thumb
    /// is somewhere along it.
    #[test]
    fn a_strip_of_presses_takes_the_one_being_stood_on() {
        let taken = Arc::new(std::sync::atomic::AtomicUsize::new(9));
        let presses: Vec<Press> = (0..3)
            .map(|at| {
                let taken = Arc::clone(&taken);
                Press::new("media-playback-start-symbolic", InEffect::No, move |_| {
                    taken.store(at, std::sync::atomic::Ordering::SeqCst);
                })
            })
            .collect();
        let row = Row::pressing(presses, 2);

        let Some(Does::Call(act)) = row.does else { panic!("a strip with nothing to press") };

        act(&Nowhere);
        assert_eq!(taken.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    /// Standing past the end is standing on the last of them. Which press the
    /// highlight is on is kept by whoever built the row, and a strip that grew
    /// shorter between two readings would otherwise be a press into nothing.
    #[test]
    fn standing_past_the_end_of_a_strip_stands_on_the_last_press() {
        let row = Row::pressing(strip(), 40);
        assert_eq!(row.across.expect("a strip").at, 2);
    }

    /// Where the thumb is along the strip is part of what is drawn, so walking
    /// it is a row that has to be drawn again. Without this the highlight
    /// moved in the panel's mind and never on the screen.
    #[test]
    fn a_strip_walked_along_is_drawn_again() {
        let here = Row::pressing(strip(), 0);
        let there = Row::pressing(strip(), 1);
        assert_eq!(here.looks_like(&there), Same::No);
        assert_eq!(here.looks_like(&Row::pressing(strip(), 0)), Same::Yes);
    }

    /// And so is which of them is lit: shuffle turned on is a strip that reads
    /// differently, on the same row, with the thumb where it was.
    #[test]
    fn a_press_that_has_come_on_is_drawn_again() {
        let off = Row::pressing(strip(), 1);
        let mut lit = strip();
        lit[0].now = true;
        assert_eq!(off.looks_like(&Row::pressing(lit, 1)), Same::No);
    }

    /// A panel that is not there, for the presses that do not ask it anything.
    struct Nowhere;

    impl Showing for Nowhere {
        fn refresh(&self) {}
        fn replace(&self, _standing_on: usize) {}
        fn forget_typing(&self) {}
        fn ask(&self, _question: &str, _then: Answer) {}
        fn sure(&self, _question: &str, _about: &str, _does: &[&str], _then: Taken) {}
        fn ask_aloud(&self, _question: &str, _then: Answer) {}
        fn note(&self, _said: &str) {}
        fn later(&self, _argv: Vec<String>) {}
        fn leave_running(&self, _argv: Vec<String>) {}
        fn open_out(&self) {}
        fn turn_to(&self, _tab: usize) {}
    }

    fn pages() -> Vec<Page> {
        ["Battery", "Sound", "Wi-Fi"]
            .map(|title| Page::new(title, Rows::Fixed(Vec::new())))
            .to_vec()
    }

    #[test]
    fn a_tab_is_found_by_the_word_on_it_however_it_is_written() {
        assert_eq!(find(&pages(), Some("sound")), 1);
        assert_eq!(find(&pages(), Some("  Wi-Fi ")), 2);
    }

    /// Tapping the bar opens the panel at the thing the icon stands for. An
    /// icon standing for something the panel no longer has is a panel, not
    /// nothing at all.
    #[test]
    fn a_name_nothing_answers_to_opens_the_first_tab() {
        assert_eq!(find(&pages(), Some("Telepathy")), 0);
        assert_eq!(find(&pages(), None), 0);
    }

    #[test]
    fn a_row_that_says_now_is_the_one_in_effect() {
        assert_eq!(Row::said("Balanced", NOW).now(), InEffect::Yes);
        assert_eq!(Row::said("Balanced", "").now(), InEffect::No);
    }

    #[test]
    fn rows_are_asked_for_at_the_moment_they_are_drawn() {
        let rows = Rows::asked(|| vec![Row::said("Speakers", "half")]);
        assert_eq!(rows.read()[0].aside, "half");
    }
}
