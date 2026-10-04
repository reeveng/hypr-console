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
//!
//! ## The letter a row stands under
//!
//! `lettered` puts a heading over every run of rows beginning with the same
//! letter, the way a shelf of anything long enough to scroll is arranged. It
//! is here rather than in either panel that wants it because what letter a row
//! stands under is a question about a list on a screen and not about media or
//! about music, and two crates answering it separately is where the two lists
//! would start disagreeing about where a file called `_draft` belongs. A
//! heading the caller wrote already is left alone and starts the letters
//! again, so a list that is sections of its own can still have letters inside
//! them. `standing` is the same answer as a number, for a caller sorting
//! before it draws: digits first, then letters, then everything that begins
//! with neither.

use std::path::PathBuf;
use std::sync::Arc;
use crate::icons::Icon;
use console_core_color::Oklch;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const THE_FIRST_PAGE: u32 = 0;


pub trait Showing {
    fn refresh(&self);

    fn replace(&self, standing_on: u32);

    fn forget_typing(&self);

    fn ask(&self, question: &str, then: Answer);

    fn sure(&self, question: &str, about: Subject<'_>, does: &[&str], then: OnChosen);

    fn ask_aloud(&self, question: &str, then: Answer);

    fn open_out(&self);

    fn toggle_full_screen(&self);

    fn zoom_by(&self, factor: f64);

    fn framed(&self) -> Result<Option<crate::zoom::Framed>, Never> {
        Ok(None)
    }

    fn select(&self, keys: Vec<String>);

    fn turn_to(&self, tab: u32);

    fn note(&self, said: &str);

    fn later(&self, arguments: Vec<String>);

    fn leave_running(&self, arguments: Vec<String>);
}

pub struct Nowhere;

impl Showing for Nowhere {
    fn refresh(&self) {}

    fn replace(&self, _standing_on: u32) {}

    fn forget_typing(&self) {}

    fn ask(&self, _question: &str, _then: Answer) {}

    fn sure(&self, _question: &str, _about: Subject<'_>, _does: &[&str], _then: OnChosen) {}

    fn ask_aloud(&self, _question: &str, _then: Answer) {}

    fn note(&self, _said: &str) {}

    fn later(&self, _arguments: Vec<String>) {}

    fn leave_running(&self, _arguments: Vec<String>) {}

    fn open_out(&self) {}

    fn toggle_full_screen(&self) {}

    fn zoom_by(&self, _factor: f64) {}

    fn select(&self, _keys: Vec<String>) {}

    fn turn_to(&self, _tab: u32) {}
}

pub type Answer = Arc<dyn Fn(&dyn Showing, &str) + Send + Sync>;

pub type OnChosen = Arc<dyn Fn(&dyn Showing, u32) + Send + Sync>;

pub type Act = Arc<dyn Fn(&dyn Showing) -> bool + Send + Sync>;

pub type Level = Arc<dyn Fn(i32) + Send + Sync>;

#[derive(Clone)]
pub enum Handler {
    Call(Act),
    Run(Vec<String>),
}

impl Handler {
    pub fn run(arguments: &[&str]) -> Result<Self, Never> {
        Ok(Handler::Run(arguments.iter().map(|word| (*word).to_string()).collect()))
    }

    pub fn call(
        act: impl Fn(&dyn Showing) -> bool + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        Ok(Handler::Call(Arc::new(act)))
    }

