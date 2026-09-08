//! What a panel is asked to draw.
//!
//! A page is what the tab says and the rows under it. A row is what it says,
//! what is written beside it, what it does, and, where there is one, what left
//! and right do to it. A row that does nothing is read rather than chosen,
//! which is what a guide is made of.
//!
//! A row carrying a picture can stack its two lines against it rather than
//! writing them under it, which is the head of a card about one thing: the
//! sleeve on the left, what it is and whose it is beside it, and the whole of
//! it one row tall instead of three.

use std::path::PathBuf;
use std::sync::Arc;
use crate::icons::Icon;
use console_core_never::Never;

pub trait Showing {
    fn refresh(&self);

    fn replace(&self, standing_on: usize);

    fn forget_typing(&self);

    fn ask(&self, question: &str, then: Answer);

    fn sure(&self, question: &str, about: &str, does: &[&str], then: Taken);

    fn ask_aloud(&self, question: &str, then: Answer);

    fn open_out(&self);

    fn turn_to(&self, tab: usize);

    fn note(&self, said: &str);

    fn later(&self, argv: Vec<String>);

    fn leave_running(&self, argv: Vec<String>);
}

pub type Answer = Arc<dyn Fn(&dyn Showing, &str) + Send + Sync>;

pub type Taken = Arc<dyn Fn(&dyn Showing, usize) + Send + Sync>;

pub type Act = Arc<dyn Fn(&dyn Showing) -> bool + Send + Sync>;

pub type Level = Arc<dyn Fn(i32) + Send + Sync>;

#[derive(Clone)]
pub enum Does {
    Call(Act),
    Run(Vec<String>),
}

impl Does {
    pub fn run(argv: &[&str]) -> Result<Self, Never> {
        Ok(Does::Run(argv.iter().map(|word| (*word).to_string()).collect()))
    }

    pub fn call(
        act: impl Fn(&dyn Showing) -> bool + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        Ok(Does::Call(Arc::new(act)))
    }

    pub fn and_stay(act: impl Fn(&dyn Showing) + Send + Sync + 'static) -> Result<Self, Never> {
        Does::call(move |showing| {
            act(showing);
            false
        })
    }
}

pub const NOW: &str = "now";

