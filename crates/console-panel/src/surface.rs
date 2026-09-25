//! The panel drawn on a surface of our own, without GTK.
//!
//! Every surface over the desktop is GTK's today, and GTK is the last thing on
//! this device deciding something this tree has already decided better. What a
//! toolkit decides is what fits, what order a press moves in, what a press
//! means and what colour a thing is, and all four are answered here already, in
//! places a screen cannot reach. What is left underneath is a rectangle the
//! compositor is reading and the protocol to keep it there, which is this.
//!
//! The notification daemon already draws on `console-draw-surface`. The keyboard
//! does too. This module brings the panels onto the same surface, following the
//! same pattern: measure text, build shapes, draw, commit, handle input.
//!
//! **Measuring is separate from drawing and comes first.** How tall a wrapped
//! line is, is the one thing placement cannot work out for itself. A `Run` is
//! measured with no surface in the room, and the height is placed with
//! arithmetic afterwards.
//!
//! **The surface is stood up before the loop and not inside it.** A layer
//! surface has no size until the compositor has configured one, and it is
//! configured only once it exists, so a loop that waits for a size before
//! asking for a surface waits for ever: every panel the host drew was a panel
//! nobody could see. The size asked for is nothing at all, which on a surface
//! anchored to all four edges is how the protocol spells the whole screen.
//!
//! **A panel that put itself away says so by going quiet.** Every other way a
//! panel ends is something the host asked for and already knows about; B on the
//! pad is not, and the host is asleep in a poll. So the far end of a pipe is
//! held for as long as this draws and dropped when it stops, which is a hangup
//! on a descriptor the host is already watching one of.
//!
//! **The loop sleeps until something says so, and has no clock.** It used to
//! wake a tenth of a second at a time, and a sixtieth while a tab was being
//! read, because three things it answers to were things it had to go and look
//! at: the flag that shuts it, the pool's words about the tab in front, and the
//! rows a reading thread hands back. Each of those says so now on the socket in
//! `frames` that the wait already watches -- shutting tells it, a thread holds
//! the subscription's receiver and tells it when a word is worth reading the
//! tab again for, and a reading tells it when it lands -- so an open panel
//! nobody touches is a process asleep in `poll`. Which topic is wanted and
//! which words are worth a reading stay with the loop, because they are the
//! tab in front's and the loop is what knows which tab that is. The one wait
//! left with a number in it is `FIRST_LOOK`: a tab with nothing on it yet
//! holds its first frame back a moment for its rows, because a card drawn
//! empty and then filled is a flash somebody sees. It waits on the same
//! socket, so a card shut during that moment is gone at once rather than at
//! the end of it.
//!
//! **The shape list is closed.** `Panel`, `Text`, and `Picture` from
//! `console-core-shapes`. Drawing is a match, and 016 makes a shape added later
//! announce itself everywhere it matters.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak, mpsc};
use std::thread;
use std::time::Duration;

use console_core_color::Oklch;
use console_core_color::palette::Wearing;
use console_core_fonts::{EM, TextStyle};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_i32};
use console_core_shapes::{
    Clip, Covers, Edge, Font, Panel as ShapePanel, Picture as ShapePicture, Pixels, Round, Shape, Weight,
    Text,
};
use console_draw_painting::{self as painting, Frame, Run};
use console_events::again::Worth;
use console_events::subscription::{Received, Subscriber, Subscriptions};
use console_program_lifetime::{BoundToParent, alongside};
use console_draw_surface::{
    Anchor, Visible, Keyboard, KeyboardEvent, Keysym, Margin, Part, PointerEvent, Room, Surface, Under,
    Wanted,
};

use crate::description;
use crate::fitting;
use crate::keys::{self, Driving, Meaning};
use console_sound_effects::catalogue::Sound;
use console_sound_effects::{Degree, SoundEffects};
use crate::marks;
use crate::page::{
    Answer, Bare, Beside, Handler, Heading, Holds, Subscription, Nudged, Page, Picture, Row, Showing, Step,
    Subject, OnChosen, WakeOutcome, Wears, walked,
};
use crate::shape;
use crate::strip;

const TAB_LEAST: i32 = EM * 40 / 9;
const OVER_ROWS: i32 = EM * 5 / 9;
const BEFORE_THE_FIRST_ROW: i32 = -1;
const WHATEVER_THE_SCREEN_IS: Size<u32> = Size { width: 0, height: 0 };
const FINGER: i32 = EM * 8 / 3;
const GLYPH: i32 = EM * 4 / 3;
const PRESS_WIDE: i32 = FINGER * 3 / 2;
const TRACK: i32 = EM / 3;
const CARD_ROUND: u32 = (EM * 8 / 9).unsigned_abs();
const CARD_EDGE: u32 = (EM / 9).unsigned_abs();
const TAB_ROUND: u32 = (EM * 2 / 3).unsigned_abs();
const ROW_ROUND: u32 = (EM * 2 / 3).unsigned_abs();
const BETWEEN_ROWS: i32 = fitting::ROW - FINGER;
const INSIDE_A_ROW: i32 = EM * 8 / 9;
const CARET: u32 = (EM / 9).unsigned_abs();
const DRAGGED: f64 = 12.0;
const A_NOTCH: f64 = 10.0;
const FIRST_LOOK: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    Expanded,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    Card,
    Screen,
}

pub struct Close(Arc<AtomicBool>);

impl Close {
    pub fn shut(&self) -> Result<(), Never> {
        self.0.store(true, Ordering::Relaxed);

        crate::frames::tell(crate::frames::Notice::Closed)
    }
}

const NO_ROOM: Size<u32> = Size { width: 0, height: 0 };

const SWATCH_EDGE: u32 = 2;

const SELECTED_MARK: i32 = EM * 5 / 6;

const SELECT_ALL: &str = "Select All";