    pub fn and_stay(act: impl Fn(&dyn Showing) + Send + Sync + 'static) -> Result<Self, Never> {
        Handler::call(move |showing| {
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

impl Bar {
    pub fn filled(self, wide: u32) -> Result<u32, Never> {
        let whole = u128::from(wide).saturating_mul(u128::from(self.at.min(self.of)));
        let share = match whole.checked_div(u128::from(self.of)) {
            Some(share) => share,
            None => 0,
        };
        let Ok(filled) = fitted::<u128, u32>(share);

        Ok(filled.min(wide))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Track {
    pub from: i32,
    pub width: u32,
}

impl Track {
    pub fn fraction_at(self, hit: i32) -> Result<f64, Never> {
        Ok(match self.width > 0 {
            true => (f64::from(hit.saturating_sub(self.from)) / f64::from(self.width)).clamp(0.0, 1.0),
            false => 0.0,
        })
    }
}

pub type Seek = Arc<dyn Fn(&dyn Showing, f64) + Send + Sync>;

#[derive(Clone, Default)]
#[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    allow(
        explicit048_no_unreal_state,
        reason = "these are the ways one row draws rather than the kinds of row there are: `naming` with `middle` and `stacked` is one row, and so is `nothing` on its own, so no enum here would have fewer states than the combinations do"
    )
)]
pub struct Row {
    pub says: String,
    pub aside: String,
    pub does: Option<Handler>,
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
    pub buttons: Option<Across>,
    pub cells: Vec<Cell>,
    pub chief: bool,
    pub headline: Option<Headline>,
    pub key: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headline {
    pub title: String,
    pub subtitle: String,
    pub icon: Option<Icon>,
    pub says: String,
    pub big: String,
    pub aside: String,
    pub alignment: Alignment,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    #[default]
    Leading,
    Trailing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Face {
    Glyph(Icon),
    Swatch(Oklch),
    Written(&'static str),
}

#[derive(Clone)]
pub struct ButtonPress {
    pub face: Face,
    pub now: Active,
    pub chief: bool,
    pub placed: Placed,
    pub style: ButtonStyle,
    pub does: Act,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Gray,
    Tinted,
    Filled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    Among,
    Corner,
}

impl ButtonPress {
    pub fn new(
        icon: Icon,
        now: Active,
        does: impl Fn(&dyn Showing) + Send + Sync + 'static,
    ) -> Result<ButtonPress, Never> {
        Ok(ButtonPress {
            face: Face::Glyph(icon),
            now,
            chief: false,
            placed: Placed::Among,
            style: ButtonStyle::Gray,
            does: Arc::new(move |showing| {
                does(showing);
                false
            }),
        })
    }

    pub fn swatch(
        color: Oklch,
        now: Active,
        does: impl Fn(&dyn Showing) + Send + Sync + 'static,
    ) -> Result<ButtonPress, Never> {
        Ok(ButtonPress {
            face: Face::Swatch(color),
            now,
            chief: false,
            placed: Placed::Among,
            style: ButtonStyle::Gray,
            does: Arc::new(move |showing| {
                does(showing);
                false
            }),
        })
    }

    pub fn labelled(
        says: &'static str,
        now: Active,
        does: impl Fn(&dyn Showing) + Send + Sync + 'static,
    ) -> Result<ButtonPress, Never> {
        Ok(ButtonPress {
            face: Face::Written(says),
            now,
            chief: false,
            placed: Placed::Among,
            style: ButtonStyle::Gray,
            does: Arc::new(move |showing| {
                does(showing);
                false
            }),
        })
    }

    pub fn chief(mut self) -> Result<ButtonPress, Never> {
        self.chief = true;

        Ok(self)
    }

    pub fn cornered(mut self) -> Result<ButtonPress, Never> {
        self.placed = Placed::Corner;

        Ok(self)
    }

    pub fn styled(mut self, style: ButtonStyle) -> Result<ButtonPress, Never> {
        self.style = style;

        Ok(self)
    }
}

#[derive(Clone)]
pub struct Across {
    pub presses: Vec<ButtonPress>,
    pub at: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    allow(
        explicit048_no_unreal_state,
        reason = "whether a cell is the one in effect and whether it carries an icon are two questions with every answer real: the hour now has a sky like every other hour"
    )
)]
pub struct Cell {
    pub says: String,
    pub now: bool,
    pub icon: Option<Icon>,
}

impl Cell {
    pub fn new(says: &str, now: Active) -> Result<Self, Never> {
        Ok(Cell { says: says.to_string(), now: now == Active::Yes, icon: None })
    }

    pub fn with_icon(mut self, icon: Icon) -> Result<Self, Never> {
        self.icon = Some(icon);

        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aside<'a>(pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subject<'a>(pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ends<'a> {
    pub less: &'a str,
    pub more: &'a str,
}

impl Row {
    pub fn text(says: &str, aside: Aside<'_>) -> Result<Self, Never> {
        Ok(Row { says: says.to_string(), aside: aside.0.to_string(), ..Row::default() })
    }

    pub fn placeholder(says: &str) -> Result<Self, Never> {
        let Ok(said) = Row::text(says, Aside(""));

        Ok(Row { nothing: true, ..said })
    }

    pub fn naming(says: &str, aside: Aside<'_>) -> Result<Self, Never> {
        let Ok(said) = Row::text(says, aside);

        Ok(Row { naming: true, ..said })
    }

    pub fn new(says: &str, aside: Aside<'_>, does: Handler) -> Result<Self, Never> {
        let Ok(said) = Row::text(says, aside);

        Ok(Row { does: Some(does), ..said })
    }

    pub fn back(
        says: &str,
        then: impl Fn(&dyn Showing) + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        let Ok(does) = Handler::and_stay(then);

        Row::new(&format!("{} {says}", crate::marks::BEFORE), Aside(""), does)
    }

    pub fn ended(mut self, ends: Ends<'_>) -> Result<Self, Never> {
        self.ends = Some((ends.less.to_string(), ends.more.to_string()));

        Ok(self)
    }

    pub fn with_level(mut self, level: Level) -> Result<Self, Never> {
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

    pub fn picture(picture: Picture) -> Result<Self, Never> {
        Ok(Row { picture, naming: true, middle: true, ..Row::default() })
    }

    pub fn headline(picture: Picture, headline: Headline) -> Result<Self, Never> {
        Ok(Row { picture, headline: Some(headline), naming: true, middle: true, ..Row::default() })
    }

    pub fn stacked(picture: Picture, says: &str, aside: Aside<'_>) -> Result<Self, Never> {
        let Ok(said) = Row::text(says, aside);

        Ok(Row { picture, naming: true, stacked: true, ..said })
    }

    pub fn chief(mut self) -> Result<Self, Never> {
        self.chief = true;

        Ok(self)
    }

    pub fn choosing(mut self, does: Handler) -> Result<Self, Never> {
        self.does = Some(does);
        self.naming = false;

        Ok(self)
    }

    pub fn in_the_middle(mut self) -> Result<Self, Never> {
        self.middle = true;

        Ok(self)
    }

    pub fn pressing(presses: Vec<ButtonPress>, at: u32) -> Result<Self, Never> {
        let Ok(last) = fitted::<_, u32>(presses.len().saturating_sub(1));
        let at = at.min(last);
        let taken = presses.clone();
        let Ok(standing) = index(at);
        let Ok(does) = Handler::call(move |showing| match taken.get(standing) {
            Some(press) => (press.does)(showing),
            None => false,
        });

        Ok(Row {
            does: Some(does),
            buttons: Some(Across { presses, at }),
            ..Row::default()
        })
    }

    pub fn celled(cells: Vec<Cell>) -> Result<Self, Never> {
        Ok(Row { cells, ..Row::default() })
    }

    pub fn naming_cells(cells: Vec<Cell>) -> Result<Self, Never> {
        let Ok(row) = Row::celled(cells);

        Ok(Row { naming: true, ..row })
    }

    pub fn selectable(mut self, key: &str) -> Result<Self, Never> {
        self.key = Some(key.to_string());

        Ok(self)
    }

    pub fn now(&self) -> Result<Active, Never> {
        Ok(match self.aside == NOW {
            true => Active::Yes,
            false => Active::No,
        })
    }

    pub fn acts(&self) -> Result<Action, Never> {
        Ok(match self.does.is_some() || self.level.is_some() || self.seek.is_some() {
            true => Action::Yes,
            false => Action::None,
        })
    }

    pub fn heading(&self) -> Result<Heading, Never> {
        let Ok(acts) = self.acts();

        let over = self.naming
            || self.nothing
            || (acts == Action::None && self.aside.is_empty() && !self.typing);

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
            buttons: across,
            cells,
            chief,
            headline,
            key,
        } = self;
        let Ok(mine) = across.as_ref().map(Across::looks).transpose();
        let Ok(theirs) = other.buttons.as_ref().map(Across::looks).transpose();

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
            && cells == &other.cells
            && chief == &other.chief
            && headline == &other.headline
            && key == &other.key;

        Ok(match alike {
            true => Same::Yes,
            false => Same::No,
        })
    }
}

type Looks = (Vec<(Face, Active, bool, ButtonStyle)>, u32);

impl Across {
    fn looks(&self) -> Result<Looks, Never> {
        Ok((
            self.presses.iter().map(|press| (press.face, press.now, press.chief, press.style)).collect(),
            self.at,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Active {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Yes,
    None,
}

pub const NUMBERS: &str = "0-9";

pub const REST: &str = "Other";

pub fn under(says: &str) -> Result<String, Never> {
    let letter = match says.chars().next() {
        Some(letter) => letter,
        None => return Ok(REST.to_string()),
    };

    Ok(match (letter.is_alphabetic(), letter.is_numeric()) {
        (true, _) => letter.to_uppercase().to_string(),
        (false, true) => NUMBERS.to_string(),
        (false, false) => REST.to_string(),
    })
}

pub fn sort_rank(says: &str) -> Result<u8, Never> {
    let Ok(under) = under(says);

    Ok(match under.as_str() {
        NUMBERS => 0,
        REST => 2,
        _letter => 1,
    })
}

pub fn lettered(rows: Vec<Row>) -> Result<Vec<Row>, Never> {
    let mut lettered: Vec<Row> = Vec::new();
    let mut standing: Option<String> = None;

    for row in rows {
        let Ok(heading) = row.heading();

        match heading {
            Heading::Yes => {
                standing = None;
                lettered.push(row);
                continue;
            },
            Heading::No => {},
        }

        let Ok(under) = under(&row.says);

        match standing.as_deref() == Some(under.as_str()) {
            true => {},
            false => {
                let Ok(naming) = Row::naming(&under, Aside(""));

                lettered.push(naming);
                standing = Some(under);
            },
        }

        lettered.push(row);
    }

    Ok(lettered)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heading {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step(pub i32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bare {
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wears {
    WhatElse,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Holds {
    Level,
    None,
}

pub fn holds(row: &Row) -> Result<Holds, Never> {
    Ok(match row.level.is_some() {
        true => Holds::Level,
        false => Holds::None,
    })
}

pub fn wears(row: &Row) -> Result<Wears, Never> {
    let Ok(bare) = bare(row);

    Ok(match (row.more.is_some(), bare) {
        (true, Bare::No) => Wears::WhatElse,
        (true, Bare::Yes) | (false, _) => Wears::None,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Beside {
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Nudged {
    Stand,
    Back,
    Level,
    None,
}

pub fn nudged(beside: Beside, wears: Wears, holds: Holds, step: i32) -> Result<Nudged, Never> {
    Ok(match (beside, wears, holds, step > 0) {
        (Beside::No, Wears::WhatElse, Holds::None, true) => Nudged::Stand,
        (Beside::Yes, _, _, false) => Nudged::Back,
        (Beside::Yes, _, _, true) => Nudged::None,
        (Beside::No, Wears::WhatElse, Holds::None, false)
        | (Beside::No, Wears::WhatElse, Holds::Level, _)
        | (Beside::No, Wears::None, _, _) => Nudged::Level,
    })
}

pub fn bare(row: &Row) -> Result<Bare, Never> {
    Ok(match &row.ends {
        Some((less, more)) => match less.is_empty() && more.is_empty() {
            true => Bare::Yes,
            false => Bare::No,
        },
        None => Bare::No,
    })
}

pub fn ends_of(row: &Row) -> Result<(&str, &str), Never> {
    Ok(match &row.ends {
        Some((less, more)) => (less.as_str(), more.as_str()),
        None => (crate::marks::LESS, crate::marks::MORE),
    })
}

pub fn walked(rows: &[Row], at: i32, step: Step) -> Result<i32, Never> {
    let Ok(whole_10) = fitted::<_, i32>(rows.len().saturating_sub(1));
    let last = whole_10;
    let stepped = |going: &i32| Some(going.saturating_add(step.0));
    let clear = std::iter::successors(stepped(&at), stepped)
        .take_while(|going| (0..=last).contains(going))
        .find(|going| {
            let Ok(going) = index(*going);

            rows.get(going).is_none_or(|row| {
                let Ok(heading) = row.heading();

                heading == Heading::No
            })
        });

    Ok(match clear {
        Some(going) => going,
        None => at,
    })
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Same {
    Yes,
    No,
}

#[derive(Clone)]
pub enum Rows {
    Computed(Arc<dyn Fn() -> Vec<Row> + Send + Sync>),
    Fixed(Vec<Row>),
}

impl Rows {
    pub fn computed(of: impl Fn() -> Vec<Row> + Send + Sync + 'static) -> Result<Self, Never> {
        Ok(Rows::Computed(Arc::new(of)))
    }

    pub fn read(&self) -> Result<Vec<Row>, Never> {
        Ok(match self {
            Rows::Computed(of) => of(),
            Rows::Fixed(rows) => rows.clone(),
        })
    }
}

#[derive(Clone)]
pub struct Watch {
    pub arguments: Vec<String>,
}

impl Watch {
    pub fn command(arguments: &[&str]) -> Result<Self, Never> {
        Ok(Watch { arguments: arguments.iter().map(|word| (*word).to_string()).collect() })
    }
}

#[derive(Clone)]
pub struct Subscription {
    pub event_group: console_program_contract::EventGroup,
    pub worth: console_events::again::Worthwhile,
}

#[derive(Clone)]
pub struct Sought {
    pub about: String,
    pub then: Answer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeOutcome {
    Woke,
    AlreadyAwake,
}

pub type OnWake = Arc<dyn Fn() -> WakeOutcome + Send + Sync>;

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
    pub listens: Option<Subscription>,
    pub stirs: Option<OnWake>,
    pub set: Set,
    pub selecting: Vec<Selecting>,
}

pub const SELECT: &str = "Select";

pub const DELETE: &str = "Delete";

pub const DELETE_SELECTED: &str = "Delete these?";

pub const SHOW_IN_FILES: &str = "Show in Files";

pub type Shows = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

pub fn shown_or_selected(
    name: &str,
    at: &std::path::Path,
    shows: impl Fn(&dyn Showing) + Send + Sync + 'static,
) -> Result<impl Fn(&dyn Showing) -> bool + Send + Sync + 'static, Never> {
    let shows: Shows = Arc::new(shows);
    let name = name.to_string();
    let key = at.to_string_lossy().to_string();

    Ok(move |showing: &dyn Showing| {
        let shows = Arc::clone(&shows);
        let key = key.clone();

        showing.sure(
            &name,
            Subject(""),
            &[SHOW_IN_FILES, SELECT],
            Arc::new(move |showing, which| match which {
                0 => shows(showing),
                _ => showing.select(vec![key.clone()]),
            }),
        );

        false
    })
}

pub type OnSelected = Arc<dyn Fn(&dyn Showing, &[String]) + Send + Sync>;

#[derive(Clone)]
pub struct Selecting {
    pub says: String,
    pub act: OnSelected,
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
            listens: None,
            stirs: None,
            set: Set::FromTheTop,
            selecting: Vec::new(),
        })
    }

    pub fn trashing_selected(self) -> Result<Self, Never> {
        self.selecting(DELETE, |showing, keys| {
            let keys = keys.to_vec();
            let about = match keys.as_slice() {
                [one] => match std::path::Path::new(one).file_name() {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => one.clone(),
                },
                [] | [_, _, ..] => format!("{} Items", keys.len()),
            };

            let then: OnChosen = Arc::new(move |showing, _| {
                let Ok(mut trashing) = console_core_external_programs::Program::Gio.arguments(&["trash", "--"]);

                trashing.extend(keys.iter().cloned());
                showing.later(trashing);
            });

            showing.sure(DELETE_SELECTED, Subject(&about), &[DELETE], then);
        })
    }

    pub fn selecting(
        mut self,
        says: &str,
        act: impl Fn(&dyn Showing, &[String]) + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.selecting.push(Selecting { says: says.to_string(), act: Arc::new(act) });

        Ok(self)
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

    pub fn with_watch(mut self, watch: Watch) -> Result<Self, Never> {
        self.watch = Some(watch);

        Ok(self)
    }

    pub fn with_subscription(
        mut self,
        event_group: console_program_contract::EventGroup,
        worth: console_events::again::Worthwhile,
    ) -> Result<Self, Never> {
        self.listens = Some(Subscription { event_group, worth });

        Ok(self)
    }

    pub fn stirring(
        mut self,
        act: impl Fn() -> WakeOutcome + Send + Sync + 'static,
    ) -> Result<Self, Never> {
        self.stirs = Some(Arc::new(act));

        Ok(self)
    }
}

pub fn find(pages: &[Page], name: Option<&str>) -> Result<u32, Never> {
    let wanted = match name.map(|name| name.trim().to_lowercase()) {
        Some(wanted) => wanted,
        None => return Ok(0),
    };

    Ok(match pages.iter().position(|page| page.title.to_lowercase() == wanted) {
        Some(at) => {
            let Ok(at) = fitted(at);

            at
        },
        None => THE_FIRST_PAGE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    #[test]
    fn a_heading_stands_over_every_run_of_one_letter_and_only_over_the_first_of_it() {
        let Ok(apple) = stays("Apple.png");
        let Ok(apricot) = stays("apricot.png");
        let Ok(boat) = stays("boat.png");

        let Ok(lettered) = lettered(vec![apple, apricot, boat]);
        let says: Vec<&str> = lettered.iter().map(|row| row.says.as_str()).collect();

        assert_eq!(says, ["A", "Apple.png", "apricot.png", "B", "boat.png"]);
    }

    #[test]
    fn a_heading_someone_else_wrote_is_left_where_it_is_and_starts_the_letters_again() {
        let Ok(album) = Row::naming("Albums", Aside(""));
        let Ok(apple) = stays("Apple.png");

        let Ok(lettered) = lettered(vec![album, apple]);
        let says: Vec<&str> = lettered.iter().map(|row| row.says.as_str()).collect();

        assert_eq!(says, ["Albums", "A", "Apple.png"]);
    }

    #[test]
    fn what_does_not_begin_with_a_letter_still_stands_under_something() {
        assert_eq!(under("beach.jpg"), Ok("B".to_string()));
        assert_eq!(under("Beach.jpg"), Ok("B".to_string()));
        assert_eq!(under("2019-07-04.jpg"), Ok(NUMBERS.to_string()));
        assert_eq!(under("_draft.png"), Ok(REST.to_string()));
        assert_eq!(under(""), Ok(REST.to_string()));
        assert_eq!(under("ไทย.jpg"), Ok("ไ".to_string()));
    }

    #[test]
    fn the_numbers_stand_before_the_letters_and_the_rest_after_them() {
        assert_eq!(sort_rank("2019.png"), Ok(0));
        assert_eq!(sort_rank("boat.png"), Ok(1));
        assert_eq!(sort_rank("_draft.png"), Ok(2));
    }

    fn stays(says: &str) -> Result<Row, Never> {
        let Ok(does) = Handler::and_stay(|_| {});

        Row::new(says, Aside(""), does)
    }

    #[test]
    fn two_rows_that_do_different_things_and_read_the_same_look_the_same() {
        let Ok(runs) = Handler::run(&["firefox"]);
        let Ok(calls) = Handler::call(|_| true);
        let Ok(one) = Row::new("Firefox", Aside(""), runs);
        let Ok(two) = Row::new("Firefox", Aside(""), calls);

        assert_eq!(one.looks_like(&two), Ok(Same::Yes));
    }

    fn runs(says: &str, aside: Aside<'_>) -> Result<Row, Never> {
        let Ok(does) = Handler::run(&["firefox"]);

        Row::new(says, aside, does)
    }

    #[test]
    fn anything_that_is_drawn_differently_is_a_row_that_must_be_drawn_again() {
        let Ok(row) = runs("Firefox", Aside(""));
        let Ok(chromium) = runs("Chromium", Aside(""));
        let Ok(now) = runs("Firefox", Aside("now"));
        let Ok(said) = Row::text("Firefox", Aside(""));
        let Ok(opened) = row.clone().opening();
        let Ok(pictured) = row.clone().picturing(Picture::Space);
        let Ok(naming) = Row::naming("Firefox", Aside(""));
        let Ok(chief) = row.clone().chief();
        let Ok(nothing) = Row::placeholder("Firefox");

        assert_eq!(row.looks_like(&chromium), Ok(Same::No));
        assert_eq!(row.looks_like(&now), Ok(Same::No));
        assert_eq!(row.looks_like(&said), Ok(Same::No));
        assert_eq!(row.looks_like(&opened), Ok(Same::No));
        assert_eq!(row.looks_like(&pictured), Ok(Same::No));
        assert_eq!(row.looks_like(&naming), Ok(Same::No));
        assert_eq!(row.looks_like(&chief), Ok(Same::No));
        assert_eq!(row.looks_like(&nothing), Ok(Same::No));
    }

    fn leveled(aside: Aside<'_>) -> Result<Row, Never> {
        let Ok(said) = Row::text("Volume", aside);

        said.with_level(Arc::new(|_| ()))
    }

    #[test]
    fn a_level_that_says_a_new_reading_is_drawn_again() {
        let Ok(quiet) = leveled(Aside("40%"));
        let Ok(loud) = leveled(Aside("60%"));

        assert_eq!(quiet.looks_like(&loud), Ok(Same::No));
        assert_eq!(quiet.looks_like(&quiet), Ok(Same::Yes));
    }

    fn strip() -> Result<Vec<ButtonPress>, Never> {
        let Ok(shuffle) = ButtonPress::new(Icon::Shuffle, Active::No, |_| ());
        let Ok(pause) = ButtonPress::new(Icon::Pause, Active::No, |_| ());
        let Ok(repeat) = ButtonPress::new(Icon::Repeat, Active::Yes, |_| ());

        Ok(vec![shuffle, pause, repeat])
    }

    fn standing_on(at: u32) -> Result<Row, Never> {
        let Ok(presses) = strip();

        Row::pressing(presses, at)
    }

    #[test]
    fn a_strip_of_presses_takes_the_one_being_stood_on() -> Result<(), Failure> {
        let taken = Arc::new(std::sync::atomic::AtomicU32::new(9));
        let presses: Vec<ButtonPress> = (0..3)
            .map(|at| {
                let taken = Arc::clone(&taken);
                let Ok(press) = ButtonPress::new(Icon::Play, Active::No, move |_| {
                    taken.store(at, std::sync::atomic::Ordering::SeqCst);
                });

                press
            })
            .collect();
        let Ok(row) = Row::pressing(presses, 2);

        let act = match row.does {
            Some(Handler::Call(act)) => act,
            Some(Handler::Run(_)) | None => return Err(Failure::from("a strip with nothing to press")),
        };

        act(&Nowhere);

        assert_eq!(taken.load(std::sync::atomic::Ordering::SeqCst), 2);

        Ok(())
    }

    #[test]
    fn standing_past_the_end_of_a_strip_stands_on_the_last_press() -> Result<(), Failure> {
        let Ok(row) = standing_on(40);
        let across = row.buttons.ok_or("a strip")?;

        assert_eq!(across.at, 2);

        Ok(())
    }

    #[test]
    fn a_strip_walked_along_is_drawn_again() {
        let Ok(here) = standing_on(0);
        let Ok(there) = standing_on(1);
        let Ok(here_again) = standing_on(0);

        assert_eq!(here.looks_like(&there), Ok(Same::No));
        assert_eq!(here.looks_like(&here_again), Ok(Same::Yes));
    }

    #[test]
    fn a_press_that_has_come_on_is_drawn_again() -> Result<(), Failure> {
        let Ok(off) = standing_on(1);
        let Ok(mut lit) = strip();
        let first = lit.first_mut().ok_or("a strip with nothing in it")?;

        first.now = Active::Yes;

        let Ok(lit) = Row::pressing(lit, 1);

        assert_eq!(off.looks_like(&lit), Ok(Same::No));

        Ok(())
    }

    fn pages() -> Result<Vec<Page>, Never> {
        Ok(["Battery", "Sound", "Wi-Fi"]
            .map(|title| {
                let Ok(page) = Page::new(title, Rows::Fixed(Vec::new()));

                page
            })
            .to_vec())
    }

    #[test]
    fn a_tab_is_found_by_the_word_on_it_however_it_is_written() {
        let Ok(pages) = pages();

        assert_eq!(find(&pages, Some("sound")), Ok(1));
        assert_eq!(find(&pages, Some("  Wi-Fi ")), Ok(2));
    }

    #[test]
    fn a_name_nothing_answers_to_opens_the_first_tab() {
        let Ok(pages) = pages();

        assert_eq!(find(&pages, Some("Telepathy")), Ok(0));
        assert_eq!(find(&pages, None), Ok(0));
    }

    #[test]
    fn a_row_that_says_now_is_the_one_in_effect() {
        let Ok(marked) = Row::text("Balanced", Aside(NOW));
        let Ok(plain) = Row::text("Balanced", Aside(""));

        assert_eq!(marked.now(), Ok(Active::Yes));
        assert_eq!(plain.now(), Ok(Active::No));
    }

    #[test]
    fn rows_are_asked_for_at_the_moment_they_are_drawn() {
        let Ok(rows) = Rows::computed(|| {
            let Ok(row) = Row::text("Speakers", Aside("half"));

            vec![row]
        });

        let Ok(read) = rows.read();

        assert_eq!(read.first().map(|row| row.aside.clone()), Some("half".to_string()));
    }
}