pub const YET: &str = "\u{2026}";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Picture {
    #[default]
    None,
    Space,
    Named(Icon),
    At(PathBuf),
    Sleeve(Option<PathBuf>),
    Showing(Option<PathBuf>),
    Playing(Option<PathBuf>),
    Written(String),
    Bar(Bar),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Bar {
    pub at: u64,
    pub of: u64,
}

pub type Seek = Arc<dyn Fn(&dyn Showing, f64) + Send + Sync>;

#[derive(Clone, Default)]
pub struct Row {
    pub says: String,
    pub aside: String,
    pub does: Option<Does>,
    pub level: Option<Level>,
    pub ends: Option<(String, String)>,
    pub more: Option<Act>,
    pub picture: Picture,
    pub tail: Option<Picture>,
    pub opens: bool,
    pub typing: bool,
    pub naming: bool,
    pub nothing: bool,
    pub seek: Option<Seek>,
    pub middle: bool,
    pub stacked: bool,
    pub across: Option<Across>,
    pub chief: bool,
}

#[derive(Clone)]
pub struct Press {
    pub icon: Icon,
    pub now: bool,
    pub chief: bool,
    pub does: Act,
}

impl Press {
    pub fn new(
        icon: Icon,
        now: InEffect,
        does: impl Fn(&dyn Showing) + Send + Sync + 'static,
    ) -> Result<Press, Never> {
        Ok(Press {
            icon,
            now: now == InEffect::Yes,
            chief: false,
            does: Arc::new(move |showing| {
                does(showing);
                false
            }),
        })
    }

    pub fn chief(mut self) -> Result<Press, Never> {
        self.chief = true;

        Ok(self)
    }
}

#[derive(Clone)]
pub struct Across {
    pub presses: Vec<Press>,
    pub at: usize,
}

impl Row {
    pub fn said(says: &str, aside: &str) -> Result<Self, Never> {
        Ok(Row { says: says.to_string(), aside: aside.to_string(), ..Row::default() })
    }

    pub fn nothing(says: &str) -> Result<Self, Never> {
        let Ok(said) = Row::said(says, "");

        Ok(Row { nothing: true, ..said })
    }

    pub fn naming(says: &str, aside: &str) -> Result<Self, Never> {
        let Ok(said) = Row::said(says, aside);

        Ok(Row { naming: true, ..said })
    }

    pub fn new(says: &str, aside: &str, does: Does) -> Result<Self, Never> {
        let Ok(said) = Row::said(says, aside);

        Ok(Row { does: Some(does), ..said })
    }

    pub fn back(
        says: &str,
        then: impl Fn(&dyn Showing) + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        let Ok(does) = Does::and_stay(then);

        Row::new(&format!("{} {says}", crate::marks::BEFORE), "", does)
    }

    pub(crate) fn line_to_type_in() -> Result<Self, Never> {
        Ok(Row { typing: true, ..Row::default() })
    }

    pub fn ended(mut self, less: &str, more: &str) -> Result<Self, Never> {
        self.ends = Some((less.to_string(), more.to_string()));

        Ok(self)
    }

    pub fn levelled(mut self, level: Level) -> Result<Self, Never> {
        self.level = Some(level);

        Ok(self)
    }

    pub fn picturing(mut self, picture: Picture) -> Result<Self, Never> {
        self.picture = picture;

        Ok(self)
    }

    pub fn tailing(mut self, tail: Picture) -> Result<Self, Never> {
        self.tail = Some(tail);

        Ok(self)
    }

    pub fn opening(mut self) -> Result<Self, Never> {
        self.opens = true;

        Ok(self)
    }

    pub fn offering(
        mut self,
        more: impl Fn(&dyn Showing) -> bool + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.more = Some(Arc::new(more));

        Ok(self)
    }

    pub fn seeking(
        mut self,
        seek: impl Fn(&dyn Showing, f64) + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.seek = Some(Arc::new(seek));

        Ok(self)
    }

    pub fn showing(picture: Picture) -> Result<Self, Never> {
        Ok(Row { picture, naming: true, middle: true, ..Row::default() })
    }

    pub fn stacked(picture: Picture, says: &str, aside: &str) -> Result<Self, Never> {
        let Ok(said) = Row::said(says, aside);

        Ok(Row { picture, naming: true, stacked: true, ..said })
    }

    pub fn chief(mut self) -> Result<Self, Never> {
        self.chief = true;

        Ok(self)
    }

    pub fn choosing(mut self, does: Does) -> Result<Self, Never> {
        self.does = Some(does);
        self.naming = false;

        Ok(self)
    }

    pub fn in_the_middle(mut self) -> Result<Self, Never> {
        self.middle = true;

        Ok(self)
    }

    pub fn pressing(presses: Vec<Press>, at: usize) -> Result<Self, Never> {
        let at = at.min(presses.len().saturating_sub(1));
        let taken = presses.clone();
        let standing = at;
        let Ok(does) = Does::call(move |showing| match taken.get(standing) {
            Some(press) => (press.does)(showing),
            None => false,
        });

        Ok(Row {
            does: Some(does),
            across: Some(Across { presses, at }),
            ..Row::default()
        })
    }

    pub fn now(&self) -> Result<InEffect, Never> {
        Ok(match self.aside == NOW {
            true => InEffect::Yes,
            false => InEffect::No,
        })
    }

    pub fn acts(&self) -> Result<Acts, Never> {
        Ok(match self.does.is_some() || self.level.is_some() || self.seek.is_some() {
            true => Acts::Yes,
            false => Acts::Nothing,
        })
    }

    pub fn heading(&self) -> Result<Heading, Never> {
        let Ok(acts) = self.acts();

        let over = self.naming
            || self.nothing
            || (acts == Acts::Nothing && self.aside.is_empty() && !self.typing);

        Ok(match over {
            true => Heading::Yes,
            false => Heading::No,
        })
    }

    pub fn looks_like(&self, other: &Row) -> Result<Same, Never> {
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
            stacked,
            across,
            chief,
        } = self;
        let Ok(mine) = across.as_ref().map(Across::looks).transpose();
        let Ok(theirs) = other.across.as_ref().map(Across::looks).transpose();

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
            && stacked == &other.stacked
            && mine == theirs
            && chief == &other.chief;

        Ok(match alike {
            true => Same::Yes,
            false => Same::No,
        })
    }
}

type Looks = (Vec<(Icon, bool, bool)>, usize);