const DESELECT_ALL: &str = "Deselect All";
const ANYTHING_WIDE: u32 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
struct MeasuredTab {
    title: Size<u32>,
    chosen: Size<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MeasuredRow {
    says: Size<u32>,
    aside: Size<u32>,
    less: Size<u32>,
    more: Size<u32>,
    cells: Vec<Size<u32>>,
    presses: Vec<Size<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeasuredMarks {
    shut: Size<u32>,
    before: Size<u32>,
    after: Size<u32>,
    into: Size<u32>,
    offers: Size<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Measured {
    tabs: Vec<MeasuredTab>,
    rows: Vec<MeasuredRow>,
    marks: MeasuredMarks,
    typed: Size<u32>,
    answers: Vec<Size<u32>>,
    note: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TabWindow {
    from: u32,
    fits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filled {
    Typed,
    Empty,
}

fn sized_as(said: &str, weight: Weight, style: TextStyle) -> Result<Size<u32>, Never> {
    let Ok(font) = font(style);

    painting::measured(Run { said, weight, width: ANYTHING_WIDE }, &font)
}

fn weight_of(row: &Row) -> Result<Weight, Never> {
    Ok(match row.naming {
        true => Weight::Bold,
        false => Weight::Plain,
    })
}

fn measure_tabs(pages: &[Page], wide: u32) -> Result<Vec<MeasuredTab>, Never> {
    let mut measured = Vec::new();
    let font = font(TextStyle::Headline)?;

    for page in pages {
        let run = Run { said: &page.title, weight: Weight::Plain, width: wide };
        let title = painting::measured(run, &font)?;

        let run = Run { said: &page.title, weight: Weight::Bold, width: wide };
        let chosen = painting::measured(run, &font)?;

        measured.push(MeasuredTab { title, chosen });
    }

    Ok(measured)
}

fn measure_rows(rows: &[Row], wide: u32) -> Result<Vec<MeasuredRow>, Never> {
    let mut measured = Vec::new();
    let row_font = font(TextStyle::Body)?;
    let aside_font = font(TextStyle::Callout)?;

    for row in rows {
        let weight = weight_of(row)?;
        let says_run = Run { said: &row.says, weight, width: wide };
        let says = painting::measured(says_run, &row_font)?;

        let aside_run = Run { said: &row.aside, weight: Weight::Plain, width: wide };
        let aside = painting::measured(aside_run, &aside_font)?;

        let Ok((less, more)) = crate::page::ends_of(row);
        let less = sized_as(less, Weight::Bold, TextStyle::Body)?;
        let more = sized_as(more, Weight::Bold, TextStyle::Body)?;

        let mut cells = Vec::new();

        for cell in &row.cells {
            let Ok(said) = sized_as(&cell.says, weight, TextStyle::Body);

            cells.push(said);
        }

        let mut presses = Vec::new();
        let Ok(glyph_font) = glyphs();

        for press in row.buttons.iter().flat_map(|across| across.presses.iter()) {
            let said = match press.face {
                crate::page::Face::Glyph(icon) => {
                    let Ok(glyph) = icon.glyph();
                    let run = Run { said: glyph, weight: Weight::Plain, width: wide };

                    painting::measured(run, &glyph_font)?
                },
                crate::page::Face::Swatch(_) => NO_ROOM,
                crate::page::Face::Written(says) => sized_as(says, Weight::Plain, TextStyle::Title)?,
            };

            presses.push(said);
        }

        measured.push(MeasuredRow { says, aside, less, more, cells, presses });
    }

    Ok(measured)
}

fn measure_marks() -> Result<MeasuredMarks, Never> {
    let Ok(shut) = sized_as(marks::SHUT, Weight::Bold, TextStyle::Body);
    let Ok(before) = sized_as(marks::BEFORE, Weight::Bold, TextStyle::Body);
    let Ok(after) = sized_as(marks::AFTER, Weight::Bold, TextStyle::Body);
    let Ok(into) = sized_as(marks::INTO, Weight::Plain, TextStyle::Body);
    let Ok(offers) = sized_as(marks::ELSE, Weight::Bold, TextStyle::Body);

    Ok(MeasuredMarks { shut, before, after, into, offers })
}

fn in_the_line(state: &State) -> Result<(String, Filled), Never> {
    match &state.asking {
        Some(asking) => {
            let said = match asking.secret {
                Secret::Yes => marks::HIDDEN.repeat(asking.typed.chars().count()),
                Secret::No => asking.typed.clone(),
            };
            let filled = match said.is_empty() {
                true => Filled::Empty,
                false => Filled::Typed,
            };

            return Ok((said, filled));
        }
        None => {},
    }

    let Ok(page) = nth(&state.pages, state.here);
    let about = page.and_then(|page| page.sought.as_ref()).map(|sought| sought.about.clone());

    Ok(match (state.typed.is_empty(), about) {
        (false, _) => (state.typed.clone(), Filled::Typed),
        (true, Some(about)) => (about, Filled::Empty),
        (true, None) => (String::new(), Filled::Empty),
    })
}

fn measured_now(
    state: &State,
    tabs: &[MeasuredTab],
    rows: &[MeasuredRow],
    marks: MeasuredMarks,
) -> Result<Measured, Never> {
    let Ok((said, _filled)) = in_the_line(state);
    let typed = sized_as(&said, Weight::Plain, TextStyle::Body)?;
    let mut answers = Vec::new();

    match &state.sure {
        Some(sure) => {
            for says in &sure.answers {
                let Ok(said) = sized_as(says, Weight::Bold, TextStyle::Body);

                answers.push(said);
            }
        }
        None => {},
    }

    let note = match &state.note {
        Some(said) => sized_as(said, Weight::Plain, TextStyle::Callout)?,
        None => NO_ROOM,
    };

    Ok(Measured { tabs: tabs.to_vec(), rows: rows.to_vec(), marks, typed, answers, note })
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Panelled {
    shapes: Vec<Shape>,
    touching: Vec<HitRegion>,
    moving: Option<Moving>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Moving {
    of: std::path::PathBuf,
    at: Point<i32>,
    room: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lands {
    Row(u32),
    Answer(u32),
    Nudge { row: u32, step: i32 },
    Else(u32),
    ButtonPress { row: u32, which: u32 },
    Seek { row: u32, from: i32, wide: u32 },
    Tab(u32),
    More(i32),
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner {
    Card,
    Row(u32),
}

impl Lands {
    fn named(self) -> Result<(&'static str, Owner), Never> {
        Ok(match self {
            Lands::Row(at) => ("press", Owner::Row(at)),
            Lands::Answer(_) => ("answer", Owner::Card),
            Lands::Nudge { row, step: _ } => ("step", Owner::Row(row)),
            Lands::Else(row) => ("else", Owner::Row(row)),
            Lands::ButtonPress { row, which: _ } => ("button", Owner::Row(row)),
            Lands::Seek { row, from: _, wide: _ } => ("seek", Owner::Row(row)),
            Lands::Tab(_) => ("tab", Owner::Card),
            Lands::More(_) => ("more", Owner::Card),
            Lands::Back => ("shut", Owner::Card),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HitRegion {
    lands: Lands,
    panel: ShapePanel,
}

fn font(style: TextStyle) -> Result<Font, Never> {
    style.font()
}

fn glyphs() -> Result<Font, Never> {
    let Ok(tall) = fitted::<i32, u32>(GLYPH);

    Ok(Font { family: console_core_fonts::ICONS.to_string(), height: tall })
}

fn widest_tab(tabs: &[MeasuredTab]) -> Result<strip::Cell, Never> {
    let Ok(pad) = fitted::<i32, u32>(strip::PAD.saturating_mul(2));
    let widest = match tabs.iter().map(|tab| tab.chosen.width.saturating_add(pad)).max() {
        Some(widest) => widest,
        None => return Ok(strip::Cell(TAB_LEAST)),
    };
    let Ok(widest) = fitted::<u32, i32>(widest);

    Ok(strip::Cell(widest.max(TAB_LEAST)))
}

fn strip_room(card: &Card) -> Result<u32, Never> {
    let Ok(card_wide_u) = fitted::<i32, u32>(card.width);
    let Ok(edge) = fitted::<i32, i32>(strip::EDGE);
    let Ok(margin) = fitted::<i32, i32>(strip::MARGIN);
    let Ok(pad) = fitted::<i32, i32>(strip::PAD);

    let inner = card_wide_u
        .saturating_sub(edge.saturating_mul(2).unsigned_abs())
        .saturating_sub(margin.saturating_mul(2).unsigned_abs())
        .saturating_sub(pad.saturating_mul(2).unsigned_abs());

    let Ok(spent) = spent();

    Ok(inner.saturating_sub(spent.unsigned_abs()))
}

pub(crate) fn spent() -> Result<i32, Never> {
    Ok(FINGER.saturating_add(strip::GAP).saturating_mul(3))
}

fn inside_left(card: &Card) -> Result<i32, Never> {
    Ok(card.x.saturating_add(strip::EDGE).saturating_add(strip::MARGIN))
}

fn inside_right(card: &Card) -> Result<i32, Never> {
    Ok(card
        .x
        .saturating_add(card.width)
        .saturating_sub(strip::EDGE)
        .saturating_sub(strip::MARGIN))
}

fn header_top(card: &Card) -> Result<i32, Never> {
    Ok(card.y.saturating_add(fitting::STRIP.saturating_sub(FINGER).saturating_div(2)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Band {
    top: i32,
    height: i32,
}

fn middle(band: Band, said: Size<u32>) -> Result<i32, Never> {
    let Ok(said_tall) = fitted::<u32, i32>(said.height);

    Ok(band.top.saturating_add(band.height.saturating_sub(said_tall).saturating_div(2)))
}

fn across(wide: i32) -> Result<u32, Never> {
    fitted::<i32, u32>(wide.max(0))
}

fn centred(
    said: &str,
    measured: Size<u32>,
    within: &ShapePanel,
    weight: Weight,
    face: Font,
    ink: Oklch,
) -> Result<Text, Never> {
    let Ok(wide) = fitted::<u32, i32>(within.size.width);
    let Ok(tall) = fitted::<u32, i32>(within.size.height);
    let Ok(said_wide) = fitted::<u32, i32>(measured.width);
    let Ok(down) = middle(Band { top: within.at.y, height: tall }, measured);

    Ok(Text {
        at: Point {
            x: within.at.x.saturating_add(wide.saturating_sub(said_wide).max(0).saturating_div(2)),
            y: down,
        },
        width: measured.width.saturating_add(2).min(within.size.width),
        said: said.to_string(),
        weight,
        font: face,
        ink,
    })
}

struct Button<'a> {
    at: Point<i32>,
    size: Size<u32>,
    said: &'a str,
    measured: Size<u32>,
    lands: Lands,
    stood: Highlight,
}

fn button(button: Button<'_>, wearing: &Wearing) -> Result<(Vec<Shape>, HitRegion), Never> {
    let (fill, ink) = match button.stood {
        Highlight::Yes => (wearing.pink, wearing.night),
        Highlight::No => (wearing.night, wearing.text),
    };
    let panel = ShapePanel {
        at: button.at,
        size: button.size,
        round: Round(TAB_ROUND),
        fill,
        edge: Edge::None,
    };
    let Ok(face) = font(TextStyle::Body);
    let Ok(words) = centred(button.said, button.measured, &panel, Weight::Bold, face, ink);

    Ok((vec![Shape::Panel(panel), Shape::Text(words)], HitRegion { lands: button.lands, panel }))
}

fn title_shapes(
    state: &State,
    measured: &Measured,
    band: Band,
    left: i32,
    wide: u32,
    wearing: &Wearing,
) -> Result<Vec<Shape>, Never> {
    let (title, said) = match (state.pages.first(), measured.tabs.first()) {
        (Some(page), Some(tab)) => (page.title.clone(), tab.chosen),
        (Some(page), None) => (page.title.clone(), NO_ROOM),
        (None, _) => return Ok(Vec::new()),
    };
    let Ok(down) = middle(band, said);
    let Ok(face) = font(TextStyle::Headline);

    Ok(vec![Shape::Text(Text {
        at: Point { x: left.saturating_add(INSIDE_A_ROW), y: down },
        width: wide,
        said: title,
        weight: Weight::Bold,
        font: face,
        ink: wearing.text,
    })])
}

fn arrow_shapes(
    state: &State,
    measured: &Measured,
    band: Band,
    ends: Span,
    wearing: &Wearing,
) -> Result<(Vec<Shape>, Vec<HitRegion>), Never> {
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let Ok(finger) = fitted::<i32, u32>(FINGER);
    let Ok(many) = fitted::<_, u32>(state.pages.len());
    let here = state.here;
    let arrows = [
        (here > 0, ends.from, marks::BEFORE, measured.marks.before, Lands::More(-1)),
        (here.saturating_add(1) < many, ends.to, marks::AFTER, measured.marks.after, Lands::More(1)),
    ];

    for (shown, at_x, said, said_size, lands) in arrows {
        match shown {
            true => {
                let Ok((drawn, reached)) = button(
                    Button {
                        at: Point { x: at_x, y: band.top },
                        size: Size { width: finger, height: finger },
                        said,
                        measured: said_size,
                        lands,
                        stood: Highlight::No,
                    },
                    wearing,
                );

                shapes.extend(drawn);
                touching.push(reached);
            }
            false => {},
        }
    }

    Ok((shapes, touching))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    from: i32,
    to: i32,
}

fn header(
    state: &State,
    window: TabWindow,
    measured: &Measured,
    card: &Card,
    wearing: &Wearing,
) -> Result<(Vec<Shape>, Vec<HitRegion>), Never> {
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let Ok(top) = header_top(card);
    let Ok(left) = inside_left(card);
    let Ok(right) = inside_right(card);
    let Ok(finger) = fitted::<i32, u32>(FINGER);
    let square = Size { width: finger, height: finger };
    let shut_x = right.saturating_sub(FINGER);

    let (drawn, reached) = button(
        Button {
            at: Point { x: shut_x, y: top },
            size: square,
            said: marks::SHUT,
            measured: measured.marks.shut,
            lands: Lands::Back,
            stood: match state.leaving {
                Leaving::Standing => Highlight::Yes,
                Leaving::No => Highlight::No,
            },
        },
        wearing,
    )?;

    shapes.extend(drawn);
    touching.push(reached);

    match state.opened {
        Opened::Expanded => return Ok((shapes, touching)),
        Opened::No => {},
    }

    let Ok(many) = fitted::<_, u32>(state.pages.len());

    match many > 1 {
        true => {},
        false => {
            let Ok(wide) = across(shut_x.saturating_sub(strip::GAP).saturating_sub(left));
            let Ok(title) = title_shapes(state, measured, Band { top, height: FINGER }, left, wide, wearing);

            shapes.extend(title);

            return Ok((shapes, touching));
        }
    }

    let showing = window.fits.min(many.saturating_sub(window.from));
    let ends = Span { from: left, to: shut_x.saturating_sub(strip::GAP).saturating_sub(FINGER) };
    let Ok((arrows, reached)) =
        arrow_shapes(state, measured, Band { top, height: FINGER }, ends, wearing);

    shapes.extend(arrows);
    touching.extend(reached);

    let Ok(strip_wide) = strip_room(card);
    let run_left = left.saturating_add(FINGER).saturating_add(strip::GAP).saturating_add(strip::PAD);
    let tab_wide = strip_wide.saturating_div(showing.max(1));
    let Ok(tab_wide_i) = fitted::<u32, i32>(tab_wide);
    let Ok(from) = index(window.from);
    let Ok(showing_many) = index(showing);
    let tab_face = font(TextStyle::Headline)?;

    for (index, (which, page)) in state.pages.iter().enumerate().skip(from).take(showing_many).enumerate() {
        let Ok(step) = fitted::<_, i32>(index);
        let Ok(tab) = fitted::<_, u32>(which);
        let here = tab == state.here;
        let cell = ShapePanel {
            at: Point { x: run_left.saturating_add(step.saturating_mul(tab_wide_i)), y: top },
            size: Size { width: tab_wide, height: finger },
            round: Round(TAB_ROUND),
            fill: wearing.panel,
            edge: Edge::None,
        };
        let said = match (measured.tabs.get(which), here) {
            (Some(measured), true) => measured.chosen,
            (Some(measured), false) => measured.title,
            (None, _) => NO_ROOM,
        };

        match here {
            true => {
                let pill_wide = said
                    .width
                    .saturating_add(INSIDE_A_ROW.unsigned_abs().saturating_mul(2))
                    .min(tab_wide);
                let Ok(pill_wide_i) = fitted::<u32, i32>(pill_wide);

                shapes.push(Shape::Panel(ShapePanel {
                    at: Point {
                        x: cell
                            .at
                            .x
                            .saturating_add(tab_wide_i.saturating_sub(pill_wide_i).saturating_div(2)),
                        y: top,
                    },
                    size: Size { width: pill_wide, height: finger },
                    round: Round(TAB_ROUND),
                    fill: match state.leaving {
                        Leaving::Standing => wearing.ground,
                        Leaving::No => wearing.pink,
                    },
                    edge: Edge::None,
                }));
            }
            false => {},
        }

        let (weight, ink) = match (here, state.leaving) {
            (true, Leaving::No) => (Weight::Bold, wearing.night),
            (true, Leaving::Standing) => (Weight::Bold, wearing.text),
            (false, _) => (Weight::Plain, wearing.soft),
        };

        let Ok(words) = centred(&page.title, said, &cell, weight, tab_face.clone(), ink);

        shapes.push(Shape::Text(words));
        touching.push(HitRegion { lands: Lands::Tab(tab), panel: cell });
    }

    Ok((shapes, touching))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Highlight {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Standing {
    at: u32,
    highlight: Highlight,
    beside: Beside,
    press: u32,
    opened: Opened,
    zoom: crate::zoom::Zoom,
    selected: Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selected {
    Yes,
    No,
}

fn beside_the_words(picture: &Picture) -> Result<i32, Never> {
    let room = strip::PICTURE.saturating_add(strip::GAP.saturating_mul(2));

    Ok(match picture {
        Picture::Space | Picture::Named(_) | Picture::At(_) | Picture::Sleeve(Some(_)) => room,
        Picture::None
        | Picture::Sleeve(None)
        | Picture::Showing(_)
        | Picture::Playing(_)
        | Picture::Written(_)
        | Picture::Bar(_) => 0,
    })
}

fn wanted_of(picture: &Picture) -> Result<Option<&Path>, Never> {
    Ok(match picture {
        Picture::At(path) => Some(path.as_path()),
        Picture::Sleeve(Some(path)) => Some(path.as_path()),
        Picture::None
        | Picture::Space
        | Picture::Named(_)
        | Picture::Sleeve(None)
        | Picture::Showing(_)
        | Picture::Playing(_)
        | Picture::Written(_)
        | Picture::Bar(_) => None,
    })
}

fn asked_for(rows: &[Row]) -> Result<(), Never> {
    let mut wanted: Vec<String> = Vec::new();

    for row in rows {
        let Ok(of) = wanted_of(&row.picture);

        match of.and_then(Path::to_str) {
            Some(of) => wanted.push(of.to_string()),
            None => {},
        }
    }

    let Ok(missing) = crate::pictures::missing(&wanted);

    crate::pictures::make(&missing)
}

fn looked_at(picture: &Picture) -> Result<Option<&Path>, Never> {
    Ok(match picture {
        Picture::Showing(Some(path)) | Picture::Playing(Some(path)) => Some(path.as_path()),
        Picture::None
        | Picture::Space
        | Picture::Named(_)
        | Picture::At(_)
        | Picture::Sleeve(_)
        | Picture::Showing(None)
        | Picture::Playing(None)
        | Picture::Written(_)
        | Picture::Bar(_) => None,
    })
}

const SHARPEST: f64 = 4096.0;

fn sharper_room(room: Size<u32>, zoom: crate::zoom::Zoom) -> Result<Size<u32>, Never> {
    let Ok(wide) = console_core_number_conversion::toward_zero_u32((f64::from(room.width) * zoom.by).min(SHARPEST));
    let Ok(tall) = console_core_number_conversion::toward_zero_u32((f64::from(room.height) * zoom.by).min(SHARPEST));

    Ok(Size { width: wide, height: tall })
}

fn zoomed_in(moving: &Moving, pixels: Pixels, zoom: crate::zoom::Zoom) -> Result<console_core_shapes::Cropped, Never> {
    let Ok(placed) = zoom.placed(Size { width: pixels.width, height: pixels.height }, moving.room);
    let Ok(across) = toward_zero_i32(placed.at.x);
    let Ok(down) = toward_zero_i32(placed.at.y);
    let Ok(wide) = console_core_number_conversion::toward_zero_u32(placed.size.width);
    let Ok(tall) = console_core_number_conversion::toward_zero_u32(placed.size.height);

    Ok(console_core_shapes::Cropped {
        at: Point { x: moving.at.x.saturating_add(across), y: moving.at.y.saturating_add(down) },
        size: Size { width: wide, height: tall },
        pixels,
        from: placed.from,
        seen: placed.seen,
    })
}

fn placed(moving: &Moving, pixels: Pixels) -> Result<ShapePicture, Never> {
    let Ok(size) = console_pictures::fitted(Size { width: pixels.width, height: pixels.height }, moving.room);
    let Ok(spare_wide) = fitted::<u32, i32>(moving.room.width.saturating_sub(size.width));
    let Ok(spare_tall) = fitted::<u32, i32>(moving.room.height.saturating_sub(size.height));

    Ok(ShapePicture {
        at: Point {
            x: moving.at.x.saturating_add(spare_wide.saturating_div(2)),
            y: moving.at.y.saturating_add(spare_tall.saturating_div(2)),
        },
        size,
        pixels,
    })
}

fn stirred(state: &State) -> Result<WakeOutcome, Never> {
    let Ok(page) = nth(&state.pages, state.here);

    Ok(match page.and_then(|page| page.stirs.as_ref()) {
        Some(stirs) => stirs(),
        None => WakeOutcome::AlreadyAwake,
    })
}

fn waited(
    surface: &mut Surface,
    panelled: &Panelled,
    logical: Size<u32>,
    was: (Dirty, Stale),
) -> Result<(Dirty, Stale), Never> {
    let (mut dirty, mut stale) = was;
    let Ok(waking) = crate::frames::waking();
    let also: Vec<std::os::fd::BorrowedFd<'_>> = waking.into_iter().collect();
    let _ = surface.wait(&also, None);
    let Ok(woken) = crate::frames::woken();

    match woken.rows {
        crate::frames::FrameReceived::Yes => stale = Stale::Yes,
        crate::frames::FrameReceived::No => {},
    }

    match woken.card {
        crate::frames::FrameReceived::Yes => dirty = Dirty::Yes,
        crate::frames::FrameReceived::No => {},
    }

    match (woken.frame, dirty, &panelled.moving) {
        (crate::frames::FrameReceived::Yes, Dirty::No, Some(moving)) => {
            let Ok(repainted) = repainted(surface, moving, logical);

            match repainted {
                Visible::Yes => {},
                Visible::NotYet => dirty = Dirty::Yes,
            }
        },
        (crate::frames::FrameReceived::Yes, Dirty::No, None)
        | (crate::frames::FrameReceived::Yes, Dirty::Yes, _)
        | (crate::frames::FrameReceived::No, _, _) => {},
    }

    Ok((dirty, stale))
}

fn repainted(surface: &mut Surface, moving: &Moving, logical: Size<u32>) -> Result<Visible, Never> {
    let Ok(now) = crate::frames::current(&moving.of, moving.room);

    let pixels = match now {
        Some(pixels) => pixels,
        None => return Ok(Visible::Yes),
    };

    let Ok(picture) = placed(moving, pixels);
    let part = Part { at: picture.at, size: picture.size };
    let points = Size { width: logical.width, height: logical.height };
    let drawing = [Shape::Picture(picture)];

    Ok(match surface.draw_over(part, |pixels, device, _scale| {
        let _ = painting::over(pixels, Frame { device, points }, &drawing);

        Ok(())
    }) {
        Ok(drawn) => drawn,
        Err(_the_compositor_has_gone) => Visible::NotYet,
    })
}

pub(crate) fn stage(rows: &[Row], room: i32) -> Result<i32, Never> {
    let first = match rows.first() {
        Some(first) => first,
        None => return Ok(0),
    };

    let Ok(many) = fitted::<_, i32>(rows.len());
    let left_over = room.saturating_sub(many.saturating_mul(fitting::ROW)).max(0);

    match &first.headline {
        Some(_) => return Ok(left_over),
        None => {},
    }

    Ok(match &first.picture {
        Picture::Showing(_) | Picture::Playing(_) => left_over,
        Picture::None
        | Picture::Space
        | Picture::Named(_)
        | Picture::At(_)
        | Picture::Sleeve(_)
        | Picture::Written(_)
        | Picture::Bar(_) => 0,
    })
}

fn shown(picture: &Picture) -> Result<Option<Pixels>, Never> {
    let wanted = wanted_of(picture)?;

    match wanted {
        Some(wanted) => crate::pictures::pixels(wanted),
        None => Ok(None),
    }
}

const LEVEL_SAID: u32 = (EM * 28 / 9).unsigned_abs();
const CARET_TALL: i32 = EM * 4 / 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lit {
    Row,
    Beside,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Line {
    row: u32,
    top: i32,
    height: i32,
    far: i32,
    ink: Oklch,
}

fn row_shapes(
    row: &Row,
    band: Band,
    card: &Card,
    standing: Standing,
    measured: &Measured,
    wearing: &Wearing,
) -> Result<Panelled, Never> {
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let Ok(measured_row) = nth(&measured.rows, standing.at);
    let measured_row = match measured_row {
        Some(found) => found.clone(),
        None => MeasuredRow {
            says: NO_ROOM,
            aside: NO_ROOM,
            less: NO_ROOM,
            more: NO_ROOM,
            cells: Vec::new(),
            presses: Vec::new(),
        },
    };
    let Ok(looked) = looked_at(&row.picture);
    let bleeds = match (standing.opened, looked) {
        (Opened::Expanded, Some(_)) => Bleeds::Yes,
        (Opened::Expanded, None) | (Opened::No, _) => Bleeds::No,
    };
    let (left, right, top, tall) = match bleeds {
        Bleeds::Yes => (card.x, card.x.saturating_add(card.width), band.top, band.height),
        Bleeds::No => {
            let Ok(left) = inside_left(card);
            let Ok(right) = inside_right(card);

            (left, right, band.top.saturating_add(BETWEEN_ROWS.saturating_div(2)), band.height.saturating_sub(BETWEEN_ROWS))
        },
    };
    let Ok(box_tall) = fitted::<i32, u32>(tall);
    let Ok(box_wide) = across(right.saturating_sub(left));
    let Ok(heading) = row.heading();
    let lit = match (standing.highlight, standing.beside) {
        (Highlight::Yes, Beside::No) => Lit::Row,
        (Highlight::Yes, Beside::Yes) => Lit::Beside,
        (Highlight::No, _) => Lit::No,
    };

    let ground = match (lit, heading) {
        (Lit::Row, _) => Some((wearing.pink, Edge::None)),
        (Lit::Beside, _) => Some((wearing.ground, Edge::Of { wide: CARD_EDGE, color: wearing.pink })),
        (Lit::No, Heading::No) => Some((wearing.ground, Edge::None)),
        (Lit::No, Heading::Yes) => None,
    };

    let ground = match (bleeds, &row.buttons) {
        (Bleeds::No, None) => ground,
        (Bleeds::Yes, _) | (Bleeds::No, Some(_)) => None,
    };

    match ground {
        Some((fill, edge)) => shapes.push(Shape::Panel(ShapePanel {
            at: Point { x: left, y: top },
            size: Size { width: box_wide, height: box_tall },
            round: Round(ROW_ROUND),
            fill,
            edge,
        })),
        None => {},
    }

    let (ink, quiet) = match (lit, heading) {
        (Lit::Row, _) => (wearing.night, wearing.night),
        (Lit::Beside | Lit::No, Heading::Yes) => (wearing.soft, wearing.soft),
        (Lit::Beside | Lit::No, Heading::No) => (wearing.text, wearing.soft),
    };

    let Ok(taken) = beside_the_words(&row.picture);
    let words_x = left.saturating_add(INSIDE_A_ROW).saturating_add(taken);
    let Ok(shown) = shown(&row.picture);

    match shown {
        Some(pixels) => {
            let Ok(square) = fitted::<i32, u32>(strip::PICTURE);

            shapes.push(Shape::Picture(ShapePicture {
                at: Point {
                    x: left.saturating_add(INSIDE_A_ROW).saturating_sub(strip::GAP),
                    y: top.saturating_add(tall.saturating_sub(strip::PICTURE).saturating_div(2)),
                },
                size: Size { width: square, height: square },
                pixels,
            }));
        }
        None => {
            let Ok(named) = named_shapes(&row.picture, Band { top, height: tall }, left, lit, wearing);

            shapes.extend(named);
        }
    }

    let moving = match looked {
        Some(path) => {
            let edges = match bleeds {
                Bleeds::Yes => 0,
                Bleeds::No => CARD_EDGE.saturating_mul(4),
            };
            let Ok(inset) = fitted::<u32, i32>(edges);
            let moving = Moving {
                of: path.to_path_buf(),
                at: Point { x: left.saturating_add(inset), y: top.saturating_add(inset) },
                room: Size {
                    width: box_wide.saturating_sub(edges.saturating_mul(2)),
                    height: box_tall.saturating_sub(edges.saturating_mul(2)),
                },
            };
            let Ok(now) = crate::frames::current(path, moving.room);

            match (now, standing.zoom.zoomed()) {
                (Some(pixels), Ok(crate::zoom::Zoomed::Whole)) => {
                    let Ok(picture) = placed(&moving, pixels);

                    shapes.push(Shape::Picture(picture));
                },
                (Some(pixels), Ok(crate::zoom::Zoomed::ZoomedIn)) => {
                    let Ok(sharper) = sharper_room(moving.room, standing.zoom);
                    let Ok(()) = crate::frames::sharpened(path, sharper);
                    let Ok(picture) = zoomed_in(&moving, pixels, standing.zoom);

                    shapes.push(Shape::Cropped(picture));
                },
                (None, _) => {},
            }

            Some(moving)
        },
        None => None,
    };

    let mut far = right.saturating_sub(INSIDE_A_ROW);
    let Ok(wears) = crate::page::wears(row);

    match wears {
        Wears::WhatElse => {
            let (drawn, reached) = button(
                Button {
                    at: Point { x: right.saturating_sub(FINGER), y: top },
                    size: Size { width: FINGER.unsigned_abs(), height: box_tall },
                    said: marks::ELSE,
                    measured: measured.marks.offers,
                    lands: Lands::Else(standing.at),
                    stood: match lit {
                        Lit::Beside => Highlight::Yes,
                        Lit::Row | Lit::No => Highlight::No,
                    },
                },
                wearing,
            )?;

            shapes.extend(drawn);
            touching.push(reached);

            far = right.saturating_sub(FINGER).saturating_sub(strip::GAP.saturating_mul(3));
        }
        Wears::None => {}
    }

    match row.opens {
        true => {
            let Ok(into_wide) = fitted::<u32, i32>(measured.marks.into.width);
            let Ok(down) = middle(Band { top, height: tall }, measured.marks.into);

            let Ok(face_body) = font(TextStyle::Body);

            shapes.push(Shape::Text(Text {
                at: Point { x: far.saturating_sub(into_wide), y: down },
                width: measured.marks.into.width.saturating_add(2),
                said: marks::INTO.to_string(),
                weight: Weight::Plain,
                font: face_body,
                ink: quiet,
            }));

            far = far.saturating_sub(into_wide).saturating_sub(strip::GAP.saturating_mul(3));
        }
        false => {}
    }

    let Ok(far) = selected_mark(&mut shapes, standing.selected, Band { top, height: tall }, (far, lit), wearing);

    match &row.buttons {
        Some(across) => {
            let strip = ButtonStrip { band: Band { top, height: tall }, span: Span { from: left, to: far.saturating_add(INSIDE_A_ROW) }, standing, lit };
            let Ok((drawn, reached)) = press_shapes(across, &measured_row, strip, wearing);

            shapes.extend(drawn);
            touching.extend(reached);

            return Ok(Panelled { shapes, touching, moving });
        }
        None => {}
    }

    let line = Line { row: standing.at, top, height: tall, far, ink };
    let Ok(holds) = crate::page::holds(row);

    let (said, pressed, far) = match holds {
        Holds::Level => level_shapes(row, &measured_row, line, wearing)?,
        Holds::None => aside_shapes(row, &measured_row, Line { ink: quiet, ..line }, box_wide)?,
    };

    shapes.extend(said);
    touching.extend(pressed);

    let fill = match lit {
        Lit::Row => wearing.night,
        Lit::Beside | Lit::No => wearing.pink,
    };
    let Ok(()) = bar_after_the_words(row, (&mut shapes, &mut touching), (Line { row: standing.at, top, height: tall, far, ink }, words_x, fill), &measured_row, wearing);

    let cells_ink = match (lit, row.naming) {
        (Lit::Row, _) => wearing.night,
        (Lit::Beside | Lit::No, true) => wearing.soft,
        (Lit::Beside | Lit::No, false) => wearing.text,
    };
    let Ok(weight) = weight_of(row);
    let across_the_row = Span { from: words_x, to: far };

    let Ok(cells) = cell_shapes(row, &measured_row, Band { top, height: tall }, across_the_row, Inked { ink: cells_ink, weight, wearing });

    shapes.extend(cells);

    let Ok(down) = middle(Band { top, height: tall }, measured_row.says);

    let Ok(wide) = across(far.saturating_sub(words_x));
    let Ok(face) = font(TextStyle::Body);

    shapes.push(Shape::Text(Text {
        at: Point { x: words_x, y: down },
        width: wide,
        said: row.says.clone(),
        weight,
        font: face,
        ink,
    }));

    Ok(Panelled { shapes, touching, moving })
}

fn bar_after_the_words(
    row: &Row,
    (shapes, touching): (&mut Vec<Shape>, &mut Vec<HitRegion>),
    (line, words_x, fill): (Line, i32, Oklch),
    measured_row: &MeasuredRow,
    wearing: &Wearing,
) -> Result<(), Never> {
    let Ok(says_wide) = fitted::<u32, i32>(measured_row.says.width);
    let mut track = Span { from: words_x.saturating_add(says_wide).saturating_add(strip::GAP.saturating_mul(3)), to: line.far };
    let Ok(bars) = bars(row);
    let Ok(holds) = crate::page::holds(row);
    let Ok((less, _more)) = crate::page::ends_of(row);

    match (bars, holds, less.is_empty()) {
        (Bars::Yes, Holds::Level, false) => {
            let less = Nudge { said: less, measured: measured_row.less, step: -1 };
            let Ok(()) = nudge_shapes((&mut *shapes, &mut *touching), less, (line, track.from), wearing);

            track.from = track.from.saturating_add(FINGER).saturating_add(strip::GAP.saturating_mul(2));
        },
        (Bars::Yes, Holds::Level, true) | (Bars::Yes, Holds::None, _) | (Bars::No, _, _) => {},
    }

    let Ok((drawn, reached)) = bar_shapes(row, (line, fill), track, wearing);

    shapes.extend(drawn);
    touching.extend(reached);

    Ok(())
}

fn selected_mark(shapes: &mut Vec<Shape>, selected: Selected, band: Band, (far, lit): (i32, Lit), wearing: &Wearing) -> Result<i32, Never> {
    match selected {
        Selected::No => return Ok(far),
        Selected::Yes => {},
    }

    let Ok(side) = fitted::<i32, u32>(SELECTED_MARK);
    let mark = Shape::Panel(ShapePanel {
        at: Point {
            x: far.saturating_sub(SELECTED_MARK),
            y: band.top.saturating_add(band.height.saturating_sub(SELECTED_MARK).saturating_div(2)),
        },
        size: Size { width: side, height: side },
        round: Round(side.saturating_div(2)),
        fill: match lit {
            Lit::Row => wearing.night,
            Lit::Beside | Lit::No => wearing.pink,
        },
        edge: Edge::None,
    });

    shapes.push(mark);

    Ok(far.saturating_sub(SELECTED_MARK).saturating_sub(strip::GAP.saturating_mul(3)))
}

fn named_shapes(picture: &Picture, band: Band, left: i32, lit: Lit, wearing: &Wearing) -> Result<Option<Shape>, Never> {
    let icon = match picture {
        Picture::Named(icon) => *icon,
        Picture::None
        | Picture::Space
        | Picture::At(_)
        | Picture::Sleeve(_)
        | Picture::Showing(_)
        | Picture::Playing(_)
        | Picture::Written(_)
        | Picture::Bar(_) => return Ok(None),
    };
    let Ok(square) = fitted::<i32, u32>(strip::PICTURE);
    let within = ShapePanel {
        at: Point {
            x: left.saturating_add(INSIDE_A_ROW).saturating_sub(strip::GAP),
            y: band.top.saturating_add(band.height.saturating_sub(strip::PICTURE).saturating_div(2)),
        },
        size: Size { width: square, height: square },
        round: Round(0),
        fill: wearing.ground,
        edge: Edge::None,
    };
    let Ok(glyph) = icon.glyph();
    let Ok(face) = glyphs();
    let Ok(measured) = measured_in(glyph, Weight::Plain, &face);
    let ink = match lit {
        Lit::Row => wearing.night,
        Lit::Beside | Lit::No => wearing.pink,
    };
    let Ok(drawn) = centred(glyph, measured, &within, Weight::Plain, face, ink);

    Ok(Some(Shape::Text(drawn)))
}

const BIG: u32 = (EM * 4).unsigned_abs();
const BIG_GLYPH: u32 = (EM * 2).unsigned_abs();

fn measured_in(said: &str, weight: Weight, face: &Font) -> Result<Size<u32>, Never> {
    painting::measured(Run { said, weight, width: ANYTHING_WIDE }, face)
}

fn written(said: &str, at: Point<i32>, weight: Weight, face: Font, ink: Oklch) -> Result<(Shape, Size<u32>), Never> {
    let Ok(measured) = measured_in(said, weight, &face);

    Ok((
        Shape::Text(Text { at, width: measured.width.saturating_add(2), said: said.to_string(), weight, font: face, ink }),
        measured,
    ))
}

fn headline_shapes(
    headline: &crate::page::Headline,
    row: &Row,
    band: Band,
    card: &Card,
    wearing: &Wearing,
) -> Result<Panelled, Never> {
    let Ok(looked) = looked_at(&row.picture);
    let Ok(left) = inside_left(card);
    let Ok(right) = inside_right(card);
    let Ok(wide) = across(right.saturating_sub(left));
    let Ok(tall) = across(band.height.saturating_sub(BETWEEN_ROWS));
    let within = ShapePanel {
        at: Point { x: left, y: band.top.saturating_add(BETWEEN_ROWS.saturating_div(2)) },
        size: Size { width: wide, height: tall },
        round: Round(ROW_ROUND),
        fill: wearing.ground,
        edge: Edge::None,
    };
    let mut shapes = vec![Shape::Panel(within)];
    let Ok(box_wide) = fitted::<u32, i32>(within.size.width);
    let Ok(box_tall) = fitted::<u32, i32>(within.size.height);
    let left = within.at.x.saturating_add(INSIDE_A_ROW);
    let top = within.at.y.saturating_add(INSIDE_A_ROW);
    let bottom = within.at.y.saturating_add(box_tall).saturating_sub(INSIDE_A_ROW);

    let Ok(title_face) = font(TextStyle::Title);
    let Ok((title, title_said)) = written(&headline.title, Point { x: left, y: top }, Weight::Bold, title_face, wearing.text);
    let Ok(title_tall) = fitted::<u32, i32>(title_said.height);
    let Ok(subtitle_face) = font(TextStyle::Footnote);
    let Ok((subtitle, _)) = written(&headline.subtitle, Point { x: left, y: top.saturating_add(title_tall) }, Weight::Plain, subtitle_face, wearing.soft);

    shapes.push(title);
    shapes.push(subtitle);

    let big_face = Font { family: console_core_fonts::LETTERS.to_string(), height: BIG };
    let Ok(big_said) = measured_in(&headline.big, Weight::Plain, &big_face);
    let Ok(big_tall) = fitted::<u32, i32>(big_said.height);
    let Ok(big_wide) = fitted::<u32, i32>(big_said.width);
    let big_top = bottom.saturating_sub(big_tall);
    let Ok((big, _)) = written(&headline.big, Point { x: left, y: big_top }, Weight::Plain, big_face, wearing.text);

    shapes.push(big);

    let beside = left.saturating_add(big_wide).saturating_add(INSIDE_A_ROW.saturating_mul(2));
    let glyph_face = Font { family: console_core_fonts::ICONS.to_string(), height: BIG_GLYPH };
    let glyph = match headline.icon {
        Some(icon) => icon.glyph()?,
        None => "",
    };
    let Ok(glyph_said) = measured_in(glyph, Weight::Plain, &glyph_face);
    let Ok(says_face) = font(TextStyle::Headline);
    let Ok(says_said) = measured_in(&headline.says, Weight::Bold, &says_face);
    let Ok(aside_face) = font(TextStyle::Callout);
    let Ok(aside_said) = measured_in(&headline.aside, Weight::Plain, &aside_face);
    let Ok(line_tall) = fitted::<u32, i32>(glyph_said.height.max(says_said.height));
    let Ok(aside_tall) = fitted::<u32, i32>(aside_said.height);
    let block = line_tall.saturating_add(aside_tall);
    let block_top = big_top.saturating_add(big_tall.saturating_sub(block).saturating_div(2));
    let Ok(glyph_wide) = fitted::<u32, i32>(BIG_GLYPH);
    let words = beside.saturating_add(glyph_wide).saturating_add(strip::GAP.saturating_mul(3));
    let line = Band { top: block_top, height: line_tall };
    let Ok(glyph_down) = middle(line, glyph_said);
    let Ok(says_down) = middle(line, says_said);
    let Ok((glyph, _)) = written(glyph, Point { x: beside, y: glyph_down }, Weight::Plain, glyph_face, wearing.pink);
    let Ok((says, _)) = written(&headline.says, Point { x: words, y: says_down }, Weight::Bold, says_face, wearing.text);
    let Ok((aside, _)) = written(&headline.aside, Point { x: words, y: block_top.saturating_add(line_tall) }, Weight::Plain, aside_face, wearing.soft);

    shapes.push(glyph);
    shapes.push(says);
    shapes.push(aside);

    let moving = match looked {
        Some(path) => {
            let Ok(inset) = fitted::<u32, i32>(CARD_EDGE.saturating_mul(4));
            let wide = box_wide.saturating_mul(9).saturating_div(20);
            let Ok(room_wide) = across(wide.saturating_sub(inset));
            let Ok(room_tall) = across(box_tall.saturating_sub(inset.saturating_mul(2)));
            let moving = Moving {
                of: path.to_path_buf(),
                at: Point {
                    x: within.at.x.saturating_add(box_wide).saturating_sub(wide),
                    y: within.at.y.saturating_add(inset),
                },
                room: Size { width: room_wide, height: room_tall },
            };
            let Ok(now) = crate::frames::current(path, moving.room);

            match now {
                Some(pixels) => {
                    let Ok(picture) = placed(&moving, pixels);

                    shapes.push(Shape::Picture(picture));
                },
                None => {},
            }

            Some(moving)
        },
        None => None,
    };

    Ok(Panelled { shapes, touching: Vec::new(), moving })
}

fn trailing_headline_shapes(headline: &crate::page::Headline, band: Band, card: &Card, wearing: &Wearing) -> Result<Panelled, Never> {
    let Ok(left) = inside_left(card);
    let Ok(right) = inside_right(card);
    let Ok(wide) = across(right.saturating_sub(left));
    let thin = CARD_EDGE.max(1);
    let rule_at = band.top.saturating_add(band.height).saturating_sub(INSIDE_A_ROW);
    let rule = ShapePanel {
        at: Point { x: left, y: rule_at },
        size: Size { width: wide, height: thin },
        round: Round(0),
        fill: wearing.edge,
        edge: Edge::None,
    };
    let end = right.saturating_sub(strip::GAP);

    let Ok(title_face) = font(TextStyle::Title);
    let Ok(title_said) = measured_in(&headline.title, Weight::Plain, &title_face);
    let Ok(title_tall) = fitted::<u32, i32>(title_said.height);
    let room = rule_at.saturating_sub(band.top).saturating_sub(INSIDE_A_ROW);
    let Ok(room) = across(room);
    let big_face = Font { family: console_core_fonts::LETTERS.to_string(), height: BIG.saturating_mul(3).min(room).max(1) };
    let Ok(big_said) = measured_in(&headline.big, Weight::Plain, &big_face);
    let Ok(big_tall) = fitted::<u32, i32>(big_said.height);
    let Ok(big_wide) = fitted::<u32, i32>(big_said.width);
    let big_top = rule_at.saturating_sub(INSIDE_A_ROW).saturating_sub(big_tall);
    let Ok((big, _)) = written(&headline.big, Point { x: end.saturating_sub(big_wide), y: big_top }, Weight::Plain, big_face, wearing.text);
    let title_top = big_top.saturating_add(big_tall).saturating_sub(title_tall);
    let Ok((title, _)) = written(&headline.title, Point { x: left.saturating_add(strip::GAP), y: title_top }, Weight::Plain, title_face, wearing.soft);

    Ok(Panelled { shapes: vec![title, big, Shape::Panel(rule)], touching: Vec::new(), moving: None })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bleeds {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy)]
struct ButtonStrip {
    band: Band,
    span: Span,
    standing: Standing,
    lit: Lit,
}

fn press_shapes(
    strip_of: &crate::page::Across,
    measured_row: &MeasuredRow,
    strip: ButtonStrip,
    wearing: &Wearing,
) -> Result<(Vec<Shape>, Vec<HitRegion>), Never> {
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let Ok(many) = fitted::<_, i32>(strip_of.presses.iter().filter(|press| press.placed == crate::page::Placed::Among).count());
    let gap = strip::GAP.saturating_mul(2);
    let room = strip.span.to.saturating_sub(strip.span.from);
    let each = match strip_of.presses.first().map(|press| press.face) {
        Some(crate::page::Face::Swatch(_)) => FINGER,
        Some(crate::page::Face::Glyph(_)) | None => PRESS_WIDE,
        Some(crate::page::Face::Written(_)) => match room.saturating_sub(many.saturating_sub(1).max(0).saturating_mul(gap)).checked_div(many) {
            Some(shared) => shared.max(PRESS_WIDE),
            None => PRESS_WIDE,
        },
    };
    let whole = many.saturating_mul(each).saturating_add(many.saturating_sub(1).max(0).saturating_mul(gap));
    let first = strip.span.from.saturating_add(room.saturating_sub(whole).max(0).saturating_div(2));
    let Ok(wide) = across(each);
    let Ok(tall) = across(strip.band.height.saturating_sub(gap));
    let down = strip.band.top.saturating_add(strip::GAP);
    let Ok(face) = glyphs();

    let corner = strip.span.to.saturating_sub(each);
    let mut step: i32 = 0;

    for (at, press) in strip_of.presses.iter().enumerate() {
        let Ok(which) = fitted::<_, u32>(at);
        let across = match press.placed {
            crate::page::Placed::Among => {
                let across = first.saturating_add(step.saturating_mul(each.saturating_add(gap)));

                step = step.saturating_add(1);

                across
            }
            crate::page::Placed::Corner => corner,
        };
        let panel = ShapePanel {
            at: Point { x: across, y: down },
            size: Size { width: wide, height: tall },
            round: Round(TAB_ROUND),
            fill: wearing.night,
            edge: Edge::None,
        };
        let stood = match (strip.lit, which == strip.standing.press) {
            (Lit::Row, true) => Highlight::Yes,
            (Lit::Row, false) | (Lit::Beside | Lit::No, _) => Highlight::No,
        };
        let icon = match press.face {
            crate::page::Face::Glyph(icon) => Some(icon),
            crate::page::Face::Written(_) => None,
            crate::page::Face::Swatch(color) => {
                let edge = match (stood, press.now) {
                    (Highlight::Yes, _) => Edge::Of { wide: SWATCH_EDGE.saturating_mul(2), color: wearing.pink },
                    (Highlight::No, crate::page::Active::Yes) => Edge::Of { wide: SWATCH_EDGE, color: wearing.text },
                    (Highlight::No, crate::page::Active::No) => Edge::None,
                };

                shapes.push(Shape::Panel(ShapePanel { fill: color, edge, ..panel }));
                touching.push(HitRegion { lands: Lands::ButtonPress { row: strip.standing.at, which }, panel });

                continue;
            },
        };
        let (fill, ink) = match (stood, press.now, press.style) {
            (Highlight::Yes, _, _) => (wearing.pink, wearing.night),
            (Highlight::No, crate::page::Active::Yes, crate::page::ButtonStyle::Gray) => (wearing.night, wearing.pink),
            (Highlight::No, crate::page::Active::Yes, crate::page::ButtonStyle::Tinted) => (wearing.ground, wearing.pink),
            (Highlight::No, crate::page::Active::No, crate::page::ButtonStyle::Gray) => (wearing.night, wearing.text),
            (Highlight::No, crate::page::Active::No, crate::page::ButtonStyle::Tinted) => (wearing.ground, wearing.text),
            (Highlight::No, _, crate::page::ButtonStyle::Filled) => (wearing.text, wearing.night),
        };
        let panel = ShapePanel { fill, ..panel };
        let said = match measured_row.presses.get(at) {
            Some(said) => *said,
            None => NO_ROOM,
        };
        let shown = match (icon, strip.standing.opened) {
            (Some(crate::icons::Icon::FullScreen), Opened::Expanded) => Some(crate::icons::Icon::LeaveFullScreen),
            (icon, Opened::Expanded | Opened::No) => icon,
        };
        let Ok(glyph) = match shown {
            Some(icon) => icon.glyph(),
            None => Ok(""),
        };
        let title = font(TextStyle::Title)?;
        let (glyph, set_in) = match press.face {
            crate::page::Face::Written(says) => (says, title),
            crate::page::Face::Glyph(_) | crate::page::Face::Swatch(_) => (glyph, face.clone()),
        };
        let Ok(words) = centred(glyph, said, &panel, Weight::Plain, set_in, ink);

        shapes.push(Shape::Panel(panel));
        shapes.push(Shape::Text(words));
        touching.push(HitRegion { lands: Lands::ButtonPress { row: strip.standing.at, which }, panel });
    }

    Ok((shapes, touching))
}

fn bar_shapes(row: &Row, (line, fill): (Line, Oklch), track: Span, wearing: &Wearing) -> Result<(Vec<Shape>, Vec<HitRegion>), Never> {
    let bar = match &row.picture {
        Picture::Bar(bar) => *bar,
        Picture::None
        | Picture::Space
        | Picture::Named(_)
        | Picture::At(_)
        | Picture::Sleeve(_)
        | Picture::Showing(_)
        | Picture::Playing(_)
        | Picture::Written(_) => return Ok((Vec::new(), Vec::new())),
    };

    let Ok(wide) = across(track.to.saturating_sub(track.from));

    match wide > 0 {
        true => {},
        false => return Ok((Vec::new(), Vec::new())),
    }

    let Ok(thin) = across(TRACK);
    let down = line.top.saturating_add(line.height.saturating_sub(TRACK).saturating_div(2));
    let Ok(filled) = bar.filled(wide);
    let under = ShapePanel {
        at: Point { x: track.from, y: down },
        size: Size { width: wide, height: thin },
        round: Round(thin.saturating_div(2)),
        fill: wearing.soft,
        edge: Edge::None,
    };
    let over = ShapePanel { size: Size { width: filled, height: thin }, fill, ..under };
    let mut shapes = vec![Shape::Panel(under)];

    match filled > 0 {
        true => shapes.push(Shape::Panel(over)),
        false => {},
    }

    let touching = match row.seek.is_some() {
        true => {
            let Ok(tall) = across(line.height);

            vec![HitRegion {
                lands: Lands::Seek { row: line.row, from: track.from, wide },
                panel: ShapePanel { at: Point { x: track.from, y: line.top }, size: Size { width: wide, height: tall }, ..under },
            }]
        },
        false => Vec::new(),
    };

    Ok((shapes, touching))
}

struct Inked<'a> {
    ink: Oklch,
    weight: Weight,
    wearing: &'a Wearing,
}

fn cell_shapes(row: &Row, measured_row: &MeasuredRow, band: Band, span: Span, inked: Inked<'_>) -> Result<Vec<Shape>, Never> {
    let mut shapes = Vec::new();
    let Ok(many) = fitted::<_, i32>(row.cells.len());
    let each = match many {
        0 => return Ok(shapes),
        many => span.to.saturating_sub(span.from).saturating_div(many),
    };
    let Ok(wide) = across(each);
    let Ok(tall) = across(band.height);
    let Ok(face) = font(TextStyle::Body);
    let lit_wide = wide.min(tall);
    let Ok(lit_wide_i) = fitted::<u32, i32>(lit_wide);

    for (at, cell) in row.cells.iter().enumerate() {
        let Ok(step) = fitted::<_, i32>(at);
        let from = span.from.saturating_add(step.saturating_mul(each));
        let said = match measured_row.cells.get(at) {
            Some(said) => *said,
            None => NO_ROOM,
        };

        match cell.icon {
            Some(icon) => {
                let Ok(glyph) = icon.glyph();
                let Ok(glyph_face) = glyphs();
                let Ok(glyph_said) = measured_in(glyph, Weight::Plain, &glyph_face);
                let Ok(glyph_wide) = fitted::<u32, i32>(glyph_face.height);
                let Ok(said_wide) = fitted::<u32, i32>(said.width);
                let gap = strip::GAP.saturating_mul(2);
                let whole = glyph_wide.saturating_add(gap).saturating_add(said_wide);
                let start = from.saturating_add(each.saturating_sub(whole).max(0).saturating_div(2));
                let Ok(glyph_down) = middle(band, glyph_said);
                let Ok(said_down) = middle(band, said);
                let Ok((drawn_glyph, _)) = written(glyph, Point { x: start, y: glyph_down }, Weight::Plain, glyph_face, inked.wearing.pink);
                let Ok((drawn_said, _)) = written(&cell.says, Point { x: start.saturating_add(glyph_wide).saturating_add(gap), y: said_down }, inked.weight, face.clone(), inked.ink);

                shapes.push(drawn_glyph);
                shapes.push(drawn_said);

                continue;
            },
            None => {},
        }

        let within = ShapePanel {
            at: Point { x: from.saturating_add(each.saturating_sub(lit_wide_i).saturating_div(2)), y: band.top },
            size: Size { width: lit_wide, height: tall },
            round: Round(lit_wide.saturating_div(2)),
            fill: inked.wearing.pink,
            edge: Edge::None,
        };
        let ink = match cell.now {
            true => {
                shapes.push(Shape::Panel(within));

                inked.wearing.night
            }
            false => inked.ink,
        };
        let Ok(words) = centred(&cell.says, said, &within, inked.weight, face.clone(), ink);

        shapes.push(Shape::Text(words));
    }

    Ok(shapes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Bars {
    Yes,
    No,
}

fn bars(row: &Row) -> Result<Bars, Never> {
    Ok(match &row.picture {
        Picture::Bar(_) => Bars::Yes,
        Picture::None
        | Picture::Space
        | Picture::Named(_)
        | Picture::At(_)
        | Picture::Sleeve(_)
        | Picture::Showing(_)
        | Picture::Playing(_)
        | Picture::Written(_) => Bars::No,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Nudge<'a> {
    said: &'a str,
    measured: Size<u32>,
    step: i32,
}

fn nudge_shapes(
    (shapes, touching): (&mut Vec<Shape>, &mut Vec<HitRegion>),
    nudge: Nudge<'_>,
    (line, x): (Line, i32),
    wearing: &Wearing,
) -> Result<(), Never> {
    let Ok(tall) = fitted::<i32, u32>(line.height);
    let (drawn, reached) = button(
        Button {
            at: Point { x, y: line.top },
            size: Size { width: FINGER.unsigned_abs(), height: tall },
            said: nudge.said,
            measured: nudge.measured,
            lands: Lands::Nudge { row: line.row, step: nudge.step },
            stood: Highlight::No,
        },
        wearing,
    )?;

    shapes.extend(drawn);
    touching.push(reached);

    Ok(())
}

fn level_said_shapes(shapes: &mut Vec<Shape>, row: &Row, measured_row: &MeasuredRow, line: Line) -> Result<i32, Never> {
    match row.aside.is_empty() {
        true => return Ok(line.far),
        false => {},
    }

    let Ok(aside_wide) = fitted::<u32, i32>(measured_row.aside.width);
    let Ok(kept) = fitted::<u32, i32>(measured_row.aside.width.max(LEVEL_SAID));
    let Ok(down) = middle(Band { top: line.top, height: line.height }, measured_row.aside);

    let Ok(face_callout) = font(TextStyle::Callout);

    shapes.push(Shape::Text(Text {
        at: Point { x: line.far.saturating_sub(aside_wide), y: down },
        width: measured_row.aside.width.saturating_add(2),
        said: row.aside.clone(),
        weight: Weight::Plain,
        font: face_callout,
        ink: line.ink,
    }));

    Ok(line.far.saturating_sub(kept).saturating_sub(strip::GAP.saturating_mul(2)))
}

fn level_shapes(
    row: &Row,
    measured_row: &MeasuredRow,
    line: Line,
    wearing: &Wearing,
) -> Result<(Vec<Shape>, Vec<HitRegion>, i32), Never> {
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let Ok((less, more)) = crate::page::ends_of(row);
    let Ok(bars) = bars(row);
    let mut far = line.far;

    match bars {
        Bars::Yes => {
            let Ok(before_the_length) = level_said_shapes(&mut shapes, row, measured_row, line);

            far = before_the_length;
        },
        Bars::No => {},
    }

    match more.is_empty() {
        false => {
            let more = Nudge { said: more, measured: measured_row.more, step: 1 };
            let Ok(()) = nudge_shapes((&mut shapes, &mut touching), more, (line, far.saturating_sub(FINGER)), wearing);

            far = far.saturating_sub(FINGER).saturating_sub(strip::GAP.saturating_mul(2));
        }
        true => {}
    }

    match bars {
        Bars::Yes => return Ok((shapes, touching, far)),
        Bars::No => {},
    }

    let Ok(before_the_level) = level_said_shapes(&mut shapes, row, measured_row, Line { far, ..line });

    far = before_the_level;

    match less.is_empty() {
        false => {
            let less = Nudge { said: less, measured: measured_row.less, step: -1 };
            let Ok(()) = nudge_shapes((&mut shapes, &mut touching), less, (line, far.saturating_sub(FINGER)), wearing);

            far = far.saturating_sub(FINGER).saturating_sub(strip::GAP.saturating_mul(3));
        }
        true => {}
    }

    Ok((shapes, touching, far))
}

fn aside_shapes(
    row: &Row,
    measured_row: &MeasuredRow,
    line: Line,
    box_wide: u32,
) -> Result<(Vec<Shape>, Vec<HitRegion>, i32), Never> {
    match row.aside.is_empty() {
        true => return Ok((Vec::new(), Vec::new(), line.far)),
        false => {},
    }

    let wide = measured_row.aside.width.min(box_wide.saturating_div(2));
    let Ok(wide_i) = fitted::<u32, i32>(wide);
    let Ok(down) = middle(Band { top: line.top, height: line.height }, measured_row.aside);
    let Ok(face_callout) = font(TextStyle::Callout);

    let said = Shape::Text(Text {
        at: Point { x: line.far.saturating_sub(wide_i), y: down },
        width: wide.saturating_add(2),
        said: row.aside.clone(),
        weight: Weight::Plain,
        font: face_callout,
        ink: line.ink,
    });

    Ok((vec![said], Vec::new(), line.far.saturating_sub(wide_i).saturating_sub(strip::GAP.saturating_mul(4))))
}

fn note_shapes(said: &str, measured: Size<u32>, card: &Card, wearing: &Wearing) -> Result<Vec<Shape>, Never> {
    let Ok(finger) = fitted::<i32, u32>(FINGER);
    let wide = measured.width.saturating_add(INSIDE_A_ROW.unsigned_abs().saturating_mul(2));
    let Ok(wide_i) = fitted::<u32, i32>(wide);
    let pill = ShapePanel {
        at: Point {
            x: card.x.saturating_add(card.width.saturating_sub(wide_i).saturating_div(2)),
            y: card
                .y
                .saturating_add(card.height)
                .saturating_sub(strip::MARGIN)
                .saturating_sub(FINGER),
        },
        size: Size { width: wide, height: finger },
        round: Round(TAB_ROUND),
        fill: wearing.night,
        edge: Edge::Of { wide: CARD_EDGE, color: wearing.pink },
    };
    let Ok(face) = font(TextStyle::Callout);
    let Ok(words) = centred(said, measured, &pill, Weight::Plain, face, wearing.text);

    Ok(vec![Shape::Panel(pill), Shape::Text(words)])
}

fn line_shapes(state: &State, typed: Size<u32>, at: Point<i32>, wide: u32, wearing: &Wearing) -> Result<Vec<Shape>, Never> {
    let Ok(finger) = fitted::<i32, u32>(FINGER);
    let Ok((said, filled)) = in_the_line(state);
    let words_x = at.x.saturating_add(INSIDE_A_ROW);
    let Ok(down) = middle(Band { top: at.y, height: FINGER }, typed);
    let Ok(caret_tall) = fitted::<i32, u32>(CARET_TALL);
    let (ink, written) = match filled {
        Filled::Typed => (wearing.text, typed.width),
        Filled::Empty => (wearing.soft, 0),
    };
    let Ok(written) = fitted::<u32, i32>(written);

    let Ok(face_body) = font(TextStyle::Body);

    Ok(vec![
        Shape::Panel(ShapePanel {
            at,
            size: Size { width: wide, height: finger },
            round: Round(TAB_ROUND),
            fill: wearing.night,
            edge: Edge::Of { wide: CARD_EDGE, color: wearing.pink },
        }),
        Shape::Text(Text {
            at: Point { x: words_x, y: down },
            width: wide.saturating_sub(INSIDE_A_ROW.unsigned_abs().saturating_mul(2)),
            said,
            weight: Weight::Plain,
            font: face_body,
            ink,
        }),
        Shape::Panel(ShapePanel {
            at: Point {
                x: words_x.saturating_add(written).saturating_add(1),
                y: at.y.saturating_add(FINGER.saturating_sub(CARET_TALL).saturating_div(2)),
            },
            size: Size { width: CARET, height: caret_tall },
            round: Round(0),
            fill: wearing.pink,
            edge: Edge::None,
        }),
    ])
}

pub(crate) struct SurfaceSize {
    width: i32,
    height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Card {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Leaving {
    Standing,
    No,
}

pub(crate) struct State {
    pages: Vec<Page>,
    rows: Vec<Row>,
    leaving: Leaving,
    here: u32,
    at: Option<u32>,
    opened: Opened,
    from_tab: u32,
    scroll: i32,
    note: Option<String>,
    typed: String,
    beside: Beside,
    asking: Option<Prompt>,
    sure: Option<Sure>,
    pressing: Option<(u32, u32)>,
    zoom: Option<(std::path::PathBuf, crate::zoom::Zoom)>,
    shown: Option<Moving>,
    selected: Vec<String>,
    framing: Framing,
}

fn still_of(picture: &Picture) -> Result<Option<&Path>, Never> {
    Ok(match picture {
        Picture::Showing(Some(path)) => Some(path.as_path()),
        Picture::None
        | Picture::Space
        | Picture::Named(_)
        | Picture::At(_)
        | Picture::Sleeve(_)
        | Picture::Showing(None)
        | Picture::Playing(_)
        | Picture::Written(_)
        | Picture::Bar(_) => None,
    })
}

fn zoom_on(state: &State, row: &Row) -> Result<crate::zoom::Zoom, Never> {
    let Ok(still) = still_of(&row.picture);

    Ok(match (still, &state.zoom) {
        (Some(still), Some((on, zoom))) => match still == on.as_path() {
            true => *zoom,
            false => crate::zoom::Zoom::default(),
        },
        (Some(_) | None, _) => crate::zoom::Zoom::default(),
    })
}

fn zoomed_to(state: &mut State, rows: &[Row], zooming: impl Fn(crate::zoom::Zoom) -> crate::zoom::Zoom) -> Result<(), Never> {
    let found = rows.iter().find_map(|row| {
        let Ok(still) = still_of(&row.picture);

        still.map(|still| (still.to_path_buf(), row))
    });

    let (still, row) = match found {
        Some(found) => found,
        None => return Ok(()),
    };

    let Ok(was) = zoom_on(state, row);

    state.zoom = Some((still, zooming(was)));

    Ok(())
}

fn panning(
    state: &State,
    moving: Option<&Moving>,
    hit: Point<i32>,
) -> Result<Option<(std::path::PathBuf, crate::zoom::Zoom, Size<u32>)>, Never> {
    let moving = match moving {
        Some(moving) => moving,
        None => return Ok(None),
    };
    let Ok(wide) = fitted::<u32, i32>(moving.room.width);
    let Ok(tall) = fitted::<u32, i32>(moving.room.height);
    let inside = hit.x >= moving.at.x
        && hit.x < moving.at.x.saturating_add(wide)
        && hit.y >= moving.at.y
        && hit.y < moving.at.y.saturating_add(tall);

    Ok(match (inside, &state.zoom) {
        (true, Some((still, zoom))) => match (still == &moving.of, zoom.zoomed()) {
            (true, Ok(crate::zoom::Zoomed::ZoomedIn)) => Some((still.clone(), *zoom, moving.room)),
            (true, Ok(crate::zoom::Zoomed::Whole)) | (false, _) => None,
        },
        (true, None) | (false, _) => None,
    })
}

fn pressed(state: &State, at: u32, row: &Row) -> Result<u32, Never> {
    let across = match &row.buttons {
        Some(across) => across,
        None => return Ok(0),
    };
    let Ok(last) = fitted::<_, u32>(across.presses.len().saturating_sub(1));

    Ok(match state.pressing {
        Some((on, which)) => match on == at {
            true => which.min(last),
            false => across.at.min(last),
        },
        None => across.at.min(last),
    })
}

fn drawn_in(panelled: &Panelled, at: u32) -> Result<Vec<String>, Never> {
    let band = panelled.touching.iter().find(|touching| touching.lands == Lands::Row(at));

    let band = match band {
        Some(band) => band.panel,
        None => return Ok(Vec::new()),
    };

    let Ok(tall) = fitted::<u32, i32>(band.size.height);
    let top = band.at.y;
    let bottom = top.saturating_add(tall);

    Ok(panelled
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) => Some(text),
            Shape::Panel(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
        })
        .filter(|text| text.at.y >= top && text.at.y < bottom)
        .map(|text| text.said.clone())
        .collect())
}

pub(crate) fn what_it_drew(
    state: &State,
    panelled: &Panelled,
    rows: &[Row],
    room: (i32, i32),
) -> Result<description::Description, Never> {
    let mut card: Vec<description::Spot> = Vec::new();
    let mut on_rows: BTreeMap<u32, Vec<description::Spot>> = BTreeMap::new();

    for touching in &panelled.touching {
        let Ok((name, whose)) = touching.lands.named();
        let Ok(across) = fitted::<u32, i32>(touching.panel.size.width);
        let Ok(down) = fitted::<u32, i32>(touching.panel.size.height);

        let spot = description::Spot {
            name: name.to_string(),
            at: (touching.panel.at.x, touching.panel.at.y),
            big: (across, down),
            scrolls: match whose {
                Owner::Card => description::Scrolls::No,
                Owner::Row(_) => description::Scrolls::Yes,
            },
        };

        match whose {
            Owner::Card => card.push(spot),
            Owner::Row(at) => on_rows.entry(at).or_default().push(spot),
        }
    }

    let mut lines: Vec<description::Line> = Vec::new();

    for (at, spots) in on_rows {
        let Ok(row) = nth(rows, at);

        let row = match row {
            Some(row) => row,
            None => continue,
        };

        let Ok(bare) = crate::page::bare(row);
        let Ok(heading) = row.heading();
        let Ok(wears) = crate::page::wears(row);
        let Ok(drew) = drawn_in(panelled, at);

        lines.push(description::Line {
            at,
            says: row.says.clone(),
            aside: row.aside.clone(),
            offers: match wears {
                Wears::WhatElse => description::Offers::Yes,
                Wears::None => description::Offers::No,
            },
            bare: match bare {
                Bare::Yes => description::Bare::Yes,
                Bare::No => description::Bare::No,
            },
            heading: match heading {
                Heading::Yes => description::Heading::Yes,
                Heading::No => description::Heading::No,
            },
            standing: match (state.at == Some(at), state.beside) {
                (true, Beside::Yes) => description::Standing::Beside,
                (true, Beside::No) => description::Standing::On,
                (false, Beside::Yes) | (false, Beside::No) => description::Standing::No,
            },
            spots,
            cells: row.cells.iter().map(|cell| cell.says.clone()).collect(),
            drew,
        });
    }

    let Ok(whose) = crate::whose::name();
    let Ok(page) = nth(&state.pages, state.here);

    Ok(description::Description {
        panel: whose,
        tab: match page {
            Some(page) => page.title.clone(),
            None => String::new(),
        },
        out: match state.opened {
            Opened::Expanded => description::Output::Yes,
            Opened::No => description::Output::No,
        },
        room,
        spots: card,
        lines,
    })
}

pub(crate) fn shapes(
    state: &State,
    card: &Card,
    fits: u32,
    measured: &Measured,
    wearing: &Wearing,
) -> Result<Panelled, Never> {
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let mut moving: Option<Moving> = None;
    let Ok(strip_tall) = fitted::<i32, i32>(fitting::STRIP);
    let Ok(row_tall) = fitted::<i32, i32>(fitting::ROW);
    let Ok(card_wide_u) = fitted::<i32, u32>(card.width);
    let Ok(card_tall_u) = fitted::<i32, u32>(card.height);
    let Ok(left) = inside_left(card);
    let Ok(right) = inside_right(card);
    let Ok(inside_wide) = across(right.saturating_sub(left));

    let (round, fill, edge) = match (state.opened, state.framing) {
        (Opened::Expanded, _) => (Round(0), wearing.night, Edge::None),
        (Opened::No, Framing::Screen) => (Round(0), wearing.panel, Edge::None),
        (Opened::No, Framing::Card) => (Round(CARD_ROUND), wearing.panel, Edge::Of { wide: CARD_EDGE, color: wearing.pink }),
    };

    shapes.push(Shape::Panel(ShapePanel {
        at: Point { x: card.x, y: card.y },
        size: Size { width: card_wide_u, height: card_tall_u },
        round,
        fill,
        edge,
    }));

    let Ok(many) = fitted(state.pages.len());
    let Ok(run) = strip::showing(strip::Tabs {
        many,
        here: state.here,
        from: state.from_tab,
        fits,
    });
    let window = TabWindow { from: run.start, fits };
    let (header_shapes, header_touching) = header(state, window, measured, card, wearing)?;

    touching.extend(header_touching);

    let over_the_rows = match state.opened {
        Opened::Expanded => header_shapes,
        Opened::No => {
            shapes.extend(header_shapes);

            Vec::new()
        },
    };

    match (&state.asking, &state.sure) {
        (Some(asking), _) => {
            let question = question_shapes(asking, state, measured, card, wearing)?;

            shapes.extend(question);

            return Ok(Panelled { shapes, touching, moving: None });
        }
        (None, Some(sure)) => {
            let (question, answers) = sure_shapes(sure, measured, card, wearing)?;

            shapes.extend(question);
            touching.extend(answers);

            return Ok(Panelled { shapes, touching, moving: None });
        }
        (None, None) => {},
    }

    let Ok(page) = nth(&state.pages, state.here);

    match page {
        Some(page) => {
            let Ok(sought_tall) = asked_about(page);
            let rows_start = match state.opened {
                Opened::Expanded => card.y,
                Opened::No => card.y.saturating_add(strip_tall).saturating_add(OVER_ROWS).saturating_add(sought_tall),
            };

            match &page.sought {
                Some(_) => {
                    let at = Point {
                        x: left,
                        y: rows_start
                            .saturating_sub(sought_tall)
                            .saturating_add(BETWEEN_ROWS.saturating_div(2)),
                    };

                    let Ok(line) = line_shapes(state, measured.typed, at, inside_wide, wearing);

                    shapes.extend(line);
                }
                None => {},
            }

            let rows = &state.rows;
            let breath = match state.opened {
                Opened::Expanded => 0,
                Opened::No => fitting::BREATH,
            };
            let bottom = card.y.saturating_add(card.height).saturating_sub(breath);
            let Ok(stage) = stage(rows, bottom.saturating_sub(rows_start));
            let card_bottom = card.y.saturating_add(card.height);
            let Ok(list_tall) = fitted::<i32, u32>(card_bottom.saturating_sub(rows_start));

            shapes.push(Shape::Clip(Clip::To { at: Point { x: card.x, y: rows_start }, size: Size { width: card_wide_u, height: list_tall } }));

            for (index, row) in rows.iter().enumerate() {
                let Ok(row_y_offset) = fitted::<_, i32>(index);
                let Ok(index) = fitted::<_, u32>(index);
                let (below, this_tall) = match index {
                    0 => (0, row_tall.saturating_add(stage)),
                    _ => (stage, row_tall),
                };
                let at_y = rows_start
                    .saturating_add(row_y_offset.saturating_mul(row_tall))
                    .saturating_add(below)
                    .saturating_sub(state.scroll);

                let row_bottom = at_y.saturating_add(this_tall);

                match row_bottom <= rows_start || at_y >= card_bottom {
                    true => {
                        continue;
                    }
                    false => {}
                }

                let highlight = match (state.at == Some(index), state.leaving) {
                    (true, Leaving::No) => Highlight::Yes,
                    (true, Leaving::Standing) | (false, _) => Highlight::No,
                };
                let Ok(press) = pressed(state, index, row);
                let Ok(zoom) = zoom_on(state, row);
                let Ok(selected) = selected_of(state, row);
                let standing = Standing { at: index, highlight, beside: state.beside, press, opened: state.opened, zoom, selected };
                let band = Band { top: at_y, height: this_tall };
                let drawn = match &row.headline {
                    Some(headline) => match headline.alignment {
                        crate::page::Alignment::Leading => headline_shapes(headline, row, band, card, wearing)?,
                        crate::page::Alignment::Trailing => trailing_headline_shapes(headline, band, card, wearing)?,
                    },
                    None => row_shapes(row, band, card, standing, measured, wearing)?,
                };

                moving = moving.or(drawn.moving);
                shapes.extend(drawn.shapes);

                let whole = at_y >= rows_start && row_bottom <= card_bottom;

                match whole {
                    true => {},
                    false => continue,
                }

                touching.extend(drawn.touching);

                let Ok(tall) = fitted::<i32, u32>(this_tall);

                touching.push(HitRegion {
                    lands: Lands::Row(index),
                    panel: ShapePanel {
                        at: Point { x: left, y: at_y },
                        size: Size { width: inside_wide, height: tall },
                        round: Round(0),
                        fill: wearing.panel,
                        edge: Edge::None,
                    },
                });
            }
        }
        None => {}
    }

    shapes.push(Shape::Clip(Clip::Lifted));
    shapes.extend(over_the_rows);

    match &state.note {
        Some(said) => {
            let Ok(noted) = note_shapes(said, measured.note, card, wearing);

            shapes.extend(noted);
        }
        None => {}
    }

    Ok(Panelled { shapes, touching, moving })
}

pub(crate) fn asked_about(page: &Page) -> Result<i32, Never> {
    Ok(match page.sought {
        Some(_) => fitting::ROW,
        None => 0,
    })
}

fn asked_at(card: &Card) -> Result<Point<i32>, Never> {
    let Ok(edge) = fitted::<i32, i32>(strip::EDGE);
    let Ok(margin) = fitted::<i32, i32>(strip::MARGIN);
    let Ok(strip_tall) = fitted::<i32, i32>(fitting::STRIP);

    Ok(Point {
        x: card.x.saturating_add(edge).saturating_add(margin),
        y: card.y.saturating_add(strip_tall).saturating_add(OVER_ROWS),
    })
}

fn asked_wide(card: &Card) -> Result<u32, Never> {
    let Ok(edge) = fitted::<i32, i32>(strip::EDGE);
    let Ok(margin) = fitted::<i32, i32>(strip::MARGIN);

    fitted::<i32, u32>(
        card.width
            .saturating_sub(edge.saturating_mul(2))
            .saturating_sub(margin.saturating_mul(2)),
    )
}

fn question_shapes(
    asking: &Prompt,
    state: &State,
    measured: &Measured,
    card: &Card,
    wearing: &Wearing,
) -> Result<Vec<Shape>, Never> {
    let mut shapes = Vec::new();
    let Ok(at) = asked_at(card);
    let Ok(wide) = asked_wide(card);

    let Ok(face_body) = font(TextStyle::Body);

    shapes.push(Shape::Text(Text {
        at,
        width: wide,
        said: asking.question.clone(),
        weight: Weight::Bold,
        font: face_body,
        ink: wearing.text,
    }));

    let line_at = Point { x: at.x, y: at.y.saturating_add(fitting::ROW) };

    let Ok(line) = line_shapes(state, measured.typed, line_at, wide, wearing);

    shapes.extend(line);

    Ok(shapes)
}

fn sure_shapes(
    sure: &Sure,
    measured: &Measured,
    card: &Card,
    wearing: &Wearing,
) -> Result<(Vec<Shape>, Vec<HitRegion>), Never> {
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let row_font = font(TextStyle::Body)?;
    let Ok(at) = asked_at(card);
    let Ok(wide) = asked_wide(card);

    shapes.push(Shape::Text(Text {
        at,
        width: wide,
        said: sure.question.clone(),
        weight: Weight::Bold,
        font: row_font.clone(),
        ink: wearing.text,
    }));

    shapes.push(Shape::Text(Text {
        at: Point { x: at.x, y: at.y.saturating_add(fitting::ROW) },
        width: wide,
        said: sure.about.clone(),
        weight: Weight::Plain,
        font: row_font,
        ink: wearing.soft,
    }));

    let Ok(many_across) = fitted::<_, u32>(sure.answers.len());

    match many_across {
        0 => return Ok((shapes, touching)),
        _ => {}
    }

    let Ok(gap) = fitted::<i32, u32>(strip::GAP.saturating_mul(2));
    let answer_wide = wide
        .saturating_sub(gap.saturating_mul(many_across.saturating_sub(1)))
        .saturating_div(many_across);
    let Ok(answer_wide_i) = fitted::<u32, i32>(answer_wide);
    let Ok(gap_i) = fitted::<u32, i32>(gap);
    let answers_y = at.y.saturating_add(fitting::ROW.saturating_mul(3));
    let Ok(finger) = fitted::<i32, u32>(FINGER);

    for (which, says) in sure.answers.iter().enumerate() {
        let Ok(step) = fitted::<_, i32>(which);
        let Ok(which) = fitted::<_, u32>(which);
        let Ok(slot) = index(which);
        let said = match measured.answers.get(slot) {
            Some(said) => *said,
            None => NO_ROOM,
        };
        let (drawn, reached) = button(
            Button {
                at: Point {
                    x: at.x.saturating_add(step.saturating_mul(answer_wide_i.saturating_add(gap_i))),
                    y: answers_y,
                },
                size: Size { width: answer_wide, height: finger },
                said: says,
                measured: said,
                lands: Lands::Answer(which),
                stood: match which == sure.at {
                    true => Highlight::Yes,
                    false => Highlight::No,
                },
            },
            wearing,
        )?;

        shapes.extend(drawn);
        touching.push(reached);
    }

    Ok((shapes, touching))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Secret {
    Yes,
    No,
}

pub(crate) struct Prompt {
    question: String,
    typed: String,
    secret: Secret,
    then: Answer,
}

pub(crate) struct Sure {
    question: String,
    about: String,
    answers: Vec<String>,
    at: u32,
    then: OnChosen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Typed {
    Edited,
    None,
}

pub(crate) fn typed_into(typed: &mut String, key: Keysym) -> Result<Typed, Never> {
    match key {
        Keysym::BackSpace => {
            let gone = typed.pop();

            return Ok(match gone {
                Some(_letter) => Typed::Edited,
                None => Typed::None,
            });
        }
        _ => {},
    }

    let letter = match key.key_char() {
        Some(letter) => letter,
        None => return Ok(Typed::None),
    };

    Ok(match letter.is_control() {
        true => Typed::None,
        false => {
            typed.push(letter);

            Typed::Edited
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    None,
    Redrawn,
    Chose(u32),
    Else(u32),
    Closing,
}

#[must_use]
pub(crate) fn stepped_onto(meaning: Meaning, before: Option<u32>, state: &State) -> Result<Option<u32>, Never> {
    let moved = match state.at == before {
        true => None,
        false => state.at,
    };

    Ok(match meaning {
        Meaning::Step(_) => moved,
        Meaning::None
        | Meaning::Choose
        | Meaning::More
        | Meaning::Nudge(_)
        | Meaning::Tab(_)
        | Meaning::Close
        | Meaning::Abandon => None,
    })
}

pub(crate) fn told(state: &mut State, meaning: Meaning, rows: &[Row]) -> Result<Outcome, Never> {
    match (state.leaving, meaning) {
        (Leaving::Standing, Meaning::Choose) => return Ok(Outcome::Closing),
        (Leaving::Standing, Meaning::Step(_) | Meaning::Nudge(_) | Meaning::More) => {
            state.leaving = Leaving::No;

            return Ok(Outcome::Redrawn);
        }
        (Leaving::Standing, Meaning::None | Meaning::Close | Meaning::Abandon | Meaning::Tab(_))
        | (Leaving::No, _) => {},
    }

    Ok(match meaning {
        Meaning::None => Outcome::None,
        Meaning::Close | Meaning::Abandon => {
            let Ok(mode) = selecting(state, rows);

            match (mode, state.opened) {
                (Selecting::Yes, _) => {
                    state.selected = Vec::new();

                    Outcome::Redrawn
                }
                (Selecting::No, Opened::Expanded) => {
                    state.opened = Opened::No;

                    Outcome::Redrawn
                }
                (Selecting::No, Opened::No) => Outcome::Closing,
            }
        },
        Meaning::Step(step) => {
            let from = match state.at {
                Some(at) => {
                    let Ok(from) = fitted::<u32, i32>(at);

                    from
                }
                None => BEFORE_THE_FIRST_ROW,
            };
            let Ok(going) = walked(rows, from, Step(step));
            let Ok(landed) = fitted::<i32, u32>(going.max(0));

            state.at = Some(landed);
            state.beside = Beside::No;

            let Ok(()) = column_kept(state, rows, landed);

            Outcome::Redrawn
        }
        Meaning::Tab(step) => {
            let from = match state.leaving {
                Leaving::Standing => strip::Stop::Outside,
                Leaving::No => strip::Stop::Tab(state.here),
            };
            let Ok(many) = fitted(state.pages.len());
            let Ok(going) = strip::along(many, from, step);

            match going {
                strip::Stop::Outside => state.leaving = Leaving::Standing,
                strip::Stop::Tab(index) => {
                    let Ok(()) = stood_on(state, index);
                }
            }

            Outcome::Redrawn
        }
        Meaning::Nudge(step) => {
            let at = match state.at {
                Some(at) => at,
                None => return Ok(Outcome::None),
            };

            let Ok(row) = nth(rows, at);

            let row = match row {
                Some(row) => row,
                None => return Ok(Outcome::None),
            };

            match &row.buttons {
                Some(across) => {
                    let Ok(from) = pressed(state, at, row);
                    let Ok(last) = fitted::<_, u32>(across.presses.len().saturating_sub(1));
                    let going = match step > 0 {
                        true => from.saturating_add(1).min(last),
                        false => from.saturating_sub(1),
                    };

                    state.pressing = Some((at, going));

                    return Ok(match going == from {
                        true => Outcome::None,
                        false => Outcome::Redrawn,
                    });
                }
                None => {}
            }

            let Ok(wears) = crate::page::wears(row);
            let Ok(holds) = crate::page::holds(row);
            let Ok(nudged) = crate::page::nudged(state.beside, wears, holds, step);

            match nudged {
                Nudged::Stand => {
                    state.beside = Beside::Yes;

                    Outcome::Redrawn
                }
                Nudged::Back => {
                    state.beside = Beside::No;

                    Outcome::Redrawn
                }
                Nudged::None => Outcome::None,
                Nudged::Level => {
                    let Ok(moved) = leveled(rows, at, step);

                    moved
                }
            }
        }
        Meaning::Choose => match (state.at, state.beside) {
            (Some(at), Beside::Yes) => {
                state.beside = Beside::No;

                Outcome::Else(at)
            }
            (Some(at), Beside::No) => Outcome::Chose(at),
            (None, _) => Outcome::None,
        },
        Meaning::More => match state.at {
            Some(at) => Outcome::Else(at),
            None => Outcome::None,
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Viewport {
    room: i32,
    y: i32,
}

pub(crate) fn scrolled(at: Option<u32>, viewport: Viewport) -> Result<i32, Never> {
    let at = match at {
        Some(at) => at,
        None => return Ok(0),
    };

    let Ok(index) = fitted::<u32, i32>(at);
    let top = index.saturating_mul(fitting::ROW);
    let under = top.saturating_add(fitting::ROW);

    Ok(match (top < viewport.y, under > viewport.y.saturating_add(viewport.room)) {
        (true, _) => top,
        (false, true) => under.saturating_sub(viewport.room),
        (false, false) => viewport.y,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Chose,
    Else,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gone {
    Closing,
    Staying,
}

struct Front {
    at: Cell<Option<u32>>,
    tab: Cell<Option<u32>>,
    opened: Cell<Option<Opened>>,
    toggled: Cell<Toggled>,
    zoomed: Cell<Option<f64>>,
    note: RefCell<Option<String>>,
    asking: RefCell<Option<Prompt>>,
    sure: RefCell<Option<Sure>>,
    framed: Option<crate::zoom::Framed>,
    selecting: RefCell<Option<Vec<String>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Toggled {
    Yes,
    No,
}

impl Front {
    fn new() -> Result<Front, Never> {
        Ok(Front {
            at: Cell::new(None),
            tab: Cell::new(None),
            opened: Cell::new(None),
            toggled: Cell::new(Toggled::No),
            zoomed: Cell::new(None),
            note: RefCell::new(None),
            asking: RefCell::new(None),
            sure: RefCell::new(None),
            framed: None,
            selecting: RefCell::new(None),
        })
    }

    fn over(state: &State) -> Result<Front, Never> {
        let Ok(front) = Front::new();

        let moving = match &state.shown {
            Some(moving) => moving,
            None => return Ok(front),
        };

        let zoom = match &state.zoom {
            Some((on, zoom)) => match *on == moving.of {
                true => *zoom,
                false => crate::zoom::Zoom::default(),
            },
            None => crate::zoom::Zoom::default(),
        };

        let framed = crate::zoom::Framed { of: moving.of.clone(), zoom, room: moving.room };

        Ok(Front { framed: Some(framed), ..front })
    }

    fn onto(self, state: &mut State) -> Result<(), Never> {
        match self.at.get() {
            Some(at) => state.at = Some(at),
            None => {},
        }

        match self.tab.get() {
            Some(tab) => {
                let Ok(last) = fitted(state.pages.len().saturating_sub(1));

                state.here = tab.min(last);
                state.selected = Vec::new();
                state.opened = Opened::No;
                state.at = None;
                state.scroll = 0;
            }
            None => {},
        }

        match self.opened.get() {
            Some(opened) => state.opened = opened,
            None => {},
        }

        match self.zoomed.get() {
            Some(factor) => {
                let rows = state.rows.clone();
                let Ok(()) = zoomed_to(state, &rows, |was| match was.times(factor) {
                    Ok(zoom) => zoom,
                });
            }
            None => {},
        }

        match self.toggled.get() {
            Toggled::Yes => {
                state.opened = match state.opened {
                    Opened::Expanded => Opened::No,
                    Opened::No => Opened::Expanded,
                };
            }
            Toggled::No => {},
        }

        match self.note.borrow().clone() {
            Some(said) => state.note = Some(said),
            None => {},
        }

        match self.asking.take() {
            Some(asking) => state.asking = Some(asking),
            None => {},
        }

        match self.sure.take() {
            Some(sure) => state.sure = Some(sure),
            None => {},
        }

        match self.selecting.take() {
            Some(keys) => state.selected = keys,
            None => {},
        }

        Ok(())
    }
}

impl Showing for Front {
    fn refresh(&self) {}

    fn replace(&self, standing_on: u32) {
        self.at.set(Some(standing_on));
    }

    fn forget_typing(&self) {}

    fn ask(&self, question: &str, then: Answer) {
        self.asking.replace(Some(Prompt {
            question: question.to_string(),
            typed: String::new(),
            secret: Secret::Yes,
            then,
        }));
    }

    fn ask_aloud(&self, question: &str, then: Answer) {
        self.asking.replace(Some(Prompt {
            question: question.to_string(),
            typed: String::new(),
            secret: Secret::No,
            then,
        }));
    }

    fn sure(&self, question: &str, about: Subject<'_>, does: &[&str], then: OnChosen) {
        let answers = std::iter::once(marks::CANCEL.to_string())
            .chain(does.iter().map(|says| (*says).to_string()))
            .collect();

        self.sure.replace(Some(Sure {
            question: question.to_string(),
            about: about.0.to_string(),
            answers,
            at: 0,
            then,
        }));
    }

    fn later(&self, arguments: Vec<String>) {
        let Ok(()) = crate::running::and_waited(arguments);
    }

    fn leave_running(&self, arguments: Vec<String>) {
        let Ok(()) = crate::running::left_running(&arguments);
    }

    fn note(&self, said: &str) {
        self.note.replace(Some(said.to_string()));
    }

    fn open_out(&self) {
        self.opened.set(Some(Opened::Expanded));
    }

    fn toggle_full_screen(&self) {
        self.toggled.set(Toggled::Yes);
    }

    fn zoom_by(&self, factor: f64) {
        self.zoomed.set(Some(factor));
    }

    fn framed(&self) -> Result<Option<crate::zoom::Framed>, Never> {
        Ok(self.framed.clone())
    }

    fn select(&self, keys: Vec<String>) {
        self.selecting.replace(Some(keys));
    }

    fn turn_to(&self, tab: u32) {
        self.tab.set(Some(tab));
    }
}

pub(crate) fn driving(state: &State) -> Result<Driving, Never> {
    match (&state.sure, &state.asking) {
        (Some(_), _) => return Ok(Driving::Sure),
        (None, Some(_)) => return Ok(Driving::Question),
        (None, None) => {},
    }

    let Ok(page) = nth(&state.pages, state.here);
    let sought = page.and_then(|page| page.sought.as_ref());

    Ok(match sought {
        Some(_) => Driving::Search,
        None => Driving::Panel,
    })
}

fn pressed_here(
    state: &mut State,
    rows: &[Row],
    key: Keysym,
    meaning: Meaning,
    driving: Driving,
) -> Result<Gone, Never> {
    match (meaning, driving) {
        (Meaning::Abandon, Driving::Question) => {
            state.asking = None;

            return Ok(Gone::Staying);
        }
        (Meaning::Abandon, Driving::Sure) => {
            state.sure = None;

            return Ok(Gone::Staying);
        }
        (Meaning::Nudge(step), Driving::Sure) => {
            let Ok(()) = leaned(state, step);

            return Ok(Gone::Staying);
        }
        (Meaning::Choose, Driving::Sure) => {
            let at = state.sure.as_ref().map(|sure| sure.at);

            match at {
                Some(at) => {
                    let Ok(()) = took(state, at);
                }
                None => {},
            }

            return Ok(Gone::Staying);
        }
        (Meaning::None, Driving::Question) => {
            match key {
                Keysym::Return | Keysym::KP_Enter => {
                    let Ok(()) = answered(state);

                    return Ok(Gone::Staying);
                }
                _ => {},
            }

            let asking = match state.asking.take() {
                Some(asking) => asking,
                None => return Ok(Gone::Staying),
            };

            let mut typed = asking.typed;
            let Ok(_changed) = typed_into(&mut typed, key);

            state.asking = Some(Prompt { typed, ..asking });

            return Ok(Gone::Staying);
        }
        (Meaning::None, Driving::Search) => {
            let mut typed = std::mem::take(&mut state.typed);
            let Ok(changed) = typed_into(&mut typed, key);

            state.typed = typed;

            match changed {
                Typed::Edited => {
                    let Ok(()) = narrowed(state);
                }
                Typed::None => {},
            }

            return Ok(Gone::Staying);
        }
        (Meaning::None | Meaning::Abandon | Meaning::Choose, _)
        | (Meaning::Close | Meaning::More | Meaning::Nudge(_), _)
        | (Meaning::Step(_) | Meaning::Tab(_), _) => {},
    }

    match (&state.asking, &state.sure) {
        (None, None) => {},
        (Some(_), _) | (None, Some(_)) => return Ok(Gone::Staying),
    }

    let Ok(told) = told(state, meaning, rows);

    carried_out(state, rows, told)
}

fn column_kept(state: &mut State, rows: &[Row], landed: u32) -> Result<(), Never> {
    let Ok(row) = nth(rows, landed);
    let across = row.and_then(|row| row.buttons.as_ref());

    match (state.pressing, across) {
        (Some((_, which)), Some(across)) => {
            let Ok(last) = fitted::<_, u32>(across.presses.len().saturating_sub(1));

            state.pressing = Some((landed, which.min(last)));
        }
        (Some(_), None) | (None, _) => {},
    }

    Ok(())
}

fn stood_on(state: &mut State, which: u32) -> Result<(), Never> {
    state.opened = Opened::No;
    state.leaving = Leaving::No;
    state.here = which;
    state.at = None;
    state.scroll = 0;
    state.beside = Beside::No;
    state.selected = Vec::new();

    Ok(())
}

fn leveled(rows: &[Row], at: u32, step: i32) -> Result<Outcome, Never> {
    let Ok(row) = nth(rows, at);
    let level = row.and_then(|row| row.level.clone());

    Ok(match level {
        Some(level) => {
            level(step);

            Outcome::Redrawn
        }
        None => Outcome::None,
    })
}

fn leaned(state: &mut State, step: i32) -> Result<(), Never> {
    let sure = match state.sure.as_mut() {
        Some(sure) => sure,
        None => return Ok(()),
    };

    let Ok(last) = fitted::<_, i32>(sure.answers.len().saturating_sub(1));
    let Ok(at) = fitted::<u32, i32>(sure.at);
    let Ok(landed) = fitted::<i32, u32>(at.saturating_add(step).clamp(0, last));

    sure.at = landed;

    Ok(())
}

fn took(state: &mut State, at: u32) -> Result<(), Never> {
    let sure = match state.sure.take() {
        Some(sure) => sure,
        None => return Ok(()),
    };

    match at.checked_sub(1) {
        None => return Ok(()),
        Some(which) => {
            let Ok(front) = Front::over(state);

            (sure.then)(&front, which);

            let Ok(()) = front.onto(state);
        }
    }

    Ok(())
}

fn answered(state: &mut State) -> Result<(), Never> {
    let asking = match state.asking.take() {
        Some(asking) => asking,
        None => return Ok(()),
    };

    let Ok(front) = Front::over(state);

    (asking.then)(&front, &asking.typed);

    let Ok(()) = front.onto(state);

    Ok(())
}

fn narrowed(state: &mut State) -> Result<(), Never> {
    let Ok(page) = nth(&state.pages, state.here);
    let sought = page.and_then(|page| page.sought.clone());

    let sought = match sought {
        Some(sought) => sought,
        None => return Ok(()),
    };

    let Ok(front) = Front::over(state);

    (sought.then)(&front, &state.typed);

    let Ok(()) = front.onto(state);

    state.at = None;
    state.scroll = 0;

    Ok(())
}

fn carried_out(state: &mut State, rows: &[Row], told: Outcome) -> Result<Gone, Never> {
    Ok(match told {
        Outcome::Closing => Gone::Closing,
        Outcome::Chose(at) => {
            let Ok(gone) = acted(state, rows, at, Selection::Chose);

            gone
        }
        Outcome::Else(at) => {
            let Ok(gone) = acted(state, rows, at, Selection::Else);

            gone
        }
        Outcome::Redrawn | Outcome::None => Gone::Staying,
    })
}

fn sought(state: &mut State, rows: &[Row], at: u32, fraction: f64) -> Result<(), Never> {
    let Ok(row) = nth(rows, at);

    let seek = match row.and_then(|row| row.seek.clone()) {
        Some(seek) => seek,
        None => return Ok(()),
    };

    let Ok(front) = Front::over(state);

    seek(&front, fraction);

    front.onto(state)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selecting {
    Yes,
    No,
}

fn selected_of(state: &State, row: &Row) -> Result<Selected, Never> {
    Ok(match &row.key {
        Some(key) => match state.selected.contains(key) {
            true => Selected::Yes,
            false => Selected::No,
        },
        None => Selected::No,
    })
}

fn still_selected(state: &State, rows: &[Row]) -> Result<Vec<String>, Never> {
    let there: std::collections::BTreeSet<&String> = rows.iter().filter_map(|row| row.key.as_ref()).collect();

    Ok(state.selected.iter().filter(|key| there.contains(key)).cloned().collect())
}

fn selecting(state: &State, rows: &[Row]) -> Result<Selecting, Never> {
    let Ok(page) = nth(&state.pages, state.here);
    let acts = page.is_some_and(|page| !page.selecting.is_empty());
    let Ok(still) = still_selected(state, rows);

    Ok(match acts && !still.is_empty() {
        true => Selecting::Yes,
        false => Selecting::No,
    })
}

fn toggled(state: &mut State, key: &str) -> Result<(), Never> {
    match state.selected.iter().any(|held| held == key) {
        true => state.selected.retain(|held| held != key),
        false => state.selected.push(key.to_string()),
    }

    Ok(())
}

fn asked_what_to_do(state: &mut State, rows: &[Row]) -> Result<(), Never> {
    let Ok(page) = nth(&state.pages, state.here);
    let acts: Vec<crate::page::Selecting> = match page {
        Some(page) => page.selecting.clone(),
        None => return Ok(()),
    };
    let Ok(keys) = still_selected(state, rows);
    let every: Vec<String> = rows.iter().filter_map(|row| row.key.clone()).collect();
    let answers = std::iter::once(marks::CANCEL.to_string())
        .chain(acts.iter().map(|act| act.says.clone()))
        .chain([SELECT_ALL.to_string(), DESELECT_ALL.to_string()])
        .collect();
    let question = format!("{} Selected", keys.len());

    let then: OnChosen = Arc::new(move |showing, which| {
        let Ok(which) = index(which);

        match (acts.get(which), which == acts.len()) {
            (Some(act), _) => {
                (act.act)(showing, &keys);
                showing.select(Vec::new());
            }
            (None, true) => showing.select(every.clone()),
            (None, false) => showing.select(Vec::new()),
        }
    });

    state.sure = Some(Sure { question, about: String::new(), answers, at: 0, then });

    Ok(())
}

fn acted(state: &mut State, rows: &[Row], at: u32, what: Selection) -> Result<Gone, Never> {
    let Ok(row) = nth(rows, at);

    let row = match row {
        Some(row) => row,
        None => return Ok(Gone::Staying),
    };

    let Ok(mode) = selecting(state, rows);

    match (mode, what, &row.key) {
        (Selecting::Yes, Selection::Chose, Some(key)) => {
            let Ok(()) = toggled(state, key);

            return Ok(Gone::Staying);
        }
        (Selecting::Yes, Selection::Else, _) => {
            let Ok(()) = asked_what_to_do(state, rows);

            return Ok(Gone::Staying);
        }
        (Selecting::Yes, Selection::Chose, None) | (Selecting::No, _, _) => {},
    }

    let Ok(front) = Front::over(state);

    let Ok(which) = pressed(state, at, row);
    let press = row.buttons.as_ref().and_then(|across| {
        let Ok(which) = index(which);

        across.presses.get(which).map(|press| Arc::clone(&press.does))
    });

    let gone = match (what, press) {
        (Selection::Chose, Some(does)) => match does(&front) {
            true => Gone::Closing,
            false => Gone::Staying,
        },
        (Selection::Chose, None) => match row.does.clone() {
            Some(Handler::Call(act)) => match act(&front) {
                true => Gone::Closing,
                false => Gone::Staying,
            },
            Some(Handler::Run(arguments)) => {
                let Ok(()) = crate::running::left_running(&arguments);

                Gone::Closing
            }
            None => Gone::Staying,
        },
        (Selection::Else, _) => match row.more.clone() {
            Some(more) => match more(&front) {
                true => Gone::Closing,
                false => Gone::Staying,
            },
            None => Gone::Staying,
        },
    };

    let Ok(()) = front.onto(state);

    Ok(gone)
}

pub fn run(
    who: &str,
    build: crate::card::Build,
    _column: i32,
    start: Option<&str>,
    while_it_is_up: OwnedFd,
    first_frame: mpsc::Sender<()>,
    tells: Option<String>,
) -> Result<(Close, thread::JoinHandle<Result<(), Never>>), Never> {
    let shut = Arc::new(AtomicBool::new(false));
    let handle_shut = Close(Arc::clone(&shut));

    let who = who.to_string();
    let start = start.map(str::to_string);

    let Ok(handed) = crate::opening::handing();

    let handle = thread::spawn(move || {
        let Ok(()) = crate::opening::handed(handed);
        let gone = while_it_is_up;
        let drawn = serve(&who, build, start.as_deref(), Framing::Card, shut, Reported { front: None, first_frame: Some(first_frame), tells });

        drop(gone);

        drawn
    });

    Ok((handle_shut, handle))
}

pub fn show(build: crate::card::Build, _column: i32, start: Option<&str>) -> Result<(), Never> {
    let Ok(whose) = crate::whose::name();
    let Ok(()) = crate::opening::started(&whose);
    let Ok(waited) = crate::picker::waited_for_screen();
    let Ok(()) = crate::opening::taking("screen", waited);

    let Ok(tells) = description::where_to();

    serve(&whose, build, start, Framing::Card, Arc::new(AtomicBool::new(false)), Reported { front: None, first_frame: None, tells })
}

pub fn app(who: &str, card: crate::card::Card) -> Result<(), Never> {
    let crate::card::Card { build, column: _, start, done } = card;
    let Ok(()) = crate::whose::named(who);
    let Ok(()) = crate::opening::started(who);
    let Ok(tells) = description::where_to();
    let Ok(()) = serve(who, build, start.as_deref(), Framing::Screen, Arc::new(AtomicBool::new(false)), Reported { front: None, first_frame: None, tells });

    done()
}

pub fn drawn_here(who: &str, card: crate::card::Card) -> Result<(), Never> {
    let crate::card::Card { build, column, start, done } = card;

    let Ok(()) = crate::whose::named(who);
    let Ok(()) = show(build, column, start.as_deref());

    done()
}

fn opened_on(pages: Vec<Page>, start: Option<&str>, framing: Framing) -> Result<State, Never> {
    Ok(State {
        here: match crate::page::find(&pages, start) {
            Ok(index) => index,
            Err(never) => match never {},
        },
        pages,
        rows: Vec::new(),
        leaving: Leaving::No,
        at: None,
        opened: Opened::No,
        from_tab: 0,
        scroll: 0,
        note: None,
        typed: String::new(),
        beside: Beside::No,
        asking: None,
        sure: None,
        pressing: None,
        zoom: None,
        shown: None,
        selected: Vec::new(),
        framing,
    })
}

fn card_taking(surface: &mut Surface, size: &SurfaceSize, opened: Opened, framing: Framing) -> Result<Card, Never> {
    let room = match opened {
        Opened::Expanded => Room::Over,
        Opened::No => Room::Around,
    };
    let Ok(()) = surface.room(room);

    card_on(size, opened, framing)
}

fn card_on(surface: &SurfaceSize, opened: Opened, framing: Framing) -> Result<Card, Never> {
    match (opened, framing) {
        (Opened::Expanded, _) | (Opened::No, Framing::Screen) => {
            return Ok(Card { x: 0, y: 0, width: surface.width, height: surface.height });
        },
        (Opened::No, Framing::Card) => {},
    }

    let Ok(wide) = shape::part_of(surface.width);
    let Ok(tall) = shape::tall_part_of(surface.height);

    let Ok(x) = fitted::<i32, i32>(surface.width.saturating_sub(wide).saturating_div(2));
    let Ok(y) = fitted::<i32, i32>(surface.height.saturating_sub(tall).saturating_div(2));

    Ok(Card { x, y, width: wide, height: tall })
}

fn covering() -> Result<Wanted, Never> {
    let Ok(whose) = crate::whose::name();

    Ok(Wanted {
        namespace: whose,
        anchor: Anchor::Whole,
        size: WHATEVER_THE_SCREEN_IS,
        margin: Margin { top: 0, right: 0, bottom: 0, left: 0 },
        keyboard: Keyboard::OnDemand,
        room: Room::Around,
        under: Under::None,
    })
}

fn nth<T>(list: &[T], at: u32) -> Result<Option<&T>, Never> {
    let Ok(at) = index(at);

    Ok(list.get(at))
}

fn sized(logical: Size<u32>) -> Result<SurfaceSize, Never> {
    let Ok(wide) = fitted::<u32, i32>(logical.width);
    let Ok(tall) = fitted::<u32, i32>(logical.height);

    Ok(SurfaceSize { width: wide, height: tall })
}

fn scroll_room(state: &State, card: &Card) -> Result<i32, Never> {
    let Ok(strip_tall) = fitted::<i32, i32>(fitting::STRIP);
    let Ok(page) = nth(&state.pages, state.here);
    let sought_tall = match page {
        Some(page) => asked_about(page)?,
        None => 0,
    };

    Ok(card.height
        .saturating_sub(strip_tall)
        .saturating_sub(OVER_ROWS)
        .saturating_sub(sought_tall))
}

fn touched(touching: &[HitRegion], hit: Point<i32>) -> Result<Option<Lands>, Never> {
    for touching in touching {
        let covers = touching.panel.covers(hit)?;

        match covers {
            Covers::Yes => return Ok(Some(touching.lands)),
            Covers::No => {},
        }
    }

    Ok(None)
}

fn watched(watching: &mut Option<(u32, BoundToParent)>, state: &State) -> Result<(), Never> {
    let already = watching.as_ref().map(|(on, _)| *on);

    match already == Some(state.here) {
        true => return Ok(()),
        false => {},
    }

    *watching = None;

    let Ok(page) = nth(&state.pages, state.here);
    let arguments = match page.and_then(|page| page.watch.as_ref()) {
        Some(watch) => &watch.arguments,
        None => return Ok(()),
    };

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(()),
    };

    let mut starting = Command::new(program);
    starting.args(rest).stdout(Stdio::null()).stderr(Stdio::null());
    let Ok(()) = console_response_times::not_a_press(&mut starting);

    match alongside(&mut starting) {
        Ok(running) => *watching = Some((state.here, running)),
        Err(fault) => eprintln!("console-panel: {program}: {fault}"),
    }

    Ok(())
}

struct InFront {
    watching: Option<(u32, BoundToParent)>,
    subscriptions: Subscriptions,
    now: Arc<Mutex<Option<Subscription>>>,
}

impl InFront {
    fn none() -> Result<Self, Never> {
        let Ok(subscriber) = console_events::subscription::connect(&[]);

        InFront::on(subscriber)
    }

    fn on(subscriber: Subscriber) -> Result<Self, Never> {
        let Ok((subscriptions, received)) = subscriber.split();
        let now = Arc::new(Mutex::new(None));
        let Ok(()) = forwarded(received, Arc::downgrade(&now));

        Ok(InFront { watching: None, subscriptions, now })
    }

    fn follow(&mut self, state: &State, page: Option<&Page>) -> Result<(), Never> {
        let Ok(()) = watched(&mut self.watching, state);

        listened(&self.subscriptions, &self.now, page)
    }
}

fn held(listening: &Mutex<Option<Subscription>>) -> Result<MutexGuard<'_, Option<Subscription>>, Never> {
    Ok(match listening.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    })
}

fn listened(
    subscriptions: &Subscriptions,
    listening: &Mutex<Option<Subscription>>,
    page: Option<&Page>,
) -> Result<(), Never> {
    let wanted = page.and_then(|page| page.listens.clone());
    let Ok(mut listening) = held(listening);

    match wanted.as_ref().map(|one| &one.topic) == listening.as_ref().map(|one| &one.topic) {
        true => return Ok(()),
        false => {},
    }

    match listening.take() {
        Some(was) => {
            let Ok(()) = subscriptions.unsubscribe(&was.topic);
        }
        None => {},
    }

    match wanted.as_ref() {
        Some(now) => {
            let Ok(()) = subscriptions.subscribe(&now.topic);
        }
        None => {},
    }

    *listening = wanted;

    Ok(())
}

fn forwarded(received: mpsc::Receiver<Received>, listening: Weak<Mutex<Option<Subscription>>>) -> Result<(), Never> {
    console_program_lifetime::threads::let_go(thread::spawn(move || {
        for event in received {
            let now = match listening.upgrade() {
                Some(now) => now,
                None => return,
            };
            let Ok(held) = held(&now);
            let Ok(worth) = worth(&event, held.as_ref());

            match worth {
                Worth::Querying => {
                    let Ok(()) = crate::frames::tell(crate::frames::Notice::Rows);
                },
                Worth::Ignoring => {},
            }
        }
    }))
}

fn worth(event: &Received, listening: Option<&Subscription>) -> Result<Worth, Never> {
    Ok(match (event, listening) {
        (Received::Connected, Some(_)) => Worth::Querying,
        (Received::Event(change), Some(one)) => match change.topic == one.topic {
            true => {
                let Ok(worth) = (one.worth)(&change.text);

                worth
            }
            false => Worth::Ignoring,
        },
        (Received::Connected | Received::Event(_), None) => Worth::Ignoring,
    })
}

struct Reading {
    here: u32,
    arrived: mpsc::Receiver<Vec<Row>>,
}

fn reading(page: &Page, here: u32) -> Result<Reading, Never> {
    let (sending, arrived) = mpsc::channel();
    let rows = page.rows.clone();
    let asking = thread::spawn(move || {
        let Ok(read) = rows.read();
        let _nobody_waits_for_a_tab_left_behind = sending.send(read);
        let Ok(()) = crate::frames::tell(crate::frames::Notice::Card);
    });
    let Ok(()) = console_program_lifetime::threads::let_go(asking);

    Ok(Reading { here, arrived })
}

struct Looked {
    heard: Result<Vec<Row>, mpsc::TryRecvError>,
    woken: Option<crate::frames::Woken>,
}

#[derive(Clone, Copy)]
struct FirstLook<'a> {
    shut: &'a AtomicBool,
    until: Duration,
}

fn first_look(reading: &Reading, rows: &[Row], look: FirstLook<'_>) -> Result<Looked, Never> {
    match rows.is_empty() {
        true => {},
        false => return Ok(Looked { heard: reading.arrived.try_recv(), woken: None }),
    }

    let Ok(deadline) = crate::frames::deadline(look.until);
    let mut woken = crate::frames::Woken::default();

    loop {
        let Ok(heard) = crate::frames::woken();
        let Ok(both) = woken.and(heard);

        woken = both;

        match (reading.arrived.try_recv(), look.shut.load(Ordering::Relaxed)) {
            (Err(mpsc::TryRecvError::Empty), false) => {},
            (Err(mpsc::TryRecvError::Empty), true) => {
                return Ok(Looked { heard: Err(mpsc::TryRecvError::Empty), woken: Some(woken) });
            }
            (landed, _) => return Ok(Looked { heard: landed, woken: Some(woken) }),
        }

        let Ok(listened) = crate::frames::listened(deadline.as_ref());

        match listened {
            crate::frames::Listened::Notified => {},
            crate::frames::Listened::RanOut => {
                return Ok(Looked { heard: reading.arrived.try_recv(), woken: Some(woken) });
            }
        }
    }
}

fn meanwhile(page: Option<&Page>) -> Result<Vec<Row>, Never> {
    Ok(match page.and_then(|page| page.meanwhile.clone()) {
        Some(at_once) => at_once(),
        None => Vec::new(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dirty {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stale {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dragged {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq)]
struct Touch {
    from: (f64, f64),
    scroll: i32,
    dragged: Dragged,
    lands: Option<Lands>,
    pans: Option<(std::path::PathBuf, crate::zoom::Zoom, Size<u32>)>,
}

enum Down {
    Woke,
    Touched(Touch),
}

fn finger_down(state: &State, panelled: &Panelled, at: (f64, f64)) -> Result<Down, Never> {
    let Ok(stirred) = stirred(state);

    match stirred {
        WakeOutcome::Woke => return Ok(Down::Woke),
        WakeOutcome::AlreadyAwake => {},
    }

    let Ok(across_at) = toward_zero_i32(at.0);
    let Ok(down_at) = toward_zero_i32(at.1);
    let Ok(lands) = touched(&panelled.touching, Point { x: across_at, y: down_at });
    let Ok(pans) = panning(state, panelled.moving.as_ref(), Point { x: across_at, y: down_at });

    Ok(Down::Touched(Touch { from: at, scroll: state.scroll, dragged: Dragged::No, lands, pans }))
}

fn outside(card: &Card, hit: Point<i32>) -> Result<Covers, Never> {
    let across_card = hit.x >= card.x && hit.x < card.x.saturating_add(card.width);
    let down_card = hit.y >= card.y && hit.y < card.y.saturating_add(card.height);

    Ok(match (across_card, down_card) {
        (true, true) => Covers::No,
        (false, _) | (true, false) => Covers::Yes,
    })
}

fn scrolled_to(state: &mut State, viewport: Viewport) -> Result<(), Never> {
    let Viewport { room, y: down } = viewport;
    let Ok(many) = fitted::<_, i32>(state.rows.len());
    let furthest = many.saturating_mul(fitting::ROW).saturating_sub(room).max(0);
    let down = down.clamp(0, furthest);

    state.scroll = down;

    let first = down.saturating_add(fitting::ROW).saturating_sub(1).saturating_div(fitting::ROW);
    let last = down.saturating_add(room).saturating_div(fitting::ROW).saturating_sub(1);
    let Ok(at) = fitted::<u32, i32>(match state.at {
        Some(at) => at,
        None => 0,
    });

    let wanted = match (at < first, at > last) {
        (true, _) => walked(&state.rows, first.saturating_sub(1), Step(1)),
        (false, true) => walked(&state.rows, last.saturating_add(1), Step(-1)),
        (false, false) => return Ok(()),
    };
    let Ok(wanted) = wanted;
    let Ok(landed) = fitted::<i32, u32>(wanted.max(0));

    state.at = Some(landed);
    state.beside = Beside::No;

    Ok(())
}

fn tapped(state: &mut State, lands: Option<Lands>, hit: Point<i32>, card: &Card) -> Result<Gone, Never> {
    let rows = state.rows.clone();

    Ok(match lands {
        Some(Lands::Row(landed)) => {
            let Ok(many) = fitted::<_, u32>(rows.len());

            match landed < many {
                true => {
                    state.at = Some(landed);
                    state.beside = Beside::No;

                    let Ok(gone) = acted(state, &rows, landed, Selection::Chose);

                    gone
                }
                false => Gone::Staying,
            }
        }
        Some(Lands::Answer(which)) => {
            let Ok(()) = took(state, which);

            Gone::Staying
        }
        Some(Lands::Nudge { row, step }) => {
            state.at = Some(row);
            state.beside = Beside::No;

            let Ok(_moved) = leveled(&rows, row, step);

            Gone::Staying
        }
        Some(Lands::Else(row)) => {
            state.at = Some(row);

            let Ok(gone) = acted(state, &rows, row, Selection::Else);

            gone
        }
        Some(Lands::ButtonPress { row, which }) => {
            state.at = Some(row);
            state.beside = Beside::No;
            state.pressing = Some((row, which));

            let Ok(gone) = acted(state, &rows, row, Selection::Chose);

            gone
        }
        Some(Lands::Seek { row, from, wide }) => {
            state.at = Some(row);
            state.beside = Beside::No;

            let Ok(fraction) = crate::page::Track { from, width: wide }.landed(hit.x);
            let Ok(()) = sought(state, &rows, row, fraction);

            Gone::Staying
        }
        Some(Lands::Tab(which)) => {
            let Ok(()) = stood_on(state, which);

            Gone::Staying
        }
        Some(Lands::More(step)) => {
            let Ok(_turned) = told(state, Meaning::Tab(step), &rows);

            Gone::Staying
        }
        Some(Lands::Back) => {
            state.leaving = Leaving::No;

            let Ok(said) = told(state, Meaning::Close, &rows);
            let Ok(gone) = carried_out(state, &rows, said);

            gone
        }
        None => {
            let Ok(away) = outside(card, hit);

            match (away, &state.asking, &state.sure) {
                (Covers::No, _, _) => Gone::Staying,
                (Covers::Yes, Some(_), _) | (Covers::Yes, None, Some(_)) => {
                    state.asking = None;
                    state.sure = None;

                    Gone::Staying
                }
                (Covers::Yes, None, None) => {
                    let Ok(said) = told(state, Meaning::Close, &rows);
                    let Ok(gone) = carried_out(state, &rows, said);

                    gone
                }
            }
        }
    })
}

fn serve(
    who: &str,
    build: crate::card::Build,
    start: Option<&str>,
    framing: Framing,
    shut: Arc<AtomicBool>,
    reported: Reported,
) -> Result<(), Never> {
    let mut reported = reported;
    let Ok(()) = crate::arrivals::follow();
    let Ok(()) = serving(who, build, start, framing, shut, &mut reported);

    let front = match reported.front {
        Some(front) => front,
        None => return Ok(()),
    };

    match console_onscreen::forget_if_still(&front) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-panel: the tab in front could not be forgotten: {fault}"),
    }

    Ok(())
}

fn built(build: crate::card::Build) -> Result<Vec<Page>, Never> {
    let pages = build();
    let Ok(()) = crate::opening::mark("built");

    Ok(pages)
}

fn first_drawn(rows: &[Row]) -> Result<(), Never> {
    let Ok(running) = crate::opening::running();

    match running {
        crate::opening::Running::Yes => {},
        crate::opening::Running::No => return Ok(()),
    }

    let Ok(many) = fitted::<_, u64>(rows.len());
    let Ok(()) = crate::opening::counted("rows", many);
    let Ok(()) = crate::opening::mark("frame");

    crate::opening::done()
}

struct Reported {
    front: Option<String>,
    first_frame: Option<mpsc::Sender<()>>,
    tells: Option<String>,
}

fn in_front(page: Option<&Page>, reported: &mut Reported) -> Result<(), Never> {
    let page = match page {
        Some(page) => page,
        None => return Ok(()),
    };

    match console_onscreen::saying(&page.title) {
        Ok(()) => reported.front = Some(page.title.clone()),
        Err(fault) => eprintln!("console-panel: the tab in front could not be written down: {fault}"),
    }

    Ok(())
}

fn serving(
    who: &str,
    build: crate::card::Build,
    start: Option<&str>,
    framing: Framing,
    shut: Arc<AtomicBool>,
    reported: &mut Reported,
) -> Result<(), Never> {
    let Ok(()) = crate::whose::named(who);

    let Ok(pages) = built(build);
    let wearing = match Wearing::worn() {
        Ok(wearing) => wearing,
        Err(why) => {
            eprintln!("console-panel: no palette: {why}");

            return Ok(());
        }
    };

    let mut surface = match Surface::connect() {
        Ok(surface) => surface,
        Err(fault) => {
            eprintln!("console-panel: no surface: {fault}");

            return Ok(());
        }
    };

    let Ok(mut state) = opened_on(pages, start, framing);

    let Ok(wanted) = covering();

    match surface.show(&wanted) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-panel: no surface to draw on: {fault}");

            return Ok(());
        }
    }

    let Ok(()) = crate::opening::mark("surface");
    let marks_measured = measure_marks()?;
    let mut drew: Option<Vec<Shape>> = None;
    let Ok(mut front) = InFront::none();
    let mut rows_of: Option<u32> = None;
    let mut asked: Option<Reading> = None;
    let mut stale = Stale::No;
    let mut dirty = Dirty::Yes;
    let mut sized_for: Option<Size<u32>> = None;
    let mut measured_tabs: Vec<MeasuredTab> = Vec::new();
    let mut measured_rows: Vec<MeasuredRow> = Vec::new();
    let mut fits: u32 = 1;
    let mut panelled = Panelled { shapes: Vec::new(), touching: Vec::new(), moving: None };
    let mut finger: Option<Touch> = None;

    loop {
        match shut.load(Ordering::Relaxed) {
            true => {
                break;
            }
            false => {}
        }

        let logical = match surface.logical() {
            Ok(Some(logical)) => logical,
            Ok(None) => {
                let Ok(waking) = crate::frames::waking();
                let also: Vec<std::os::fd::BorrowedFd<'_>> = waking.into_iter().collect();
                let _ = surface.wait(&also, None);
                let Ok(woken) = crate::frames::woken();

                match woken.rows {
                    crate::frames::FrameReceived::Yes => stale = Stale::Yes,
                    crate::frames::FrameReceived::No => {},
                }

                dirty = Dirty::Yes;

                continue;
            }
            Err(_) => return Ok(()),
        };

        let Ok(surface_size) = sized(logical);
        let Ok(card) = card_taking(&mut surface, &surface_size, state.opened, state.framing);
        let card_wide_u32 = fitted::<i32, u32>(card.width)?;

        match sized_for == Some(logical) {
            true => {},
            false => {
                let Ok(spent) = spent();
                let room = strip::room(strip::Card { width: card.width, spent })?;

                let Ok(tabs) = measure_tabs(&state.pages, card_wide_u32);
                let Ok(cell) = widest_tab(&tabs);
                let Ok(fitting_tabs) = strip::fits(room, cell);
                let Ok(rows) = measure_rows(&state.rows, card_wide_u32);

                measured_tabs = tabs;
                fits = fitting_tabs;
                measured_rows = rows;
                sized_for = Some(logical);
                dirty = Dirty::Yes;
            }
        }

        let Ok(page) = nth(&state.pages, state.here);
        let Ok(()) = front.follow(&state, page);

        match rows_of == Some(state.here) {
            true => {},
            false => {
                let Ok(()) = in_front(page, reported);
                let Ok(first) = meanwhile(page);

                state.rows = first;

                let Ok(rows) = measure_rows(&state.rows, card_wide_u32);

                measured_rows = rows;
                rows_of = Some(state.here);
                stale = Stale::Yes;
                dirty = Dirty::Yes;
            }
        }

        match (stale, page) {
            (Stale::Yes, Some(page)) => {
                let Ok(started) = reading(page, state.here);

                asked = Some(started);
                stale = Stale::No;
            }
            (Stale::Yes, None) | (Stale::No, _) => {},
        }

        let arrived = asked.as_ref().map(|reading| {
            let Ok(looked) = first_look(reading, &state.rows, FirstLook { shut: &shut, until: FIRST_LOOK });

            (reading.here, looked)
        });

        let arrived = match arrived {
            Some((here, Looked { heard, woken })) => {
                match woken.map(|woken| woken.rows) {
                    Some(crate::frames::FrameReceived::Yes) => stale = Stale::Yes,
                    Some(crate::frames::FrameReceived::No) | None => {},
                }

                match woken {
                    Some(_what_the_first_look_heard_is_drawn_whole) => dirty = Dirty::Yes,
                    None => {},
                }

                Some((here, heard))
            }
            None => None,
        };

        match arrived {
            Some((here, Ok(read))) => {
                asked = None;

                match here == state.here {
                    true => {
                        state.rows = read;

                        let Ok(()) = asked_for(&state.rows);

                        let Ok(rows) = measure_rows(&state.rows, card_wide_u32);

                        measured_rows = rows;

                        let Ok(many) = fitted::<_, u32>(state.rows.len());

                        match state.at.is_some_and(|at| at >= many) {
                            true => state.at = None,
                            false => {},
                        }

                        dirty = Dirty::Yes;
                    }
                    false => {},
                }
            }
            Some((_, Err(mpsc::TryRecvError::Disconnected))) => asked = None,
            Some((_, Err(mpsc::TryRecvError::Empty))) | None => {},
        }

        match (state.at, state.rows.is_empty()) {
            (None, false) => {
                let Ok(first) = walked(&state.rows, BEFORE_THE_FIRST_ROW, Step(1));
                let Ok(landed) = fitted::<i32, u32>(first.max(0));

                state.at = Some(landed);
            }
            (None, true) | (Some(_), _) => {},
        }

        let Ok(room) = scroll_room(&state, &card);

        match dirty {
            Dirty::No => {},
            Dirty::Yes => {
                match finger.as_ref().map(|finger| finger.dragged) {
                    Some(Dragged::Yes) => {},
                    Some(Dragged::No) | None => {
                        let Ok(down_to) = scrolled(state.at, Viewport { room, y: state.scroll });

                        state.scroll = down_to;
                    }
                }

                let measured = measured_now(&state, &measured_tabs, &measured_rows, marks_measured)?;

                let Ok(drawn) = shapes(&state, &card, fits, &measured, &wearing);

                panelled = drawn;
                state.shown = panelled.moving.clone();

                match drew.as_ref() == Some(&panelled.shapes) {
                    true => {},
                    false => {
                        let _ = surface.resize(logical);

                        let points = Size { width: logical.width, height: logical.height };
                        let drawing = &panelled.shapes;

                        let _ = surface.draw(|pixels, device, _scale| {
                            let frame = Frame { device, points };
                            let _ = painting::onto(pixels, frame, drawing);

                            Ok(())
                        });

                        match reported.first_frame.take() {
                            Some(first_frame) => {
                                let _ = first_frame.send(());
                            }
                            None => {},
                        }

                        let Ok(()) = first_drawn(&state.rows);

                        let Ok(told) = what_it_drew(
                            &state,
                            &panelled,
                            &state.rows,
                            (surface_size.width, surface_size.height),
                        );
                        let Ok(()) = description::wrote(&told, reported.tells.as_deref());

                        drew = Some(panelled.shapes.clone());
                    }
                }

                dirty = Dirty::No;
            }
        }

        let Ok((now_dirty, now_stale)) = waited(&mut surface, &panelled, logical, (dirty, stale));

        (dirty, stale) = (now_dirty, now_stale);

        let pressed = surface.keyboard_events()?;

        'pressed: for event in pressed {
            match event {
                KeyboardEvent::Down { key } => {
                    dirty = Dirty::Yes;

                    let Ok(stirred) = stirred(&state);

                    match stirred {
                        WakeOutcome::Woke => {
                            stale = Stale::Yes;

                            continue 'pressed;
                        },
                        WakeOutcome::AlreadyAwake => {},
                    }

                    let rows = state.rows.clone();
                    let Ok(driving) = driving(&state);
                    let Ok(meaning) = keys::meaning(key, driving);
                    let before = state.at;
                    let Ok(gone) = pressed_here(&mut state, &rows, key, meaning, driving);

                    match gone {
                        Gone::Closing => return Ok(()),
                        Gone::Staying => {},
                    }

                    let Ok(landed) = stepped_onto(meaning, before, &state);

                    match landed {
                        Some(row) => {
                            let Ok(effects) = SoundEffects::chosen();
                            let Ok(root) = Degree::climbing(row);
                            let Ok(cue) = Sound::Step.cue();

                            match effects {
                                SoundEffects::On => match console_sound_effects::play(&cue, root) {
                                    Ok(()) => {},
                                    Err(fault) => eprintln!("console-panel: a step went unheard: {fault}"),
                                },
                                SoundEffects::Off => {},
                            }
                        }
                        None => {},
                    }

                    match meaning {
                        Meaning::Step(_) | Meaning::Tab(_) | Meaning::Close | Meaning::Abandon => {},
                        Meaning::None | Meaning::Choose | Meaning::More | Meaning::Nudge(_) => {
                            stale = Stale::Yes;
                        }
                    }
                }
            }
        }

        let pointer_events = surface.pointer_events()?;

        for event in pointer_events {
            match event {
                PointerEvent::Down { at } => {
                    let Ok(down) = finger_down(&state, &panelled, at);

                    finger = match down {
                        Down::Woke => {
                            stale = Stale::Yes;
                            dirty = Dirty::Yes;

                            None
                        },
                        Down::Touched(held) => Some(held),
                    };
                }
                PointerEvent::Moved { at } => match finger.as_mut() {
                    Some(held) => {
                        let by = at.1 - held.from.1;

                        match (held.dragged, by.abs() >= DRAGGED) {
                            (Dragged::No, false) => {},
                            (Dragged::Yes, _) | (Dragged::No, true) => {
                                held.dragged = Dragged::Yes;

                                match &held.pans {
                                    Some((still, zoom, seen)) => {
                                        let Ok(panned) =
                                            zoom.panned(Point { x: at.0 - held.from.0, y: by }, *seen);

                                        state.zoom = Some((still.clone(), panned));
                                    }
                                    None => {
                                        let Ok(by) = toward_zero_i32(by);
                                        let Ok(()) =
                                            scrolled_to(&mut state, Viewport { room, y: held.scroll.saturating_sub(by) });
                                    }
                                }

                                dirty = Dirty::Yes;
                            }
                        }
                    }
                    None => {
                        let Ok(across_at) = toward_zero_i32(at.0);
                        let Ok(down_at) = toward_zero_i32(at.1);
                        let Ok(lands) = touched(&panelled.touching, Point { x: across_at, y: down_at });

                        match (lands, state.leaving) {
                            (Some(Lands::Row(row)), Leaving::No) => match state.at == Some(row) {
                                true => {},
                                false => {
                                    let Ok(many) = fitted::<_, u32>(state.rows.len());
                                    let Ok(found) = nth(&state.rows, row);
                                    let Ok(heading) = match found {
                                        Some(found) => found.heading(),
                                        None => Ok(Heading::Yes),
                                    };

                                    match (row < many, heading) {
                                        (true, Heading::No) => {
                                            state.at = Some(row);
                                            state.beside = Beside::No;
                                            dirty = Dirty::Yes;
                                        }
                                        (true, Heading::Yes) | (false, _) => {},
                                    }
                                }
                            },
                            (Some(_) | None, _) => {},
                        }
                    }
                },
                PointerEvent::Scrolled { by } => {
                    let Ok(by) = toward_zero_i32(by * f64::from(fitting::ROW) / A_NOTCH);
                    let down_to = state.scroll.saturating_add(by);
                    let Ok(()) = scrolled_to(&mut state, Viewport { room, y: down_to });

                    dirty = Dirty::Yes;
                }
                PointerEvent::Pinched { by } => {
                    match finger.as_mut() {
                        Some(held) => {
                            held.dragged = Dragged::Yes;
                            held.pans = None;
                        }
                        None => {},
                    }

                    let rows = state.rows.clone();
                    let Ok(()) = zoomed_to(&mut state, &rows, |was| match was.times(by) {
                        Ok(zoom) => zoom,
                    });

                    dirty = Dirty::Yes;
                }
                PointerEvent::Up => {
                    let lifted = finger.take();

                    match lifted {
                        Some(Touch { from, dragged: Dragged::No, lands, scroll: _, pans: _ }) => {
                            let Ok(across_at) = toward_zero_i32(from.0);
                            let Ok(down_at) = toward_zero_i32(from.1);
                            let Ok(gone) =
                                tapped(&mut state, lands, Point { x: across_at, y: down_at }, &card);

                            match gone {
                                Gone::Closing => return Ok(()),
                                Gone::Staying => {},
                            }

                            match lands {
                                Some(Lands::Tab(_) | Lands::More(_) | Lands::Back) | None => {},
                                Some(
                                    Lands::Row(_)
                                    | Lands::Answer(_)
                                    | Lands::Nudge { .. }
                                    | Lands::Else(_)
                                    | Lands::ButtonPress { .. }
                                    | Lands::Seek { .. },
                                ) => {
                                    stale = Stale::Yes;
                                }
                            }

                            dirty = Dirty::Yes;
                        }
                        Some(Touch { dragged: Dragged::Yes, .. }) | None => {},
                    }
                }
                PointerEvent::Left => {
                    finger = None;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use console_core_color::palette::{self, Wearing};
    use console_core_never::Never;

    use crate::page::{Aside, Handler, Page, Picture, Row, Rows};
    use console_core_number_conversion::fitted;
    use console_core_shapes::{Clip, Shape};
    use std::time::Duration;
    use super::stage;
    use crate::shape;
    use crate::surface::{HitRegion, Panelled, what_it_drew};

    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    const NOTHING_TAKEN: u32 = u32::MAX;

    use super::{
        beside_the_words, carried_out, driving, measure_rows, measure_tabs, pressed_here, scrolled,
        shapes, stepped_onto, told, typed_into, wanted_of, Beside, Card, Driving, Gone, Keysym, Lands, Measured,
        Meaning, Opened, Path, State, Outcome, Typed, Viewport,
    };

    fn wearing() -> Wearing {
        let mut map = BTreeMap::new();

        map.insert("panel".into(), "1a1a1a".into());
        map.insert("text".into(), "eeeeee".into());
        map.insert("edge".into(), "333333".into());
        map.insert("soft".into(), "999999".into());
        map.insert("coral".into(), "ff6b6b".into());
        map.insert("ground".into(), "111111".into());
        map.insert("fill".into(), "222222".into());
        map.insert("night".into(), "0a0a0a".into());
        map.insert("pink".into(), "ffb0d0".into());

        let Ok(spent_map) = palette::read(&mock_palette(&map));

        Wearing::out_of(&spent_map).expect("wearing")
    }

    fn mock_palette(map: &BTreeMap<String, String>) -> String {
        map.iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn the_handler(arguments: &[&str]) -> Handler {
        let Ok(does) = Handler::run(arguments);

        does
    }

    fn rows_start(card: &Card) -> i32 {
        card.y.saturating_add(crate::fitting::STRIP).saturating_add(super::OVER_ROWS)
    }

    fn highlighted_rows(panelled: &super::Panelled, card: &Card, wearing: &Wearing) -> Vec<i32> {
        panelled
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                console_core_shapes::Shape::Panel(panel) => Some(panel),
                console_core_shapes::Shape::Text(_)
                | console_core_shapes::Shape::Picture(_)
                | console_core_shapes::Shape::Cropped(_)
                | console_core_shapes::Shape::Line(_) | console_core_shapes::Shape::Clip(_) => {
                    None
                }
            })
            .filter(|panel| panel.fill == wearing.pink)
            .filter(|panel| panel.at.y >= rows_start(card))
            .map(|panel| panel.at.y)
            .collect()
    }

    fn said_row(says: &str) -> Row {
        let Ok(row) = Row::said(says, Aside(""));

        row
    }

    fn wide_of(card: &Card) -> u32 {
        let Ok(wide) = console_core_number_conversion::fitted::<i32, u32>(card.width);

        wide
    }

    fn fits_for(card: &Card) -> u32 {
        let Ok(room) = crate::strip::room(crate::strip::Card { width: card.width, spent: 0 });
        let Ok(fits) = crate::strip::fits(room, crate::strip::Cell(80));

        fits
    }

    fn state_of(pages: Vec<Page>) -> State {
        State {
            rows: first_rows(&pages),
            leaving: super::Leaving::No,
            pages,
            here: 0,
            at: None,
            opened: Opened::No,
            from_tab: 0,
            scroll: 0,
            note: None,
            typed: String::new(),
            beside: Beside::No,
            asking: None,
            sure: None,
            pressing: None,
            zoom: None,
            shown: None,
            selected: Vec::new(),
            framing: super::Framing::Card,
        }
    }

    fn page(title: &str, says: &[&str]) -> Page {
        let rows: Vec<Row> = says
            .iter()
            .map(|text| Row::said(text, Aside("")).expect("row"))
            .collect();

        Page::new(title, Rows::Fixed(rows)).expect("page")
    }

    fn measured_for(pages: &[Page], wide: u32) -> Measured {
        let tabs = measure_tabs(pages, wide).expect("tabs");
        let rows = match pages.first() {
            Some(page) => {
                let rows = page.rows.read().expect("rows");

                measure_rows(&rows, wide).expect("rows")
            }
            None => Vec::new(),
        };

        Measured {
            tabs,
            rows,
            marks: super::measure_marks().expect("marks"),
            typed: super::NO_ROOM,
            answers: Vec::new(),
            note: super::NO_ROOM,
        }
    }

    fn card_for(surface_wide: i32, surface_tall: i32) -> Card {
        let Ok(wide) = shape::part_of(surface_wide);
        let Ok(tall) = shape::tall_part_of(surface_tall);
        let Ok(x) = console_core_number_conversion::fitted::<i32, i32>(
            surface_wide.saturating_sub(wide).saturating_div(2),
        );
        let Ok(y) = console_core_number_conversion::fitted::<i32, i32>(
            surface_tall.saturating_sub(tall).saturating_div(2),
        );

        Card { x, y, width: wide, height: tall }
    }

    fn first_rows(pages: &[Page]) -> Vec<Row> {
        match pages.first() {
            Some(page) => page.rows.read().expect("rows"),
            None => Vec::new(),
        }
    }

    fn painted(state: &State, into: &std::path::Path) {
        let Ok(spent) = palette::read(include_str!("../../../files/usr/local/lib/console/palette.sh"));
        let wearing = Wearing::out_of(&spent).expect("the palette the device wears");
        let card = card_for(1024, 640);
        let measured_tabs = measure_tabs(&state.pages, wide_of(&card)).expect("tabs");
        let measured_rows = measure_rows(&state.rows, wide_of(&card)).expect("rows");
        let marks = super::measure_marks().expect("marks");
        let measured = super::measured_now(state, &measured_tabs, &measured_rows, marks).expect("measured");
        let Ok(panelled) = shapes(state, &card, fits_for(&card), &measured, &wearing);
        let device = console_core_geometry::Size { width: 2560, height: 1600 };
        let points = console_core_geometry::Size { width: 1024, height: 640 };
        let mut pixels = vec![0; 2560 * 1600 * 4];

        console_draw_painting::onto(&mut pixels, console_draw_painting::Frame { device, points }, &panelled.shapes)
            .expect("the panel should draw");
        std::fs::write(into, &pixels).expect("the picture");
    }

    #[test]
    fn a_picture_of_a_panel_is_written_when_someone_asks_for_one() {
        let into = match std::env::var("CONSOLE_PANEL_PICTURE") {
            Ok(into) => std::path::PathBuf::from(into),
            Err(_no_one_wants_to_look_at_one) => return,
        };
        let level: crate::page::Level = Arc::new(|_step| {});
        let rows = vec![
            Row::naming("Output", Aside("")).expect("row"),
            Row::said("Volume", Aside("40%")).expect("row").leveled(Arc::clone(&level)).expect("row"),
            Row::said("Speakers", Aside("In use")).expect("row"),
            Row::said("Bluetooth", Aside("")).expect("row").opening().expect("row"),
            Row::said("Ferry.jpg", Aside("2.1 MB")).expect("row").offering(|_showing| false).expect("row"),
        ];
        let tabs = vec![
            Page::new("Sound", Rows::Fixed(rows.clone())).expect("page"),
            Page::new("Network", Rows::Fixed(Vec::new())).expect("page"),
            Page::new("Display", Rows::Fixed(Vec::new())).expect("page"),
        ];
        let mut state = state_of(tabs);

        state.at = Some(1);
        painted(&state, &into.join("tabs.bgra"));

        let single = vec![Page::new("Downloads", Rows::Fixed(rows.clone())).expect("page")];
        let mut state = state_of(single);

        state.at = Some(4);
        state.beside = Beside::Yes;
        painted(&state, &into.join("single.bgra"));

        let sought = Page::new("Menu", Rows::Fixed(rows)).expect("page");
        let sought = sought.searching("Type to narrow the list", |_showing, _word| {}).expect("page");
        let mut state = state_of(vec![sought]);

        state.typed = "fir".to_string();
        state.at = Some(2);
        painted(&state, &into.join("search.bgra"));
    }

    #[test]
    fn every_visible_row_gets_a_touching_entry() {
        let pages = vec![page("test", &["alpha", "bravo", "charlie"])];
        let wearing = wearing();
        let card = card_for(1024, 640);
        let measured = measured_for(&pages, card.width as u32);
        let fits = crate::strip::fits(
            crate::strip::room(crate::strip::Card { width: card.width, spent: 0 }).expect("room"),
            crate::strip::Cell(80),
        )
        .expect("fits");

        let state = State {
            rows: first_rows(&pages),
            leaving: super::Leaving::No,
            pages,
            here: 0,
            at: None,
            opened: Opened::No,
            from_tab: 0,
            scroll: 0,
            note: None,
            typed: String::new(),
            beside: Beside::No,
            asking: None,
            sure: None,
            pressing: None,
            zoom: None,
            shown: None,
            selected: Vec::new(),
            framing: super::Framing::Card,
        };

        let panelled = shapes(
            &state,
            &card,
            fits,
            &measured,
            &wearing,
        )
        .expect("shapes");

        let Ok(rows) = landing_on_rows(&panelled);

        assert_eq!(rows.len(), 3, "one touching per row");
    }

    #[test]
    fn a_row_scrolled_half_out_of_view_is_drawn_and_cut_at_the_edge() {
        let says: Vec<String> = (0..30).map(|n| format!("row {n}")).collect();
        let said: Vec<&str> = says.iter().map(String::as_str).collect();
        let pages = vec![page("test", &said)];
        let wearing = wearing();
        let card = card_for(1024, 640);
        let measured = measured_for(&pages, wide_of(&card));
        let mut state = state_of(pages);

        state.scroll = crate::fitting::ROW / 2;

        let panelled = shapes(&state, &card, fits_for(&card), &measured, &wearing).expect("shapes");
        let drawn: Vec<&str> = panelled
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Text(words) => Some(words.said.as_str()),
                Shape::Panel(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
            })
            .collect();

        assert!(drawn.contains(&"row 0"), "the first row is half in view and was not drawn: {drawn:?}");

        let clips: Vec<&Clip> = panelled
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Clip(clip) => Some(clip),
                Shape::Panel(_) | Shape::Text(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) => None,
            })
            .collect();

        let first_top = clips.first().and_then(|clip| match clip {
            Clip::To { at, .. } => Some(at.y),
            Clip::Lifted => None,
        });

        assert_eq!(first_top, Some(rows_start(&card)), "the rows are cut to the list: {clips:?}");
        assert_eq!(clips.last(), Some(&&Clip::Lifted), "the clip is lifted before whatever stands over the rows");

        let Ok(rows) = landing_on_rows(&panelled);
        let card_bottom = card.y + card.height;

        for row in &rows {
            let top = row.panel.at.y;
            let bottom = top + row.panel.size.height as i32;

            assert!(top >= rows_start(&card) && bottom <= card_bottom, "a row cut by the edge answers a finger: {top}..{bottom}");
        }

        let Ok(rows_drawn) = fitted::<_, u32>(drawn.iter().filter(|said| said.starts_with("row ")).count());
        let Ok(rows_touched) = fitted::<_, u32>(rows.len());

        assert_eq!(rows_drawn, rows_touched + 2, "the rows cut at the top and at the bottom are both drawn: {drawn:?}");
    }

    #[test]
    fn rows_are_placed_at_expected_positions() {
        let pages = vec![page("test", &["one", "two", "three"])];
        let wearing = wearing();
        let card = card_for(1024, 640);
        let measured = measured_for(&pages, card.width as u32);
        let fits = crate::strip::fits(
            crate::strip::room(crate::strip::Card { width: card.width, spent: 0 }).expect("room"),
            crate::strip::Cell(80),
        )
        .expect("fits");

        let strip_tall = crate::fitting::STRIP;
        let rows_start = card.y + strip_tall + 10;
        let row_tall = crate::fitting::ROW;

        let state = State {
            rows: first_rows(&pages),
            leaving: super::Leaving::No,
            pages,
            here: 0,
            at: None,
            opened: Opened::No,
            from_tab: 0,
            scroll: 0,
            note: None,
            typed: String::new(),
            beside: Beside::No,
            asking: None,
            sure: None,
            pressing: None,
            zoom: None,
            shown: None,
            selected: Vec::new(),
            framing: super::Framing::Card,
        };

        let panelled = shapes(
            &state,
            &card,
            fits,
            &measured,
            &wearing,
        )
        .expect("shapes");

        let Ok(rows) = landing_on_rows(&panelled);

        for (index, touching) in rows.iter().enumerate() {
            let expected_y = rows_start + console_core_number_conversion::fitted::<_, i32>(index).expect("an index").saturating_mul(row_tall);

            assert_eq!(
                touching.panel.at.y, expected_y,
                "row {index} should be at y={expected_y}"
            );
        }
    }

    #[test]
    fn highlight_selects_the_right_row() {
        let pages = vec![page("test", &["a", "b", "c"])];
        let wearing = wearing();
        let card = card_for(1024, 640);
        let measured = measured_for(&pages, card.width as u32);
        let fits = crate::strip::fits(
            crate::strip::room(crate::strip::Card { width: card.width, spent: 0 }).expect("room"),
            crate::strip::Cell(80),
        )
        .expect("fits");

        let state = State {
            rows: first_rows(&pages),
            leaving: super::Leaving::No,
            pages,
            here: 0,
            at: Some(1),
            opened: Opened::No,
            from_tab: 0,
            scroll: 0,
            note: None,
            typed: String::new(),
            beside: Beside::No,
            asking: None,
            sure: None,
            pressing: None,
            zoom: None,
            shown: None,
            selected: Vec::new(),
            framing: super::Framing::Card,
        };

        let panelled = shapes(
            &state,
            &card,
            fits,
            &measured,
            &wearing,
        )
        .expect("shapes");

        let highlights = highlighted_rows(&panelled, &card, &wearing);

        assert_eq!(highlights.len(), 1, "exactly one row is standing under the highlight");

        let Some(under_the_second_row) = highlights.first() else {
            panic!("the highlight was counted and then could not be found")
        };

        assert_eq!(
            *under_the_second_row,
            rows_start(&card)
                .saturating_add(crate::fitting::ROW)
                .saturating_add(super::BETWEEN_ROWS.saturating_div(2)),
            "the highlight is behind the row the pad is standing on"
        );
    }

    #[test]
    fn no_highlight_when_at_is_none() {
        let pages = vec![page("test", &["x", "y"])];
        let wearing = wearing();
        let card = card_for(1024, 640);
        let measured = measured_for(&pages, card.width as u32);
        let fits = crate::strip::fits(
            crate::strip::room(crate::strip::Card { width: card.width, spent: 0 }).expect("room"),
            crate::strip::Cell(80),
        )
        .expect("fits");

        let state = State {
            rows: first_rows(&pages),
            leaving: super::Leaving::No,
            pages,
            here: 0,
            at: None,
            opened: Opened::No,
            from_tab: 0,
            scroll: 0,
            note: None,
            typed: String::new(),
            beside: Beside::No,
            asking: None,
            sure: None,
            pressing: None,
            zoom: None,
            shown: None,
            selected: Vec::new(),
            framing: super::Framing::Card,
        };

        let panelled = shapes(
            &state,
            &card,
            fits,
            &measured,
            &wearing,
        )
        .expect("shapes");

        assert_eq!(
            highlighted_rows(&panelled, &card, &wearing).len(),
            0,
            "nothing is standing under a highlight until something is standing on a row"
        );
    }

    #[test]
    fn the_dpad_lands_on_a_row_that_does_something_and_steps_over_the_heading_over_it() {
        let Ok(heading) = Row::nothing("Sound");
        let Ok(first) = Row::new("Volume", Aside(""), the_handler(&["console-volume"]));
        let Ok(second) = Row::new("Balance", Aside(""), the_handler(&["console-balance"]));
        let rows = vec![heading, first, second];
        let Ok(page) = Page::new("Settings", Rows::Fixed(rows.clone()));
        let mut state = state_of(vec![page]);

        let Ok(down) = told(&mut state, Meaning::Step(1), &rows);

        assert_eq!(down, Outcome::Redrawn);
        assert_eq!(state.at, Some(1), "the first press skips the heading");

        let Ok(_again) = told(&mut state, Meaning::Step(1), &rows);

        assert_eq!(state.at, Some(2));

        let Ok(_at_the_end) = told(&mut state, Meaning::Step(1), &rows);

        assert_eq!(state.at, Some(2), "the list does not wrap past its last row");
    }

    #[test]
    fn a_step_is_heard_only_when_it_lands_somewhere_else() {
        let Ok(first) = Row::new("Volume", Aside(""), the_handler(&["console-volume"]));
        let Ok(second) = Row::new("Balance", Aside(""), the_handler(&["console-balance"]));
        let rows = vec![first, second];
        let Ok(page) = Page::new("Settings", Rows::Fixed(rows.clone()));
        let mut state = state_of(vec![page]);

        let before = state.at;
        let Ok(_down) = told(&mut state, Meaning::Step(1), &rows);

        assert_eq!(stepped_onto(Meaning::Step(1), before, &state), Ok(Some(0)));

        let before = state.at;
        let Ok(_down) = told(&mut state, Meaning::Step(1), &rows);

        assert_eq!(stepped_onto(Meaning::Step(1), before, &state), Ok(Some(1)));

        let before = state.at;
        let Ok(_at_the_end) = told(&mut state, Meaning::Step(1), &rows);

        assert_eq!(stepped_onto(Meaning::Step(1), before, &state), Ok(None), "the end of the list is silent");
        assert_eq!(stepped_onto(Meaning::Tab(1), Some(0), &state), Ok(None), "a tab is not a step");
    }

    #[test]
    fn a_is_the_row_doing_what_it_says_it_does() {
        let pressed = Arc::new(AtomicBool::new(false));
        let told_it_was = Arc::clone(&pressed);
        let Ok(does) =
            Handler::and_stay(move |_showing| told_it_was.store(true, Ordering::Relaxed));
        let Ok(row) = Row::new("Rename", Aside(""), does);
        let rows = vec![row];
        let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
        let mut state = state_of(vec![page]);

        let Ok(_down) = told(&mut state, Meaning::Step(1), &rows);
        let Ok(chose) = told(&mut state, Meaning::Choose, &rows);

        assert_eq!(chose, Outcome::Chose(0));

        let Ok(gone) = carried_out(&mut state, &rows, chose);

        assert_eq!(gone, Gone::Staying, "a row that stays leaves the panel up");
        assert!(pressed.load(Ordering::Relaxed), "the row was chosen and nothing ran");
    }

    #[test]
    fn what_a_row_asks_for_while_it_runs_is_what_the_panel_does_next() {
        let Ok(does) = Handler::and_stay(|showing| {
            showing.note("Copied");
            showing.turn_to(1);
        });
        let Ok(row) = Row::new("Copy", Aside(""), does);
        let rows = vec![row];
        let Ok(here) = Page::new("Files", Rows::Fixed(rows.clone()));
        let Ok(there) = Page::new("Places", Rows::Fixed(Vec::new()));
        let mut state = state_of(vec![here, there]);

        let Ok(_gone) = carried_out(&mut state, &rows, Outcome::Chose(0));

        assert_eq!(state.note.as_deref(), Some("Copied"));
        assert_eq!(state.here, 1, "the row asked for the tab beside it");
    }

    #[test]
    fn b_over_a_picture_puts_the_picture_away_and_not_the_panel() {
        let mut state = state_of(vec![page("One", &["a"])]);

        state.opened = Opened::Expanded;

        let Ok(back) = told(&mut state, Meaning::Close, &[]);

        assert_eq!(back, Outcome::Redrawn);
        assert_eq!(state.opened, Opened::No);

        let Ok(away) = told(&mut state, Meaning::Close, &[]);

        assert_eq!(away, Outcome::Closing, "the second press is the panel's");
    }

    #[test]
    fn b_puts_the_panel_away_and_the_shoulders_turn_the_page() {
        let mut state = state_of(vec![page("One", &["a"]), page("Two", &["b"])]);

        let Ok(along) = told(&mut state, Meaning::Tab(1), &[]);

        assert_eq!(along, Outcome::Redrawn);
        assert_eq!(state.here, 1);

        let Ok(away) = told(&mut state, Meaning::Close, &[]);

        assert_eq!(away, Outcome::Closing);
    }

    #[test]
    fn a_highlight_under_the_card_brings_the_list_up_to_it() {
        let room = crate::fitting::ROW.saturating_mul(4);

        let top = Viewport { room, y: 0 };

        assert_eq!(scrolled(None, top), Ok(0));
        assert_eq!(scrolled(Some(2), top), Ok(0), "a row already in the room does not move it");
        assert_eq!(
            scrolled(Some(5), top),
            Ok(crate::fitting::ROW.saturating_mul(2)),
            "the sixth row of four is two rows down"
        );
        assert_eq!(
            scrolled(Some(1), Viewport { room, y: crate::fitting::ROW.saturating_mul(3) }),
            Ok(crate::fitting::ROW),
            "stepping back up brings the row to the top"
        );
    }

    #[test]
    fn a_letter_goes_into_the_line_and_a_backspace_takes_it_out_again() {
        let mut typed = String::new();

        assert_eq!(typed_into(&mut typed, Keysym::a), Ok(Typed::Edited));
        assert_eq!(typed_into(&mut typed, Keysym::b), Ok(Typed::Edited));
        assert_eq!(typed, "ab");

        assert_eq!(typed_into(&mut typed, Keysym::BackSpace), Ok(Typed::Edited));
        assert_eq!(typed, "a");

        assert_eq!(typed_into(&mut typed, Keysym::Return), Ok(Typed::None));
        assert_eq!(typed_into(&mut typed, Keysym::Down), Ok(Typed::None));
        assert_eq!(typed, "a", "a key that means something to the panel is not a letter");

        let Ok(nothing) = typed_into(&mut String::new(), Keysym::BackSpace);

        assert_eq!(nothing, Typed::None, "nothing to take out of an empty line");
    }

    #[test]
    fn a_page_that_can_be_searched_is_driven_as_a_search_line() {
        let Ok(plain) = Page::new("Settings", Rows::Fixed(Vec::new()));
        let Ok(sought) = Page::new("Menu", Rows::Fixed(Vec::new()));
        let Ok(sought) = sought.searching("Type to narrow the list", |_showing, _word| {});

        assert_eq!(driving(&state_of(vec![plain])), Ok(Driving::Panel));
        assert_eq!(driving(&state_of(vec![sought])), Ok(Driving::Search));
    }

    #[test]
    fn a_picture_being_looked_at_is_drawn_across_the_card_with_its_rows_still_under_it() {
        let _turn = crate::frames::ONE_TEST_AT_A_TIME.lock();
        let at = std::env::temp_dir().join(format!("console-panel-looked-at-{}.png", std::process::id()));
        let Ok(mut making) = console_core_external_programs::Program::Ffmpeg.command();
        let made = making
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=red:s=400x300", "-frames:v", "1"])
            .arg(&at)
            .status();

        assert!(made.is_ok_and(|how| how.success()), "ffmpeg made no picture to look at");

        let Ok(looked) = Row::showing(Picture::Showing(Some(at.clone())));
        let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![looked, said_row("red.png"), said_row("1 of 3")]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);
        let patience = console_waiting::Schedule::of(Duration::from_secs(20)).expect("a patience");

        let Ok(drawn) = console_waiting::found(patience, || {
            let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);

            Ok(panelled.shapes.iter().find_map(|shape| match shape {
                Shape::Picture(picture) => Some(picture.size),
                Shape::Panel(_) | Shape::Text(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
            }))
        });
        let _ = std::fs::remove_file(&at);

        let drawn = drawn.expect("the picture being looked at was never drawn");
        let Ok(row_tall) = fitted::<i32, u32>(crate::fitting::ROW);

        assert!(drawn.height > row_tall.saturating_mul(3), "a picture looked at is drawn the size of a row: {drawn:?}");
        assert!(drawn.width.saturating_mul(3).abs_diff(drawn.height.saturating_mul(4)) <= 4, "the picture is stretched: {drawn:?}");

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let Ok(rows) = landing_on_rows(&panelled);
        let bottom = card.y.saturating_add(card.height);

        assert_eq!(rows.len(), 3, "every row under the picture is still on the card");

        for row in rows {
            let Ok(tall) = fitted::<u32, i32>(row.panel.size.height);

            assert!(row.panel.at.y.saturating_add(tall) <= bottom, "a row fell off the card: {row:?}");
        }
    }

    #[test]
    fn a_film_on_the_card_says_where_its_frames_go_and_a_frame_of_any_shape_fits_there() {
        let _turn = crate::frames::ONE_TEST_AT_A_TIME.lock();
        let Ok(playing) = Row::showing(Picture::Playing(Some("/nowhere/a-film.mkv".into())));
        let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![playing, said_row("a-film.mkv")]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let moving = panelled.moving.expect("a film on the card says where its frames go");
        let Ok(row_tall) = fitted::<i32, u32>(crate::fitting::ROW);

        assert!(moving.room.height > row_tall.saturating_mul(3), "{moving:?}");

        for (wide, tall) in [(1920, 1080), (1080, 1920), (16, 16)] {
            let pixels = console_core_shapes::Pixels { width: wide, height: tall, stride: 4, bytes: std::sync::Arc::new(Vec::new()) };
            let Ok(picture) = super::placed(&moving, pixels);
            let Ok(room_right) = fitted::<u32, i32>(moving.room.width);
            let Ok(room_bottom) = fitted::<u32, i32>(moving.room.height);
            let Ok(right) = fitted::<u32, i32>(picture.size.width);
            let Ok(bottom) = fitted::<u32, i32>(picture.size.height);

            assert!(picture.at.x >= moving.at.x && picture.at.y >= moving.at.y, "{picture:?}");
            assert!(picture.at.x + right <= moving.at.x + room_right, "{picture:?} spills out of {moving:?}");
            assert!(picture.at.y + bottom <= moving.at.y + room_bottom, "{picture:?} spills out of {moving:?}");
            assert!(
                (u64::from(picture.size.width) * u64::from(tall)).abs_diff(u64::from(picture.size.height) * u64::from(wide))
                    <= u64::from(wide.max(tall)),
                "{wide}x{tall} drawn {:?}",
                picture.size
            );
        }
    }

    #[test]
    fn only_a_page_that_opens_on_a_picture_gives_its_first_row_the_room() {
        let Ok(looked) = Row::showing(Picture::Showing(None));
        let Ok(playing) = Row::showing(Picture::Playing(None));
        let room = crate::fitting::ROW.saturating_mul(10);

        assert_eq!(stage(&[looked, said_row("a"), said_row("b")], room), Ok(crate::fitting::ROW.saturating_mul(7)));
        assert_eq!(stage(&[playing], room), Ok(crate::fitting::ROW.saturating_mul(9)));
        assert_eq!(stage(&[said_row("a"), said_row("b")], room), Ok(0));
        assert_eq!(stage(&[], room), Ok(0));
    }

    #[test]
    fn the_line_to_type_in_stands_over_the_rows_and_pushes_them_down() {
        let Ok(page) = Page::new("Menu", Rows::Fixed(vec![said_row("Aether")]));
        let Ok(page) = page.searching("Type to narrow the list", |_showing, _word| {});
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);

        let Ok(rows) = landing_on_rows(&panelled);

        let Some(first) = rows.first() else {
            panic!("a page with one row drew no row")
        };

        assert_eq!(
            first.panel.at.y,
            rows_start(&card).saturating_add(crate::fitting::ROW),
            "the rows begin a line below where they would without one to type in"
        );
    }

    #[test]
    fn a_row_of_cells_draws_every_cell_and_lights_the_one_that_is_now() {
        let Ok(before) = crate::page::Cell::new("8", crate::page::Active::No);
        let Ok(today) = crate::page::Cell::new("9", crate::page::Active::Yes);
        let Ok(row) = Row::celled(vec![before, today]);
        let Ok(page) = Page::new("Calendar", Rows::Fixed(vec![row]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let said: Vec<&super::Text> = panelled.shapes.iter().filter_map(|shape| match shape {
            super::Shape::Text(text) => Some(text),
            super::Shape::Panel(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => None,
        }).collect();
        let eight = said.iter().find(|text| text.said == "8").expect("the eighth was not drawn");
        let nine = said.iter().find(|text| text.said == "9").expect("the ninth was not drawn");

        assert!(eight.at.x < nine.at.x, "the cells stand in the order they were given");
        assert_eq!(nine.ink, wearing.night, "today is not lit");
        assert_eq!(eight.ink, wearing.text);
    }

    fn transport(taken: &Arc<std::sync::atomic::AtomicU32>) -> Row {
        let presses: Vec<crate::page::ButtonPress> = [crate::icons::Icon::Previous, crate::icons::Icon::Play, crate::icons::Icon::Next]
            .into_iter()
            .zip(0u32..)
            .map(|(icon, at)| {
                let taken = Arc::clone(taken);
                let Ok(press) = crate::page::ButtonPress::new(icon, crate::page::Active::No, move |_| {
                    taken.store(at, std::sync::atomic::Ordering::SeqCst);
                });

                press
            })
            .collect();
        let Ok(row) = Row::pressing(presses, 1);

        row
    }

    #[test]
    fn a_strip_of_buttons_draws_each_one_where_a_finger_can_press_it() {
        let taken = Arc::new(std::sync::atomic::AtomicU32::new(9));
        let rows = vec![transport(&taken)];
        let Ok(page) = Page::new("Viewing", Rows::Fixed(rows.clone()));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let mut state = state_of(pages);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let presses: Vec<&HitRegion> =
            panelled.touching.iter().filter(|touching| matches!(touching.lands, Lands::ButtonPress { .. })).collect();

        assert_eq!(presses.len(), 3, "a transport with three buttons drew {} of them", presses.len());
        assert!(presses.windows(2).all(|two| two[0].panel.at.x < two[1].panel.at.x), "the buttons stand in order");

        let Ok(play) = crate::icons::Icon::Play.glyph();
        let drew_play = panelled.shapes.iter().any(|shape| match shape {
            super::Shape::Text(text) => text.said == play,
            super::Shape::Panel(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => false,
        });

        assert!(drew_play, "nothing on the glass says where play is");

        let Some(next) = presses.get(2) else { panic!("no third button") };
        let Ok(gone) = super::tapped(&mut state, Some(next.lands), next.panel.at, &card);

        assert_eq!(gone, Gone::Staying);
        assert_eq!(taken.load(std::sync::atomic::Ordering::SeqCst), 2, "a tap on next pressed something else");
    }

    #[test]
    fn a_strip_of_buttons_stands_on_the_card_with_no_row_behind_it() {
        let taken = Arc::new(std::sync::atomic::AtomicU32::new(9));
        let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![transport(&taken)]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let mut state = state_of(pages);

        state.at = Some(0);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let Ok(button_wide) = fitted::<i32, u32>(super::PRESS_WIDE);
        let behind: Vec<&super::ShapePanel> = panelled
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                super::Shape::Panel(panel) => Some(panel),
                super::Shape::Text(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => None,
            })
            .filter(|panel| panel.fill == wearing.ground || panel.fill == wearing.pink)
            .filter(|panel| panel.size.width > button_wide)
            .collect();

        assert!(behind.is_empty(), "the buttons sit on a row: {:?}", behind.iter().map(|panel| panel.size.width).collect::<Vec<_>>());
    }

    fn keypad_page() -> Page {
        let Ok(number) = Row::headline(
            Picture::None,
            crate::page::Headline { title: "12 +".to_string(), big: "34".to_string(), alignment: crate::page::Alignment::Trailing, ..crate::page::Headline::default() },
        );
        let mut rows = vec![number];

        for _ in 0..5 {
            let presses: Vec<crate::page::ButtonPress> = ["7", "8", "9", "+"]
                .into_iter()
                .map(|says| {
                    let Ok(press) = crate::page::ButtonPress::written(says, crate::page::Active::No, |_| ());

                    press
                })
                .collect();
            let Ok(row) = Row::pressing(presses, 0);

            rows.push(row);
        }

        let Ok(page) = Page::new("Calculator", Rows::Fixed(rows));

        page
    }

    #[test]
    fn a_row_of_written_keys_fills_the_card_from_edge_to_edge() {
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![keypad_page()];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);
        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let keys: Vec<&super::ShapePanel> = panelled
            .touching
            .iter()
            .filter(|region| matches!(region.lands, super::Lands::ButtonPress { row: 1, which: _ }))
            .map(|region| &region.panel)
            .collect();
        let Ok(left) = super::inside_left(&card);
        let Ok(right) = super::inside_right(&card);
        let Some((first, last)) = keys.first().zip(keys.last()) else {
            panic!("the keypad drew no keys");
        };
        let Ok(last_wide) = fitted::<u32, i32>(last.size.width);
        let reaches = last.at.x.saturating_add(last_wide);

        assert_eq!(keys.len(), 4);
        assert!(first.at.x.saturating_sub(left).abs() <= 4, "the first key stands at {}, off the edge at {left}", first.at.x);
        assert!(right.saturating_sub(reaches).abs() <= 4, "the last key ends at {reaches}, short of the edge at {right}");
    }

    #[test]
    fn a_trailing_headline_draws_its_number_at_the_right_and_under_the_title_strip() {
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![keypad_page()];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);
        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let Some(number) = panelled.shapes.iter().find_map(|shape| match shape {
            super::Shape::Text(text) => match text.said == "34" {
                true => Some(text),
                false => None,
            },
            super::Shape::Panel(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => None,
        }) else {
            panic!("the number was not drawn");
        };
        let Some(key) = panelled.touching.iter().find(|region| matches!(region.lands, super::Lands::ButtonPress { row: 1, which: 0 })) else {
            panic!("the keypad drew no keys");
        };
        let Ok(right) = super::inside_right(&card);
        let Ok(number_wide) = fitted::<u32, i32>(number.width);
        let big = number.font.height;
        let Ok(big_tall) = fitted::<u32, i32>(big);

        assert!(number.at.x > right.saturating_div(2), "the number stands at {} rather than at the right", number.at.x);
        assert!(number.at.x.saturating_add(number_wide) <= right.saturating_add(4), "the number runs past the edge");
        assert!(number.at.y.saturating_add(big_tall) <= key.panel.at.y, "the number runs into the keys");
        assert!(number.at.y >= card.y.saturating_add(super::strip::EDGE), "the number rises out of the card");
        assert!(big > super::BIG, "the number is drawn at {big}, no larger than a headline with a title beside it");
    }

    #[test]
    fn left_and_right_walk_a_strip_of_buttons_and_a_press_takes_the_one_stood_on() {
        let taken = Arc::new(std::sync::atomic::AtomicU32::new(9));
        let rows = vec![transport(&taken)];
        let Ok(page) = Page::new("Viewing", Rows::Fixed(rows.clone()));
        let mut state = state_of(vec![page]);

        state.at = Some(0);

        let _ = pressed(&mut state, &rows, Keysym::Return);

        assert_eq!(taken.load(std::sync::atomic::Ordering::SeqCst), 1, "A before moving is the button the program stood on");

        let _ = pressed(&mut state, &rows, Keysym::Left);
        let _ = pressed(&mut state, &rows, Keysym::Left);
        let _ = pressed(&mut state, &rows, Keysym::Return);

        assert_eq!(taken.load(std::sync::atomic::Ordering::SeqCst), 0, "left twice from play is previous, and no further");

        let _ = pressed(&mut state, &rows, Keysym::Right);
        let _ = pressed(&mut state, &rows, Keysym::Right);
        let _ = pressed(&mut state, &rows, Keysym::Return);

        assert_eq!(taken.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn a_finger_on_a_card_that_has_gone_quiet_wakes_it_rather_than_pressing_what_is_under_it() {
        let Ok(row) = Row::said("a.png", Aside(""));
        let Ok(asleep) = Page::new("Viewing", Rows::Fixed(vec![row.clone()])).and_then(|page| page.stirring(|| crate::page::WakeOutcome::Woke));
        let Ok(awake) = Page::new("Viewing", Rows::Fixed(vec![row])).and_then(|page| page.stirring(|| crate::page::WakeOutcome::AlreadyAwake));
        let nothing = super::Panelled { shapes: Vec::new(), touching: Vec::new(), moving: None };

        let Ok(woke) = super::finger_down(&state_of(vec![asleep]), &nothing, (10.0, 10.0));
        let Ok(held) = super::finger_down(&state_of(vec![awake]), &nothing, (10.0, 10.0));

        assert!(matches!(woke, super::Down::Woke), "a tap on a quiet card pressed what was under it and the controls never came back");
        assert!(matches!(held, super::Down::Touched(_)), "a tap on an awake card did not press");
    }

    #[test]
    fn a_press_put_in_the_corner_stands_at_the_far_end_and_the_rest_stay_in_the_middle() {
        let Ok(a) = crate::page::ButtonPress::new(crate::icons::Icon::ZoomOut, crate::page::Active::No, |_| {});
        let Ok(b) = crate::page::ButtonPress::new(crate::icons::Icon::ZoomIn, crate::page::Active::No, |_| {});
        let Ok(whole) = crate::page::ButtonPress::new(crate::icons::Icon::FullScreen, crate::page::Active::No, |_| {});
        let Ok(whole) = whole.cornered();
        let across = crate::page::Across { presses: vec![a, b, whole], at: 0 };
        let measured = super::MeasuredRow { says: super::NO_ROOM, aside: super::NO_ROOM, less: super::NO_ROOM, more: super::NO_ROOM, cells: Vec::new(), presses: Vec::new() };
        let strip = super::ButtonStrip {
            band: super::Band { top: 0, height: 60 },
            span: super::Span { from: 0, to: 1000 },
            standing: super::Standing {
                at: 0,
                highlight: super::Highlight::No,
                beside: Beside::No,
                press: 0,
                opened: Opened::No,
                zoom: crate::zoom::Zoom::default(),
                selected: super::Selected::No,
            },
            lit: super::Lit::No,
        };
        let Ok((_, touching)) = super::press_shapes(&across, &measured, strip, &wearing());
        let far = |which: u32| {
            touching
                .iter()
                .find(|region| region.lands == super::Lands::ButtonPress { row: 0, which })
                .map(|region| region.panel.at.x + i32::try_from(region.panel.size.width).expect("a width"))
                .expect("the press was drawn")
        };

        assert_eq!(far(2), 1000, "the corner press is not at the end of the row");
        assert!(far(1) < 600, "the presses in the middle made room for the one in the corner");
        assert!(far(0) > 400, "the presses in the middle were pushed to the start");
    }

    #[test]
    fn an_app_is_the_whole_screen_and_a_panel_is_a_card_on_it() {
        let screen = super::SurfaceSize { width: 1024, height: 640 };
        let Ok(app) = super::card_on(&screen, Opened::No, super::Framing::Screen);
        let Ok(panel) = super::card_on(&screen, Opened::No, super::Framing::Card);

        assert_eq!(app, Card { x: 0, y: 0, width: 1024, height: 640 }, "an app left a margin round itself, which is a panel");
        assert!(panel.width < 1024 && panel.height < 640, "a panel took the whole screen, which is an app");
    }

    #[test]
    fn opened_out_is_the_whole_screen_and_the_picture_runs_to_its_edges() {
        let screen = super::SurfaceSize { width: 1024, height: 640 };
        let Ok(whole) = super::card_on(&screen, Opened::Expanded, super::Framing::Card);
        let Ok(card) = super::card_on(&screen, Opened::No, super::Framing::Card);

        assert_eq!(whole, Card { x: 0, y: 0, width: 1024, height: 640 }, "opened out left a margin round the card");
        assert!(card.width < 1024 && card.height < 640, "a card that is not opened out is still a card");

        let Ok(row) = Row::showing(Picture::Playing(Some("/nowhere/a-film.mkv".into())));
        let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![row]));
        let wearing = wearing();
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&whole));
        let fits = fits_for(&whole);
        let mut state = state_of(pages);

        state.opened = Opened::Expanded;
        state.at = Some(0);

        let Ok(panelled) = shapes(&state, &whole, fits, &measured, &wearing);
        let Some(moving) = panelled.moving else { panic!("the film has nowhere to be drawn") };

        assert_eq!(moving.at, super::Point { x: 0, y: 0 }, "the picture does not start at the corner of the screen");
        assert_eq!(moving.room.width, 1024, "the picture is not given the width of the screen");

        let lit = panelled.shapes.iter().any(|shape| match shape {
            super::Shape::Panel(panel) => panel.fill == wearing.pink,
            super::Shape::Text(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => false,
        });

        assert!(!lit, "something pink stands round a picture opened out");
    }

    fn a_folder_that_selects(acted_on: Arc<std::sync::Mutex<Vec<String>>>, opened: Arc<std::sync::Mutex<u32>>) -> State {
        let rows: Vec<Row> = ["beach.jpg", "dune.jpg", "sea.jpg"]
            .iter()
            .map(|name| {
                let opened = Arc::clone(&opened);
                let Ok(opens) = Handler::and_stay(move |_| {
                    let Ok(mut count) = opened.lock() else { return };

                    *count = count.saturating_add(1);
                });
                let Ok(row) = Row::new(name, crate::page::Aside(""), opens);
                let Ok(row) = row.selectable(name);

                row
            })
            .collect();
        let Ok(page) = Page::new("Pictures", Rows::Fixed(rows));
        let Ok(page) = page.selecting("Delete", move |_, keys| {
            let Ok(mut acted) = acted_on.lock() else { return };

            *acted = keys.to_vec();
        });

        state_of(vec![page])
    }

    #[test]
    fn once_something_is_selected_a_press_marks_rather_than_opens_and_y_acts_on_every_mark() {
        let acted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let opened = Arc::new(std::sync::Mutex::new(0));
        let mut state = a_folder_that_selects(Arc::clone(&acted), Arc::clone(&opened));
        let rows = state.rows.clone();

        let Ok(_opens) = carried_out(&mut state, &rows, Outcome::Chose(0));

        assert_eq!(opened.lock().map(|count| *count).ok(), Some(1), "with nothing selected a press opens");

        use crate::page::Showing as _;
        let Ok(front) = super::Front::over(&state);

        front.select(vec!["beach.jpg".to_string()]);

        let Ok(()) = front.onto(&mut state);
        let Ok(_marks) = carried_out(&mut state, &rows, Outcome::Chose(2));
        let Ok(_marks) = carried_out(&mut state, &rows, Outcome::Chose(1));
        let Ok(_unmarks) = carried_out(&mut state, &rows, Outcome::Chose(1));

        assert_eq!(opened.lock().map(|count| *count).ok(), Some(1), "a press while selecting opened the thing");
        assert_eq!(state.selected, vec!["beach.jpg".to_string(), "sea.jpg".to_string()]);

        let Ok(_asks) = carried_out(&mut state, &rows, Outcome::Else(0));
        let Ok(()) = super::took(&mut state, 1);

        assert_eq!(acted.lock().map(|keys| keys.clone()).ok(), Some(vec!["beach.jpg".to_string(), "sea.jpg".to_string()]));
        assert!(state.selected.is_empty(), "the selection outlived what was done with it");
    }

    fn landing(state: &State, card: &Card, lands: Lands) -> HitRegion {
        let wearing = wearing();
        let measured = measured_for(&state.pages, wide_of(card));
        let Ok(panelled) = shapes(state, card, fits_for(card), &measured, &wearing);

        match panelled.touching.into_iter().find(|region| region.lands == lands) {
            Some(region) => region,
            None => panic!("nothing a finger can reach lands on {lands:?}"),
        }
    }

    fn marks_drawn(state: &State, card: &Card) -> u32 {
        let wearing = wearing();
        let measured = measured_for(&state.pages, wide_of(card));
        let Ok(panelled) = shapes(state, card, fits_for(card), &measured, &wearing);
        let Ok(side) = fitted::<i32, u32>(super::SELECTED_MARK);

        let Ok(drawn) = fitted::<_, u32>(panelled
            .shapes
            .iter()
            .filter(|shape| match shape {
                super::Shape::Panel(panel) => panel.size == console_core_geometry::Size { width: side, height: side },
                super::Shape::Text(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => false,
            })
            .count());

        drawn
    }

    #[test]
    fn a_finger_alone_can_select_act_on_and_put_down_a_selection() {
        let acted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let opened = Arc::new(std::sync::Mutex::new(0));
        let mut state = a_folder_that_selects(Arc::clone(&acted), Arc::clone(&opened));
        let rows: Vec<Row> = state
            .rows
            .iter()
            .cloned()
            .map(|row| {
                let key = row.key.clone().unwrap_or_else(String::new);
                let Ok(row) = row.offering(move |showing| {
                    showing.select(vec![key.clone()]);

                    false
                });

                row
            })
            .collect();
        let Ok(page) = Page::new("Pictures", Rows::Fixed(rows.clone()));
        let Ok(page) = page.selecting("Delete", {
            let acted = Arc::clone(&acted);

            move |_, keys| {
                let Ok(mut held) = acted.lock() else { return };

                *held = keys.to_vec();
            }
        });

        state.pages = vec![page];
        state.rows = rows;

        let card = card_for(1024, 640);

        assert_eq!(marks_drawn(&state, &card), 0, "a mark is drawn before anything is selected");

        let dots = landing(&state, &card, Lands::Else(0));
        let Ok(_selects) = super::tapped(&mut state, Some(dots.lands), dots.panel.at, &card);
        let row = landing(&state, &card, Lands::Row(2));
        let Ok(_marks) = super::tapped(&mut state, Some(row.lands), row.panel.at, &card);

        assert_eq!(state.selected, vec!["beach.jpg".to_string(), "sea.jpg".to_string()]);
        assert_eq!(marks_drawn(&state, &card), 2, "what is selected is not marked on the glass");
        assert_eq!(opened.lock().map(|count| *count).ok(), Some(0), "a tap while selecting opened the thing");

        let dots = landing(&state, &card, Lands::Else(1));
        let Ok(_asks) = super::tapped(&mut state, Some(dots.lands), dots.panel.at, &card);
        let delete = landing(&state, &card, Lands::Answer(1));
        let Ok(_deletes) = super::tapped(&mut state, Some(delete.lands), delete.panel.at, &card);

        assert_eq!(acted.lock().map(|keys| keys.clone()).ok(), Some(vec!["beach.jpg".to_string(), "sea.jpg".to_string()]));
        assert!(state.selected.is_empty());

        state.selected = vec!["dune.jpg".to_string()];

        let shut = landing(&state, &card, Lands::Back);
        let Ok(gone) = super::tapped(&mut state, Some(shut.lands), shut.panel.at, &card);

        assert_eq!(gone, Gone::Staying, "the \u{d7} closed the panel instead of putting the selection down");
        assert!(state.selected.is_empty());
    }

    #[test]
    fn b_puts_a_selection_down_before_it_closes_anything() {
        let acted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let opened = Arc::new(std::sync::Mutex::new(0));
        let mut state = a_folder_that_selects(acted, opened);
        let rows = state.rows.clone();

        state.selected = vec!["dune.jpg".to_string()];

        assert_eq!(told(&mut state, Meaning::Close, &rows), Ok(Outcome::Redrawn));
        assert!(state.selected.is_empty());
        assert_eq!(told(&mut state, Meaning::Close, &rows), Ok(Outcome::Closing));
    }

    #[test]
    fn select_all_marks_every_row_that_can_be_marked() {
        let acted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let opened = Arc::new(std::sync::Mutex::new(0));
        let mut state = a_folder_that_selects(acted, opened);
        let rows = state.rows.clone();

        state.selected = vec!["dune.jpg".to_string()];

        let Ok(_asks) = carried_out(&mut state, &rows, Outcome::Else(1));
        let Ok(()) = super::took(&mut state, 2);

        assert_eq!(state.selected.len(), 3, "{:?}", state.selected);
    }

    #[test]
    fn a_choice_made_over_a_zoomed_picture_is_told_what_was_on_the_screen() {
        let still = std::path::PathBuf::from("/nowhere/a.png");
        let room = console_core_geometry::Size { width: 1280, height: 800 };
        let mut state = state_of(Vec::new());
        let Ok(closer) = crate::zoom::Zoom::default().times(crate::zoom::STEP);

        state.shown = Some(super::Moving { of: still.clone(), at: console_core_geometry::Point { x: 0, y: 0 }, room });
        state.zoom = Some((still.clone(), closer));

        use crate::page::Showing as _;
        let Ok(front) = super::Front::over(&state);

        assert_eq!(
            front.framed(),
            Ok(Some(crate::zoom::Framed { of: still.clone(), zoom: closer, room })),
            "the viewer cannot crop to what it cannot see"
        );

        state.zoom = Some((std::path::PathBuf::from("/nowhere/b.png"), closer));
        let Ok(front) = super::Front::over(&state);

        assert_eq!(
            front.framed().map(|framed| framed.map(|framed| framed.zoom)),
            Ok(Some(crate::zoom::Zoom::default())),
            "another picture's zoom was handed on as this one's"
        );
    }

    #[test]
    fn the_zoom_buttons_and_the_full_screen_button_reach_the_picture_on_the_screen() {
        let Ok(row) = Row::showing(Picture::Showing(Some("/nowhere/a.png".into())));
        let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![row]));
        let mut state = state_of(vec![page]);
        use crate::page::Showing as _;

        let Ok(front) = super::Front::new();

        front.zoom_by(crate::zoom::STEP);
        front.toggle_full_screen();

        let Ok(()) = front.onto(&mut state);
        let rows = state.rows.clone();
        let Some(row) = rows.first() else { panic!("the page lost its picture") };
        let Ok(zoom) = super::zoom_on(&state, row);

        assert_eq!(zoom.by, crate::zoom::STEP, "the zoom button did not zoom the picture");
        assert_eq!(state.opened, Opened::Expanded, "the full screen button did not open the picture out");

        let Ok(front) = super::Front::new();

        front.toggle_full_screen();

        let Ok(()) = front.onto(&mut state);

        assert_eq!(state.opened, Opened::No, "pressed again, full screen did not give the card back");
    }

    #[test]
    fn a_finger_drags_a_zoomed_picture_and_scrolls_a_whole_one() {
        let still = std::path::PathBuf::from("/nowhere/a.png");
        let moving = super::Moving {
            of: still.clone(),
            at: super::Point { x: 100, y: 100 },
            room: super::Size { width: 800, height: 400 },
        };
        let Ok(row) = Row::showing(Picture::Showing(Some(still.clone())));
        let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![row]));
        let mut state = state_of(vec![page]);
        let on_it = super::Point { x: 500, y: 300 };
        let Ok(whole) = super::panning(&state, Some(&moving), on_it);

        assert_eq!(whole, None, "a whole picture was dragged rather than the page scrolled");

        let Ok(closer) = crate::zoom::Zoom::default().times(crate::zoom::STEP);

        state.zoom = Some((still.clone(), closer));

        let Ok(zoomed) = super::panning(&state, Some(&moving), on_it);
        let Ok(beside) = super::panning(&state, Some(&moving), super::Point { x: 50, y: 300 });

        assert_eq!(zoomed, Some((still, closer, moving.room)), "a zoomed picture does not follow the finger");
        assert_eq!(beside, None, "a finger beside the picture dragged it");
    }

    #[test]
    fn a_bar_draws_how_far_along_it_is_and_a_tap_on_it_goes_there() {
        let landed = Arc::new(std::sync::Mutex::new(None));
        let hearing = Arc::clone(&landed);
        let Ok(nothing) = Handler::and_stay(|_| {});
        let Ok(row) = Row::new("0:25", Aside("1:40"), nothing);
        let Ok(row) = row.picturing(Picture::Bar(crate::page::Bar { at: 25, of: 100 }));
        let Ok(row) = row.seeking(move |_, fraction| {
            let _ = hearing.lock().map(|mut landed| *landed = Some(fraction));
        });
        let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![row]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let mut state = state_of(pages);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let Some(seek) = panelled.touching.iter().find(|touching| matches!(touching.lands, Lands::Seek { .. })) else {
            panic!("a bar that seeks has nowhere to tap")
        };
        let whole = seek.panel.size.width;
        let filled = panelled.shapes.iter().any(|shape| match shape {
            super::Shape::Panel(panel) => {
                panel.fill == wearing.pink && panel.at.x == seek.panel.at.x && panel.size.width == whole / 4
            },
            super::Shape::Text(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => false,
        });

        assert!(filled, "a quarter of the way through is not a quarter of the bar lit");

        let Ok(wide) = console_core_number_conversion::fitted::<u32, i32>(whole);
        let hit = super::Point { x: seek.panel.at.x.saturating_add(wide * 3 / 4), y: seek.panel.at.y };
        let _ = super::tapped(&mut state, Some(seek.lands), hit, &card);
        let Ok(landed) = landed.lock().map(|landed| *landed) else { panic!("poisoned") };

        assert!(landed.is_some_and(|fraction| (fraction - 0.75).abs() < 0.01), "{landed:?}");
    }

    #[test]
    fn a_bar_that_steps_holds_its_buttons_at_its_own_ends_and_its_length_past_them() {
        let Ok(nothing) = Handler::and_stay(|_| {});
        let Ok(row) = Row::new("0:50", Aside("2:37"), nothing);
        let Ok(row) = row.picturing(Picture::Bar(crate::page::Bar { at: 50, of: 157 }));
        let Ok(row) = row.leveled(Arc::new(|_step| {}));
        let Ok(row) = row.seeking(|_, _fraction| {});
        let Ok(page) = Page::new("Now Playing", Rows::Fixed(vec![row]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let landing = |wanted: fn(&Lands) -> bool| {
            panelled.touching.iter().find(|touching| wanted(&touching.lands)).map(|touching| touching.panel)
        };
        let (Some(seek), Some(less), Some(more)) = (
            landing(|lands| matches!(lands, Lands::Seek { .. })),
            landing(|lands| matches!(lands, Lands::Nudge { step: -1, .. })),
            landing(|lands| matches!(lands, Lands::Nudge { step: 1, .. })),
        ) else {
            panic!("the bar, its minus or its plus is missing")
        };
        let length = panelled.shapes.iter().find_map(|shape| match shape {
            super::Shape::Text(text) => (text.said == "2:37").then_some(text.at.x),
            super::Shape::Panel(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => None,
        });
        let Ok(bar_wide) = console_core_number_conversion::fitted::<u32, i32>(seek.size.width);
        let bar_ends = seek.at.x.saturating_add(bar_wide);

        assert!(less.at.x < seek.at.x, "the minus is not before the bar: {less:?} {seek:?}");
        assert!(more.at.x >= bar_ends, "the plus is not after the bar: {more:?} {seek:?}");
        assert!(length.is_some_and(|x| x > more.at.x), "the length is not past the plus: {length:?} {more:?}");
    }

    #[test]
    fn a_bar_on_the_lit_row_fills_in_the_dark_rather_than_pink_on_pink() {
        let Ok(nothing) = Handler::and_stay(|_| {});
        let Ok(row) = Row::new("0:25", Aside("1:40"), nothing);
        let Ok(row) = row.picturing(Picture::Bar(crate::page::Bar { at: 25, of: 100 }));
        let Ok(row) = row.seeking(|_, _fraction| {});
        let Ok(page) = Page::new("Now Playing", Rows::Fixed(vec![row]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let mut state = state_of(pages);

        state.at = Some(0);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let Some(seek) = panelled.touching.iter().find(|touching| matches!(touching.lands, Lands::Seek { .. })) else {
            panic!("a bar that seeks has nowhere to tap")
        };
        let whole = seek.panel.size.width;
        let filled = panelled.shapes.iter().any(|shape| match shape {
            super::Shape::Panel(panel) => {
                panel.fill == wearing.night && panel.at.x == seek.panel.at.x && panel.size.width == whole / 4
            },
            super::Shape::Text(_) | super::Shape::Picture(_) | super::Shape::Cropped(_) | super::Shape::Line(_) | super::Shape::Clip(_) => false,
        });

        assert!(filled, "the lit row's bar is not filled in the dark");
    }

    #[test]
    fn what_a_card_says_it_drew_names_every_part_a_hand_could_land_on() {
        let Ok(row) = Row::said("Ferry.jpg", Aside("2.1 MB"));
        let Ok(row) = row.offering(|_showing| false);
        let rows = vec![row];
        let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let mut state = state_of(pages);

        state.at = Some(0);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);
        let Ok(told) = what_it_drew(&state, &panelled, &rows, (1024, 640));

        assert_eq!(told.tab, "Files");

        let Ok(worn) = told.wearing("shut");

        assert!(worn.is_some(), "a card with no way out drawn on it");

        let Ok(worn) = told.wearing("tab");

        assert!(worn.is_none(), "one tab is the panel's title, not a strip with nothing to turn to");

        let line = told.line_saying("Ferry.jpg").expect("the row").expect("the row");

        assert_eq!(line.standing, crate::description::Standing::On);
        assert_eq!(line.offers, crate::description::Offers::Yes);

        let Ok(worn) = line.wearing("else");

        assert!(worn.is_some(), "a row offering something and drawing no mark for it");

        let Ok(every) = told.every_spot();

        for spot in every {
            assert_eq!(
                crate::description::reachable(spot, told.room),
                Ok(crate::description::Reachable::Yes),
                "{} at {:?} of {:?} is off the glass",
                spot.name,
                spot.at,
                spot.big
            );
        }
    }

    fn landing_on_rows(panelled: &Panelled) -> Result<Vec<&HitRegion>, Never> {
        Ok(panelled
            .touching
            .iter()
            .filter(|touching| matches!(touching.lands, Lands::Row(_)))
            .collect())
    }

    fn pressed(state: &mut State, rows: &[Row], key: Keysym) -> Gone {
        let Ok(driving) = driving(state);
        let Ok(meaning) = crate::keys::meaning(key, driving);
        let Ok(gone) = pressed_here(state, rows, key, meaning, driving);

        gone
    }

    #[test]
    fn a_row_that_asks_whether_somebody_is_sure_is_answered_left_and_right() {
        let taken = Arc::new(AtomicU32::new(NOTHING_TAKEN));
        let told_which = Arc::clone(&taken);
        let answering: crate::page::OnChosen =
            Arc::new(move |_showing, which| told_which.store(which, Ordering::Relaxed));
        let Ok(does) = Handler::and_stay(move |showing| {
            showing.sure(
                "Throw away",
                crate::page::Subject("Ferry.jpg"),
                &["Throw Away"],
                Arc::clone(&answering),
            );
        });
        let Ok(row) = Row::new("Throw away", Aside(""), does);
        let rows = vec![row];
        let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
        let mut state = state_of(vec![page]);

        let Ok(_up) = carried_out(&mut state, &rows, Outcome::Chose(0));

        assert_eq!(driving(&state), Ok(Driving::Sure), "a question is what is being driven");

        let _cancelled = pressed(&mut state, &rows, Keysym::Return);

        assert_eq!(
            taken.load(Ordering::Relaxed),
            NOTHING_TAKEN,
            "the answer standing first is the one that undoes the asking"
        );
        assert_eq!(driving(&state), Ok(Driving::Panel), "and the list is back");

        let Ok(_again) = carried_out(&mut state, &rows, Outcome::Chose(0));
        let _leant = pressed(&mut state, &rows, Keysym::Right);
        let _took = pressed(&mut state, &rows, Keysym::Return);

        assert_eq!(taken.load(Ordering::Relaxed), 0, "the first thing it offers to do is done");
        assert_eq!(driving(&state), Ok(Driving::Panel));
    }

    #[test]
    fn a_row_that_asks_a_question_is_typed_into_and_answered_by_return() {
        let heard = Arc::new(Mutex::new(String::new()));
        let told_what = Arc::clone(&heard);
        let answering: crate::page::Answer = Arc::new(move |_showing, word| {
            match told_what.lock() {
                Ok(mut said) => said.push_str(word),
                Err(_poisoned) => {},
            }
        });
        let Ok(does) = Handler::and_stay(move |showing| {
            showing.ask_aloud("What should it be called", Arc::clone(&answering));
        });
        let Ok(row) = Row::new("Rename", Aside(""), does);
        let rows = vec![row];
        let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
        let mut state = state_of(vec![page]);

        let Ok(_up) = carried_out(&mut state, &rows, Outcome::Chose(0));

        assert_eq!(driving(&state), Ok(Driving::Question));

        for key in [Keysym::h, Keysym::i] {
            let _typed = pressed(&mut state, &rows, key);
        }

        let _answered = pressed(&mut state, &rows, Keysym::Return);

        let said = match heard.lock() {
            Ok(said) => said.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };

        assert_eq!(said, "hi", "what was typed is what the row was told");
        assert_eq!(driving(&state), Ok(Driving::Panel), "and the question is gone");
    }

    #[test]
    fn a_question_standing_over_the_list_is_what_is_drawn() {
        let Ok(page) = Page::new("Files", Rows::Fixed(vec![said_row("Ferry.jpg")]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let mut state = state_of(pages);

        state.sure = Some(super::Sure {
            question: "Throw away".to_string(),
            about: "Ferry.jpg".to_string(),
            answers: vec![crate::marks::CANCEL.to_string(), "Throw Away".to_string()],
            at: 1,
            then: Arc::new(|_showing, _which| {}),
        });

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);

        let said: Vec<String> = panelled
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                console_core_shapes::Shape::Text(text) => Some(text.said.clone()),
                console_core_shapes::Shape::Panel(_)
                | console_core_shapes::Shape::Picture(_)
                | console_core_shapes::Shape::Cropped(_)
                | console_core_shapes::Shape::Line(_) | console_core_shapes::Shape::Clip(_) => {
                    None
                }
            })
            .collect();

        assert!(said.contains(&"Throw away".to_string()), "the question: {said:?}");
        assert!(said.contains(&"Ferry.jpg".to_string()), "what it is about: {said:?}");
        assert!(said.contains(&crate::marks::CANCEL.to_string()), "the way out: {said:?}");
        assert!(
            !said.contains(&"Ferry.jpg".to_string().repeat(2)),
            "the rows under it are not drawn as well"
        );
        assert_eq!(
            panelled.touching.iter().filter(|touching| matches!(touching.lands, Lands::Answer(_))).count(),
            2,
            "both answers can be pressed by a thumb"
        );
    }

    #[test]
    fn right_on_a_row_that_offers_something_stands_beside_it_and_a_runs_it() {
        let offered = Arc::new(AtomicBool::new(false));
        let told_it_was = Arc::clone(&offered);
        let Ok(row) = Row::said("Ferry.jpg", Aside("2.1 MB"));
        let Ok(row) = row.offering(move |_showing| {
            told_it_was.store(true, Ordering::Relaxed);

            false
        });
        let rows = vec![row];
        let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
        let mut state = state_of(vec![page]);

        let Ok(_down) = told(&mut state, Meaning::Step(1), &rows);
        let Ok(stood) = told(&mut state, Meaning::Nudge(1), &rows);

        assert_eq!(stood, Outcome::Redrawn);
        assert_eq!(state.beside, Beside::Yes, "the highlight is on what else the row offers");

        let Ok(chose) = told(&mut state, Meaning::Choose, &rows);

        assert_eq!(chose, Outcome::Else(0));
        assert_eq!(state.beside, Beside::No, "and it comes back to the row");

        let Ok(_ran) = carried_out(&mut state, &rows, chose);

        assert!(offered.load(Ordering::Relaxed), "what else the row offers was never run");
    }

    #[test]
    fn right_on_a_row_that_holds_a_level_moves_the_level_instead() {
        let moved = Arc::new(AtomicU32::new(0));
        let told_it_was = Arc::clone(&moved);
        let Ok(row) = Row::said("Volume", Aside("40%"));
        let Ok(row) = row.leveled(Arc::new(move |step| {
            let Ok(step) = console_core_number_conversion::fitted::<i32, u32>(step.max(0));

            told_it_was.store(step.saturating_add(1), Ordering::Relaxed);
        }));
        let rows = vec![row];
        let Ok(page) = Page::new("Sound", Rows::Fixed(rows.clone()));
        let mut state = state_of(vec![page]);

        state.at = Some(0);

        let Ok(nudged) = told(&mut state, Meaning::Nudge(1), &rows);

        assert_eq!(nudged, Outcome::Redrawn);
        assert_eq!(state.beside, Beside::No, "a row with a level has nowhere to stand beside");
        assert_eq!(moved.load(Ordering::Relaxed), 2, "the level was moved on");
    }

    #[test]
    fn a_row_that_has_a_square_at_its_front_starts_its_words_after_it() {
        let Ok(room) = beside_the_words(&crate::page::Picture::Space);
        let Ok(none) = beside_the_words(&crate::page::Picture::None);
        let Ok(at) = beside_the_words(&crate::page::Picture::At("/x.png".into()));

        assert!(room > crate::strip::PICTURE, "the square and the gap after it");
        assert_eq!(at, room, "a picture takes the room a blank square holds open");
        assert_eq!(none, 0, "and a row with nothing at its front gives none of it away");
    }

    #[test]
    fn a_row_whose_picture_is_not_made_yet_asks_for_it_rather_than_drawing_it() {
        let at = crate::page::Picture::At("/usr/share/x.png".into());
        let Ok(wanted) = wanted_of(&at);
        let Ok(sleeve) = wanted_of(&crate::page::Picture::Sleeve(None));
        let Ok(named) = wanted_of(&crate::page::Picture::Named(crate::icons::Icon::Folder));

        assert_eq!(wanted.and_then(Path::to_str), Some("/usr/share/x.png"));
        assert_eq!(sleeve, None, "a sleeve nobody found is nothing to ask the maker for");
        assert_eq!(named, None, "and an icon is a name the store is not keyed by");
    }

    #[test]
    fn a_row_with_a_level_draws_its_two_marks_where_a_thumb_can_find_them() {
        let Ok(row) = Row::said("Volume", Aside("40%"));
        let Ok(row) = row.leveled(Arc::new(|_step| {}));
        let Ok(page) = Page::new("Sound", Rows::Fixed(vec![row]));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);

        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);

        let marks: Vec<Lands> = panelled
            .touching
            .iter()
            .map(|touching| touching.lands)
            .filter(|lands| matches!(lands, Lands::Nudge { row: _, step: _ }))
            .collect();

        assert_eq!(
            marks,
            vec![Lands::Nudge { row: 0, step: 1 }, Lands::Nudge { row: 0, step: -1 }],
            "both ends of the level can be pressed"
        );

        let said: Vec<String> = panelled
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                console_core_shapes::Shape::Text(text) => Some(text.said.clone()),
                console_core_shapes::Shape::Panel(_)
                | console_core_shapes::Shape::Picture(_)
                | console_core_shapes::Shape::Cropped(_)
                | console_core_shapes::Shape::Line(_) | console_core_shapes::Shape::Clip(_) => {
                    None
                }
            })
            .collect();

        assert!(said.contains(&crate::marks::LESS.to_string()), "the end that takes away: {said:?}");
        assert!(said.contains(&crate::marks::MORE.to_string()), "the end that adds: {said:?}");
        assert!(said.contains(&"40%".to_string()), "and what it is now: {said:?}");
    }

    #[test]
    fn a_tabs_watch_runs_while_the_tab_is_in_front_and_not_after() {
        let watch = crate::page::Watch::anything(&["sleep", "60"]).expect("a watch");
        let looking = page("Bluetooth", &["Search for Devices"]).watching(watch).expect("watching");
        let mut state = state_of(vec![looking, page("Wi-Fi", &["Home"])]);
        let mut watching = None;

        super::watched(&mut watching, &state).expect("watched");

        let at = match &watching {
            Some((0, running)) => std::path::PathBuf::from(format!("/proc/{}", running.id().expect("an id"))),
            Some(_) | None => panic!("the Bluetooth tab is in front and nothing it watches is running"),
        };

        assert!(at.exists(), "the watch was started and is not running");

        state.here = 1;
        super::watched(&mut watching, &state).expect("watched");

        assert!(watching.is_none(), "a tab with nothing to watch is holding a watch");
        assert!(!at.exists(), "the tab went and {} is still running, which is a scan no one asked for", at.display());
    }

    fn a_pool_watching(folder: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let at = std::env::temp_dir().join(format!("console-panel-{folder}-{}.sock", std::process::id()));
        let into = std::env::temp_dir().join(format!("console-panel-{folder}-{}", std::process::id()));
        std::fs::create_dir_all(&into).expect("a folder");

        let serving = at.clone();
        let _ = std::thread::spawn(move || console_events::serving::serve(&serving, console_events::sources::hold));
        let began = std::time::Instant::now();

        while !at.exists() && began.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }

        (at, into)
    }

    fn woke_within(seconds: i64, nanoseconds: i64) -> crate::frames::Woken {
        let Ok(waking) = crate::frames::waking();
        let waking = waking.expect("a socket the loop is woken on");
        let mut watch = [rustix::event::PollFd::new(&waking, rustix::event::PollFlags::IN)];
        let within = rustix::event::Timespec { tv_sec: seconds, tv_nsec: nanoseconds };
        let _ = rustix::event::poll(&mut watch, Some(&within));
        let Ok(woken) = crate::frames::woken();

        woken
    }

    fn woke_at_all(seconds: i64, nanoseconds: i64) -> crate::frames::FrameReceived {
        let Ok(waking) = crate::frames::waking();
        let waking = waking.expect("a socket the loop is woken on");
        let mut watch = [rustix::event::PollFd::new(&waking, rustix::event::PollFlags::IN)];
        let within = rustix::event::Timespec { tv_sec: seconds, tv_nsec: nanoseconds };
        let woke = match rustix::event::poll(&mut watch, Some(&within)) {
            Ok(0) | Err(_) => crate::frames::FrameReceived::No,
            Ok(_ready) => crate::frames::FrameReceived::Yes,
        };
        let Ok(_drained) = crate::frames::woken();

        woke
    }

    fn told_after_writing(
        at: &std::path::Path,
        into: &std::path::Path,
        worth: console_events::again::Worthwhile,
    ) -> crate::frames::FrameReceived {
        let subscriber = console_events::subscription::connect_at(at, &[]).expect("a subscriber");
        let front = super::InFront::on(subscriber).expect("a page in front");
        let topic = console_program_contract::Topic::Path(into.to_path_buf());
        let looking = page("Sound", &["Speakers"]).listening(topic, worth).expect("listening");
        let Ok(_before) = crate::frames::woken();

        super::listened(&front.subscriptions, &front.now, Some(&looking)).expect("listened");

        assert_eq!(
            woke_within(2, 0).rows,
            crate::frames::FrameReceived::Yes,
            "getting into the pool did not wake the loop to read the tab again"
        );

        let _replayed = woke_within(0, 100_000_000);
        let began = std::time::Instant::now();
        let mut told = crate::frames::FrameReceived::No;

        while told == crate::frames::FrameReceived::No && began.elapsed() < Duration::from_secs(2) {
            std::fs::write(into.join("changed"), b"").expect("a change on disk");
            told = woke_within(0, 50_000_000).rows;
        }

        told
    }

    #[test]
    fn a_tab_in_front_is_read_again_when_the_pool_says_its_topic_changed() {
        let _turn = crate::frames::ONE_TEST_AT_A_TIME.lock();
        let (at, into) = a_pool_watching("listening");
        let told = told_after_writing(&at, &into, console_events::again::anything);
        let _ = std::fs::remove_dir_all(&into);
        let _ = std::fs::remove_file(&at);

        assert_eq!(told, crate::frames::FrameReceived::Yes, "the pool said the topic changed and the loop was never woken to read the tab again");
    }

    #[test]
    fn a_change_the_tab_does_not_care_about_does_not_read_it_again() {
        fn nothing(_line: &str) -> Result<console_events::again::Worth, Never> {
            Ok(console_events::again::Worth::Ignoring)
        }

        let _turn = crate::frames::ONE_TEST_AT_A_TIME.lock();
        let (at, into) = a_pool_watching("ignoring");
        let told = told_after_writing(&at, &into, nothing);
        let _ = std::fs::remove_dir_all(&into);
        let _ = std::fs::remove_file(&at);

        assert_eq!(told, crate::frames::FrameReceived::No, "a line the tab said was not worth asking after read it again");
    }

    #[test]
    fn a_panel_told_to_shut_wakes_a_loop_asleep_with_no_timeout() {
        let _turn = crate::frames::ONE_TEST_AT_A_TIME.lock();
        let Ok(_before) = crate::frames::woken();
        let shut = Arc::new(AtomicBool::new(false));
        let Ok(()) = super::Close(Arc::clone(&shut)).shut();

        assert!(shut.load(Ordering::Relaxed), "shutting did not say so");
        assert_eq!(woke_at_all(0, 0), crate::frames::FrameReceived::Yes, "a panel was told to shut and the loop asleep in poll was never woken to read it");
    }

    fn looked_while(shut: &Arc<AtomicBool>, closing: Closing) -> console_waiting::Outcome {
        let (_rows_that_never_come, arrived) = std::sync::mpsc::channel();
        let (finished, done) = std::sync::mpsc::channel();
        let looking = Arc::clone(shut);
        let first = std::thread::spawn(move || {
            let reading = super::Reading { here: 0, arrived };
            let look = super::FirstLook { shut: &looking, until: Duration::from_secs(3600) };
            let Ok(_looked) = super::first_look(&reading, &[], look);
            let _ = finished.send(());
        });
        let Ok(()) = console_program_lifetime::threads::let_go(first);

        let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(10));
        let Ok(outcome) = console_waiting::until(patience, || {
            match closing {
                Closing::Announced => {
                    let Ok(()) = super::Close(Arc::clone(shut)).shut();
                }
                Closing::Unannounced => {},
            }

            Ok(match done.try_recv() {
                Ok(()) => console_waiting::Ready::Yes,
                Err(_still_looking) => console_waiting::Ready::NotYet,
            })
        });

        outcome
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Closing {
        Announced,
        Unannounced,
    }

    #[test]
    fn a_panel_shut_while_it_waits_for_its_first_rows_stops_waiting() {
        let _turn = crate::frames::ONE_TEST_AT_A_TIME.lock();
        let Ok(_before) = crate::frames::woken();
        let shut = Arc::new(AtomicBool::new(false));

        assert_eq!(
            looked_while(&shut, Closing::Announced),
            console_waiting::Outcome::Happened,
            "a panel was shut while its first rows were still being read, and it waited for them anyway"
        );
    }

    #[test]
    fn a_panel_already_shut_does_not_wait_for_its_first_rows() {
        let _turn = crate::frames::ONE_TEST_AT_A_TIME.lock();
        let Ok(_before) = crate::frames::woken();
        let shut = Arc::new(AtomicBool::new(true));

        assert_eq!(
            looked_while(&shut, Closing::Unannounced),
            console_waiting::Outcome::Happened,
            "the word that a panel was shut was read before its first look, and the look waited anyway"
        );
    }

    #[test]
    fn a_reading_that_lands_wakes_the_loop_waiting_for_it() {
        let _turn = crate::frames::ONE_TEST_AT_A_TIME.lock();
        let Ok(_before) = crate::frames::woken();
        let reading = super::reading(&page("Now", &["one"]), 0).expect("a reading");

        assert_eq!(woke_at_all(5, 0), crate::frames::FrameReceived::Yes, "the rows were read and the loop asleep in poll was never woken to draw them");
        assert_eq!(reading.arrived.try_recv().map(|rows| rows.len()), Ok(1), "the loop was woken before the rows it was woken for");
    }

    fn written_on(rows: Vec<Row>) -> Vec<console_core_shapes::Text> {
        let Ok(page) = Page::new("Now", Rows::Fixed(rows));
        let wearing = wearing();
        let card = card_for(1024, 640);
        let pages = vec![page];
        let measured = measured_for(&pages, wide_of(&card));
        let fits = fits_for(&card);
        let state = state_of(pages);
        let Ok(panelled) = shapes(&state, &card, fits, &measured, &wearing);

        panelled
            .shapes
            .into_iter()
            .filter_map(|shape| match shape {
                console_core_shapes::Shape::Text(text) => Some(text),
                console_core_shapes::Shape::Panel(_)
                | console_core_shapes::Shape::Picture(_)
                | console_core_shapes::Shape::Cropped(_)
                | console_core_shapes::Shape::Line(_) | console_core_shapes::Shape::Clip(_) => None,
            })
            .collect()
    }

    fn glyph_of(icon: crate::icons::Icon) -> String {
        let Ok(glyph) = icon.glyph();

        glyph.to_string()
    }

    #[test]
    fn an_icon_beside_a_row_is_drawn_rather_than_left_as_a_gap() {
        let Ok(row) = said_row("Tomorrow  Rain").picturing(Picture::Named(crate::icons::Icon::Rain));
        let said: Vec<String> = written_on(vec![row]).into_iter().map(|text| text.said).collect();

        assert!(said.contains(&glyph_of(crate::icons::Icon::Rain)), "the rain is not on the row: {said:?}");
    }

    #[test]
    fn a_cell_with_an_icon_draws_the_icon_before_its_words() {
        let Ok(cell) = crate::page::Cell::new("Wind  10 km/h", crate::page::Active::No);
        let Ok(cell) = cell.with_icon(crate::icons::Icon::Wind);
        let Ok(row) = Row::celled(vec![cell]);
        let written = written_on(vec![row]);
        let glyph = written.iter().find(|text| text.said == glyph_of(crate::icons::Icon::Wind));
        let words = written.iter().find(|text| text.said == "Wind  10 km/h");

        match (glyph, words) {
            (Some(glyph), Some(words)) => assert!(glyph.at.x < words.at.x, "the icon comes after its words"),
            (None, _) | (_, None) => panic!("the icon or its words are missing: {written:?}"),
        }
    }

    #[test]
    fn a_headline_draws_the_temperature_large_with_its_sky_beside_an_icon() {
        let headline = crate::page::Headline {
            title: "Brussels".to_string(),
            subtitle: "Thursday, updated 02:30".to_string(),
            icon: Some(crate::icons::Icon::CloudSun),
            says: "Partly Cloudy".to_string(),
            big: "17\u{b0}".to_string(),
            aside: "H:21\u{b0}  L:15\u{b0}".to_string(),
            alignment: crate::page::Alignment::Leading,
        };
        let Ok(row) = Row::headline(Picture::Showing(None), headline);
        let written = written_on(vec![row, said_row("under it")]);
        let said: Vec<&str> = written.iter().map(|text| text.said.as_str()).collect();
        let big = written.iter().find(|text| text.said == "17\u{b0}");
        let body = written.iter().find(|text| text.said == "under it");

        for wanted in ["Brussels", "Thursday, updated 02:30", "Partly Cloudy", "H:21\u{b0}  L:15\u{b0}"] {
            assert!(said.contains(&wanted), "{wanted} is missing: {said:?}");
        }
        assert!(said.contains(&glyph_of(crate::icons::Icon::CloudSun).as_str()), "the sky has no icon: {said:?}");

        match (big, body) {
            (Some(big), Some(body)) => assert!(big.font.height > body.font.height.saturating_mul(3), "the temperature is not large"),
            (None, _) | (_, None) => panic!("the temperature or the row under it is missing: {said:?}"),
        }
    }
}
