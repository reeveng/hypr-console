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
    /// Whether this row is one of a row of buttons rather than a list item.
    ///
    /// A transport bar is a row of presses, not a list of choices. The card
    /// the row sits in is wider than the icon so the whole row reads as one
    /// strip of buttons across the panel. Walked over by the d-pad like any
    /// other row, but drawn differently so a hand on a touch screen meets a
    /// wide target rather than a thin line.
    pub transport: bool,
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

    /// The same, drawn as one of a row of buttons across the panel.
    ///
    /// Where the row is part of a strip of presses rather than one of a list
    /// of choices. The card is wider than the icon, the icon is bigger than
    /// the words on other rows, and the highlight is a pill on the icon.
    pub fn transport(mut self) -> Self {
        self.transport = true;
        self
    }

    /// Whether this row is the one in effect.
    pub fn now(&self) -> bool {
        self.aside == NOW
    }

    /// Whether anything happens to this row when it is stood on.
    ///
    /// Not the same as having something to do. The screen and the speakers are
    /// held at a level and chosen for nothing, so a row with no `does` on it is
    /// as often the one thing on its tab anybody touches as it is a heading.
    pub fn acts(&self) -> bool {
        self.does.is_some() || self.level.is_some() || self.seek.is_some()
    }

    /// Whether the row is a heading: a word over the rows under it.
    ///
    /// Nothing happens to it and there is nothing beside it either, which is
    /// what separates a heading from a row that is there to be read. The guide
    /// is a panel of rows to be read, a button on one side and what it does on
    /// the other, and a highlight that walked past everything nothing happens
    /// to walked past most of the page to land in the middle of it.
    pub fn heading(&self) -> bool {
        self.naming || self.nothing || (!self.acts() && self.aside.is_empty() && !self.typing)
    }
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
        assert!(Row::said("Balanced", NOW).now());
        assert!(!Row::said("Balanced", "").now());
    }

    #[test]
    fn rows_are_asked_for_at_the_moment_they_are_drawn() {
        let rows = Rows::asked(|| vec![Row::said("Speakers", "half")]);
        assert_eq!(rows.read()[0].aside, "half");
    }
}