impl Across {
    fn looks(&self) -> Result<Looks, Never> {
        Ok((
            self.presses.iter().map(|press| (press.icon, press.now, press.chief)).collect(),
            self.at,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InEffect {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acts {
    Yes,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heading {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Same {
    Yes,
    No,
}

#[derive(Clone)]
pub enum Rows {
    Asked(Arc<dyn Fn() -> Vec<Row> + Send + Sync>),
    Fixed(Vec<Row>),
}

impl Rows {
    pub fn asked(of: impl Fn() -> Vec<Row> + Send + Sync + 'static) -> Result<Self, Never> {
        Ok(Rows::Asked(Arc::new(of)))
    }

    pub fn read(&self) -> Result<Vec<Row>, Never> {
        Ok(match self {
            Rows::Asked(of) => of(),
            Rows::Fixed(rows) => rows.clone(),
        })
    }
}

#[derive(Clone)]
pub enum About {
    Anything,
    Saying(String),
}

#[derive(Clone)]
pub struct Watch {
    pub argv: Vec<String>,
    pub about: About,
}

impl Watch {
    pub fn on(argv: &[&str], about: &str) -> Result<Self, Never> {
        Ok(Watch {
            argv: argv.iter().map(|word| (*word).to_string()).collect(),
            about: About::Saying(about.to_string()),
        })
    }

    pub fn anything(argv: &[&str]) -> Result<Self, Never> {
        Ok(Watch {
            argv: argv.iter().map(|word| (*word).to_string()).collect(),
            about: About::Anything,
        })
    }
}

#[derive(Clone)]
pub struct Sought {
    pub about: String,
    pub then: Answer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stirred {
    Woke,
    Awake,
}

pub type Stirring = Arc<dyn Fn() -> Stirred + Send + Sync>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Set {
    #[default]
    FromTheTop,
    InTheMiddle,
}

#[derive(Clone)]
pub struct Page {
    pub title: String,
    pub rows: Rows,
    pub sought: Option<Sought>,
    pub back: Option<Act>,
    pub entered: Option<Act>,
    pub meanwhile: Option<Arc<dyn Fn() -> Vec<Row> + Send + Sync>>,
    pub watch: Option<Watch>,
    pub stirs: Option<Stirring>,
    pub set: Set,
}

impl Page {
    pub fn new(title: &str, rows: Rows) -> Result<Self, Never> {
        Ok(Page {
            title: title.to_string(),
            rows,
            sought: None,
            back: None,
            entered: None,
            meanwhile: None,
            watch: None,
            stirs: None,
            set: Set::FromTheTop,
        })
    }

    pub fn in_the_middle(mut self) -> Result<Self, Never> {
        self.set = Set::InTheMiddle;

        Ok(self)
    }

    pub fn searching(
        mut self,
        about: &str,
        act: impl Fn(&dyn Showing, &str) + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.sought = Some(Sought { about: about.to_string(), then: Arc::new(act) });

        Ok(self)
    }

    pub fn on_back(
        mut self,
        act: impl Fn(&dyn Showing) -> bool + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.back = Some(Arc::new(act));

        Ok(self)
    }

    pub fn on_arriving(
        mut self,
        act: impl Fn(&dyn Showing) + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.entered = Some(Arc::new(move |showing| {
            act(showing);
            false
        }));

        Ok(self)
    }

    pub fn meanwhile(
        mut self,
        rows: impl Fn() -> Vec<Row> + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.meanwhile = Some(Arc::new(rows));

        Ok(self)
    }

    pub fn watching(mut self, watch: Watch) -> Result<Self, Never> {
        self.watch = Some(watch);

        Ok(self)
    }

    pub fn stirring(
        mut self,
        act: impl Fn() -> Stirred + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.stirs = Some(Arc::new(act));

        Ok(self)
    }
}

pub fn find(pages: &[Page], name: Option<&str>) -> Result<usize, Never> {
    let wanted = match name.map(|name| name.trim().to_lowercase()) {
        Some(wanted) => wanted,
        None => return Ok(0),
    };

    Ok(pages.iter().position(|page| page.title.to_lowercase() == wanted).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_rows_that_do_different_things_and_read_the_same_look_the_same() {
        let Ok(runs) = Does::run(&["firefox"]);
        let Ok(calls) = Does::call(|_| true);
        let Ok(one) = Row::new("Firefox", "", runs);
        let Ok(two) = Row::new("Firefox", "", calls);

        assert_eq!(one.looks_like(&two), Ok(Same::Yes));
    }

    fn runs(says: &str, aside: &str) -> Row {
        let Ok(does) = Does::run(&["firefox"]);
        let Ok(row) = Row::new(says, aside, does);

        row
    }

    #[test]
    fn anything_that_is_drawn_differently_is_a_row_that_must_be_drawn_again() {
        let row = runs("Firefox", "");
        let Ok(said) = Row::said("Firefox", "");
        let Ok(opened) = row.clone().opening();
        let Ok(pictured) = row.clone().picturing(Picture::Space);
        let Ok(naming) = Row::naming("Firefox", "");
        let Ok(chief) = row.clone().chief();
        let Ok(nothing) = Row::nothing("Firefox");

        assert_eq!(row.looks_like(&runs("Chromium", "")), Ok(Same::No));
        assert_eq!(row.looks_like(&runs("Firefox", "now")), Ok(Same::No));
        assert_eq!(row.looks_like(&said), Ok(Same::No));
        assert_eq!(row.looks_like(&opened), Ok(Same::No));
        assert_eq!(row.looks_like(&pictured), Ok(Same::No));
        assert_eq!(row.looks_like(&naming), Ok(Same::No));
        assert_eq!(row.looks_like(&chief), Ok(Same::No));
        assert_eq!(row.looks_like(&nothing), Ok(Same::No));
    }

    fn levelled(says: &str, aside: &str) -> Row {
        let Ok(said) = Row::said(says, aside);
        let Ok(row) = said.levelled(Arc::new(|_| ()));

        row
    }

    #[test]
    fn a_level_that_says_a_new_reading_is_drawn_again() {
        let quiet = levelled("Volume", "40%");
        let loud = levelled("Volume", "60%");

        assert_eq!(quiet.looks_like(&loud), Ok(Same::No));
        assert_eq!(quiet.looks_like(&quiet.clone()), Ok(Same::Yes));
    }

    fn press(icon: Icon, now: InEffect) -> Press {
        let Ok(press) = Press::new(icon, now, |_| ());

        press
    }

    fn strip() -> Vec<Press> {
        vec![
            press(Icon::Shuffle, InEffect::No),
            press(Icon::Pause, InEffect::No),
            press(Icon::Repeat, InEffect::Yes),
        ]
    }

    fn pressing(presses: Vec<Press>, at: usize) -> Row {
        let Ok(row) = Row::pressing(presses, at);

        row
    }

    #[test]
    fn a_strip_of_presses_takes_the_one_being_stood_on() {
        let taken = Arc::new(std::sync::atomic::AtomicUsize::new(9));
        let presses: Vec<Press> = (0..3)
            .map(|at| {
                let taken = Arc::clone(&taken);
                let Ok(press) = Press::new(Icon::Play, InEffect::No, move |_| {
                    taken.store(at, std::sync::atomic::Ordering::SeqCst);
                });

                press
            })
            .collect();
        let row = pressing(presses, 2);

        let act = match row.does {
            Some(Does::Call(act)) => act,
            Some(Does::Run(_)) | None => panic!("a strip with nothing to press"),
        };

        act(&Nowhere);

        assert_eq!(taken.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn standing_past_the_end_of_a_strip_stands_on_the_last_press() {
        let row = pressing(strip(), 40);

        assert_eq!(row.across.expect("a strip").at, 2);
    }

    #[test]
    fn a_strip_walked_along_is_drawn_again() {
        let here = pressing(strip(), 0);
        let there = pressing(strip(), 1);

        assert_eq!(here.looks_like(&there), Ok(Same::No));
        assert_eq!(here.looks_like(&pressing(strip(), 0)), Ok(Same::Yes));
    }

    #[test]
    fn a_press_that_has_come_on_is_drawn_again() {
        let off = pressing(strip(), 1);
        let mut lit = strip();
        lit[0].now = true;

        assert_eq!(off.looks_like(&pressing(lit, 1)), Ok(Same::No));
    }

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
            .map(|title| {
                let Ok(page) = Page::new(title, Rows::Fixed(Vec::new()));

                page
            })
            .to_vec()
    }

    #[test]
    fn a_tab_is_found_by_the_word_on_it_however_it_is_written() {
        assert_eq!(find(&pages(), Some("sound")), Ok(1));
        assert_eq!(find(&pages(), Some("  Wi-Fi ")), Ok(2));
    }

    #[test]
    fn a_name_nothing_answers_to_opens_the_first_tab() {
        assert_eq!(find(&pages(), Some("Telepathy")), Ok(0));
        assert_eq!(find(&pages(), None), Ok(0));
    }

    #[test]
    fn a_row_that_says_now_is_the_one_in_effect() {
        let Ok(marked) = Row::said("Balanced", NOW);
        let Ok(plain) = Row::said("Balanced", "");

        assert_eq!(marked.now(), Ok(InEffect::Yes));
        assert_eq!(plain.now(), Ok(InEffect::No));
    }

    #[test]
    fn rows_are_asked_for_at_the_moment_they_are_drawn() {
        let Ok(rows) = Rows::asked(|| {
            let Ok(row) = Row::said("Speakers", "half");

            vec![row]
        });

        let Ok(read) = rows.read();

        assert_eq!(read.first().map(|row| row.aside.clone()), Some("half".to_string()));
    }
}
