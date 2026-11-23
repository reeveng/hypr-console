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
//! nobody touches is a process asleep in `poll`. Which event group is wanted
//! and which words are worth a reading stay with the loop, because they are the
//! tab in front's and the loop is what knows which tab that is. While rows
//! are loading, the selected tab draws a cached preview or a loading message.
//!
//! **The shape list is closed.** `Panel`, `Text`, and `Picture` from
//! `console-core-shapes`. Drawing is a match, and 016 makes a shape added later
//! announce itself everywhere it matters.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

use console_actor::Actor;
use console_core_color::Oklch;
use console_core_color::palette::Wearing;
use console_core_controls::{
  Answer as Answered, Choice, Inks as ControlInks, Lettering, Picker, Press, Slider, Switch,
};
use console_core_fonts::{EM, TextStyle};
use console_core_geometry::{Point, Rectangle, Size};
use console_core_highlight::{Activation, Heading as Toward};
use console_core_iteration::Step as Stepped;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_i32};
use console_core_shapes::{
  Clip, Covers, Edge, Font, Panel as ShapePanel, Picture as ShapePicture, Pixels, Round, Shape,
  Text, Weight,
};
pub use console_draw_painting::text::FontCatalog;
use console_draw_painting::{self as painting, Frame, Run};
pub use console_draw_surface::Drawing;
pub use console_draw_surface::standing::{DrawingSender, drawing_notifications};
use console_events::again::Worth;
use console_events::subscription::{Received, Subscriber, Subscriptions};
use console_input_text::{InputDeletion, TextField, Typed};
use console_operating_system_linux::program_lifetime::BoundToParent;
use console_operating_system_linux::program_runtime::commands::{
  Linux as CommandPlatform, Timing,
};
use console_program_runtime::commands::{Invocation, Streams};

use console_draw_surface::{
  Anchor, Composition, InputContent, Keyboard, KeyboardEvent, KeyboardFocus, Keysym, Margin,
  Part, PointerEvent, Room, Surface, Surrounding, TextInputState, Under, Visible, Wanted, Window,
};

use crate::description;
use crate::fitting;
use crate::keys::{self, Driving, Meaning};
use crate::marks;
use crate::page::{
  ActionOutcome, Answer, Bare, Beside, Choices, Confirmation, Control, Handler, Heading, Holds,
  Nudged, Page, Picture, Progress, Row, Step, Subscription, WakeOutcome, Wears, walked,
};
use crate::shape;
use crate::strip;
use crate::tasks::{self, TaskEvent, Tasks};
use console_sound_effects::catalogue::Sound;
use console_sound_effects::{Degree, SoundEffects};

mod accessibility;
mod effects;

const TAB_LEAST: i32 = EM * 40 / 9;
const OVER_ROWS: i32 = EM * 5 / 9;
const BEFORE_THE_FIRST_ROW: i32 = -1;
const WHATEVER_THE_SCREEN_IS: Size<u32> = Size {
  width: 0,
  height: 0,
};
const FINGER: i32 = EM * 8 / 3;
const GLYPH: i32 = EM * 4 / 3;
const PRESS_WIDE: i32 = FINGER * 3 / 2;
const CONTROL_TALL: i32 = EM * 14 / 9;
const SLIDER_MOST: i32 = EM * 14;
const PICKER_MOST: i32 = EM * 22;
const CARD_ROUND: u32 = (EM * 8 / 9).unsigned_abs();
const CARD_EDGE: u32 = (EM / 9).unsigned_abs();
const TAB_ROUND: u32 = (EM * 2 / 3).unsigned_abs();
const ROW_ROUND: u32 = (EM * 2 / 3).unsigned_abs();
const BETWEEN_ROWS: i32 = fitting::ROW - FINGER;
const INSIDE_A_ROW: i32 = EM * 8 / 9;
const CARET: u32 = (EM / 9).unsigned_abs();
const DRAGGED: f64 = 12.0;
const A_NOTCH: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
  Expanded,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
  Card,
  Window,
}

pub struct Close(mpsc::SyncSender<()>);

impl Close {
  pub fn shut(&self) -> Result<(), Never> {
    match self.0.try_send(()) {
      Ok(()) | Err(mpsc::TrySendError::Full(())) => {
        crate::frames::tell(crate::frames::Notice::Closed)
      }
      Err(mpsc::TrySendError::Disconnected(())) => Ok(()),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StopRequest {
  Open,
  Requested,
}

struct CloseWatch {
  pending: Option<mpsc::Receiver<()>>,
  state: std::cell::Cell<StopRequest>,
}

impl CloseWatch {
  fn pair() -> Result<(Close, Self), Never> {
    let (close, pending) = mpsc::sync_channel(1);

    Ok((
      Close(close),
      Self {
        pending: Some(pending),
        state: std::cell::Cell::new(StopRequest::Open),
      },
    ))
  }

  fn local() -> Result<Self, Never> {
    Ok(Self {
      pending: None,
      state: std::cell::Cell::new(StopRequest::Open),
    })
  }

  fn stop(&self) -> Result<StopRequest, Never> {
    match (self.state.get(), self.pending.as_ref()) {
      (StopRequest::Requested, _) => Ok(StopRequest::Requested),
      (StopRequest::Open, None) => Ok(StopRequest::Open),
      (StopRequest::Open, Some(inbox)) => match inbox.try_recv() {
        Ok(()) => {
          self.state.set(StopRequest::Requested);
          Ok(StopRequest::Requested)
        }
        Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => {
          Ok(StopRequest::Open)
        }
      },
    }
  }
}

const NO_ROOM: Size<u32> = Size {
  width: 0,
  height: 0,
};

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

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum RowIdentity {
  Key(String),
  Position(u64),
}

impl RowIdentity {
  #[must_use]
  fn of(row: &Row, at: u64) -> Result<Self, Never> {
    Ok(
      row
        .key
        .as_ref()
        .map_or(Self::Position(at), |key| Self::Key(key.clone())),
    )
  }
}

struct CachedRow {
  row: Row,
  measured: MeasuredRow,
}

type RowMeasurements = RefCell<BTreeMap<RowIdentity, CachedRow>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeasuredMarks {
  shut: Size<u32>,
  before: Size<u32>,
  after: Size<u32>,
  into: Size<u32>,
  offers: Size<u32>,
}

pub(crate) struct Measured<'a> {
  tabs: &'a [MeasuredTab],
  rows: &'a RowMeasurements,
  marks: MeasuredMarks,
  typed: MeasuredLine,
  answers: Vec<Size<u32>>,
  reveal: Size<u32>,
  note: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MeasuredLine {
  size: Size<u32>,
  cursor: MeasuredCursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeasuredCursor {
  Editing(u32),
  Composing { underline: (u32, u32), caret: Caret },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Caret {
  At(u32),
  Hidden,
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

  painting::measure_text(
    Run {
      said,
      weight,
      width: ANYTHING_WIDE,
    },
    &font,
  )
}

fn weight_of(row: &Row) -> Result<Weight, Never> {
  Ok(match ::core::primitive::bool::then_some(row.naming, ()) {
    ::core::option::Option::Some(()) => Weight::Bold,
    ::core::option::Option::None => Weight::Plain,
  })
}

fn measure_tabs(pages: &[Page], wide: u32) -> Result<Vec<MeasuredTab>, Never> {
  let mut measured = Vec::new();
  let font = font(TextStyle::Headline)?;

  for page in pages {
    let run = Run {
      said: &page.title,
      weight: Weight::Plain,
      width: wide,
    };
    let title = painting::measure_text(run, &font)?;

    let run_2 = Run {
      said: &page.title,
      weight: Weight::Bold,
      width: wide,
    };
    let chosen = painting::measure_text(run_2, &font)?;

    measured.push(MeasuredTab { title, chosen });
  }

  Ok(measured)
}

fn measure_row(row: &Row, wide: u32) -> Result<MeasuredRow, Never> {
  let row_font = font(TextStyle::Body)?;
  let aside_font = font(TextStyle::Callout)?;

  let weight = weight_of(row)?;
  let says_run = Run {
    said: &row.says,
    weight,
    width: wide,
  };
  let says = painting::measure_text(says_run, &row_font)?;

  let aside_run = Run {
    said: &row.aside,
    weight: Weight::Plain,
    width: wide,
  };
  let aside = painting::measure_text(aside_run, &aside_font)?;

  let Ok((less, more)) = crate::page::ends_of(row);
  let less_2 = sized_as(less, Weight::Bold, TextStyle::Body)?;
  let more_2 = sized_as(more, Weight::Bold, TextStyle::Body)?;

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
        let run = Run {
          said: glyph,
          weight: Weight::Plain,
          width: wide,
        };

        painting::measure_text(run, &glyph_font)?
      }
      crate::page::Face::Swatch(_) => NO_ROOM,
      crate::page::Face::Written(says_2) => sized_as(says_2, Weight::Plain, TextStyle::Title)?,
    };

    presses.push(said);
  }

  Ok(MeasuredRow {
    says,
    aside,
    less: less_2,
    more: more_2,
    cells,
    presses,
  })
}

#[must_use]
fn same_measurements(old: &Row, new: &Row) -> Result<crate::page::Same, Never> {
  let same = old.says == new.says
    && old.aside == new.aside
    && old.naming == new.naming
    && old.ends == new.ends
    && old
      .cells
      .iter()
      .map(|cell| &cell.says)
      .eq(new.cells.iter().map(|cell| &cell.says))
    && old
      .buttons
      .iter()
      .flat_map(|buttons| buttons.presses.iter())
      .map(|press| press.face)
      .eq(
        new
          .buttons
          .iter()
          .flat_map(|buttons| buttons.presses.iter())
          .map(|press| press.face),
      );

  Ok(match same.then_some(()) {
    Some(()) => crate::page::Same::Yes,
    None => crate::page::Same::No,
  })
}

fn measure_marks() -> Result<MeasuredMarks, Never> {
  let Ok(shut) = sized_as(marks::SHUT, Weight::Bold, TextStyle::Body);
  let Ok(before) = sized_as(marks::BEFORE, Weight::Bold, TextStyle::Body);
  let Ok(after) = sized_as(marks::AFTER, Weight::Bold, TextStyle::Body);
  let Ok(into) = sized_as(marks::INTO, Weight::Plain, TextStyle::Body);
  let Ok(offers) = sized_as(marks::ELSE, Weight::Bold, TextStyle::Body);

  Ok(MeasuredMarks {
    shut,
    before,
    after,
    into,
    offers,
  })
}

fn in_the_line(state: &State) -> Result<(String, String, Filled), Never> {
  match &state.asking {
    Some(asking) => {
      let Ok((typed, before)) = editing_line(&asking.typed, state.preedit.as_ref());
      let (said, before_2) = match asking.secret {
        Secret::Hidden => {
          let Ok(letters) = console_input_text::grapheme_count(&typed);
          let Ok(caret) = console_input_text::grapheme_count(&before);
          let Ok(letters_2) = index(letters);
          let Ok(caret_2) = index(caret);

          (
            marks::HIDDEN.repeat(letters_2),
            marks::HIDDEN.repeat(caret_2),
          )
        }
        Secret::Shown | Secret::No => (typed, before),
      };
      let filled = match ::core::primitive::bool::then_some(said.is_empty(), ()) {
        ::core::option::Option::Some(()) => Filled::Empty,
        ::core::option::Option::None => Filled::Typed,
      };

      return Ok((said, before_2, filled));
    }
    None => {}
  }

  let Ok(page) = nth(&state.pages, state.here);
  let about = page
    .and_then(|page_2| page_2.sought.as_ref())
    .map(|sought| sought.about.clone());
  let Ok((typed, before)) = editing_line(&state.typed, state.preedit.as_ref());

  Ok(match (typed.is_empty(), about) {
    (false, _) => (typed, before, Filled::Typed),
    (true, Some(about_2)) => (about_2, String::new(), Filled::Empty),
    (true, None) => (String::new(), String::new(), Filled::Empty),
  })
}

fn editing_line(
  field: &TextField,
  preedit: Option<&Composition>,
) -> Result<(String, String), Never> {
  let Ok(text) = field.text();
  let Ok(prefix) = field.before_caret();

  let preedit_2 = match preedit {
    Some(matched_preedit) => matched_preedit,
    ::core::option::Option::None => return Ok((text.to_owned(), prefix.to_owned())),
  };

  let mut text_2 = text.to_owned();
  text_2.insert_str(prefix.len(), &preedit_2.text);
  let Ok(relative) = index(preedit_2.cursor_end.max(0));
  let before = match (preedit_2.cursor_end >= 0).then_some(()) {
    Some(()) => preedit_2
      .text
      .get(..relative)
      .map_or(preedit_2.text.as_str(), |composed| composed),
    None => preedit_2.text.as_str(),
  };
  Ok((text_2, format!("{prefix}{before}")))
}

#[must_use]
fn text_input_state(state: &State, card: &Card) -> Result<Option<TextInputState>, Never> {
  match { ::core::primitive::bool::then_some(state.sure.is_some(), ()) } {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let (field, at, sensitive, identity) = match state.asking.as_ref() {
    Some(prompt) => {
      let Ok(at) = asked_at(card);
      let at_2 = Point {
        x: at.x,
        y: at.y.saturating_add(fitting::ROW),
      };
      (
        &prompt.typed,
        at_2,
        prompt.secret != Secret::No,
        ::core::primitive::u64::wrapping_shl(1_u64, 32_u32) | u64::from(prompt.identity),
      )
    }
    None => {
      let Ok(page) = nth(&state.pages, state.here);

      match page.and_then(|current| current.sought.as_ref()) {
        Some(_search) => {}
        None => return Ok(None),
      }

      let Ok(left) = inside_left(card);
      let top = match state.opened {
        Opened::Expanded => card.y.saturating_sub(FINGER),
        Opened::No => card
          .y
          .saturating_add(fitting::STRIP)
          .saturating_add(OVER_ROWS),
      };
      (
        &state.typed,
        Point { x: left, y: top },
        false,
        u64::from(state.here),
      )
    }
  };
  let content = match sensitive.then_some(()) {
    Some(()) => InputContent::Password,
    None => {
      let Ok(text) = field.text();
      let Ok(before) = field.before_caret();
      let Ok(cursor) = fitted(before.len());
      InputContent::Plain(Some(Surrounding {
        text: text.to_owned(),
        cursor,
      }))
    }
  };
  let Ok((_, before, _)) = in_the_line(state);
  let Ok(measured) = sized_as(&before, Weight::Plain, TextStyle::Body);
  let Ok(width) = fitted::<u32, i32>(measured.width);
  let x = at.x.saturating_add(INSIDE_A_ROW).saturating_add(width).min(
    card
      .x
      .saturating_add(card.width)
      .saturating_sub(INSIDE_A_ROW),
  );
  let cursor = Part {
    at: Point { x, y: at.y.max(0) },
    size: Size {
      width: CARET,
      height: FINGER.unsigned_abs(),
    },
  };
  Ok(Some(TextInputState {
    content,
    cursor: Some(cursor),
    identity,
  }))
}

fn apply_text_input(
  state: &mut State,
  change: console_draw_surface::TextInputChange,
) -> Result<Dirty, Never> {
  let Ok(driving) = driving(state);
  let changed = state.preedit != change.preedit;
  state.preedit = change.preedit;
  let composed = match change.commit.as_deref() {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => "",
  };
  let deletion = InputDeletion {
    before: change.delete_before,
    after: change.delete_after,
  };
  let edited = match (driving, state.asking.as_mut()) {
    (Driving::Question, Some(prompt)) => {
      let Ok(typed) = prompt.typed.apply_input(deletion, composed);

      typed
    }
    (Driving::Search, _) => {
      let Ok(typed) = state.typed.apply_input(deletion, composed);

      match { ::core::primitive::bool::then_some(typed == Typed::Edited, ()) } {
        ::core::option::Option::Some(()) => {
          let Ok(()) = narrowed(&mut state.model, &state.rows);
        }
        ::core::option::Option::None => {}
      }

      typed
    }
    _ => {
      state.preedit = None;
      Typed::None
    }
  };
  Ok(
    match { ::core::primitive::bool::then_some(edited == Typed::Edited || changed, ()) } {
      ::core::option::Option::Some(()) => Dirty::Yes,
      ::core::option::Option::None => Dirty::No,
    },
  )
}

fn measured_now<'a>(
  state: &State,
  tabs: &'a [MeasuredTab],
  rows: &'a RowMeasurements,
  marks: MeasuredMarks,
) -> Result<Measured<'a>, Never> {
  let Ok((said, before, _filled)) = in_the_line(state);
  let Ok(size) = sized_as(&said, Weight::Plain, TextStyle::Body);
  let Ok(caret) = sized_as(&before, Weight::Plain, TextStyle::Body);
  let cursor = match state.preedit.as_ref() {
    Some(preedit) => {
      let Ok(underline) = preedit_underline(state, preedit);
      let visibility = match (preedit.cursor_begin >= 0 || preedit.cursor_end >= 0).then_some(())
      {
        Some(()) => Caret::At(caret.width),
        None => Caret::Hidden,
      };
      MeasuredCursor::Composing {
        underline,
        caret: visibility,
      }
    }
    None => MeasuredCursor::Editing(caret.width),
  };
  let typed = MeasuredLine { size, cursor };
  let mut answers = Vec::new();

  match &state.sure {
    Some(sure) => {
      for says in &sure.answers {
        let Ok(said_2) = sized_as(says, Weight::Bold, TextStyle::Body);

        answers.push(said_2);
      }
    }
    None => {}
  }

  let offered = match &state.asking {
    Some(asking) => asking.secret.offered()?,
    None => None,
  };

  let reveal = match offered {
    Some(says) => sized_as(says, Weight::Bold, TextStyle::Body)?,
    None => NO_ROOM,
  };

  let Ok(noting) = noted_now(state);
  let note = match noting {
    Some(said_3) => sized_as(said_3, Weight::Plain, TextStyle::Callout)?,
    None => NO_ROOM,
  };

  Ok(Measured {
    tabs,
    rows,
    marks,
    typed,
    answers,
    reveal,
    note,
  })
}

fn preedit_underline(state: &State, preedit: &Composition) -> Result<(u32, u32), Never> {
  let (field, masked) = state
    .asking
    .as_ref()
    .map_or((&state.typed, false), |prompt| {
      (&prompt.typed, prompt.secret == Secret::Hidden)
    });
  let Ok(before) = field.before_caret();
  let through = format!("{before}{}", preedit.text);
  let width = |text: &str| -> Result<u32, Never> {
    let display = match ::core::primitive::bool::then_some(masked, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(many) = console_input_text::grapheme_count(text);
        let Ok(many_2) = index(many);
        marks::HIDDEN.repeat(many_2)
      }
      ::core::option::Option::None => text.to_owned(),
    };
    let Ok(measured) = sized_as(&display, Weight::Plain, TextStyle::Body);
    Ok(measured.width)
  };
  let Ok(start) = width(before);
  let Ok(end) = width(&through);

  Ok((start, end))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Panelled {
  shapes: Vec<Shape>,
  touching: Vec<HitRegion>,
  moving: Option<Moving>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Moving {
  of: std::path::PathBuf,
  at: Point<i32>,
  room: Size<u32>,
  inks: Inks,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Inks {
  ground: Oklch,
  words: Oklch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lands {
  Row(u32),
  Answer(u32),
  Nudge { row: u32, step: i32 },
  Else(u32),
  ButtonPress { row: u32, which: u32 },
  Seek { row: u32, frame: Rectangle<i32> },
  Slider { row: u32, frame: Rectangle<i32> },
  Picker { row: u32, frame: Rectangle<i32> },
  Tab(u32),
  More(i32),
  Back,
  Reveal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner {
  Card,
  Row(u32),
}

impl Lands {
  fn label_and_owner(self) -> Result<(&'static str, Owner), Never> {
    Ok(match self {
      Lands::Row(at) => ("press", Owner::Row(at)),
      Lands::Answer(_) => ("answer", Owner::Card),
      Lands::Nudge { row, step: _ } => ("step", Owner::Row(row)),
      Lands::Else(row) => ("else", Owner::Row(row)),
      Lands::ButtonPress { row, which: _ } => ("button", Owner::Row(row)),
      Lands::Seek { row, frame: _ } => ("seek", Owner::Row(row)),
      Lands::Slider { row, frame: _ } => ("slide", Owner::Row(row)),
      Lands::Picker { row, frame: _ } => ("pick", Owner::Row(row)),
      Lands::Tab(_) => ("tab", Owner::Card),
      Lands::More(_) => ("more", Owner::Card),
      Lands::Back => ("shut", Owner::Card),
      Lands::Reveal => ("reveal", Owner::Card),
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

  Ok(Font {
    family: console_core_fonts::ICONS.to_string(),
    height: tall,
  })
}

fn widest_tab(tabs: &[MeasuredTab]) -> Result<strip::Cell, Never> {
  let Ok(pad) = fitted::<i32, u32>(strip::PAD.saturating_mul(2));
  let widest = match tabs
    .iter()
    .map(|tab| tab.chosen.width.saturating_add(pad))
    .max()
  {
    Some(widest) => widest,
    None => return Ok(strip::Cell(TAB_LEAST)),
  };
  let Ok(widest_2) = fitted::<u32, i32>(widest);

  Ok(strip::Cell(widest_2.max(TAB_LEAST)))
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
  Ok(
    card
      .x
      .saturating_add(strip::EDGE)
      .saturating_add(strip::MARGIN),
  )
}

fn inside_right(card: &Card) -> Result<i32, Never> {
  Ok(
    card
      .x
      .saturating_add(card.width)
      .saturating_sub(strip::EDGE)
      .saturating_sub(strip::MARGIN),
  )
}

fn header_top(card: &Card) -> Result<i32, Never> {
  Ok(
    card
      .y
      .saturating_add(fitting::STRIP.saturating_sub(FINGER).saturating_div(2)),
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Band {
  top: i32,
  height: i32,
}

fn middle(band: Band, said: Size<u32>) -> Result<i32, Never> {
  let Ok(said_tall) = fitted::<u32, i32>(said.height);

  Ok(
    band
      .top
      .saturating_add(band.height.saturating_sub(said_tall).saturating_div(2)),
  )
}

fn width(wide: i32) -> Result<u32, Never> {
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
  let Ok(down) = middle(
    Band {
      top: within.at.y,
      height: tall,
    },
    measured,
  );

  Ok(Text {
    at: Point {
      x: within
        .at
        .x
        .saturating_add(wide.saturating_sub(said_wide).max(0).saturating_div(2)),
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
  let Ok(words) = centred(
    button.said,
    button.measured,
    &panel,
    Weight::Bold,
    face,
    ink,
  );

  Ok((
    vec![Shape::Panel(panel), Shape::Text(words)],
    HitRegion {
      lands: button.lands,
      panel,
    },
  ))
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
    at: Point {
      x: left.saturating_add(INSIDE_A_ROW),
      y: down,
    },
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
    (
      here > 0,
      ends.from,
      marks::BEFORE,
      measured.marks.before,
      Lands::More(-1),
    ),
    (
      here.saturating_add(1) < many,
      ends.to,
      marks::AFTER,
      measured.marks.after,
      Lands::More(1),
    ),
  ];

  for (shown, at_x, said, said_size, lands) in arrows {
    match ::core::primitive::bool::then_some(shown, ()) {
      ::core::option::Option::Some(()) => {
        let Ok((drawn, reached)) = button(
          Button {
            at: Point {
              x: at_x,
              y: band.top,
            },
            size: Size {
              width: finger,
              height: finger,
            },
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
      ::core::option::Option::None => {}
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
  let square = Size {
    width: finger,
    height: finger,
  };
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
    Opened::No => {}
  }

  let Ok(many) = fitted::<_, u32>(state.pages.len());

  match ::core::primitive::bool::then_some(many > 1, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      let Ok(wide) = width(shut_x.saturating_sub(strip::GAP).saturating_sub(left));
      let Ok(title) = title_shapes(
        state,
        measured,
        Band {
          top,
          height: FINGER,
        },
        left,
        wide,
        wearing,
      );

      shapes.extend(title);

      return Ok((shapes, touching));
    }
  }

  let ends = Span {
    from: left,
    to: shut_x.saturating_sub(strip::GAP).saturating_sub(FINGER),
  };
  let Ok((arrows, reached_2)) = arrow_shapes(
    state,
    measured,
    Band {
      top,
      height: FINGER,
    },
    ends,
    wearing,
  );

  shapes.extend(arrows);
  touching.extend(reached_2);

  let Ok((tabs, tabs_reached)) = tab_shapes(
    state,
    window,
    measured,
    card,
    Point { x: left, y: top },
    wearing,
  );
  shapes.extend(tabs);
  touching.extend(tabs_reached);

  Ok((shapes, touching))
}

fn tab_shapes(
  state: &State,
  window: TabWindow,
  measured: &Measured<'_>,
  card: &Card,
  origin: Point<i32>,
  wearing: &Wearing,
) -> Result<(Vec<Shape>, Vec<HitRegion>), Never> {
  let mut shapes = Vec::new();
  let mut touching = Vec::new();
  let Ok(many) = fitted::<_, u32>(state.pages.len());
  let showing = window.fits.min(many.saturating_sub(window.from));
  let Ok(finger) = fitted::<i32, u32>(FINGER);
  let Point { x: left, y: top } = origin;
  let Ok(strip_wide) = strip_room(card);
  let run_left = left
    .saturating_add(FINGER)
    .saturating_add(strip::GAP)
    .saturating_add(strip::PAD);
  let tab_wide = strip_wide.saturating_div(showing.max(1));
  let Ok(tab_wide_i) = fitted::<u32, i32>(tab_wide);
  let Ok(from) = index(window.from);
  let Ok(showing_many) = index(showing);
  let tab_face = font(TextStyle::Headline)?;

  for (index, (which, page)) in state
    .pages
    .iter()
    .enumerate()
    .skip(from)
    .take(showing_many)
    .enumerate()
  {
    let Ok(step) = fitted::<_, i32>(index);
    let Ok(tab) = fitted::<_, u32>(which);
    let here = tab == state.here;
    let cell = ShapePanel {
      at: Point {
        x: run_left.saturating_add(step.saturating_mul(tab_wide_i)),
        y: top,
      },
      size: Size {
        width: tab_wide,
        height: finger,
      },
      round: Round(TAB_ROUND),
      fill: wearing.panel,
      edge: Edge::None,
    };
    let said = match (measured.tabs.get(which), here) {
      (Some(measured_2), true) => measured_2.chosen,
      (Some(measured_3), false) => measured_3.title,
      (None, _) => NO_ROOM,
    };

    match ::core::primitive::bool::then_some(here, ()) {
      ::core::option::Option::Some(()) => {
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
          size: Size {
            width: pill_wide,
            height: finger,
          },
          round: Round(TAB_ROUND),
          fill: match state.leaving {
            Leaving::Standing => wearing.ground,
            Leaving::No => wearing.pink,
          },
          edge: Edge::None,
        }));
      }
      ::core::option::Option::None => {}
    }

    let (weight, ink) = match (here, state.leaving) {
      (true, Leaving::No) => (Weight::Bold, wearing.night),
      (true, Leaving::Standing) => (Weight::Bold, wearing.text),
      (false, _) => (Weight::Plain, wearing.soft),
    };

    let Ok(words) = centred(&page.title, said, &cell, weight, tab_face.clone(), ink);

    shapes.push(Shape::Text(words));
    touching.push(HitRegion {
      lands: Lands::Tab(tab),
      panel: cell,
    });
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
      Some(of_2) => wanted.push(of_2.to_string()),
      None => {}
    }
  }

  let Ok(missing) = crate::pictures::missing_pictures(&wanted);

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
  let Ok(wide) = console_core_number_conversion::toward_zero_u32(
    (f64::from(room.width) * zoom.by).min(SHARPEST),
  );
  let Ok(tall) = console_core_number_conversion::toward_zero_u32(
    (f64::from(room.height) * zoom.by).min(SHARPEST),
  );

  Ok(Size {
    width: wide,
    height: tall,
  })
}

fn zoomed_in(
  moving: &Moving,
  pixels: Pixels,
  zoom: crate::zoom::Zoom,
) -> Result<console_core_shapes::Cropped, Never> {
  let Ok(placed) = zoom.place(
    Size {
      width: pixels.width,
      height: pixels.height,
    },
    moving.room,
  );
  let Ok(across) = toward_zero_i32(placed.at.x);
  let Ok(down) = toward_zero_i32(placed.at.y);
  let Ok(wide) = console_core_number_conversion::toward_zero_u32(placed.size.width);
  let Ok(tall) = console_core_number_conversion::toward_zero_u32(placed.size.height);

  Ok(console_core_shapes::Cropped {
    at: Point {
      x: moving.at.x.saturating_add(across),
      y: moving.at.y.saturating_add(down),
    },
    size: Size {
      width: wide,
      height: tall,
    },
    pixels,
    from: placed.from,
    seen: placed.seen,
  })
}

fn place_picture<P: std::borrow::Borrow<Pixels>>(
  moving: &Moving,
  pixels: P,
) -> Result<ShapePicture<P>, Never> {
  let source = pixels.borrow();
  let Ok(size) = console_pictures::fit_within(
    Size {
      width: source.width,
      height: source.height,
    },
    moving.room,
  );
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

fn said_over<P>(
  moving: &Moving,
  picture: &ShapePicture<P>,
  said: &[String],
) -> Result<Vec<Shape>, Never> {
  match ::core::primitive::bool::then_some(said.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(Vec::new()),
    ::core::option::Option::None => {}
  }

  let said_2 = said.join("\n");
  let Ok(face) = font(TextStyle::Body);
  let wrap = picture.size.width.saturating_mul(9).saturating_div(10);
  let Ok(measured) = painting::measure_text(
    Run {
      said: &said_2,
      weight: Weight::Plain,
      width: wrap,
    },
    &face,
  );
  let Ok(wide) = fitted::<u32, i32>(measured.width);
  let Ok(tall) = fitted::<u32, i32>(measured.height);
  let Ok(picture_wide) = fitted::<u32, i32>(picture.size.width);
  let Ok(picture_tall) = fitted::<u32, i32>(picture.size.height);
  let pad = INSIDE_A_ROW.saturating_div(3);
  let Ok(padding) = fitted::<i32, u32>(pad.saturating_mul(2));
  let left = picture
    .at
    .x
    .saturating_add(picture_wide.saturating_sub(wide).saturating_div(2));
  let top = picture
    .at
    .y
    .saturating_add(picture_tall)
    .saturating_sub(tall)
    .saturating_sub(INSIDE_A_ROW);
  let backing = ShapePanel {
    at: Point {
      x: left.saturating_sub(pad),
      y: top.saturating_sub(pad),
    },
    size: Size {
      width: measured.width.saturating_add(padding),
      height: measured.height.saturating_add(padding),
    },
    round: Round(ROW_ROUND.saturating_div(2)),
    fill: moving.inks.ground,
    edge: Edge::None,
  };
  let words = Text {
    at: Point { x: left, y: top },
    width: measured.width.saturating_add(2),
    said: said_2,
    weight: Weight::Plain,
    font: face,
    ink: moving.inks.words,
  };

  Ok(vec![Shape::Panel(backing), Shape::Text(words)])
}

fn stirred(state: &State) -> Result<WakeOutcome, Never> {
  let Ok(page) = nth(&state.pages, state.here);

  Ok(match page.and_then(|page_2| page_2.stirs.as_ref()) {
    Some(stirs) => stirs(),
    None => WakeOutcome::AlreadyAwake,
  })
}

enum Overlay {
  Note,
  None,
}

fn wait_for_frame(
  surface: &mut Surface,
  panelled: &Panelled,
  logical: Size<u32>,
  was: (Dirty, Stale),
  overlay: Overlay,
) -> Result<(Dirty, Stale), console_draw_surface::SurfaceError> {
  let (mut dirty, mut stale) = was;
  crate::frames::with_waking(|waking| {
    let also: Vec<_> = waking.into_iter().collect();
    surface.wait(&also, None)
  })?;
  let Ok(woken) = crate::frames::wake_state();

  match woken.rows {
    crate::frames::FrameReceived::Yes => stale = Stale::Yes,
    crate::frames::FrameReceived::No => {}
  }

  match woken.card {
    crate::frames::FrameReceived::Yes => dirty = Dirty::Yes,
    crate::frames::FrameReceived::No => {}
  }

  match (woken.frame, dirty, &panelled.moving, overlay) {
    (crate::frames::FrameReceived::Yes, Dirty::No, Some(_), Overlay::Note) => dirty = Dirty::Yes,
    (crate::frames::FrameReceived::Yes, Dirty::No, Some(moving), Overlay::None) => {
      let Ok(repainted) = repainted(surface, moving, logical);

      match repainted {
        Visible::Yes => {}
        Visible::NotYet => dirty = Dirty::Yes,
      }
    }
    (crate::frames::FrameReceived::Yes, Dirty::No, None, _)
    | (crate::frames::FrameReceived::Yes, Dirty::Yes, _, _)
    | (crate::frames::FrameReceived::No, _, _, _) => {}
  }

  Ok((dirty, stale))
}

fn repainted(
  surface: &mut Surface,
  moving: &Moving,
  logical: Size<u32>,
) -> Result<Visible, Never> {
  let Ok(painted) = crate::frames::with_current(
    &moving.of,
    moving.room,
    surface,
    |surface_2, pixels, caption| {
      let Ok(picture) = place_picture(moving, pixels);
      let part = Part {
        at: picture.at,
        size: picture.size,
      };
      let local = ShapePicture {
        at: Point { x: 0, y: 0 },
        size: picture.size,
        pixels,
      };
      let Ok(caption_here) = said_over(moving, &local, caption);

      match surface_2.draw_layer(part, |target, device, _scale| {
        let _painted = painting::over_picture(
          target,
          Frame {
            device,
            points: part.size,
          },
          local,
          &caption_here,
        );
        Ok(())
      }) {
        Ok(Visible::Yes) => return Ok(Visible::Yes),
        Ok(Visible::NotYet) => {
          let Ok(()) = surface_2.clear_layer();
        }
        Err(fault) => {
          console_core_logging::warn!("console-panel: painting the video layer: {fault}");
          let Ok(()) = surface_2.clear_layer();
        }
      }

      let Ok(said) = said_over(moving, &picture, caption);

      Ok(
        match surface_2.draw_over(part, |target, device, _scale| {
          let _painted = painting::over_picture(
            target,
            Frame {
              device,
              points: logical,
            },
            picture,
            &said,
          );

          Ok(())
        }) {
          Ok(drawn) => drawn,
          Err(fault) => {
            console_core_logging::warn!(
              "console-panel: the frame could not be repainted: {fault}"
            );
            Visible::NotYet
          }
        },
      )
    },
  );

  Ok(match painted {
    Some(visible) => visible,
    None => Visible::Yes,
  })
}

pub(crate) fn stage(rows: &[Row], room: i32) -> Result<i32, Never> {
  let first = match rows.first() {
    Some(first) => first,
    None => return Ok(0),
  };

  let Ok(many) = fitted::<_, i32>(rows.len());
  let left_over = room
    .saturating_sub(many.saturating_mul(fitting::ROW))
    .max(0);

  match &first.headline {
    Some(_) => return Ok(left_over),
    None => {}
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
    Some(wanted_2) => crate::pictures::pixels(wanted_2),
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
  let Ok(wide) = fitted::<i32, u32>(card.width);
  let mut checkout = measured.rows.try_borrow_mut();
  let mut fallback = BTreeMap::new();
  let rows = match checkout.as_deref_mut() {
    Ok(cached) => cached,
    Err(_another_view_holds_the_cache) => &mut fallback,
  };
  let Ok(fixed) = fitted::<u32, u64>(standing.at);
  let Ok(identity) = RowIdentity::of(row, fixed);
  let Ok(measured_row_2) = cached_measurement(rows, row, identity, wide);
  let Ok(RowSurface {
    frame,
    lit,
    ink,
    quiet,
    words_x,
    moving,
  }) = row_surface(row, band, card, standing, wearing, &mut shapes);
  let RowFrame {
    top,
    tall,
    left,
    right,
    size,
  } = frame;
  let box_wide = size.width;
  let box_tall = size.height;

  let mut far = right.saturating_sub(INSIDE_A_ROW);
  let Ok(wears) = crate::page::wears(row);

  match wears {
    Wears::WhatElse => {
      let (drawn, reached) = button(
        Button {
          at: Point {
            x: right.saturating_sub(FINGER),
            y: top,
          },
          size: Size {
            width: FINGER.unsigned_abs(),
            height: box_tall,
          },
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

      far = right
        .saturating_sub(FINGER)
        .saturating_sub(strip::GAP.saturating_mul(3));
    }
    Wears::None => {}
  }

  match ::core::primitive::bool::then_some(row.opens, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(into_wide) = fitted::<u32, i32>(measured.marks.into.width);
      let Ok(down) = middle(Band { top, height: tall }, measured.marks.into);

      let Ok(face_body) = font(TextStyle::Body);

      shapes.push(Shape::Text(Text {
        at: Point {
          x: far.saturating_sub(into_wide),
          y: down,
        },
        width: measured.marks.into.width.saturating_add(2),
        said: marks::INTO.to_string(),
        weight: Weight::Plain,
        font: face_body,
        ink: quiet,
      }));

      far = far
        .saturating_sub(into_wide)
        .saturating_sub(strip::GAP.saturating_mul(3));
    }
    ::core::option::Option::None => {}
  }

  let Ok(far_2) = selected_mark(
    &mut shapes,
    standing.selected,
    Band { top, height: tall },
    (far, lit),
    wearing,
  );

  match &row.buttons {
    Some(across) => {
      let strip = ButtonStrip {
        band: Band { top, height: tall },
        span: Span {
          from: left,
          to: far_2.saturating_add(INSIDE_A_ROW),
        },
        standing,
        lit,
      };
      let Ok((drawn, reached)) = press_shapes(across, measured_row_2, strip, wearing);

      shapes.extend(drawn);
      touching.extend(reached);

      return Ok(Panelled {
        shapes,
        touching,
        moving,
      });
    }
    None => {}
  }

  let text = RowText {
    band: Band { top, height: tall },
    words_x,
    far: far_2,
    width: box_wide,
    lit,
    ink,
    quiet,
  };
  let Ok(()) = row_words(
    row,
    measured_row_2,
    standing,
    text,
    wearing,
    RowDrawing {
      shapes: &mut shapes,
      touching: &mut touching,
    },
  );

  Ok(Panelled {
    shapes,
    touching,
    moving,
  })
}

fn cached_measurement<'a>(
  rows: &'a mut BTreeMap<RowIdentity, CachedRow>,
  row: &Row,
  identity: RowIdentity,
  wide: u32,
) -> Result<&'a MeasuredRow, Never> {
  let cached = rows.entry(identity).or_insert_with(|| {
    let Ok(measured) = measure_row(row, wide);

    CachedRow {
      row: row.clone(),
      measured,
    }
  });
  let Ok(measurements_match) = same_measurements(&cached.row, row);

  match measurements_match {
    crate::page::Same::No => {
      let Ok(measured_2) = measure_row(row, wide);
      *cached = CachedRow {
        row: row.clone(),
        measured: measured_2,
      };
    }
    crate::page::Same::Yes => {}
  }

  Ok(&cached.measured)
}

struct RowFrame {
  top: i32,
  tall: i32,
  left: i32,
  right: i32,
  size: Size<u32>,
}

struct RowSurface {
  frame: RowFrame,
  lit: Lit,
  ink: Oklch,
  quiet: Oklch,
  words_x: i32,
  moving: Option<Moving>,
}

fn row_surface(
  row: &Row,
  band: Band,
  card: &Card,
  standing: Standing,
  wearing: &Wearing,
  shapes: &mut Vec<Shape>,
) -> Result<RowSurface, Never> {
  let Ok(looked) = looked_at(&row.picture);
  let bleeds = match (standing.opened, looked) {
    (Opened::Expanded, Some(_)) => Bleeds::Yes,
    (Opened::Expanded, None) | (Opened::No, _) => Bleeds::No,
  };
  let (left, right, top, tall) = match bleeds {
    Bleeds::Yes => (
      card.x,
      card.x.saturating_add(card.width),
      band.top,
      band.height,
    ),
    Bleeds::No => {
      let Ok(left) = inside_left(card);
      let Ok(right) = inside_right(card);

      (
        left,
        right,
        band.top.saturating_add(BETWEEN_ROWS.saturating_div(2)),
        band.height.saturating_sub(BETWEEN_ROWS),
      )
    }
  };
  let Ok(box_tall) = fitted::<i32, u32>(tall);
  let Ok(box_wide) = width(right.saturating_sub(left));
  let frame = RowFrame {
    top,
    tall,
    left,
    right,
    size: Size {
      width: box_wide,
      height: box_tall,
    },
  };
  let Ok(heading) = row.heading();
  let lit = match (standing.highlight, standing.beside) {
    (Highlight::Yes, Beside::No) => Lit::Row,
    (Highlight::Yes, Beside::Yes) => Lit::Beside,
    (Highlight::No, _) => Lit::No,
  };

  let ground = match (lit, heading) {
    (Lit::Row, _) => Some((wearing.pink, Edge::None)),
    (Lit::Beside, _) => Some((
      wearing.ground,
      Edge::Of {
        wide: CARD_EDGE,
        color: wearing.pink,
      },
    )),
    (Lit::No, Heading::No) => Some((wearing.ground, Edge::None)),
    (Lit::No, Heading::Yes) => None,
  };

  let ground_2 = match (bleeds, &row.buttons) {
    (Bleeds::No, None) => ground,
    (Bleeds::Yes, _) | (Bleeds::No, Some(_)) => None,
  };

  match ground_2 {
    Some((fill, edge)) => shapes.push(Shape::Panel(ShapePanel {
      at: Point { x: left, y: top },
      size: Size {
        width: box_wide,
        height: box_tall,
      },
      round: Round(ROW_ROUND),
      fill,
      edge,
    })),
    None => {}
  }

  let Ok(progress) = row.progress();
  let (ink, quiet) = match (lit, heading, progress) {
    (Lit::Row, _, _) => (wearing.night, wearing.night),
    (Lit::Beside | Lit::No, Heading::Yes, _)
    | (Lit::Beside | Lit::No, Heading::No, Progress::Running) => (wearing.soft, wearing.soft),
    (Lit::Beside | Lit::No, Heading::No, Progress::Idle) => (wearing.text, wearing.soft),
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
        size: Size {
          width: square,
          height: square,
        },
        pixels,
      }));
    }
    None => {
      let Ok(named) = named_shapes(&row.picture, Band { top, height: tall }, left, lit, wearing);

      shapes.extend(named);
    }
  }

  let Ok(moving) = row_moving(looked, bleeds, &frame, standing, wearing, shapes);

  Ok(RowSurface {
    frame,
    lit,
    ink,
    quiet,
    words_x,
    moving,
  })
}

fn row_moving(
  looked: Option<&Path>,
  bleeds: Bleeds,
  frame: &RowFrame,
  standing: Standing,
  wearing: &Wearing,
  shapes: &mut Vec<Shape>,
) -> Result<Option<Moving>, Never> {
  let left = frame.left;
  let top = frame.top;
  let Size {
    width: box_wide,
    height: box_tall,
  } = frame.size;
  let moving = match looked {
    Some(path) => {
      let edges = match bleeds {
        Bleeds::Yes => 0,
        Bleeds::No => CARD_EDGE.saturating_mul(4),
      };
      let Ok(inset) = fitted::<u32, i32>(edges);
      let moving = Moving {
        of: path.to_path_buf(),
        inks: Inks {
          ground: wearing.night,
          words: wearing.text,
        },
        at: Point {
          x: left.saturating_add(inset),
          y: top.saturating_add(inset),
        },
        room: Size {
          width: box_wide.saturating_sub(edges.saturating_mul(2)),
          height: box_tall.saturating_sub(edges.saturating_mul(2)),
        },
      };
      let Ok(now) = crate::frames::snapshot(path, moving.room);

      match (now, standing.zoom.zoomed()) {
        (Some((pixels, caption)), Ok(crate::zoom::Zoomed::Whole)) => {
          let Ok(picture) = place_picture(&moving, pixels);
          let Ok(said) = said_over(&moving, &picture, &caption);

          shapes.push(Shape::Picture(picture));
          shapes.extend(said);
        }
        (Some((pixels, _caption)), Ok(crate::zoom::Zoomed::ZoomedIn)) => {
          let Ok(sharper) = sharper_room(moving.room, standing.zoom);
          let Ok(()) = crate::frames::sharpened(path, sharper);
          let Ok(picture) = zoomed_in(&moving, pixels, standing.zoom);

          shapes.push(Shape::Cropped(picture));
        }
        (None, _) => {}
      }

      Some(moving)
    }
    None => None,
  };

  Ok(moving)
}

struct RowText {
  band: Band,
  words_x: i32,
  far: i32,
  width: u32,
  lit: Lit,
  ink: Oklch,
  quiet: Oklch,
}

struct RowDrawing<'a> {
  shapes: &'a mut Vec<Shape>,
  touching: &'a mut Vec<HitRegion>,
}

fn row_words(
  row: &Row,
  measured_row_2: &MeasuredRow,
  standing: Standing,
  text: RowText,
  wearing: &Wearing,
  drawing: RowDrawing<'_>,
) -> Result<(), Never> {
  let RowText {
    band: Band { top, height: tall },
    words_x,
    far: far_2,
    width: box_wide,
    lit,
    ink,
    quiet,
  } = text;
  let line = Line {
    row: standing.at,
    top,
    height: tall,
    far: far_2,
    ink,
  };
  let Ok(holds) = crate::page::holds(row);

  let (said, pressed, far_3) = match holds {
    Holds::Level => level_shapes(row, measured_row_2, line, wearing)?,
    Holds::Control(Control::Switch(switch)) => switch_shapes(switch, line, wearing)?,
    Holds::Control(Control::Slider(slider)) => slider_shapes(
      slider,
      (row, measured_row_2),
      (line, words_x),
      (lit, wearing),
    )?,
    Holds::Choices => picker_shapes(row, measured_row_2, (line, words_x), wearing)?,
    Holds::None => aside_shapes(row, measured_row_2, Line { ink: quiet, ..line }, box_wide)?,
  };

  drawing.shapes.extend(said);
  drawing.touching.extend(pressed);

  let Ok(()) = bar_after_the_words(
    row,
    (&mut *drawing.shapes, &mut *drawing.touching),
    (Line { far: far_3, ..line }, words_x, lit),
    measured_row_2,
    wearing,
  );

  let cells_ink = match (lit, row.naming) {
    (Lit::Row, _) => wearing.night,
    (Lit::Beside | Lit::No, true) => wearing.soft,
    (Lit::Beside | Lit::No, false) => wearing.text,
  };
  let Ok(weight) = weight_of(row);
  let across_the_row = Span {
    from: words_x,
    to: far_3,
  };

  let Ok(cells) = cell_shapes(
    row,
    measured_row_2,
    Band { top, height: tall },
    across_the_row,
    Inked {
      ink: cells_ink,
      weight,
      wearing,
    },
  );

  drawing.shapes.extend(cells);

  let Ok(down) = middle(Band { top, height: tall }, measured_row_2.says);

  let Ok(wide_2) = width(far_3.saturating_sub(words_x));
  let Ok(face) = font(TextStyle::Body);

  drawing.shapes.push(Shape::Text(Text {
    at: Point {
      x: words_x,
      y: down,
    },
    width: wide_2,
    said: row.says.clone(),
    weight,
    font: face,
    ink,
  }));

  Ok(())
}

fn bar_after_the_words(
  row: &Row,
  (shapes, touching): (&mut Vec<Shape>, &mut Vec<HitRegion>),
  (line, words_x, lit): (Line, i32, Lit),
  measured_row: &MeasuredRow,
  wearing: &Wearing,
) -> Result<(), Never> {
  let Ok(worn) = ControlInks::worn(wearing);
  let (set, knob) = match lit {
    Lit::Row => (wearing.night, wearing.night),
    Lit::Beside | Lit::No => (wearing.pink, wearing.text),
  };
  let inks = ControlInks {
    track: wearing.soft,
    set,
    knob,
    ..worn
  };
  let Ok(says_wide) = fitted::<u32, i32>(measured_row.says.width);
  let mut track = Span {
    from: words_x
      .saturating_add(says_wide)
      .saturating_add(strip::GAP.saturating_mul(3)),
    to: line.far,
  };
  let Ok(bars) = bars(row);
  let Ok(holds) = crate::page::holds(row);
  let Ok((less, _more)) = crate::page::ends_of(row);

  match (bars, holds, less.is_empty()) {
    (Bars::Yes, Holds::Level, false) => {
      let less_2 = Nudge {
        said: less,
        measured: measured_row.less,
        step: -1,
      };
      let Ok(()) = nudge_shapes(
        (&mut *shapes, &mut *touching),
        less_2,
        (line, track.from),
        wearing,
      );

      track.from = track
        .from
        .saturating_add(FINGER)
        .saturating_add(strip::GAP.saturating_mul(2));
    }
    (Bars::Yes, Holds::Level, true)
    | (Bars::Yes, Holds::Control(_) | Holds::Choices | Holds::None, _)
    | (Bars::No, _, _) => {}
  }

  let Ok((drawn, reached)) = bar_shapes(row, (line, inks), track);

  shapes.extend(drawn);
  touching.extend(reached);

  Ok(())
}

fn selected_mark(
  shapes: &mut Vec<Shape>,
  selected: Selected,
  band: Band,
  (far, lit): (i32, Lit),
  wearing: &Wearing,
) -> Result<i32, Never> {
  match selected {
    Selected::No => return Ok(far),
    Selected::Yes => {}
  }

  let Ok(side) = fitted::<i32, u32>(SELECTED_MARK);
  let mark = Shape::Panel(ShapePanel {
    at: Point {
      x: far.saturating_sub(SELECTED_MARK),
      y: band
        .top
        .saturating_add(band.height.saturating_sub(SELECTED_MARK).saturating_div(2)),
    },
    size: Size {
      width: side,
      height: side,
    },
    round: Round(side.saturating_div(2)),
    fill: match lit {
      Lit::Row => wearing.night,
      Lit::Beside | Lit::No => wearing.pink,
    },
    edge: Edge::None,
  });

  shapes.push(mark);

  Ok(
    far
      .saturating_sub(SELECTED_MARK)
      .saturating_sub(strip::GAP.saturating_mul(3)),
  )
}

fn named_shapes(
  picture: &Picture,
  band: Band,
  left: i32,
  lit: Lit,
  wearing: &Wearing,
) -> Result<Option<Shape>, Never> {
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
      y: band
        .top
        .saturating_add(band.height.saturating_sub(strip::PICTURE).saturating_div(2)),
    },
    size: Size {
      width: square,
      height: square,
    },
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
  painting::measure_text(
    Run {
      said,
      weight,
      width: ANYTHING_WIDE,
    },
    face,
  )
}

fn text_shape(
  said: &str,
  at: Point<i32>,
  weight: Weight,
  face: Font,
  ink: Oklch,
) -> Result<(Shape, Size<u32>), Never> {
  let Ok(measured) = measured_in(said, weight, &face);

  Ok((
    Shape::Text(Text {
      at,
      width: measured.width.saturating_add(2),
      said: said.to_string(),
      weight,
      font: face,
      ink,
    }),
    measured,
  ))
}

fn render_row(
  row: &Row,
  band: Band,
  card: &Card,
  standing: Standing,
  measured: &Measured<'_>,
  wearing: &Wearing,
) -> Result<Panelled, Never> {
  match &row.headline {
    Some(headline) => match headline.alignment {
      crate::page::Alignment::Leading => headline_shapes(headline, row, band, card, wearing),
      crate::page::Alignment::Trailing => trailing_headline_shapes(headline, band, card, wearing),
    },
    None => row_shapes(row, band, card, standing, measured, wearing),
  }
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
  let Ok(wide) = width(right.saturating_sub(left));
  let Ok(tall) = width(band.height.saturating_sub(BETWEEN_ROWS));
  let within = ShapePanel {
    at: Point {
      x: left,
      y: band.top.saturating_add(BETWEEN_ROWS.saturating_div(2)),
    },
    size: Size {
      width: wide,
      height: tall,
    },
    round: Round(ROW_ROUND),
    fill: wearing.ground,
    edge: Edge::None,
  };
  let mut shapes = vec![Shape::Panel(within)];
  let Ok(box_wide) = fitted::<u32, i32>(within.size.width);
  let Ok(box_tall) = fitted::<u32, i32>(within.size.height);
  let left_2 = within.at.x.saturating_add(INSIDE_A_ROW);
  let top = within.at.y.saturating_add(INSIDE_A_ROW);
  let bottom = within
    .at
    .y
    .saturating_add(box_tall)
    .saturating_sub(INSIDE_A_ROW);

  let Ok(()) = headline_titles(headline, Point { x: left_2, y: top }, wearing, &mut shapes);
  let Ok(()) = headline_summary(
    headline,
    Point {
      x: left_2,
      y: bottom,
    },
    wearing,
    &mut shapes,
  );

  let moving = match looked {
    Some(path) => {
      let Ok(inset) = fitted::<u32, i32>(CARD_EDGE.saturating_mul(4));
      let wide_2 = box_wide.saturating_mul(9).saturating_div(20);
      let Ok(room_wide) = width(wide_2.saturating_sub(inset));
      let Ok(room_tall) = width(box_tall.saturating_sub(inset.saturating_mul(2)));
      let moving = Moving {
        of: path.to_path_buf(),
        inks: Inks {
          ground: wearing.night,
          words: wearing.text,
        },
        at: Point {
          x: within.at.x.saturating_add(box_wide).saturating_sub(wide_2),
          y: within.at.y.saturating_add(inset),
        },
        room: Size {
          width: room_wide,
          height: room_tall,
        },
      };
      let Ok(now) = crate::frames::snapshot(path, moving.room);

      match now {
        Some((pixels, caption)) => {
          let Ok(picture) = place_picture(&moving, pixels);
          let Ok(said) = said_over(&moving, &picture, &caption);

          shapes.push(Shape::Picture(picture));
          shapes.extend(said);
        }
        None => {}
      }

      Some(moving)
    }
    None => None,
  };

  Ok(Panelled {
    shapes,
    touching: Vec::new(),
    moving,
  })
}

fn headline_titles(
  headline: &crate::page::Headline,
  origin: Point<i32>,
  wearing: &Wearing,
  shapes: &mut Vec<Shape>,
) -> Result<(), Never> {
  let Point { x: left_2, y: top } = origin;
  let Ok(title_face) = font(TextStyle::Title);
  let Ok((title, title_said)) = text_shape(
    &headline.title,
    Point { x: left_2, y: top },
    Weight::Bold,
    title_face,
    wearing.text,
  );
  let Ok(title_tall) = fitted::<u32, i32>(title_said.height);
  let Ok(subtitle_face) = font(TextStyle::Footnote);
  let Ok((subtitle, _)) = text_shape(
    &headline.subtitle,
    Point {
      x: left_2,
      y: top.saturating_add(title_tall),
    },
    Weight::Plain,
    subtitle_face,
    wearing.soft,
  );

  shapes.push(title);
  shapes.push(subtitle);

  Ok(())
}

fn headline_summary(
  headline: &crate::page::Headline,
  base: Point<i32>,
  wearing: &Wearing,
  shapes: &mut Vec<Shape>,
) -> Result<(), Never> {
  let Point {
    x: left_2,
    y: bottom,
  } = base;
  let big_face = Font {
    family: console_core_fonts::LETTERS.to_string(),
    height: BIG,
  };
  let Ok(big_said) = measured_in(&headline.big, Weight::Plain, &big_face);
  let Ok(big_tall) = fitted::<u32, i32>(big_said.height);
  let Ok(big_wide) = fitted::<u32, i32>(big_said.width);
  let big_top = bottom.saturating_sub(big_tall);
  let Ok((big, _)) = text_shape(
    &headline.big,
    Point {
      x: left_2,
      y: big_top,
    },
    Weight::Plain,
    big_face,
    wearing.text,
  );

  shapes.push(big);

  let beside = left_2
    .saturating_add(big_wide)
    .saturating_add(INSIDE_A_ROW.saturating_mul(2));
  headline_details(
    headline,
    beside,
    Band {
      top: big_top,
      height: big_tall,
    },
    wearing,
    shapes,
  )
}

fn headline_details(
  headline: &crate::page::Headline,
  beside: i32,
  band: Band,
  wearing: &Wearing,
  shapes: &mut Vec<Shape>,
) -> Result<(), Never> {
  let Band {
    top: big_top,
    height: big_tall,
  } = band;
  let glyph_face = Font {
    family: console_core_fonts::ICONS.to_string(),
    height: BIG_GLYPH,
  };
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
  let words = beside
    .saturating_add(glyph_wide)
    .saturating_add(strip::GAP.saturating_mul(3));
  let line = Band {
    top: block_top,
    height: line_tall,
  };
  let Ok(glyph_down) = middle(line, glyph_said);
  let Ok(says_down) = middle(line, says_said);
  let Ok((glyph_2, _)) = text_shape(
    glyph,
    Point {
      x: beside,
      y: glyph_down,
    },
    Weight::Plain,
    glyph_face,
    wearing.pink,
  );
  let Ok((says, _)) = text_shape(
    &headline.says,
    Point {
      x: words,
      y: says_down,
    },
    Weight::Bold,
    says_face,
    wearing.text,
  );
  let Ok((aside, _)) = text_shape(
    &headline.aside,
    Point {
      x: words,
      y: block_top.saturating_add(line_tall),
    },
    Weight::Plain,
    aside_face,
    wearing.soft,
  );

  shapes.push(glyph_2);
  shapes.push(says);
  shapes.push(aside);

  Ok(())
}

fn trailing_headline_shapes(
  headline: &crate::page::Headline,
  band: Band,
  card: &Card,
  wearing: &Wearing,
) -> Result<Panelled, Never> {
  let Ok(left) = inside_left(card);
  let Ok(right) = inside_right(card);
  let Ok(wide) = width(right.saturating_sub(left));
  let thin = CARD_EDGE.max(1);
  let rule_at = band
    .top
    .saturating_add(band.height)
    .saturating_sub(INSIDE_A_ROW);
  let rule = ShapePanel {
    at: Point {
      x: left,
      y: rule_at,
    },
    size: Size {
      width: wide,
      height: thin,
    },
    round: Round(0),
    fill: wearing.edge,
    edge: Edge::None,
  };
  let end = right.saturating_sub(strip::GAP);

  let Ok(title_face) = font(TextStyle::Title);
  let Ok(title_said) = measured_in(&headline.title, Weight::Plain, &title_face);
  let Ok(title_tall) = fitted::<u32, i32>(title_said.height);
  let room = rule_at
    .saturating_sub(band.top)
    .saturating_sub(INSIDE_A_ROW);
  let Ok(room_2) = width(room);
  let big_face = Font {
    family: console_core_fonts::LETTERS.to_string(),
    height: BIG.saturating_mul(3).min(room_2).max(1),
  };
  let Ok(big_said) = measured_in(&headline.big, Weight::Plain, &big_face);
  let Ok(big_tall) = fitted::<u32, i32>(big_said.height);
  let Ok(big_wide) = fitted::<u32, i32>(big_said.width);
  let big_top = rule_at
    .saturating_sub(INSIDE_A_ROW)
    .saturating_sub(big_tall);
  let Ok((big, _)) = text_shape(
    &headline.big,
    Point {
      x: end.saturating_sub(big_wide),
      y: big_top,
    },
    Weight::Plain,
    big_face,
    wearing.text,
  );
  let title_top = big_top.saturating_add(big_tall).saturating_sub(title_tall);
  let Ok((title, _)) = text_shape(
    &headline.title,
    Point {
      x: left.saturating_add(strip::GAP),
      y: title_top,
    },
    Weight::Plain,
    title_face,
    wearing.soft,
  );

  Ok(Panelled {
    shapes: vec![title, big, Shape::Panel(rule)],
    touching: Vec::new(),
    moving: None,
  })
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
  let Ok(many) = fitted::<_, i32>(
    strip_of
      .presses
      .iter()
      .filter(|press| press.placed == crate::page::Placed::Among)
      .count(),
  );
  let gap = strip::GAP.saturating_mul(2);
  let room = strip.span.to.saturating_sub(strip.span.from);
  let each = match strip_of.presses.first().map(|press| press.face) {
    Some(crate::page::Face::Swatch(_)) => FINGER,
    Some(crate::page::Face::Glyph(_)) | None => PRESS_WIDE,
    Some(crate::page::Face::Written(_)) => match room
      .saturating_sub(many.saturating_sub(1).max(0).saturating_mul(gap))
      .checked_div(many)
    {
      Some(shared) => shared.max(PRESS_WIDE),
      None => PRESS_WIDE,
    },
  };
  let whole = many
    .saturating_mul(each)
    .saturating_add(many.saturating_sub(1).max(0).saturating_mul(gap));
  let first = strip
    .span
    .from
    .saturating_add(room.saturating_sub(whole).max(0).saturating_div(2));
  let Ok(wide) = width(each);
  let Ok(tall) = width(strip.band.height.saturating_sub(gap));
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
      crate::page::Placed::Frame => corner,
    };
    let panel = ShapePanel {
      at: Point { x: across, y: down },
      size: Size {
        width: wide,
        height: tall,
      },
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
          (Highlight::Yes, _) => Edge::Of {
            wide: SWATCH_EDGE.saturating_mul(2),
            color: wearing.pink,
          },
          (Highlight::No, crate::page::Active::Yes) => Edge::Of {
            wide: SWATCH_EDGE,
            color: wearing.text,
          },
          (Highlight::No, crate::page::Active::No) => Edge::None,
        };

        shapes.push(Shape::Panel(ShapePanel {
          fill: color,
          edge,
          ..panel
        }));
        touching.push(HitRegion {
          lands: Lands::ButtonPress {
            row: strip.standing.at,
            which,
          },
          panel,
        });

        continue;
      }
    };
    let (fill, ink) = match (stood, press.now, press.style) {
      (Highlight::Yes, _, _) => (wearing.pink, wearing.night),
      (Highlight::No, crate::page::Active::Yes, crate::page::ButtonStyle::Gray) => {
        (wearing.night, wearing.pink)
      }
      (Highlight::No, crate::page::Active::Yes, crate::page::ButtonStyle::Tinted) => {
        (wearing.ground, wearing.pink)
      }
      (Highlight::No, crate::page::Active::No, crate::page::ButtonStyle::Gray) => {
        (wearing.night, wearing.text)
      }
      (Highlight::No, crate::page::Active::No, crate::page::ButtonStyle::Tinted) => {
        (wearing.ground, wearing.text)
      }
      (Highlight::No, _, crate::page::ButtonStyle::Filled) => (wearing.text, wearing.night),
    };
    let panel_2 = ShapePanel { fill, ..panel };
    let said = match measured_row.presses.get(at) {
      Some(said) => *said,
      None => NO_ROOM,
    };
    let shown = match (icon, strip.standing.opened) {
      (Some(crate::icons::Icon::FullScreen), Opened::Expanded) => {
        Some(crate::icons::Icon::LeaveFullScreen)
      }
      (icon_2, Opened::Expanded | Opened::No) => icon_2,
    };
    let Ok(glyph) = match shown {
      Some(icon_3) => icon_3.glyph(),
      None => Ok(""),
    };
    let title = font(TextStyle::Title)?;
    let (glyph_2, set_in) = match press.face {
      crate::page::Face::Written(says) => (says, title),
      crate::page::Face::Glyph(_) | crate::page::Face::Swatch(_) => (glyph, face.clone()),
    };
    let Ok(words) = centred(glyph_2, said, &panel_2, Weight::Plain, set_in, ink);

    shapes.push(Shape::Panel(panel_2));
    shapes.push(Shape::Text(words));
    touching.push(HitRegion {
      lands: Lands::ButtonPress {
        row: strip.standing.at,
        which,
      },
      panel: panel_2,
    });
  }

  Ok((shapes, touching))
}

fn bar_shapes(
  row: &Row,
  (line, inks): (Line, ControlInks),
  track: Span,
) -> Result<(Vec<Shape>, Vec<HitRegion>), Never> {
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

  let wide = track.to.saturating_sub(track.from);

  match ::core::primitive::bool::then_some(wide > 0, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok((Vec::new(), Vec::new())),
  }

  let Ok(slider) = bar.slider();
  let Ok(frame) = control_frame(
    Line {
      far: track.to,
      ..line
    },
    wide,
  );
  let Ok(shapes) = slider.shapes(frame, inks);

  let touching = match ::core::primitive::bool::then_some(row.seek.is_some(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(tall) = width(line.height);
      let Ok(across) = width(wide);

      vec![HitRegion {
        lands: Lands::Seek {
          row: line.row,
          frame,
        },
        panel: ShapePanel {
          at: Point {
            x: track.from,
            y: line.top,
          },
          size: Size {
            width: across,
            height: tall,
          },
          round: Round(0),
          fill: inks.track,
          edge: Edge::None,
        },
      }]
    }
    ::core::option::Option::None => Vec::new(),
  };

  Ok((shapes, touching))
}

struct Inked<'a> {
  ink: Oklch,
  weight: Weight,
  wearing: &'a Wearing,
}

fn cell_shapes(
  row: &Row,
  measured_row: &MeasuredRow,
  band: Band,
  span: Span,
  inked: Inked<'_>,
) -> Result<Vec<Shape>, Never> {
  let mut shapes = Vec::new();
  let Ok(many) = fitted::<_, i32>(row.cells.len());
  let each = match many {
    0 => return Ok(shapes),
    many_2 => span.to.saturating_sub(span.from).saturating_div(many_2),
  };
  let Ok(wide) = width(each);
  let Ok(tall) = width(band.height);
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
        let Ok((drawn_glyph, _)) = text_shape(
          glyph,
          Point {
            x: start,
            y: glyph_down,
          },
          Weight::Plain,
          glyph_face,
          inked.wearing.pink,
        );
        let Ok((drawn_said, _)) = text_shape(
          &cell.says,
          Point {
            x: start.saturating_add(glyph_wide).saturating_add(gap),
            y: said_down,
          },
          inked.weight,
          face.clone(),
          inked.ink,
        );

        shapes.push(drawn_glyph);
        shapes.push(drawn_said);

        continue;
      }
      None => {}
    }

    let within = ShapePanel {
      at: Point {
        x: from.saturating_add(each.saturating_sub(lit_wide_i).saturating_div(2)),
        y: band.top,
      },
      size: Size {
        width: lit_wide,
        height: tall,
      },
      round: Round(lit_wide.saturating_div(2)),
      fill: inked.wearing.pink,
      edge: Edge::None,
    };
    let ink = match ::core::primitive::bool::then_some(cell.now, ()) {
      ::core::option::Option::Some(()) => {
        shapes.push(Shape::Panel(within));

        inked.wearing.night
      }
      ::core::option::Option::None => inked.ink,
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
      size: Size {
        width: FINGER.unsigned_abs(),
        height: tall,
      },
      said: nudge.said,
      measured: nudge.measured,
      lands: Lands::Nudge {
        row: line.row,
        step: nudge.step,
      },
      stood: Highlight::No,
    },
    wearing,
  )?;

  shapes.extend(drawn);
  touching.push(reached);

  Ok(())
}

fn level_said_shapes(
  shapes: &mut Vec<Shape>,
  row: &Row,
  measured_row: &MeasuredRow,
  line: Line,
) -> Result<i32, Never> {
  match ::core::primitive::bool::then_some(row.aside.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(line.far),
    ::core::option::Option::None => {}
  }

  let Ok(aside_wide) = fitted::<u32, i32>(measured_row.aside.width);
  let Ok(kept) = fitted::<u32, i32>(measured_row.aside.width.max(LEVEL_SAID));
  let Ok(down) = middle(
    Band {
      top: line.top,
      height: line.height,
    },
    measured_row.aside,
  );

  let Ok(face_callout) = font(TextStyle::Callout);

  shapes.push(Shape::Text(Text {
    at: Point {
      x: line.far.saturating_sub(aside_wide),
      y: down,
    },
    width: measured_row.aside.width.saturating_add(2),
    said: row.aside.clone(),
    weight: Weight::Plain,
    font: face_callout,
    ink: line.ink,
  }));

  Ok(
    line
      .far
      .saturating_sub(kept)
      .saturating_sub(strip::GAP.saturating_mul(2)),
  )
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
    }
    Bars::No => {}
  }

  match ::core::primitive::bool::then_some(more.is_empty(), ()) {
    ::core::option::Option::None => {
      let more_2 = Nudge {
        said: more,
        measured: measured_row.more,
        step: 1,
      };
      let Ok(()) = nudge_shapes(
        (&mut shapes, &mut touching),
        more_2,
        (line, far.saturating_sub(FINGER)),
        wearing,
      );

      far = far
        .saturating_sub(FINGER)
        .saturating_sub(strip::GAP.saturating_mul(2));
    }
    ::core::option::Option::Some(()) => {}
  }

  match bars {
    Bars::Yes => return Ok((shapes, touching, far)),
    Bars::No => {}
  }

  let Ok(before_the_level) =
    level_said_shapes(&mut shapes, row, measured_row, Line { far, ..line });

  far = before_the_level;

  match ::core::primitive::bool::then_some(less.is_empty(), ()) {
    ::core::option::Option::None => {
      let less_2 = Nudge {
        said: less,
        measured: measured_row.less,
        step: -1,
      };
      let Ok(()) = nudge_shapes(
        (&mut shapes, &mut touching),
        less_2,
        (line, far.saturating_sub(FINGER)),
        wearing,
      );

      far = far
        .saturating_sub(FINGER)
        .saturating_sub(strip::GAP.saturating_mul(3));
    }
    ::core::option::Option::Some(()) => {}
  }

  Ok((shapes, touching, far))
}

fn control_frame(line: Line, wide: i32) -> Result<Rectangle<i32>, Never> {
  let top = line
    .top
    .saturating_add(line.height.saturating_sub(CONTROL_TALL).saturating_div(2));

  Ok(Rectangle {
    origin: Point {
      x: line.far.saturating_sub(wide),
      y: top,
    },
    size: Size {
      width: wide,
      height: CONTROL_TALL,
    },
  })
}

fn switch_shapes(
  switch: Switch,
  line: Line,
  wearing: &Wearing,
) -> Result<(Vec<Shape>, Vec<HitRegion>, i32), Never> {
  let Ok(tall) = width(CONTROL_TALL);
  let Ok(sized) = Switch::sized(tall);
  let Ok(wide) = fitted::<u32, i32>(sized.width);
  let Ok(frame) = control_frame(line, wide);
  let Ok(inks) = ControlInks::worn(wearing);
  let Ok(shapes) = switch.shapes(frame, inks);

  Ok((
    shapes,
    Vec::new(),
    frame.origin.x.saturating_sub(strip::GAP.saturating_mul(4)),
  ))
}

fn slider_shapes(
  slider: Slider,
  (row, measured_row): (&Row, &MeasuredRow),
  (line, words_x): (Line, i32),
  (lit, wearing): (Lit, &Wearing),
) -> Result<(Vec<Shape>, Vec<HitRegion>, i32), Never> {
  let mut shapes = Vec::new();
  let Ok(far) = level_said_shapes(&mut shapes, row, measured_row, line);
  let Ok(says_wide) = fitted::<u32, i32>(measured_row.says.width);
  let from = words_x
    .saturating_add(says_wide)
    .saturating_add(strip::GAP.saturating_mul(3));
  let wide = far.saturating_sub(from).clamp(0, SLIDER_MOST);
  let Ok(frame) = control_frame(Line { far, ..line }, wide);
  let Ok(worn) = ControlInks::worn(wearing);

  let inks = match lit {
    Lit::Row => ControlInks {
      knob: wearing.night,
      ..worn
    },
    Lit::Beside | Lit::No => worn,
  };

  let Ok(drawn) = slider.shapes(frame, inks);
  let Ok(across) = width(wide);
  let Ok(tall) = width(line.height);
  let reached = HitRegion {
    lands: Lands::Slider {
      row: line.row,
      frame,
    },
    panel: ShapePanel {
      at: Point {
        x: frame.origin.x,
        y: line.top,
      },
      size: Size {
        width: across,
        height: tall,
      },
      round: Round(0),
      fill: wearing.panel,
      edge: Edge::None,
    },
  };

  shapes.extend(drawn);

  Ok((
    shapes,
    vec![reached],
    frame.origin.x.saturating_sub(strip::GAP.saturating_mul(3)),
  ))
}

fn choices_of(choices: &Choices) -> Result<Vec<Choice<'_>>, Never> {
  let Ok(face) = font(TextStyle::Body);

  Ok(
    choices
      .said
      .iter()
      .map(|said| {
        let Ok(measured) = measured_in(said, Weight::Plain, &face);

        Choice {
          said,
          wide: measured.width,
        }
      })
      .collect(),
  )
}

fn picker_frame(picker: &Picker<'_>, (line, from): (Line, i32)) -> Result<Rectangle<i32>, Never> {
  let Ok(tall) = width(CONTROL_TALL);
  let Ok(wanted) = picker.segmented_wide(tall);
  let Ok(wanted_2) = fitted::<u32, i32>(wanted);
  let room = line.far.saturating_sub(from).max(0);

  let wide = match ::core::primitive::bool::then_some(wanted_2 <= room, ()) {
    ::core::option::Option::Some(()) => wanted_2,
    ::core::option::Option::None => room.min(PICKER_MOST),
  };

  control_frame(line, wide)
}

fn picker_shapes(
  row: &Row,
  measured_row: &MeasuredRow,
  (line, words_x): (Line, i32),
  wearing: &Wearing,
) -> Result<(Vec<Shape>, Vec<HitRegion>, i32), Never> {
  let choices = match &row.choices {
    Some(choices) => choices,
    None => return Ok((Vec::new(), Vec::new(), line.far)),
  };

  let Ok(measured) = choices_of(choices);
  let picker = Picker {
    choices: &measured,
    chosen: choices.chosen,
  };
  let Ok(says_wide) = fitted::<u32, i32>(measured_row.says.width);
  let from = words_x
    .saturating_add(says_wide)
    .saturating_add(strip::GAP.saturating_mul(3));
  let Ok(frame) = picker_frame(&picker, (line, from));
  let Ok(face) = font(TextStyle::Body);
  let lettering = Lettering {
    font: face,
    weight: Weight::Plain,
    tall: measured_row.says.height,
  };
  let Ok(inks) = ControlInks::worn(wearing);
  let Ok(shapes) = picker.shapes(frame, inks, &lettering);
  let Ok(across) = width(frame.size.width);
  let Ok(tall) = width(line.height);
  let reached = HitRegion {
    lands: Lands::Picker {
      row: line.row,
      frame,
    },
    panel: ShapePanel {
      at: Point {
        x: frame.origin.x,
        y: line.top,
      },
      size: Size {
        width: across,
        height: tall,
      },
      round: Round(0),
      fill: wearing.panel,
      edge: Edge::None,
    },
  };

  Ok((
    shapes,
    vec![reached],
    frame.origin.x.saturating_sub(strip::GAP.saturating_mul(3)),
  ))
}

fn aside_shapes(
  row: &Row,
  measured_row: &MeasuredRow,
  line: Line,
  box_wide: u32,
) -> Result<(Vec<Shape>, Vec<HitRegion>, i32), Never> {
  match ::core::primitive::bool::then_some(row.aside.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok((Vec::new(), Vec::new(), line.far)),
    ::core::option::Option::None => {}
  }

  let wide = measured_row.aside.width.min(box_wide.saturating_div(2));
  let Ok(wide_i) = fitted::<u32, i32>(wide);
  let Ok(down) = middle(
    Band {
      top: line.top,
      height: line.height,
    },
    measured_row.aside,
  );
  let Ok(face_callout) = font(TextStyle::Callout);

  let said = Shape::Text(Text {
    at: Point {
      x: line.far.saturating_sub(wide_i),
      y: down,
    },
    width: wide.saturating_add(2),
    said: row.aside.clone(),
    weight: Weight::Plain,
    font: face_callout,
    ink: line.ink,
  });

  Ok((
    vec![said],
    Vec::new(),
    line
      .far
      .saturating_sub(wide_i)
      .saturating_sub(strip::GAP.saturating_mul(4)),
  ))
}

fn note_shapes(
  said: &str,
  measured: Size<u32>,
  card: &Card,
  wearing: &Wearing,
) -> Result<Vec<Shape>, Never> {
  let Ok(finger) = fitted::<i32, u32>(FINGER);
  let wide = measured
    .width
    .saturating_add(INSIDE_A_ROW.unsigned_abs().saturating_mul(2));
  let Ok(wide_i) = fitted::<u32, i32>(wide);
  let pill = ShapePanel {
    at: Point {
      x: card
        .x
        .saturating_add(card.width.saturating_sub(wide_i).saturating_div(2)),
      y: card
        .y
        .saturating_add(card.height)
        .saturating_sub(strip::MARGIN)
        .saturating_sub(FINGER),
    },
    size: Size {
      width: wide,
      height: finger,
    },
    round: Round(TAB_ROUND),
    fill: wearing.night,
    edge: Edge::Of {
      wide: CARD_EDGE,
      color: wearing.pink,
    },
  };
  let Ok(face) = font(TextStyle::Callout);
  let Ok(words) = centred(said, measured, &pill, Weight::Plain, face, wearing.text);

  Ok(vec![Shape::Panel(pill), Shape::Text(words)])
}

fn line_shapes(
  state: &State,
  typed: MeasuredLine,
  at: Point<i32>,
  wide: u32,
  wearing: &Wearing,
) -> Result<Vec<Shape>, Never> {
  let Ok(finger) = fitted::<i32, u32>(FINGER);
  let Ok((said, _before, filled)) = in_the_line(state);
  let words_x = at.x.saturating_add(INSIDE_A_ROW);
  let Ok(down) = middle(
    Band {
      top: at.y,
      height: FINGER,
    },
    typed.size,
  );
  let Ok(caret_tall) = fitted::<i32, u32>(CARET_TALL);
  let (caret, preedit) = match typed.cursor {
    MeasuredCursor::Editing(width) => (Caret::At(width), None),
    MeasuredCursor::Composing { underline, caret } => (caret, Some(underline)),
  };
  let ink = match filled {
    Filled::Typed => wearing.text,
    Filled::Empty => wearing.soft,
  };

  let Ok(face_body) = font(TextStyle::Body);

  let mut shapes = vec![
    Shape::Panel(ShapePanel {
      at,
      size: Size {
        width: wide,
        height: finger,
      },
      round: Round(TAB_ROUND),
      fill: wearing.night,
      edge: Edge::Of {
        wide: CARD_EDGE,
        color: wearing.pink,
      },
    }),
    Shape::Text(Text {
      at: Point {
        x: words_x,
        y: down,
      },
      width: wide.saturating_sub(INSIDE_A_ROW.unsigned_abs().saturating_mul(2)),
      said,
      weight: Weight::Plain,
      font: face_body,
      ink,
    }),
  ];
  match caret {
    Caret::At(width) => {
      let Ok(written) = fitted::<u32, i32>(match filled {
        Filled::Typed => width,
        Filled::Empty => 0,
      });
      shapes.push(Shape::Panel(ShapePanel {
        at: Point {
          x: words_x.saturating_add(written).saturating_add(1),
          y: at
            .y
            .saturating_add(FINGER.saturating_sub(CARET_TALL).saturating_div(2)),
        },
        size: Size {
          width: CARET,
          height: caret_tall,
        },
        round: Round(0),
        fill: wearing.pink,
        edge: Edge::None,
      }));
    }
    Caret::Hidden => {}
  }

  match preedit {
    Some((from, through)) => {
      let room = wide.saturating_sub(INSIDE_A_ROW.unsigned_abs().saturating_mul(2));
      let Ok(left) = fitted::<u32, i32>(from.min(room));
      let across = through.min(room).saturating_sub(from.min(room));
      shapes.push(Shape::Panel(ShapePanel {
        at: Point {
          x: words_x.saturating_add(left),
          y: at.y.saturating_add(FINGER.saturating_sub(6)),
        },
        size: Size {
          width: across,
          height: 2,
        },
        round: Round(0),
        fill: wearing.pink,
        edge: Edge::None,
      }));
    }
    ::core::option::Option::None => {}
  }

  Ok(shapes)
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
  rows: Vec<Row>,
  model: CardModel,
  loading: bool,
}

pub(crate) struct CardModel {
  pages: Vec<Page>,
  leaving: Leaving,
  here: u32,
  at: Option<u32>,
  opened: Opened,
  from_tab: u32,
  scroll: i32,
  note: Option<String>,
  typed: TextField,
  preedit: Option<Composition>,
  prompt_identity: u32,
  beside: Beside,
  asking: Option<Prompt>,
  sure: Option<Sure>,
  pressing: Option<(u32, u32)>,
  zoom: Option<(std::path::PathBuf, crate::zoom::Zoom)>,
  shown: Option<Moving>,
  selected: Vec<String>,
  framing: Framing,
  tasks: Actor<Tasks>,
  live: Option<String>,
}

impl std::ops::Deref for State {
  type Target = CardModel;

  fn deref(&self) -> &Self::Target {
    &self.model
  }
}

impl std::ops::DerefMut for State {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.model
  }
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

fn zoom_on(state: &CardModel, row: &Row) -> Result<crate::zoom::Zoom, Never> {
  let Ok(still) = still_of(&row.picture);

  Ok(match (still, &state.zoom) {
    (Some(still_2), Some((on, zoom))) => {
      match ::core::primitive::bool::then_some(still_2 == on.as_path(), ()) {
        ::core::option::Option::Some(()) => *zoom,
        ::core::option::Option::None => crate::zoom::Zoom::default(),
      }
    }
    (Some(_) | None, _) => crate::zoom::Zoom::default(),
  })
}

fn zoomed_to(
  state: &mut CardModel,
  rows: &[Row],
  zooming: impl Fn(crate::zoom::Zoom) -> crate::zoom::Zoom,
) -> Result<(), Never> {
  let found = rows.iter().find_map(|row| {
    let Ok(still) = still_of(&row.picture);

    still.map(|still_2| (still_2.to_path_buf(), row))
  });

  let (still, row) = match found {
    Some(found_2) => found_2,
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
  let moving_3 = match moving {
    Some(moving_2) => moving_2,
    None => return Ok(None),
  };
  let Ok(wide) = fitted::<u32, i32>(moving_3.room.width);
  let Ok(tall) = fitted::<u32, i32>(moving_3.room.height);
  let inside = hit.x >= moving_3.at.x
    && hit.x < moving_3.at.x.saturating_add(wide)
    && hit.y >= moving_3.at.y
    && hit.y < moving_3.at.y.saturating_add(tall);

  Ok(match (inside, &state.zoom) {
    (true, Some((still, zoom))) => match (still == &moving_3.of, zoom.zoomed()) {
      (true, Ok(crate::zoom::Zoomed::ZoomedIn)) => Some((still.clone(), *zoom, moving_3.room)),
      (true, Ok(crate::zoom::Zoomed::Whole)) | (false, _) => None,
    },
    (true, None) | (false, _) => None,
  })
}

fn pressed(state: &CardModel, at: u32, row: &Row) -> Result<u32, Never> {
  let across = match &row.buttons {
    Some(across) => across,
    None => return Ok(0),
  };
  let Ok(last) = fitted::<_, u32>(across.presses.len().saturating_sub(1));

  Ok(match state.pressing {
    Some((on, which)) => match ::core::primitive::bool::then_some(on == at, ()) {
      ::core::option::Option::Some(()) => which.min(last),
      ::core::option::Option::None => across.at.min(last),
    },
    None => across.at.min(last),
  })
}

fn drawn_in(panelled: &Panelled, at: u32) -> Result<Vec<String>, Never> {
  let band = panelled
    .touching
    .iter()
    .find(|touching| touching.lands == Lands::Row(at));

  let band_3 = match band {
    Some(band_2) => band_2.panel,
    None => return Ok(Vec::new()),
  };

  let Ok(tall) = fitted::<u32, i32>(band_3.size.height);
  let top = band_3.at.y;
  let bottom = top.saturating_add(tall);

  Ok(
    panelled
      .shapes
      .iter()
      .filter_map(|shape| match shape {
        Shape::Text(text) => Some(text),
        Shape::Panel(_)
        | Shape::Picture(_)
        | Shape::Cropped(_)
        | Shape::Line(_)
        | Shape::Clip(_) => None,
      })
      .filter(|text| text.at.y >= top && text.at.y < bottom)
      .map(|text| text.said.clone())
      .collect(),
  )
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
    let Ok((name, whose)) = touching.lands.label_and_owner();
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

    let row_3 = match row {
      Some(row_2) => row_2,
      None => continue,
    };

    let Ok(bare) = crate::page::bare(row_3);
    let Ok(heading) = row_3.heading();
    let Ok(wears) = crate::page::wears(row_3);
    let Ok(drew) = drawn_in(panelled, at);

    lines.push(description::Line {
      at,
      says: row_3.says.clone(),
      aside: row_3.aside.clone(),
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
      cells: row_3.cells.iter().map(|cell| cell.says.clone()).collect(),
      drew,
    });
  }

  let Ok(whose) = crate::whose::name();
  let Ok(page) = nth(&state.pages, state.here);

  Ok(description::Description {
    panel: whose,
    tab: match page {
      Some(page_2) => page_2.title.clone(),
      None => String::new(),
    },
    out: match state.opened {
      Opened::Expanded => description::Output::Yes,
      Opened::No => description::Output::No,
    },
    room,
    spots: card,
    lines,
    presses: 0,
    keyboard: KeyboardFocus::Elsewhere,
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
  let Ok(inside_wide) = width(right.saturating_sub(left));

  let (round, fill, edge) = match (state.opened, state.framing) {
    (Opened::Expanded, _) => (Round(0), wearing.night, Edge::None),
    (Opened::No, Framing::Window) => (Round(0), wearing.panel, Edge::None),
    (Opened::No, Framing::Card) => (
      Round(CARD_ROUND),
      wearing.panel,
      Edge::Of {
        wide: CARD_EDGE,
        color: wearing.pink,
      },
    ),
  };

  shapes.push(Shape::Panel(ShapePanel {
    at: Point {
      x: card.x,
      y: card.y,
    },
    size: Size {
      width: card_wide_u,
      height: card_tall_u,
    },
    round,
    fill,
    edge,
  }));

  let Ok(many) = fitted(state.pages.len());
  let Ok(run) = strip::visible_range(strip::Tabs {
    many,
    here: state.here,
    from: state.from_tab,
    fits,
  });
  let window = TabWindow {
    from: run.start,
    fits,
  };
  let (header_shapes, header_touching) = header(state, window, measured, card, wearing)?;

  touching.extend(header_touching);

  let over_the_rows = match state.opened {
    Opened::Expanded => header_shapes,
    Opened::No => {
      shapes.extend(header_shapes);

      Vec::new()
    }
  };

  match (&state.asking, &state.sure) {
    (Some(asking), _) => {
      let (question, reveal) = question_shapes(asking, state, measured, card, wearing)?;

      shapes.extend(question);
      touching.retain(|target| target.lands == Lands::Back);
      touching.extend(reveal);

      return Ok(Panelled {
        shapes,
        touching,
        moving: None,
      });
    }
    (None, Some(sure)) => {
      let (question, answers) = sure_shapes(sure, measured, card, wearing)?;

      shapes.extend(question);
      touching.retain(|target| target.lands == Lands::Back);
      touching.extend(answers);

      return Ok(Panelled {
        shapes,
        touching,
        moving: None,
      });
    }
    (None, None) => {}
  }

  let Ok(page) = nth(&state.pages, state.here);

  match page {
    Some(page_2) => {
      let Ok(sought_tall) = asked_about(page_2);
      let rows_start = match state.opened {
        Opened::Expanded => card.y,
        Opened::No => card
          .y
          .saturating_add(strip_tall)
          .saturating_add(OVER_ROWS)
          .saturating_add(sought_tall),
      };

      match &page_2.sought {
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
        None => {}
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

      shapes.push(Shape::Clip(Clip::To {
        at: Point {
          x: card.x,
          y: rows_start,
        },
        size: Size {
          width: card_wide_u,
          height: list_tall,
        },
      }));

      if state.loading {
        let Ok(face) = font(TextStyle::Body);
        shapes.push(Shape::Text(Text {
          at: Point {
            x: left,
            y: rows_start.saturating_add(INSIDE_A_ROW),
          },
          width: inside_wide,
          said: "Loading…".to_owned(),
          weight: Weight::Plain,
          font: face,
          ink: wearing.soft,
        }));
      }

      let Ok(count) = fitted::<_, u64>(rows.len());
      let Ok(window_2) = crate::row_window::visible_rows(crate::row_window::RowViewport {
        count,
        height: card_bottom.saturating_sub(rows_start),
        scroll: state.scroll,
        stage,
        row: row_tall,
      });
      let Ok(first) = index(window_2.start);
      let Ok(last) = index(window_2.end);
      let visible = match rows.get(first..last) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => &[],
      };
      let visible_keys: std::collections::BTreeSet<_> = visible
        .iter()
        .enumerate()
        .map(|(offset, row)| {
          let Ok(position) = fitted::<_, u64>(first.saturating_add(offset));
          let Ok(identity) = RowIdentity::of(row, position);
          identity
        })
        .collect();
      let _a_nested_view_keeps_its_measurements = measured
        .rows
        .try_borrow_mut()
        .map(|mut cache| cache.retain(|key, _| visible_keys.contains(key)));

      for (offset, row) in visible.iter().enumerate() {
        let ::core::result::Result::Ok(index) =
          ::console_core_number_conversion::fixed_index(first.saturating_add(offset));
        let Ok(row_y_offset) =
          fitted::<_, i32>(match ::console_core_number_conversion::index(index) {
            ::core::result::Result::Ok(native_position) => native_position,
          });
        let Ok(index_2) =
          fitted::<_, u32>(match ::console_core_number_conversion::index(index) {
            ::core::result::Result::Ok(native_position) => native_position,
          });
        let (below, this_tall) = match index_2 {
          0 => (0, row_tall.saturating_add(stage)),
          _ => (stage, row_tall),
        };
        let at_y = rows_start
          .saturating_add(row_y_offset.saturating_mul(row_tall))
          .saturating_add(below)
          .saturating_sub(state.scroll);

        let row_bottom = at_y.saturating_add(this_tall);

        let highlight = match (state.at == Some(index_2), state.leaving) {
          (true, Leaving::No) => Highlight::Yes,
          (true, Leaving::Standing) | (false, _) => Highlight::No,
        };
        let Ok(press) = pressed(state, index_2, row);
        let Ok(zoom) = zoom_on(state, row);
        let Ok(selected) = selected_of(state, row);
        let standing = Standing {
          at: index_2,
          highlight,
          beside: state.beside,
          press,
          opened: state.opened,
          zoom,
          selected,
        };
        let band = Band {
          top: at_y,
          height: this_tall,
        };
        let Ok(drawn) = render_row(row, band, card, standing, measured, wearing);

        moving = moving.or(drawn.moving);
        shapes.extend(drawn.shapes);

        let whole = at_y >= rows_start && row_bottom <= card_bottom;

        match ::core::primitive::bool::then_some(whole, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => continue,
        }

        touching.extend(drawn.touching);

        let Ok(tall) = fitted::<i32, u32>(this_tall);

        touching.push(HitRegion {
          lands: Lands::Row(index_2),
          panel: ShapePanel {
            at: Point { x: left, y: at_y },
            size: Size {
              width: inside_wide,
              height: tall,
            },
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

  let Ok(noting) = noted_now(state);

  match noting {
    Some(said) => {
      let Ok(noted) = note_shapes(said, measured.note, card, wearing);

      shapes.extend(noted);
    }
    None => {}
  }

  Ok(Panelled {
    shapes,
    touching,
    moving,
  })
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
    card
      .width
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
) -> Result<(Vec<Shape>, Vec<HitRegion>), Never> {
  let mut shapes = Vec::new();
  let mut touching = Vec::new();
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

  let line_at = Point {
    x: at.x,
    y: at.y.saturating_add(fitting::ROW),
  };

  let Ok(line) = line_shapes(state, measured.typed, line_at, wide, wearing);

  shapes.extend(line);

  let Ok(offered) = asking.secret.offered();

  match offered {
    Some(says) => {
      let Ok(finger) = fitted::<i32, u32>(FINGER);
      let (drawn, reached) = button(
        Button {
          at: Point {
            x: at.x,
            y: at.y.saturating_add(fitting::ROW.saturating_mul(3)),
          },
          size: Size {
            width: wide,
            height: finger,
          },
          said: says,
          measured: measured.reveal,
          lands: Lands::Reveal,
          stood: Highlight::No,
        },
        wearing,
      )?;

      shapes.extend(drawn);
      touching.push(reached);
    }
    None => {}
  }

  Ok((shapes, touching))
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
    at: Point {
      x: at.x,
      y: at.y.saturating_add(fitting::ROW),
    },
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
    let Ok(which_2) = fitted::<_, u32>(which);
    let Ok(slot) = index(which_2);
    let said = match measured.answers.get(slot) {
      Some(said) => *said,
      None => NO_ROOM,
    };
    let (drawn, reached) = button(
      Button {
        at: Point {
          x: at
            .x
            .saturating_add(step.saturating_mul(answer_wide_i.saturating_add(gap_i))),
          y: answers_y,
        },
        size: Size {
          width: answer_wide,
          height: finger,
        },
        said: says,
        measured: said,
        lands: Lands::Answer(which_2),
        stood: match ::core::primitive::bool::then_some(which_2 == sure.at, ()) {
          ::core::option::Option::Some(()) => Highlight::Yes,
          ::core::option::Option::None => Highlight::No,
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
  Hidden,
  Shown,
  No,
}

impl Secret {
  fn toggle(self) -> Result<Secret, Never> {
    Ok(match self {
      Secret::Hidden => Secret::Shown,
      Secret::Shown => Secret::Hidden,
      Secret::No => Secret::No,
    })
  }

  fn offered(self) -> Result<Option<&'static str>, Never> {
    Ok(match self {
      Secret::Hidden => Some(marks::SHOW),
      Secret::Shown => Some(marks::HIDE),
      Secret::No => None,
    })
  }
}

fn toggle_secret(state: &mut CardModel) -> Result<(), Never> {
  match state.asking.as_mut() {
    Some(asking) => {
      let Ok(secret) = asking.secret.toggle();

      asking.secret = secret;
    }
    None => {}
  }

  Ok(())
}

pub(crate) struct Prompt {
  question: String,
  typed: TextField,
  secret: Secret,
  then: Answer,
  identity: u32,
}

pub(crate) struct Sure {
  question: String,
  about: String,
  answers: Vec<String>,
  at: u32,
  then: Confirmation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
  None,
  Redrawn,
  Chose(u32),
  Else(u32),
  Picked { row: u32, which: u32 },
  Back,
  Closing,
}

#[must_use]
pub(crate) fn stepped_onto(
  meaning: Meaning,
  before: Option<u32>,
  state: &State,
) -> Result<Option<u32>, Never> {
  let moved = match ::core::primitive::bool::then_some(state.at == before, ()) {
    ::core::option::Option::Some(()) => None,
    ::core::option::Option::None => state.at,
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

pub(crate) fn apply_meaning(
  state: &mut CardModel,
  meaning: Meaning,
  rows: &[Row],
) -> Result<Outcome, Never> {
  match (state.leaving, meaning) {
    (Leaving::Standing, Meaning::Choose) => return Ok(Outcome::Closing),
    (Leaving::Standing, Meaning::Step(_) | Meaning::Nudge(_) | Meaning::More) => {
      state.leaving = Leaving::No;

      return Ok(Outcome::Redrawn);
    }
    (Leaving::Standing, Meaning::None | Meaning::Close | Meaning::Abandon | Meaning::Tab(_))
    | (Leaving::No, _) => {}
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
        (Selecting::No, Opened::No) => {
          let Ok(page) = nth(&state.pages, state.here);

          match page.and_then(|page| page.back.as_ref()) {
            Some(_) => Outcome::Back,
            None => Outcome::Closing,
          }
        }
      }
    }
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
        None => match ::core::primitive::bool::then_some(rows.is_empty(), ()) {
          ::core::option::Option::Some(()) => return Ok(Outcome::None),
          ::core::option::Option::None => {
            state.at = Some(0);
            0
          }
        },
      };

      let Ok(row) = nth(rows, at);

      let row_3 = match row {
        Some(row_2) => row_2,
        None => return Ok(Outcome::None),
      };

      match &row_3.buttons {
        Some(across) => {
          let Ok(from) = pressed(state, at, row_3);
          let Ok(last) = fitted::<_, u32>(across.presses.len().saturating_sub(1));
          let going = match ::core::primitive::bool::then_some(step > 0, ()) {
            ::core::option::Option::Some(()) => from.saturating_add(1).min(last),
            ::core::option::Option::None => from.saturating_sub(1),
          };

          state.pressing = Some((at, going));

          return Ok(
            match ::core::primitive::bool::then_some(going == from, ()) {
              ::core::option::Option::Some(()) => Outcome::None,
              ::core::option::Option::None => Outcome::Redrawn,
            },
          );
        }
        None => {}
      }

      let Ok(wears) = crate::page::wears(row_3);
      let Ok(holds) = crate::page::holds(row_3);
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
          let Ok(moved) = nudge_level(rows, at, step);

          moved
        }
      }
    }
    Meaning::Choose => {
      let selection = match (state.beside == Beside::Yes).then_some(()) {
        ::core::option::Option::Some(()) => Selection::Else,
        ::core::option::Option::None => Selection::Chose,
      };

      match state.at.is_some().then_some(()) {
        ::core::option::Option::Some(()) => {
          state.beside = Beside::No;
        }
        ::core::option::Option::None => {}
      }

      return row_action(Activation::Focused, state.at, selection);
    }
    Meaning::More => return row_action(Activation::Focused, state.at, Selection::Else),
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Viewport {
  room: i32,
  y: i32,
}

pub(crate) fn scrolled(at: Option<u32>, viewport: Viewport) -> Result<i32, Never> {
  let at_3 = match at {
    Some(at_2) => at_2,
    None => return Ok(0),
  };

  let Ok(index) = fitted::<u32, i32>(at_3);
  let top = index.saturating_mul(fitting::ROW);
  let under = top.saturating_add(fitting::ROW);

  Ok(
    match (
      top < viewport.y,
      under > viewport.y.saturating_add(viewport.room),
    ) {
      (true, _) => top,
      (false, true) => under.saturating_sub(viewport.room),
      (false, false) => viewport.y,
    },
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
  Chose,
  Else,
}

#[must_use]
fn row_action(
  activation: Activation<u32>,
  focused: Option<u32>,
  selection: Selection,
) -> Result<Outcome, Never> {
  let Ok(target) = activation.resolve(|| focused, Some);

  Ok(match (target, selection) {
    (Some(row), Selection::Chose) => Outcome::Chose(row),
    (Some(row), Selection::Else) => Outcome::Else(row),
    (None, _) => Outcome::None,
  })
}

fn noted_now(state: &State) -> Result<Option<&String>, Never> {
  Ok(state.live.as_ref().or(state.note.as_ref()))
}

fn noted(
  state: &mut CardModel,
  notes: Vec<console_program_contract::Notification>,
) -> Result<(), Never> {
  let last = match notes.last() {
    Some(last) => last,
    None => return Ok(()),
  };
  let Ok(written) = tasks::written(last);

  state.note = Some(written);

  Ok(())
}

pub(crate) fn driving(state: &State) -> Result<Driving, Never> {
  match (&state.sure, &state.asking) {
    (Some(_), _) => return Ok(Driving::Sure),
    (None, Some(_)) => return Ok(Driving::Question),
    (None, None) => {}
  }

  let Ok(page) = nth(&state.pages, state.here);
  let sought = page.and_then(|page_2| page_2.sought.as_ref());

  Ok(match sought {
    Some(_) => Driving::Search,
    None => Driving::Panel,
  })
}

fn pressed_here(
  state: &mut CardModel,
  rows: &[Row],
  key: Keysym,
  meaning: Meaning,
  driving: Driving,
) -> Result<ActionOutcome, Never> {
  match (meaning, driving) {
    (Meaning::Abandon, Driving::Question) => {
      state.asking = None;

      return Ok(ActionOutcome::Stay);
    }
    (Meaning::Abandon, Driving::Sure) => {
      state.sure = None;

      return Ok(ActionOutcome::Stay);
    }
    (Meaning::Nudge(step), Driving::Sure) => {
      let Ok(()) = leaned(state, step);

      return Ok(ActionOutcome::Stay);
    }
    (Meaning::Choose, Driving::Sure) => {
      let Ok(()) = answered(state, rows, Activation::Focused);

      return Ok(ActionOutcome::Stay);
    }
    (Meaning::More, Driving::Question) => {
      let Ok(()) = toggle_secret(state);

      return Ok(ActionOutcome::Stay);
    }
    (Meaning::None, Driving::Question) => {
      match key {
        Keysym::Return | Keysym::KP_Enter => {
          let Ok(()) = submit_answer(state, rows);

          return Ok(ActionOutcome::Stay);
        }
        _ => {}
      }

      let asking = match state.asking.take() {
        Some(asking) => asking,
        None => return Ok(ActionOutcome::Stay),
      };

      let mut typed = asking.typed;
      let Ok(_changed) = typed.typed(key);

      state.asking = Some(Prompt { typed, ..asking });

      return Ok(ActionOutcome::Stay);
    }
    (Meaning::None, Driving::Search) => {
      let Ok(changed) = state.typed.typed(key);

      match changed {
        Typed::Edited => {
          let Ok(()) = narrowed(state, rows);
        }
        Typed::Moved | Typed::None => {}
      }

      return Ok(ActionOutcome::Stay);
    }
    (Meaning::None | Meaning::Abandon | Meaning::Choose, _)
    | (Meaning::Close | Meaning::More | Meaning::Nudge(_), _)
    | (Meaning::Step(_) | Meaning::Tab(_), _) => {}
  }

  match (&state.asking, &state.sure) {
    (None, None) => {}
    (Some(_), _) | (None, Some(_)) => return Ok(ActionOutcome::Stay),
  }

  let Ok(told) = apply_meaning(state, meaning, rows);

  carried_out(state, rows, told)
}

fn column_kept(state: &mut CardModel, rows: &[Row], landed: u32) -> Result<(), Never> {
  let Ok(row) = nth(rows, landed);
  let across = row.and_then(|row_2| row_2.buttons.as_ref());

  match (state.pressing, across) {
    (Some((_, which)), Some(across_2)) => {
      let Ok(last) = fitted::<_, u32>(across_2.presses.len().saturating_sub(1));

      state.pressing = Some((landed, which.min(last)));
    }
    (Some(_), None) | (None, _) => {}
  }

  Ok(())
}

fn stood_on(state: &mut CardModel, which: u32) -> Result<(), Never> {
  state.opened = Opened::No;
  state.leaving = Leaving::No;
  state.here = which;
  state.at = None;
  state.scroll = 0;
  state.beside = Beside::No;
  state.selected = Vec::new();

  Ok(())
}

fn nudge_level(rows: &[Row], at: u32, step: i32) -> Result<Outcome, Never> {
  let Ok(found) = nth(rows, at);

  let row = match found {
    Some(row) => row,
    None => return Ok(Outcome::None),
  };

  let toward = match ::core::primitive::bool::then_some(step > 0, ()) {
    ::core::option::Option::Some(()) => Toward::Right,
    ::core::option::Option::None => Toward::Left,
  };

  let Ok(holds) = crate::page::holds(row);

  Ok(match holds {
    Holds::Control(Control::Switch(switch)) => {
      let Ok(answer) = switch.moved(toward);

      match answer {
        Answered::Now(_) => Outcome::Chose(at),
        Answered::Same | Answered::Passes => Outcome::None,
      }
    }
    Holds::Control(Control::Slider(slider)) => {
      let Ok(answer) = slider.moved(toward);

      match (answer, row.level.as_ref()) {
        (Answered::Now(_), Some(level)) => {
          level(step);

          Outcome::Redrawn
        }
        (Answered::Now(_), None) | (Answered::Same | Answered::Passes, _) => Outcome::None,
      }
    }
    Holds::Choices => {
      let Ok(answer) = chosen_toward(row, toward);

      match answer {
        Answered::Now(which) => Outcome::Picked { row: at, which },
        Answered::Same | Answered::Passes => Outcome::None,
      }
    }
    Holds::Level => match row.level.as_ref() {
      Some(level) => {
        level(step);

        Outcome::Redrawn
      }
      None => Outcome::None,
    },
    Holds::None => Outcome::None,
  })
}

fn chosen_toward(row: &Row, toward: Toward) -> Result<Answered<u32>, Never> {
  match &row.choices {
    Some(choices) => {
      let Ok(measured) = choices_of(choices);

      Picker {
        choices: &measured,
        chosen: choices.chosen,
      }
      .moved(toward)
    }
    None => Ok(Answered::Passes),
  }
}

fn chosen_under(
  row: &Row,
  (press, frame): (Press, Rectangle<i32>),
) -> Result<Answered<u32>, Never> {
  match &row.choices {
    Some(choices) => {
      let Ok(measured) = choices_of(choices);

      Picker {
        choices: &measured,
        chosen: choices.chosen,
      }
      .pressed(press, frame)
    }
    None => Ok(Answered::Passes),
  }
}

fn press_picker(
  state: &mut CardModel,
  rows: &[Row],
  row: u32,
  (press, frame): (Press, Rectangle<i32>),
) -> Result<(), Never> {
  state.at = Some(row);
  state.beside = Beside::No;
  let Ok(found) = nth(rows, row);

  if let Some(found) = found {
    let Ok(answer) = chosen_under(found, (press, frame));

    if let Answered::Now(which) = answer {
      let Ok(()) = picked(state, rows, PickedArguments { at: row, which });
    }
  }

  Ok(())
}

fn picked(
  state: &mut CardModel,
  rows: &[Row],
  PickedArguments { at, which }: PickedArguments,
) -> Result<(), Never> {
  let Ok(found) = nth(rows, at);
  let choose = found
    .and_then(|row| row.choices.as_ref())
    .map(|choices| &choices.choose);

  match choose {
    Some(choose) => effects::dispatch(state, rows, effects::Source::Focused, |showing, _| {
      choose(showing, which);
    }),
    None => Ok(()),
  }
}

struct PickedArguments {
  at: u32,
  which: u32,
}

fn slid(
  rows: &[Row],
  at: u32,
  (press, frame): (Press, Rectangle<i32>),
  held: Option<u32>,
) -> Result<Option<u32>, Never> {
  let Ok(row) = nth(rows, at);
  let carried = row.map(|row_2| (row_2.control, row_2.level.as_ref()));

  let (slider, level) = match carried {
    Some((Some(Control::Slider(slider)), Some(level))) => (slider, level),
    Some((Some(Control::Slider(_)), None) | (Some(Control::Switch(_)) | None, _)) | None => {
      return Ok(None);
    }
  };

  let Ok(most) = slider.most();

  let Ok(standing) = match held {
    Some(step) => Slider::new(step, most),
    None => Ok(slider),
  };

  let Ok(answer) = standing.pressed(press, frame);

  Ok(match answer {
    Answered::Now(now) => {
      let Ok(was) = standing.at();
      let Ok(is) = now.at();
      let Ok(from) = fitted::<u32, i32>(was);
      let Ok(to) = fitted::<u32, i32>(is);

      level(to.saturating_sub(from));

      Some(is)
    }
    Answered::Same | Answered::Passes => None,
  })
}

fn sliding_under(lands: Option<Lands>) -> Result<Option<(u32, Rectangle<i32>)>, Never> {
  Ok(match lands {
    Some(Lands::Slider { row, frame }) => Some((row, frame)),
    Some(
      Lands::Row(_)
      | Lands::Answer(_)
      | Lands::Nudge { .. }
      | Lands::Else(_)
      | Lands::ButtonPress { .. }
      | Lands::Seek { .. }
      | Lands::Picker { .. }
      | Lands::Tab(_)
      | Lands::More(_)
      | Lands::Back
      | Lands::Reveal,
    )
    | None => None,
  })
}

fn sought_under(
  rows: &[Row],
  at: u32,
  (hit, frame): (Point<i32>, Rectangle<i32>),
) -> Result<Option<f64>, Never> {
  let Ok(row) = nth(rows, at);

  let bar = match row.map(|row_2| &row_2.picture) {
    Some(Picture::Bar(bar)) => *bar,
    Some(
      Picture::None
      | Picture::Space
      | Picture::Named(_)
      | Picture::At(_)
      | Picture::Sleeve(_)
      | Picture::Showing(_)
      | Picture::Playing(_)
      | Picture::Written(_),
    )
    | None => return Ok(None),
  };

  let Ok(slider) = bar.slider();
  let Ok(answer) = slider.pressed(
    Press::Tap(Point {
      x: hit.x,
      y: frame.origin.y,
    }),
    frame,
  );

  Ok(match answer {
    Answered::Now(landed) => {
      let Ok(fraction) = crate::page::fraction(landed);

      Some(fraction)
    }
    Answered::Same | Answered::Passes => None,
  })
}

fn leaned(state: &mut CardModel, step: i32) -> Result<(), Never> {
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

fn took(state: &mut CardModel, rows: &[Row], at: u32) -> Result<(), Never> {
  let sure = match state.sure.take() {
    Some(sure) => sure,
    None => return Ok(()),
  };

  match at.checked_sub(1) {
    None => Ok(()),
    Some(which) => effects::dispatch(state, rows, effects::Source::Focused, |showing, _| {
      (sure.then)(showing, which);
    }),
  }
}

fn submit_answer(state: &mut CardModel, rows: &[Row]) -> Result<(), Never> {
  let asking = match state.asking.take() {
    Some(asking) => asking,
    None => return Ok(()),
  };

  let Ok(typed) = asking.typed.text();

  effects::dispatch(state, rows, effects::Source::Focused, |showing, _| {
    (asking.then)(showing, typed);
  })
}

fn narrowed(state: &mut CardModel, rows: &[Row]) -> Result<(), Never> {
  let Ok(page) = nth(&state.pages, state.here);
  let sought = page.and_then(|page_2| page_2.sought.clone());

  let sought_3 = match sought {
    Some(sought_2) => sought_2,
    None => return Ok(()),
  };

  let Ok(()) = effects::dispatch(state, rows, effects::Source::Focused, |showing, model| {
    let Ok(typed) = model.typed.text();
    (sought_3.then)(showing, typed);
  });

  state.at = None;
  state.scroll = 0;

  Ok(())
}

fn carried_out(
  state: &mut CardModel,
  rows: &[Row],
  told: Outcome,
) -> Result<ActionOutcome, Never> {
  Ok(match told {
    Outcome::Closing => ActionOutcome::Close,
    Outcome::Back => {
      let Ok(page) = nth(&state.pages, state.here);
      let back = page.and_then(|page| page.back.clone());

      match back {
        Some(back) => {
          let Ok(outcome) =
            effects::dispatch(state, rows, effects::Source::Focused, |showing, _| {
              back(showing)
            });

          outcome
        }
        None => ActionOutcome::Close,
      }
    }
    Outcome::Chose(at) => {
      let Ok(gone) = acted(state, rows, at, Selection::Chose);

      gone
    }
    Outcome::Else(at) => {
      let Ok(gone) = acted(state, rows, at, Selection::Else);

      gone
    }
    Outcome::Picked { row, which } => {
      let Ok(()) = picked(state, rows, self::PickedArguments { at: row, which });

      ActionOutcome::Stay
    }
    Outcome::Redrawn | Outcome::None => ActionOutcome::Stay,
  })
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the seek position is a dimensionless fraction of a recording's duration"
  )
)]
fn sought(state: &mut CardModel, rows: &[Row], at: u32, fraction: f64) -> Result<(), Never> {
  let Ok(row) = nth(rows, at);

  let seek = match row.and_then(|row_2| row_2.seek.as_ref()) {
    Some(seek) => seek,
    None => return Ok(()),
  };

  effects::dispatch(state, rows, effects::Source::Focused, |showing, _| {
    seek(showing, fraction);
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selecting {
  Yes,
  No,
}

fn selected_of(state: &CardModel, row: &Row) -> Result<Selected, Never> {
  Ok(match &row.key {
    Some(key) => match ::core::primitive::bool::then_some(state.selected.contains(key), ()) {
      ::core::option::Option::Some(()) => Selected::Yes,
      ::core::option::Option::None => Selected::No,
    },
    None => Selected::No,
  })
}

fn still_selected(state: &CardModel, rows: &[Row]) -> Result<Vec<String>, Never> {
  let there: std::collections::BTreeSet<&String> =
    rows.iter().filter_map(|row| row.key.as_ref()).collect();

  Ok(
    state
      .selected
      .iter()
      .filter(|key| there.contains(key))
      .cloned()
      .collect(),
  )
}

fn selecting(state: &CardModel, rows: &[Row]) -> Result<Selecting, Never> {
  let Ok(page) = nth(&state.pages, state.here);
  let acts = page.is_some_and(|page_2| !page_2.selecting.is_empty());
  let Ok(still) = still_selected(state, rows);

  Ok(
    match ::core::primitive::bool::then_some(acts && !still.is_empty(), ()) {
      ::core::option::Option::Some(()) => Selecting::Yes,
      ::core::option::Option::None => Selecting::No,
    },
  )
}

fn toggled(state: &mut CardModel, key: &str) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(state.selected.iter().any(|held| held == key), ()) {
    ::core::option::Option::Some(()) => state.selected.retain(|held| held != key),
    ::core::option::Option::None => state.selected.push(key.to_string()),
  }

  Ok(())
}

fn asked_what_to_do(state: &mut CardModel, rows: &[Row]) -> Result<(), Never> {
  let Ok(page) = nth(&state.pages, state.here);
  let acts: Vec<crate::page::Selecting> = match page {
    Some(page_2) => page_2.selecting.clone(),
    None => return Ok(()),
  };
  let Ok(keys) = still_selected(state, rows);
  let every: Vec<String> = rows.iter().filter_map(|row| row.key.clone()).collect();
  let answers = std::iter::once(marks::CANCEL.to_string())
    .chain(acts.iter().map(|act| act.says.clone()))
    .chain([SELECT_ALL.to_string(), DESELECT_ALL.to_string()])
    .collect();
  let question = format!("{} Selected", keys.len());

  let then: Confirmation = Box::new(move |showing, which| {
    let Ok(which_2) = index(which);

    match (acts.get(which_2), which_2 == acts.len()) {
      (Some(act), _) => {
        (act.act)(showing, &keys);
        showing.select(Vec::new());
      }
      (None, true) => showing.select(every),
      (None, false) => showing.select(Vec::new()),
    }
  });

  state.sure = Some(Sure {
    question,
    about: String::new(),
    answers,
    at: 0,
    then,
  });

  Ok(())
}

fn acted(
  state: &mut CardModel,
  rows: &[Row],
  at: u32,
  what: Selection,
) -> Result<ActionOutcome, Never> {
  let Ok(row) = nth(rows, at);

  let row_3 = match row {
    Some(row_2) => row_2,
    None => return Ok(ActionOutcome::Stay),
  };

  let Ok(mode) = selecting(state, rows);

  match (mode, what, &row_3.key) {
    (Selecting::Yes, Selection::Chose, Some(key)) => {
      let Ok(()) = toggled(state, key);

      return Ok(ActionOutcome::Stay);
    }
    (Selecting::Yes, Selection::Else, _) => {
      let Ok(()) = asked_what_to_do(state, rows);

      return Ok(ActionOutcome::Stay);
    }
    (Selecting::Yes, Selection::Chose, None) | (Selecting::No, _, _) => {}
  }

  let Ok(which) = pressed(state, at, row_3);
  let press = row_3.buttons.as_ref().and_then(|across| {
    let Ok(which_2) = index(which);

    across.presses.get(which_2).map(|press| &press.does)
  });

  effects::dispatch(
    state,
    rows,
    effects::Source::Row(row_3),
    |showing, _| match (what, press) {
      (Selection::Chose, Some(does)) => does(showing),
      (Selection::Chose, None) => match &row_3.does {
        Some(Handler::Call(act)) => act(showing),
        Some(Handler::Run(arguments)) => {
          let Ok(()) = crate::running::left_running(&arguments);

          ActionOutcome::Close
        }
        None => ActionOutcome::Stay,
      },
      (Selection::Else, _) => match &row_3.more {
        Some(more) => more(showing),
        None => ActionOutcome::Stay,
      },
    },
  )
}

pub struct HostedDrawing {
  pub fonts: FontCatalog,
  pub while_it_is_up: OwnedFd,
  pub first_frame: DrawingSender,
}

pub fn prepare_fonts() -> Result<FontCatalog, Never> {
  FontCatalog::discover(|| {
    let Ok(_marks) = measure_marks();
  })
}

pub fn run(
  who: &str,
  (build, tasks): (crate::card::Build, Actor<Tasks>),
  _column: i32,
  start: Option<&str>,
  drawing: HostedDrawing,
  tells: Option<String>,
) -> Result<(Close, thread::JoinHandle<Result<(), Never>>), Never> {
  let Ok((handle_shut, shut)) = CloseWatch::pair();

  let who_2 = who.to_string();
  let start_2 = start.map(str::to_string);

  let Ok(handed) = crate::opening::handing();

  let handle = thread::spawn(move || {
    let Ok(()) = crate::opening::handed(handed);
    let HostedDrawing {
      fonts,
      while_it_is_up,
      first_frame,
    } = drawing;
    let Ok(()) = fonts.use_on_draw_thread();
    let drawn = serve(
      &who_2,
      (build, tasks),
      start_2.as_deref(),
      Framing::Card,
      shut,
      Reported {
        front: None,
        first_frame: FirstFrame::Hosted(first_frame),
        tells,
        presses: 0,
        told_presses: 0,
        keyboard: KeyboardFocus::Elsewhere,
        told_keyboard: KeyboardFocus::Elsewhere,
      },
    );

    drop(while_it_is_up);

    drawn
  });

  Ok((handle_shut, handle))
}

pub fn show(
  build: impl FnOnce() -> Vec<Page> + Send + 'static,
  _column: i32,
  start: Option<&str>,
) -> Result<(), Never> {
  let Ok(whose) = crate::whose::name();
  let Ok(()) = crate::opening::started(&whose);
  let Ok(waited) = crate::picker::waited_for_screen();
  let Ok(()) = crate::opening::taking("screen", waited);

  let Ok(tells) = description::where_to();

  with_tasks_of_its_own(|tasks| {
    let Ok(shut) = CloseWatch::local();

    serve(
      &whose,
      (Box::new(build), tasks),
      start,
      Framing::Card,
      shut,
      Reported {
        front: None,
        first_frame: FirstFrame::LocalPicker,
        tells,
        presses: 0,
        told_presses: 0,
        keyboard: KeyboardFocus::Elsewhere,
        told_keyboard: KeyboardFocus::Elsewhere,
      },
    )
  })
}

pub fn app(who: &str, card: crate::card::Card) -> Result<(), Never> {
  let crate::card::Card {
    build,
    column: _,
    start,
    done,
  } = card;
  let Ok(()) = crate::whose::set_owner(who);
  let Ok(()) = crate::opening::started(who);
  let Ok(tells) = description::where_to();
  let Ok(()) = with_tasks_of_its_own(|tasks| {
    let Ok(shut) = CloseWatch::local();

    serve(
      who,
      (build, tasks),
      start.as_deref(),
      Framing::Window,
      shut,
      Reported {
        front: None,
        first_frame: FirstFrame::Reported,
        tells,
        presses: 0,
        told_presses: 0,
        keyboard: KeyboardFocus::Elsewhere,
        told_keyboard: KeyboardFocus::Elsewhere,
      },
    )
  });

  done()
}

pub fn drawn_here(who: &str, card: crate::card::Card) -> Result<(), Never> {
  let crate::card::Card {
    build,
    column,
    start,
    done,
  } = card;

  let Ok(()) = crate::whose::set_owner(who);
  let Ok(()) = show(build, column, start.as_deref());

  done()
}

fn opened_on(
  pages: Vec<Page>,
  start: Option<&str>,
  framing: Framing,
  tasks: Actor<Tasks>,
) -> Result<State, Never> {
  Ok(State {
    rows: Vec::new(),
    loading: false,
    model: CardModel {
      here: match crate::page::find(&pages, start) {
        Ok(index) => index,
        Err(never) => match never {},
      },
      pages,
      leaving: Leaving::No,
      at: None,
      opened: Opened::No,
      from_tab: 0,
      scroll: 0,
      note: None,
      typed: TextField::default(),
      preedit: None,
      prompt_identity: 0,
      beside: Beside::No,
      asking: None,
      sure: None,
      pressing: None,
      zoom: None,
      shown: None,
      selected: Vec::new(),
      framing,
      tasks,
      live: None,
    },
  })
}

fn card_taking(
  surface: &mut Surface,
  size: &Size<i32>,
  opened: Opened,
  framing: Framing,
) -> Result<Card, Never> {
  let room = match opened {
    Opened::Expanded => Room::Over,
    Opened::No => Room::Around,
  };
  let Ok(()) = surface.room(room);

  card_on(size, opened, framing)
}

fn card_on(surface: &Size<i32>, opened: Opened, framing: Framing) -> Result<Card, Never> {
  match (opened, framing) {
    (Opened::Expanded, _) | (Opened::No, Framing::Window) => {
      return Ok(Card {
        x: 0,
        y: 0,
        width: surface.width,
        height: surface.height,
      });
    }
    (Opened::No, Framing::Card) => {}
  }

  let Ok(wide) = shape::part_of(surface.width);
  let Ok(tall) = shape::tall_part_of(surface.height);

  let Ok(x) = fitted::<i32, i32>(surface.width.saturating_sub(wide).saturating_div(2));
  let Ok(y) = fitted::<i32, i32>(surface.height.saturating_sub(tall).saturating_div(2));

  Ok(Card {
    x,
    y,
    width: wide,
    height: tall,
  })
}

fn covering() -> Result<Wanted, Never> {
  let Ok(whose) = crate::whose::name();

  Ok(Wanted {
    namespace: whose,
    anchor: Anchor::Whole,
    size: WHATEVER_THE_SCREEN_IS,
    margin: Margin {
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
    },
    keyboard: Keyboard::OnDemand,
    room: Room::Around,
    under: Under::None,
  })
}

fn windowed() -> Result<Window, Never> {
  let Ok(whose) = crate::whose::name();

  Ok(Window {
    app_id: whose.clone(),
    title: whose,
  })
}

fn nth<T>(list: &[T], at: u32) -> Result<Option<&T>, Never> {
  let Ok(at_2) = index(at);

  Ok(list.get(at_2))
}

fn sized(logical: Size<u32>) -> Result<Size<i32>, Never> {
  let Ok(wide) = fitted::<u32, i32>(logical.width);
  let Ok(tall) = fitted::<u32, i32>(logical.height);

  Ok(Size {
    width: wide,
    height: tall,
  })
}

fn scroll_room(state: &State, card: &Card) -> Result<i32, Never> {
  let Ok(strip_tall) = fitted::<i32, i32>(fitting::STRIP);
  let Ok(page) = nth(&state.pages, state.here);
  let sought_tall = match page {
    Some(page_2) => asked_about(page_2)?,
    None => 0,
  };

  Ok(
    card
      .height
      .saturating_sub(strip_tall)
      .saturating_sub(OVER_ROWS)
      .saturating_sub(sought_tall),
  )
}

fn hit_test(touching: &[HitRegion], hit: Point<i32>) -> Result<Option<Lands>, Never> {
  let target_at = |point| {
    touching.iter().find_map(|region| {
      let Ok(covers) = region.panel.covers(point);

      (covers == Covers::Yes).then_some(region.lands)
    })
  };

  Activation::At(hit).resolve(|| None, target_at)
}

fn sync_watch(watching: &mut Option<(u32, BoundToParent)>, state: &State) -> Result<(), Never> {
  let already = watching.as_ref().map(|(on, _)| *on);

  match ::core::primitive::bool::then_some(already == Some(state.here), ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  *watching = None;

  let Ok(page) = nth(&state.pages, state.here);
  let arguments = match page.and_then(|page_2| page_2.watch.as_ref()) {
    Some(watch) => &watch.arguments,
    None => return Ok(()),
  };

  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Ok(()),
  };

  let invocation = Invocation {
    program: std::path::PathBuf::from(program),
    arguments: rest.to_vec(),
  };
  let platform = CommandPlatform {
    timing: Timing::Background,
  };

  match platform.alongside(&invocation, Streams::QUIET) {
    Ok(running) => *watching = Some((state.here, running)),
    Err(fault) => console_core_logging::warn!("console-panel: {program}: {fault}"),
  }

  Ok(())
}

const FRONT_EVENTS: u16 = 32;

struct InFront {
  watching: Option<(u32, BoundToParent)>,
  subscriptions: Subscriptions,
  now: Option<Subscription>,
  events: mpsc::Receiver<Received>,
}

impl InFront {
  fn none() -> Result<Self, Never> {
    let Ok(subscriber) = console_events::subscription::connect(&[]);

    InFront::on(subscriber)
  }

  fn on(subscriber: Subscriber) -> Result<Self, Never> {
    let Ok((subscriptions, received)) = subscriber.split();
    let Ok(capacity) = index(FRONT_EVENTS);
    let (tell, events) = mpsc::sync_channel(capacity);
    let Ok(()) = forwarded(received, tell);

    Ok(InFront {
      watching: None,
      subscriptions,
      now: None,
      events,
    })
  }

  fn follow(&mut self, state: &State, page: Option<&Page>) -> Result<(), Never> {
    let Ok(()) = sync_watch(&mut self.watching, state);

    self.listen(page)
  }

  fn listen(&mut self, page: Option<&Page>) -> Result<(), Never> {
    let wanted = page.and_then(|current| current.listens.clone());

    match (wanted.as_ref().map(|one| &one.event_group)
      == self.now.as_ref().map(|one| &one.event_group))
    .then_some(())
    {
      Some(()) => return Ok(()),
      None => {}
    }

    match self.now.take() {
      Some(former) => {
        let Ok(()) = self.subscriptions.unsubscribe(&former.event_group);
      }
      None => {}
    }

    match wanted.as_ref() {
      Some(current) => {
        let Ok(()) = self.subscriptions.subscribe(&current.event_group);
      }
      None => {}
    }

    self.now = wanted;
    Ok(())
  }

  fn changed(&self) -> Result<Worth, Never> {
    let Ok(batch) = index(FRONT_EVENTS);
    let mut count = 0u16;
    let mut changed = Worth::Ignoring;

    for event in self.events.try_iter().take(batch) {
      count = count.saturating_add(1);
      let Ok(wanted) = worth(&event, self.now.as_ref());

      match wanted {
        Worth::Querying => changed = Worth::Querying,
        Worth::Ignoring => {}
      }
    }

    Ok(
      match (count == FRONT_EVENTS && self.now.is_some()).then_some(()) {
        Some(()) => Worth::Querying,
        None => changed,
      },
    )
  }
}

fn forwarded(
  received: mpsc::Receiver<Received>,
  tell: mpsc::SyncSender<Received>,
) -> Result<(), Never> {
  console_program_lifetime::threads::let_go(thread::spawn(move || {
    #[cfg_attr(
      dylint_lib = "explicit055_no_unbounded_loop",
      expect(
        explicit055_no_unbounded_loop,
        reason = "the compositor subscription is owned by InFront; dropping the panel closes this source and the destination mailbox, either of which ends the forwarder"
      )
    )]
    for event in received {
      match tell.try_send(event) {
        Ok(()) | Err(mpsc::TrySendError::Full(_)) => {
          let Ok(()) = crate::frames::subscription_ready();
        }
        Err(mpsc::TrySendError::Disconnected(_panel_ended)) => return,
      }
    }
  }))
}

fn worth(event: &Received, listening: Option<&Subscription>) -> Result<Worth, Never> {
  Ok(match (event, listening) {
    (Received::Connected, Some(_)) => Worth::Querying,
    (Received::Event(change), Some(one)) => {
      match ::core::primitive::bool::then_some(change.event_group == one.event_group, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(worth) = (one.worth)(&change.text);

          worth
        }
        ::core::option::Option::None => Worth::Ignoring,
      }
    }
    (Received::Connected | Received::Event(_), None) => Worth::Ignoring,
  })
}

struct Reading {
  here: u32,
  arrived: mpsc::Receiver<Vec<Row>>,
}

fn reading(page: &Page, here: u32) -> Result<Reading, Never> {
  let (sending, arrived) = mpsc::sync_channel(1);
  let rows = page.rows.clone();
  let asking = thread::spawn(move || {
    let Ok(read) = rows.read();
    if sending.send(read).is_ok() {
      let Ok(()) = crate::frames::tell(crate::frames::Notice::Card);
    }
  });
  let Ok(()) = console_program_lifetime::threads::let_go(asking);

  Ok(Reading { here, arrived })
}

fn meanwhile(page: Option<&Page>) -> Result<Vec<Row>, Never> {
  Ok(match page.and_then(|page_2| page_2.meanwhile.clone()) {
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
  slid: Option<u32>,
  picker_drag: Option<Point<i32>>,
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
    WakeOutcome::AlreadyAwake => {}
  }

  let Ok(across_at) = toward_zero_i32(at.0);
  let Ok(down_at) = toward_zero_i32(at.1);
  let Ok(lands) = hit_test(
    &panelled.touching,
    Point {
      x: across_at,
      y: down_at,
    },
  );
  let Ok(pans) = panning(
    state,
    panelled.moving.as_ref(),
    Point {
      x: across_at,
      y: down_at,
    },
  );

  Ok(Down::Touched(Touch {
    from: at,
    scroll: state.scroll,
    dragged: Dragged::No,
    lands,
    slid: None,
    picker_drag: None,
    pans,
  }))
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
  let furthest = many
    .saturating_mul(fitting::ROW)
    .saturating_sub(room)
    .max(0);
  let down_2 = down.clamp(0, furthest);

  state.scroll = down_2;

  let first = down_2
    .saturating_add(fitting::ROW)
    .saturating_sub(1)
    .saturating_div(fitting::ROW);
  let last = down_2
    .saturating_add(room)
    .saturating_div(fitting::ROW)
    .saturating_sub(1);
  let Ok(at) = fitted::<u32, i32>(match state.at {
    Some(at) => at,
    None => 0,
  });

  let wanted = match (at < first, at > last) {
    (true, _) => walked(&state.rows, first.saturating_sub(1), Step(1)),
    (false, true) => walked(&state.rows, last.saturating_add(1), Step(-1)),
    (false, false) => return Ok(()),
  };
  let Ok(wanted_2) = wanted;
  let Ok(landed) = fitted::<i32, u32>(wanted_2.max(0));

  state.at = Some(landed);
  state.beside = Beside::No;

  Ok(())
}

fn tapped(
  state: &mut State,
  lands: Option<Lands>,
  hit: Point<i32>,
  card: &Card,
) -> Result<ActionOutcome, Never> {
  let rows = &state.rows;
  let model = &mut state.model;

  Ok(match lands {
    Some(Lands::Row(landed)) => {
      let Ok(many) = fitted::<_, u32>(rows.len());

      match ::core::primitive::bool::then_some(landed < many, ()) {
        ::core::option::Option::Some(()) => {
          model.at = Some(landed);
          model.beside = Beside::No;

          let Ok(action) = row_action(Activation::At(landed), model.at, Selection::Chose);
          let Ok(gone) = carried_out(model, rows, action);

          gone
        }
        ::core::option::Option::None => ActionOutcome::Stay,
      }
    }
    Some(Lands::Answer(which)) => {
      let Ok(()) = answered(model, rows, Activation::At(which));

      ActionOutcome::Stay
    }
    Some(Lands::Nudge { row, step }) => {
      model.at = Some(row);
      model.beside = Beside::No;

      let Ok(moved) = nudge_level(rows, row, step);
      let Ok(outcome) = carried_out(model, rows, moved);

      outcome
    }
    Some(Lands::Else(row)) => {
      model.at = Some(row);

      let Ok(action) = row_action(Activation::At(row), model.at, Selection::Else);
      let Ok(gone) = carried_out(model, rows, action);

      gone
    }
    Some(Lands::ButtonPress { row, which }) => {
      model.at = Some(row);
      model.beside = Beside::No;
      model.pressing = Some((row, which));

      let Ok(action) = row_action(Activation::At(row), model.at, Selection::Chose);
      let Ok(gone) = carried_out(model, rows, action);

      gone
    }
    Some(Lands::Seek { row, frame }) => {
      model.at = Some(row);
      model.beside = Beside::No;

      let Ok(landed) = sought_under(rows, row, (hit, frame));

      match landed {
        Some(fraction) => {
          let Ok(()) = sought(model, rows, row, fraction);
        }
        None => {}
      }

      ActionOutcome::Stay
    }
    Some(Lands::Slider { row, frame }) => {
      model.at = Some(row);
      model.beside = Beside::No;

      let on_the_track = Point {
        x: hit.x,
        y: frame.origin.y,
      };
      let Ok(_moved) = slid(rows, row, (Press::Tap(on_the_track), frame), None);

      ActionOutcome::Stay
    }
    Some(Lands::Picker { row, frame }) => {
      let on_the_picker = Point {
        x: hit.x,
        y: frame.origin.y,
      };
      let Ok(()) = press_picker(model, rows, row, (Press::Tap(on_the_picker), frame));

      ActionOutcome::Stay
    }
    Some(Lands::Tab(which)) => {
      let Ok(()) = stood_on(model, which);

      ActionOutcome::Stay
    }
    Some(Lands::More(step)) => {
      let Ok(_turned) = apply_meaning(model, Meaning::Tab(step), rows);

      ActionOutcome::Stay
    }
    Some(Lands::Reveal) => {
      let Ok(()) = toggle_secret(model);

      ActionOutcome::Stay
    }
    Some(Lands::Back) => {
      model.leaving = Leaving::No;

      match (model.asking.take(), model.sure.take()) {
        (None, None) => {
          let Ok(said) = apply_meaning(model, Meaning::Close, rows);
          let Ok(gone) = carried_out(model, rows, said);

          gone
        }
        (Some(_), _) | (None, Some(_)) => ActionOutcome::Stay,
      }
    }
    None => {
      let Ok(away) = outside(card, hit);

      match (away, &model.asking, &model.sure) {
        (Covers::No, _, _) => ActionOutcome::Stay,
        (Covers::Yes, Some(_), _) | (Covers::Yes, None, Some(_)) => {
          model.asking = None;
          model.sure = None;

          ActionOutcome::Stay
        }
        (Covers::Yes, None, None) => {
          let Ok(said) = apply_meaning(model, Meaning::Close, rows);
          let Ok(gone) = carried_out(model, rows, said);

          gone
        }
      }
    }
  })
}

fn answered(
  state: &mut CardModel,
  rows: &[Row],
  activation: Activation<u32>,
) -> Result<(), Never> {
  let Ok(answer) = activation.resolve(|| state.sure.as_ref().map(|sure| sure.at), Some);

  match answer {
    Some(which) => took(state, rows, which),
    None => Ok(()),
  }
}

fn with_tasks_of_its_own(
  drawing: impl FnOnce(Actor<Tasks>) -> Result<(), Never>,
) -> Result<(), Never> {
  let booted = match console_actor::boot::<Tasks>(()) {
    Ok(booted) => booted,
    Err(fault) => {
      console_core_logging::warn!(
        "console-panel: nothing to keep what a press starts in, so nothing is drawn: {fault}"
      );

      return Ok(());
    }
  };
  let console_actor::Booted {
    actor,
    scope,
    effects: _nothing_runs_before_a_press,
  } = booted;
  let Ok(()) = drawing(actor);

  scope.close()
}

fn serve(
  who: &str,
  (build, tasks): (crate::card::Build, Actor<Tasks>),
  start: Option<&str>,
  framing: Framing,
  shut: CloseWatch,
  reported: Reported,
) -> Result<(), Never> {
  let mut reported_2 = reported;
  crate::frames::with_waking(|_| ());
  let Ok(()) = crate::arrivals::follow();
  let Ok(_nothing_to_say_before_a_card_is_up) = tasks::told(&tasks, TaskEvent::CardUp);
  let Ok(()) = serving(
    who,
    (build, tasks.clone()),
    start,
    framing,
    shut,
    &mut reported_2,
  );
  let Ok(_said_as_notifications_once_nothing_is_up) = tasks::told(&tasks, TaskEvent::CardGone);

  let front = match reported_2.front {
    Some(front) => front,
    None => return Ok(()),
  };

  match console_onscreen::forget_if_still(&front) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!(
      "console-panel: the tab in front could not be forgotten: {fault}"
    ),
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
    crate::opening::Running::Yes => {}
    crate::opening::Running::No => return Ok(()),
  }

  let Ok(many) = fitted::<_, u64>(rows.len());
  let Ok(()) = crate::opening::counted("rows", many);
  let Ok(()) = crate::opening::mark("frame");

  crate::opening::finish()
}

enum FirstFrame {
  Hosted(DrawingSender),
  LocalPicker,
  Reported,
}

impl FirstFrame {
  fn draw<Fault>(
    &mut self,
    draw: impl FnOnce(Option<DrawingSender>) -> Result<(), Fault>,
    mark_picker: impl FnOnce() -> Result<(), Never>,
  ) -> Result<(), Fault> {
    match std::mem::replace(self, Self::Reported) {
      Self::Hosted(finished) => draw(Some(finished)),
      Self::LocalPicker => {
        let drawn = draw(None);
        drawn?;
        let Ok(()) = mark_picker();

        Ok(())
      }
      Self::Reported => draw(None),
    }
  }
}

struct Reported {
  front: Option<String>,
  first_frame: FirstFrame,
  tells: Option<String>,
  presses: u64,
  told_presses: u64,
  keyboard: KeyboardFocus,
  told_keyboard: KeyboardFocus,
}

fn tell(
  state: &State,
  panelled: &Panelled,
  room: (i32, i32),
  presses: u64,
  reported: &mut Reported,
) -> Result<(), Never> {
  let Ok(told) = what_it_drew(state, panelled, &state.rows, room);
  let told_2 = description::Description {
    presses,
    keyboard: reported.keyboard,
    ..told
  };
  let Ok(()) = description::wrote(&told_2, reported.tells.as_deref());

  reported.told_presses = presses;
  reported.told_keyboard = reported.keyboard;

  Ok(())
}

fn in_front(page: Option<&Page>, reported: &mut Reported) -> Result<(), Never> {
  let page_3 = match page {
    Some(page_2) => page_2,
    None => return Ok(()),
  };

  match console_onscreen::write_tab(&page_3.title) {
    Ok(()) => reported.front = Some(page_3.title.clone()),
    Err(fault) => console_core_logging::warn!(
      "console-panel: the tab in front could not be written down: {fault}"
    ),
  }

  Ok(())
}

fn entered(state: &mut CardModel) -> Result<ActionOutcome, Never> {
  let Ok(page) = nth(&state.pages, state.here);
  let arrive = page.and_then(|page| page.entered.clone());

  match arrive {
    Some(arrive) => effects::dispatch(state, &[], effects::Source::Page, |showing, _| {
      arrive(showing)
    }),
    None => Ok(ActionOutcome::Stay),
  }
}

fn serving(
  who: &str,
  (build, tasks): (crate::card::Build, Actor<Tasks>),
  start: Option<&str>,
  framing: Framing,
  shut: CloseWatch,
  reported: &mut Reported,
) -> Result<(), Never> {
  let Ok(()) = crate::whose::set_owner(who);

  let Ok(pages) = built(build);
  let wearing = match Wearing::worn() {
    Ok(wearing) => wearing,
    Err(why) => {
      console_core_logging::warn!("console-panel: no palette: {why}");

      return Ok(());
    }
  };

  let mut surface = match Surface::connect() {
    Ok(surface) => surface,
    Err(fault) => {
      console_core_logging::warn!("console-panel: no surface: {fault}");

      return Ok(());
    }
  };

  let Ok(state) = opened_on(pages, start, framing, tasks);

  let shown = match framing {
    Framing::Card => {
      let Ok(wanted) = covering();

      surface.show(&wanted)
    }
    Framing::Window => {
      let Ok(window) = windowed();

      surface.show_window(&window)
    }
  };

  match shown {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("console-panel: no surface to draw on: {fault}");

      return Ok(());
    }
  }

  let Ok(()) = crate::opening::mark("surface");
  let marks_measured = measure_marks()?;
  let Ok(()) = crate::opening::mark("fonts");
  let Ok(none) = InFront::none();
  let Ok(accessibility) = accessibility::Bridge::new(who);
  let turning = Turning {
    surface,
    accessibility,
    state,
    painted: Visible::NotYet,
    front: none,
    rows_of: None,
    asked: None,
    stale: Stale::No,
    dirty: Dirty::Yes,
    sized_for: None,
    measured_tabs: Vec::new(),
    measured_rows: RefCell::new(BTreeMap::new()),
    fits: 1,
    panelled: Panelled {
      shapes: Vec::new(),
      touching: Vec::new(),
      moving: None,
    },
    finger: None,
    reported,
  };
  let still = Still {
    shut: &shut,
    wearing: &wearing,
    marks_measured,
  };

  match console_core_iteration::iterate(turning, |turning_2| turned(turning_2, still)) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("console-panel: the event loop stopped: {fault:?}"),
  }

  Ok(())
}

struct Turning<'a> {
  surface: Surface,
  accessibility: accessibility::Bridge,
  state: State,
  painted: Visible,
  front: InFront,
  rows_of: Option<u32>,
  asked: Option<Reading>,
  stale: Stale,
  dirty: Dirty,
  sized_for: Option<(Size<u32>, u32)>,
  measured_tabs: Vec<MeasuredTab>,
  measured_rows: RowMeasurements,
  fits: u32,
  panelled: Panelled,
  finger: Option<Touch>,
  reported: &'a mut Reported,
}

#[derive(Clone, Copy)]
struct Still<'a> {
  shut: &'a CloseWatch,
  wearing: &'a Wearing,
  marks_measured: MeasuredMarks,
}

fn turned<'a>(
  mut turning: Turning<'a>,
  still: Still<'_>,
) -> Result<Stepped<Turning<'a>, ()>, Never> {
  let Ok(outcome) = turn_once(&mut turning, still);

  Ok(match outcome {
    ActionOutcome::Close => Stepped::Halt(()),
    ActionOutcome::Stay => Stepped::Again(turning),
  })
}

fn turn_once(turning: &mut Turning<'_>, still: Still<'_>) -> Result<ActionOutcome, Never> {
  let Still {
    shut,
    wearing,
    marks_measured,
  } = still;
  let Turning {
    surface,
    state,
    painted,
    front,
    rows_of,
    asked,
    stale,
    dirty,
    sized_for,
    measured_tabs,
    measured_rows,
    fits,
    panelled,
    finger,
    reported,
    ..
  } = turning;

  let Ok(stopping) = shut.stop();

  match stopping {
    StopRequest::Requested => return Ok(ActionOutcome::Close),
    StopRequest::Open => {}
  }

  let logical = match surface.logical() {
    Ok(Some(logical)) => logical,
    Ok(None) => {
      let waited = crate::frames::with_waking(|waking| {
        let also: Vec<_> = waking.into_iter().collect();
        surface.wait(&also, None)
      });
      match waited {
        Ok(()) => {}
        Err(fault) => {
          console_core_logging::warn!("console-panel: waiting for the initial size: {fault}");
          return Ok(ActionOutcome::Close);
        }
      }

      let Ok(woken) = crate::frames::wake_state();

      match woken.rows {
        crate::frames::FrameReceived::Yes => *stale = Stale::Yes,
        crate::frames::FrameReceived::No => {}
      }

      *dirty = Dirty::Yes;

      return Ok(ActionOutcome::Stay);
    }
    Err(_) => return Ok(ActionOutcome::Close),
  };

  let Ok(surface_size) = sized(logical);
  let Ok(card) = card_taking(surface, &surface_size, state.opened, state.framing);
  let card_wide_u32 = fitted::<i32, u32>(card.width)?;

  let sizing = (logical, card_wide_u32);

  match ::core::primitive::bool::then_some(*sized_for == Some(sizing), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      let Ok(spent) = spent();
      let room = strip::room(strip::Card {
        width: card.width,
        spent,
      })?;

      let Ok(tabs) = measure_tabs(&state.pages, card_wide_u32);
      let Ok(cell) = widest_tab(&tabs);
      let Ok(fitting_tabs) = strip::fits(room, cell);

      *measured_tabs = tabs;
      *fits = fitting_tabs;
      measured_rows.get_mut().clear();
      *sized_for = Some(sizing);
      *dirty = Dirty::Yes;
    }
  }

  let entering = *rows_of != Some(state.here);

  if entering {
    let Ok(outcome) = entered(&mut state.model);

    if outcome == ActionOutcome::Close {
      return Ok(outcome);
    }
  }

  let Ok(page) = nth(&state.model.pages, state.model.here);
  let Ok(()) = front.follow(state, page);
  let Ok(changed) = front.changed();

  match changed {
    Worth::Querying => *stale = Stale::Yes,
    Worth::Ignoring => {}
  }

  match entering {
    false => {}
    true => {
      let Ok(()) = in_front(page, reported);
      let Ok(first) = meanwhile(page);
      let Ok(running) = tasks::running_now(&state.tasks);
      let Ok(first_2) = tasks::shown(first, &running);
      let Ok(live) = running.live();

      state.rows = first_2;
      state.model.live = live;

      match (state.at.is_none() && !state.rows.is_empty()).then_some(()) {
        ::core::option::Option::Some(()) => {
          state.model.at = Some(0);
        }
        ::core::option::Option::None => {}
      }

      measured_rows.get_mut().clear();
      *rows_of = Some(state.here);
      *stale = Stale::Yes;
      *dirty = Dirty::Yes;
    }
  }

  if let (Stale::Yes, Some(page)) = (*stale, page) {
    match &page.rows {
      crate::page::Rows::Fixed(rows) => {
        let rows = rows.clone();
        let Ok(()) = apply_rows(state, rows);
        *asked = None;
        *dirty = Dirty::Yes;
      }
      crate::page::Rows::Computed(_) => {
        let Ok(started) = reading(page, state.here);
        *asked = Some(started);
      }
    }
    *stale = Stale::No;
  }

  match asked
    .as_ref()
    .map(|reading| (reading.here, reading.arrived.try_recv()))
  {
    Some((here, Ok(read))) => {
      *asked = None;

      match ::core::primitive::bool::then_some(here == state.here, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = apply_rows(state, read);
          *dirty = Dirty::Yes;
        }
        ::core::option::Option::None => {}
      }
    }
    Some((_, Err(mpsc::TryRecvError::Disconnected))) => {
      *asked = None;
      *dirty = Dirty::Yes;
    }
    Some((_, Err(mpsc::TryRecvError::Empty))) | None => {}
  }

  if shut.stop() == Ok(StopRequest::Requested) {
    return Ok(ActionOutcome::Close);
  }

  match (state.at, state.rows.is_empty()) {
    (None, false) => {
      let Ok(first) = walked(&state.rows, BEFORE_THE_FIRST_ROW, Step(1));
      let Ok(landed) = fitted::<i32, u32>(first.max(0));

      state.at = Some(landed);
    }
    (None, true) | (Some(_), _) => {}
  }

  let Ok(room) = scroll_room(state, &card);
  let mut accessibility_changed = accessibility::ViewChange::Unchanged;
  let loading = asked.is_some() && state.rows.is_empty();
  if state.loading != loading {
    state.loading = loading;
    *dirty = Dirty::Yes;
  }

  match dirty {
    Dirty::No => {}
    Dirty::Yes => {
      match finger.as_ref().map(|finger_2| finger_2.dragged) {
        Some(Dragged::Yes) => {}
        Some(Dragged::No) | None => {
          let Ok(down_to) = scrolled(
            state.at,
            Viewport {
              room,
              y: state.scroll,
            },
          );

          state.scroll = down_to;
        }
      }

      let measured = measured_now(state, measured_tabs, measured_rows, marks_measured)?;

      let Ok(drawn) = shapes(state, &card, *fits, &measured, wearing);
      let already_drawn = *painted == Visible::Yes && panelled.shapes == drawn.shapes;
      let Ok(note) = noted_now(&state);

      match {
        ::core::primitive::bool::then_some(panelled.moving != drawn.moving || note.is_some(), ())
      } {
        ::core::option::Option::Some(()) => {
          let Ok(()) = surface.clear_layer();
        }
        ::core::option::Option::None => {}
      }

      *panelled = drawn;
      accessibility_changed = accessibility::ViewChange::Different;
      state.shown = panelled.moving.clone();

      let drawn_size = (surface_size.width, surface_size.height);

      if !already_drawn {
        if let Err(fault) = redraw_card(surface, state, panelled, logical, reported) {
          console_core_logging::warn!("console-panel: the frame could not be drawn: {fault}");
          return Ok(ActionOutcome::Close);
        }
      }

      let Ok(keyboard) = surface.keyboard_focus();
      reported.keyboard = keyboard;

      // A preview can paint while the input's rows are still being computed.
      // Let the driver advance only once it can see the result of the input.
      let presses = match asked {
        Some(_) => reported.told_presses,
        None => reported.presses,
      };
      let told_already =
        (presses, reported.keyboard) == (reported.told_presses, reported.told_keyboard);
      if !already_drawn || !told_already {
        let Ok(()) = tell(state, panelled, drawn_size, presses, reported);
      }

      *painted = Visible::Yes;
      *dirty = Dirty::No;
    }
  }

  handle_input(turning, &card, logical, room, accessibility_changed)
}

fn redraw_card(
  surface: &mut Surface,
  state: &State,
  panelled: &Panelled,
  points: Size<u32>,
  reported: &mut Reported,
) -> Result<(), console_draw_surface::standing::SurfaceError> {
  let Ok(()) = surface.resize(points);
  let paint = |pixels: &mut [u8], device, _scale| {
    let frame = Frame { device, points };

    match painting::onto(pixels, frame, &panelled.shapes) {
      Ok(()) => {}
      Err(fault) => console_core_logging::warn!("console-panel: painting the card: {fault}"),
    }

    Ok(())
  };

  let drawn = reported.first_frame.draw(
    |finished| match finished {
      Some(finished) => surface.draw_notifying(finished, paint),
      None => surface.draw(paint),
    },
    crate::picker::mark_drawn,
  );
  drawn?;

  let Ok(()) = first_drawn(&state.rows);

  Ok(())
}

fn apply_rows(state: &mut State, read: Vec<Row>) -> Result<(), Never> {
  let Ok(running) = tasks::running_now(&state.tasks);
  let Ok(read_2) = tasks::shown(read, &running);
  let Ok(notes) = tasks::told(&state.tasks, TaskEvent::Read);
  let Ok(()) = noted(state, notes);
  let Ok(live) = running.live();

  state.rows = read_2;
  state.live = live;

  match (state.at.is_none() && !state.rows.is_empty()).then_some(()) {
    ::core::option::Option::Some(()) => {
      state.at = Some(0);
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = asked_for(&state.rows);

  let Ok(many) = fitted::<_, u32>(state.rows.len());

  match ::core::primitive::bool::then_some(state.at.is_some_and(|at| at >= many), ()) {
    ::core::option::Option::Some(()) => state.at = None,
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn handle_input(
  turning: &mut Turning<'_>,
  card: &Card,
  logical: Size<u32>,
  room: i32,
  accessibility_changed: accessibility::ViewChange,
) -> Result<ActionOutcome, Never> {
  let Turning {
    surface,
    accessibility,
    state,
    panelled,
    asked,
    finger,
    reported,
    dirty,
    stale,
    ..
  } = turning;
  let Ok(text_input) = text_input_state(state, card);

  match text_input {
    ::core::option::Option::None => {
      state.preedit = None;
    }
    ::core::option::Option::Some(_) => {}
  }

  let Ok(()) = surface.set_text_input(text_input);
  let Ok(focus) = surface.keyboard_focus();
  let Ok(()) = accessibility.update(accessibility_changed, focus, state, &panelled);
  let Ok(note) = noted_now(&state);
  let overlay = match note {
    Some(_) => Overlay::Note,
    None => Overlay::None,
  };
  let updates = wait_for_frame(surface, panelled, logical, (*dirty, *stale), overlay);

  match updates {
    Ok((redraw, reread)) => (*dirty, *stale) = (redraw, reread),
    Err(fault) => {
      console_core_logging::warn!("console-panel: waiting for the frame: {fault}");
      return Ok(ActionOutcome::Close);
    }
  }

  let Ok(focus_2) = surface.keyboard_focus();
  let accessibility_focus = match asked {
    Some(_) => KeyboardFocus::Elsewhere,
    None => focus_2,
  };
  let Ok(accessible_action) = accessibility.action(accessibility_focus, &panelled);

  match accessible_action {
    Some(request) => {
      let action = request.action;
      let Ok(outcome) = accessibility::perform(state, card, room, request);

      match outcome {
        ActionOutcome::Close => return Ok(ActionOutcome::Close),
        ActionOutcome::Stay => {}
      }

      match matches!(
        action,
        accesskit::Action::Click
          | accesskit::Action::Increment
          | accesskit::Action::Decrement
          | accesskit::Action::SetValue
      )
      .then_some(())
      {
        Some(()) => *stale = Stale::Yes,
        None => {}
      }

      *dirty = Dirty::Yes;
    }
    None => {}
  }

  let mut input = PanelInput {
    state,
    view: panelled,
    card: card,
    room,
    finger,
    reported,
    dirty,
    stale,
  };
  input.process(surface)
}

struct PanelInput<'a> {
  state: &'a mut State,
  view: &'a Panelled,
  card: &'a Card,
  room: i32,
  finger: &'a mut Option<Touch>,
  reported: &'a mut Reported,
  dirty: &'a mut Dirty,
  stale: &'a mut Stale,
}

impl PanelInput<'_> {
  fn process(&mut self, surface: &mut Surface) -> Result<ActionOutcome, Never> {
    let Ok(changes) = surface.text_input_changes();

    for change in changes {
      let Ok(changed) = apply_text_input(self.state, change);

      match changed {
        Dirty::Yes => *self.dirty = Dirty::Yes,
        Dirty::No => {}
      }
    }

    let keys = surface.keyboard_events()?;

    for KeyboardEvent::Down { key } in keys {
      let Ok(outcome) = self.key(key);

      match outcome {
        ActionOutcome::Close => return Ok(ActionOutcome::Close),
        ActionOutcome::Stay => {}
      }
    }

    let Ok(keyboard) = surface.keyboard_focus();

    match (keyboard != self.reported.told_keyboard).then_some(()) {
      Some(()) => *self.dirty = Dirty::Yes,
      None => {}
    }

    let pointers = surface.pointer_events()?;

    for event in pointers {
      let Ok(outcome) = self.pointer(event);

      match outcome {
        ActionOutcome::Close => return Ok(ActionOutcome::Close),
        ActionOutcome::Stay => {}
      }
    }

    Ok(ActionOutcome::Stay)
  }

  fn key(&mut self, key: Keysym) -> Result<ActionOutcome, Never> {
    *self.dirty = Dirty::Yes;
    self.reported.presses = self.reported.presses.saturating_add(1);
    let Ok(wake) = stirred(self.state);

    match wake {
      WakeOutcome::Woke => {
        *self.stale = Stale::Yes;
        return Ok(ActionOutcome::Stay);
      }
      WakeOutcome::AlreadyAwake => {}
    }

    let Ok(driving) = driving(self.state);
    let Ok(meaning) = keys::meaning(key, driving);
    let before = self.state.at;
    self.state.preedit = None;
    let Ok(outcome) = pressed_here(
      &mut self.state.model,
      &self.state.rows,
      key,
      meaning,
      driving,
    );

    match outcome {
      ActionOutcome::Close => return Ok(outcome),
      ActionOutcome::Stay => {}
    }

    let Ok(landed) = stepped_onto(meaning, before, self.state);
    let Ok(()) = step_sound(landed);

    match meaning {
      Meaning::Step(_) | Meaning::Tab(_) | Meaning::Abandon => {}
      Meaning::None | Meaning::Choose | Meaning::Close | Meaning::More | Meaning::Nudge(_) => {
        *self.stale = Stale::Yes
      }
    }

    Ok(ActionOutcome::Stay)
  }

  fn pointer(&mut self, event: PointerEvent) -> Result<ActionOutcome, Never> {
    match event {
      PointerEvent::Down { at } => {
        let Ok(down) = finger_down(self.state, self.view, at);
        *self.finger = match down {
          Down::Woke => {
            *self.stale = Stale::Yes;
            *self.dirty = Dirty::Yes;
            None
          }
          Down::Touched(held) => Some(held),
        };
      }
      PointerEvent::Moved { at } => {
        let Ok(()) = self.moved(at);
      }
      PointerEvent::Scrolled { by } => {
        let Ok(pixels) = toward_zero_i32(by * f64::from(fitting::ROW) / A_NOTCH);
        let y = self.state.scroll.saturating_add(pixels);
        let Ok(()) = scrolled_to(self.state, Viewport { room: self.room, y });
        *self.dirty = Dirty::Yes;
      }
      PointerEvent::Pinched { by } => {
        match self.finger.as_mut() {
          Some(held) => {
            held.dragged = Dragged::Yes;
            held.picker_drag = None;
            held.pans = None;
          }
          None => {}
        }

        let Ok(()) = zoomed_to(&mut self.state.model, &self.state.rows, |was| {
          let Ok(zoom) = was.times(by);
          zoom
        });
        *self.dirty = Dirty::Yes;
      }
      PointerEvent::Up => return self.lifted(),
      PointerEvent::Left => *self.finger = None,
    }

    Ok(ActionOutcome::Stay)
  }

  fn moved(&mut self, at: (f64, f64)) -> Result<(), Never> {
    let held = match self.finger.as_mut() {
      Some(touch) => touch,
      None => return self.hovered(at),
    };
    let Ok(sliding) = sliding_under(held.lands);

    match sliding {
      Some((row, frame)) => {
        let Ok(finger_at) = pointer_position(at);
        let Ok(step) = slid(
          &self.state.rows,
          row,
          (Press::Drag(finger_at), frame),
          held.slid,
        );
        held.dragged = Dragged::Yes;

        match step {
          Some(level) => {
            held.slid = Some(level);
            *self.stale = Stale::Yes;
            *self.dirty = Dirty::Yes;
          }
          None => {}
        }

        return Ok(());
      }
      None => {}
    }

    let across = (at.0 - held.from.0).abs();
    let by = at.1 - held.from.1;

    if matches!(held.lands, Some(Lands::Picker { .. }))
      && (held.picker_drag.is_some()
        || (held.dragged == Dragged::No && across >= DRAGGED && across > by.abs()))
    {
      let Ok(at) = pointer_position(at);
      held.dragged = Dragged::Yes;
      held.picker_drag = Some(at);
      return Ok(());
    }

    match (held.dragged, by.abs() >= DRAGGED) {
      (Dragged::No, false) => return Ok(()),
      (Dragged::Yes, _) | (Dragged::No, true) => held.dragged = Dragged::Yes,
    }

    match &held.pans {
      Some((still, zoom, seen)) => {
        let Ok(panned) = zoom.panned(
          Point {
            x: at.0 - held.from.0,
            y: by,
          },
          *seen,
        );
        self.state.zoom = Some((still.clone(), panned));
      }
      None => {
        let Ok(pixels) = toward_zero_i32(by);
        let y = held.scroll.saturating_sub(pixels);
        let Ok(()) = scrolled_to(self.state, Viewport { room: self.room, y });
      }
    }

    *self.dirty = Dirty::Yes;
    Ok(())
  }

  fn hovered(&mut self, at: (f64, f64)) -> Result<(), Never> {
    let Ok(pointer) = pointer_position(at);
    let Ok(lands) = hit_test(&self.view.touching, pointer);
    let index = match (lands, self.state.leaving) {
      (Some(Lands::Row(index)), Leaving::No) => index,
      (Some(_) | None, _) => return Ok(()),
    };

    match (self.state.at == Some(index)).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(hit) = nth(&self.state.rows, index);
    let Ok(heading) = match hit {
      Some(row) => row.heading(),
      None => Ok(Heading::Yes),
    };

    match heading {
      Heading::No => {
        self.state.at = Some(index);
        self.state.beside = Beside::No;
        *self.dirty = Dirty::Yes;
      }
      Heading::Yes => {}
    }

    Ok(())
  }

  fn lifted(&mut self) -> Result<ActionOutcome, Never> {
    let Some(touch) = self.finger.take() else {
      return Ok(ActionOutcome::Stay);
    };

    if let (Some(Lands::Picker { row, frame }), Some(at)) = (touch.lands, touch.picker_drag) {
      let Ok(()) = press_picker(
        &mut self.state.model,
        &self.state.rows,
        row,
        (Press::Drag(at), frame),
      );
      self.reported.presses = self.reported.presses.saturating_add(1);
      *self.stale = Stale::Yes;
      *self.dirty = Dirty::Yes;
      return Ok(ActionOutcome::Stay);
    }

    if touch.dragged == Dragged::Yes {
      return Ok(ActionOutcome::Stay);
    }
    self.reported.presses = self.reported.presses.saturating_add(1);
    let Ok(at) = pointer_position(touch.from);
    let Ok(outcome) = tapped(self.state, touch.lands, at, self.card);

    match outcome {
      ActionOutcome::Close => return Ok(outcome),
      ActionOutcome::Stay => {}
    }

    match touch.lands {
      Some(Lands::Tab(_) | Lands::More(_) | Lands::Reveal) => {}
      None => {
        if outside(self.card, at) == Ok(Covers::Yes) {
          *self.stale = Stale::Yes;
        }
      }
      Some(
        Lands::Back
        | Lands::Row(_)
        | Lands::Answer(_)
        | Lands::Nudge { .. }
        | Lands::Else(_)
        | Lands::ButtonPress { .. }
        | Lands::Seek { .. }
        | Lands::Slider { .. }
        | Lands::Picker { .. },
      ) => *self.stale = Stale::Yes,
    }

    *self.dirty = Dirty::Yes;
    Ok(ActionOutcome::Stay)
  }
}

fn pointer_position((x, y): (f64, f64)) -> Result<Point<i32>, Never> {
  let Ok(across) = toward_zero_i32(x);
  let Ok(down) = toward_zero_i32(y);
  Ok(Point { x: across, y: down })
}

fn step_sound(row: Option<u32>) -> Result<(), Never> {
  let landed = match row {
    Some(index) => index,
    None => return Ok(()),
  };
  let Ok(effects) = SoundEffects::current();

  match effects {
    SoundEffects::On => {
      let Ok(root) = Degree::degree_at(landed);
      let Ok(cue) = Sound::Step.cue();

      match console_sound_effects::play(&cue, root) {
        Ok(()) => {}
        Err(fault) => console_core_logging::warn!("console-panel: a step went unheard: {fault}"),
      }
    }
    SoundEffects::Off => {}
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use console_core_color::palette::{self, Wearing};
  use console_core_geometry::Size;
  use console_core_never::Never;

  use super::stage;
  use crate::page::{ActionOutcome, Aside, Handler, Page, Picture, Row, Rows};
  use crate::surface::{HitRegion, Panelled, what_it_drew};
  use console_core_number_conversion::{fitted, index};
  use console_core_shapes::{Clip, Shape};
  use std::path::PathBuf;
  use std::time::Duration;

  use std::sync::mpsc;

  type Failure = Box<dyn std::error::Error>;

  const SAMPLE_PALETTE: &str = "edge=333333\nfill=222222\nground=111111\nnight=0a0a0a\npanel=1a1a1a\npink=ffb0d0\ncoral=ff6b6b\nsoft=999999\ntext=eeeeee";

  const SCREEN: Size<i32> = Size {
    width: 1024,
    height: 640,
  };

  use super::{
    Beside, Card, Driving, Keysym, Lands, Meaning, Opened, Outcome, Path, State, TextField,
    Viewport, apply_meaning, beside_the_words, carried_out, driving, measure_tabs, pressed_here,
    scrolled, shapes, stepped_onto, wanted_of,
  };

  #[test]
  fn a_local_picker_is_marked_open_after_its_first_frame() {
    let mut frame = super::FirstFrame::LocalPicker;
    let calls = super::RefCell::new(Vec::new());
    let draw = |finished: Option<super::DrawingSender>| {
      assert!(finished.is_none());
      calls.borrow_mut().push("draw");

      Ok::<(), Never>(())
    };
    let mark_picker = || {
      calls.borrow_mut().push("mark open");

      Ok(())
    };

    let Ok(()) = frame.draw(draw, mark_picker);
    let Ok(()) = frame.draw(draw, mark_picker);

    assert_eq!(calls.into_inner(), ["draw", "mark open", "draw"]);
  }

  #[test]
  fn a_failed_first_frame_leaves_the_local_picker_unmarked() {
    let mut frame = super::FirstFrame::LocalPicker;
    let marked = std::cell::Cell::new(false);
    let drawn = frame.draw(
      |_| Err("draw failed"),
      || {
        marked.set(true);

        Ok(())
      },
    );

    assert_eq!(drawn, Err("draw failed"));
    assert!(!marked.get());
  }

  #[test]
  fn a_tab_without_rows_displays_loading_and_keeps_its_tab_controls() -> Result<(), Failure> {
    let Ok(sound) = page("Sound", &["Speakers"]);
    let Ok(battery) = page("Battery", &[]);
    let mut state = state_of(vec![sound, battery])?;
    state.here = 1;
    state.rows.clear();
    state.loading = true;
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let Ok(panelled) = drawn(&state, &card, &wearing);

    assert!(panelled.shapes.iter().any(|shape| {
      matches!(shape, console_core_shapes::Shape::Text(text) if text.said == "Loading…")
    }));
    assert!(
      panelled
        .touching
        .iter()
        .any(|region| region.lands == Lands::Tab(0))
    );
    assert!(
      !panelled
        .touching
        .iter()
        .any(|region| matches!(region.lands, Lands::Row(_)))
    );

    Ok(())
  }

  fn plain_inks() -> Result<super::Inks, Never> {
    let ground = console_core_color::Oklch::BLACK;
    let words = console_core_color::Oklch::WHITE;

    Ok(super::Inks { ground, words })
  }

  fn sample_palette() -> Result<Wearing, Failure> {
    let Ok(spent) = palette::read(SAMPLE_PALETTE);
    let wearing = Wearing::out_of(&spent)?;

    Ok(wearing)
  }

  fn collected<T>(reports: &mpsc::Receiver<T>) -> Result<Vec<T>, Never> {
    Ok(reports.try_iter().take(32).collect())
  }

  fn empty_answer() -> Result<crate::page::Answer, Never> {
    let Ok(answer) = crate::page::retained(|_: &dyn crate::page::Showing, _: &str| {});
    Ok(answer)
  }

  fn ignored_choice() -> Result<crate::page::OnChosen, Never> {
    let Ok(choice) = crate::page::retained(|_: &dyn crate::page::Showing, _: u32| {});
    Ok(choice)
  }

  fn rows_start(card: &Card) -> Result<i32, Never> {
    Ok(
      card
        .y
        .saturating_add(crate::fitting::STRIP)
        .saturating_add(super::OVER_ROWS),
    )
  }

  fn highlighted_rows(
    panelled: &super::Panelled,
    card: &Card,
    wearing: &Wearing,
  ) -> Result<Vec<i32>, Never> {
    let Ok(start) = rows_start(card);

    Ok(
      panelled
        .shapes
        .iter()
        .filter_map(|shape| match shape {
          console_core_shapes::Shape::Panel(panel) => Some(panel),
          console_core_shapes::Shape::Text(_)
          | console_core_shapes::Shape::Picture(_)
          | console_core_shapes::Shape::Cropped(_)
          | console_core_shapes::Shape::Line(_)
          | console_core_shapes::Shape::Clip(_) => None,
        })
        .filter(|panel| panel.fill == wearing.pink)
        .filter(|panel| panel.at.y >= start)
        .map(|panel| panel.at.y)
        .collect(),
    )
  }

  fn said_row(says: &str) -> Result<Row, Never> {
    Row::text(says, Aside(""))
  }

  #[test]
  fn input_method_preedit_is_provisional_and_passwords_keep_surrounding_text_private()
  -> Result<(), Failure> {
    let Ok(page) = page("Files", &[]);
    let mut state = state_of(vec![page])?;
    let Ok(answer) = empty_answer();
    let Ok(()) = super::effects::dispatch(
      &mut state.model,
      &state.rows,
      super::effects::Source::Focused,
      |showing, _| showing.ask("Password", answer),
    );

    let pending = console_draw_surface::Composition {
      text: "仮名".to_owned(),
      cursor_begin: 3,
      cursor_end: 3,
    };
    let Ok(changed) = super::apply_text_input(
      &mut state,
      console_draw_surface::TextInputChange {
        preedit: Some(pending),
        ..Default::default()
      },
    );
    assert_eq!(changed, super::Dirty::Yes);
    let asking = state.asking.as_ref().ok_or("missing password prompt")?;
    assert_eq!(asking.typed.text(), Ok(""));
    let Ok((shown, before, _)) = super::in_the_line(&state);
    assert_eq!(shown, crate::marks::HIDDEN.repeat(2));
    assert_eq!(before, crate::marks::HIDDEN);

    let Ok(_changed) = super::apply_text_input(
      &mut state,
      console_draw_surface::TextInputChange {
        commit: Some("仮名".to_owned()),
        ..Default::default()
      },
    );
    assert!(state.preedit.is_none());
    let asking_2 = state.asking.as_ref().ok_or("missing password prompt")?;
    assert_eq!(asking_2.typed.text(), Ok("仮名"));
    let Ok(card) = card();
    let Ok(active) = super::text_input_state(&state, &card);
    let input = active.ok_or("missing input state")?;
    assert_eq!(input.content, console_draw_surface::InputContent::Password);
    Ok(())
  }

  #[test]
  fn a_row_keeps_its_measurement_identity_when_moved_but_text_changes_invalidate_it() {
    let Ok(original) = Row::text("Old name", Aside("file"));
    let Ok(row) = original.selectable("file-id");
    let Ok(renamed) = Row::text("New name", Aside("file"));
    let Ok(new) = renamed.selectable("file-id");
    assert!(super::RowIdentity::of(&row, 2) == super::RowIdentity::of(&new, 5));
    assert_eq!(
      super::same_measurements(&row, &new),
      Ok(crate::page::Same::No)
    );
  }

  #[test]
  fn accessibility_updates_focus_without_resending_rows_and_checks_action_targets()
  -> Result<(), Failure> {
    use super::accessibility::{Snapshot, TreeChange};
    use accesskit::{Action, ActionRequest, Role, TreeId};
    use std::hash::RandomState;

    let Ok(first) = Row::text("First file", Aside("File"));
    let Ok(first_2) = first.selectable("/files/one");
    let Ok(second) = Row::text("Second file", Aside("File"));
    let Ok(second_2) = second.selectable("/files/two");
    let Ok(page) = Page::new("Files", Rows::Fixed(vec![first_2, second_2]));
    let mut state = state_of(vec![page])?;
    let Ok(card) = card();
    let wearing = sample_palette()?;
    let ids = RandomState::new();
    state.at = Some(0);
    let Ok(view) = drawn(&state, &card, &wearing);
    let Ok(initial) = Snapshot::from_panel(&ids, "Files", &state, &view);
    let Ok(update) = initial.changes(&Snapshot::default(), TreeChange::InitialTree);
    assert!(update.nodes.iter().any(|(id, node)| {
      *id == update.focus && node.role() == Role::ListItem && node.label() == Some("First file")
    }));

    state.at = Some(1);
    let Ok(view_2) = drawn(&state, &card, &wearing);
    let Ok(next) = Snapshot::from_panel(&ids, "Files", &state, &view_2);
    let Ok(moved) = next.changes(&initial, TreeChange::ChangedNodes);
    assert_ne!(moved.focus, update.focus);
    assert!(moved.nodes.is_empty());
    let request = |action| ActionRequest {
      action,
      target_tree: TreeId::ROOT,
      target_node: moved.focus,
      data: None,
    };
    let Ok(focus) = next.action(request(Action::Focus), &view_2);
    let Ok(edit) = next.action(request(Action::SetValue), &view_2);
    assert!(focus.is_some());
    assert!(edit.is_none());
    let hidden = Panelled {
      touching: Vec::new(),
      ..view_2
    };
    let Ok(hidden_focus) = next.action(request(Action::Focus), &hidden);
    assert!(hidden_focus.is_none());
    Ok(())
  }

  #[test]
  fn accessibility_keeps_a_revealed_password_out_of_the_tree() -> Result<(), Failure> {
    use super::accessibility::{Snapshot, TreeChange};
    use accesskit::Role;

    let Ok(page) = page("Network", &[]);
    let mut state = state_of(vec![page])?;
    let Ok(answer) = empty_answer();
    let Ok(()) = super::effects::dispatch(
      &mut state.model,
      &state.rows,
      super::effects::Source::Focused,
      |showing, _| showing.ask("Password", answer),
    );
    let prompt = state.asking.as_mut().ok_or("missing password prompt")?;
    let Ok(typed) = TextField::holding("this must stay private");
    prompt.typed = typed;
    prompt.secret = super::Secret::Shown;
    let Ok(card) = card();
    let wearing = sample_palette()?;
    let Ok(view) = drawn(&state, &card, &wearing);
    let ids = std::hash::RandomState::new();
    let Ok(snapshot) = Snapshot::from_panel(&ids, "Network", &state, &view);
    let Ok(update) = snapshot.changes(&Snapshot::default(), TreeChange::InitialTree);
    let (_, input) = update
      .nodes
      .iter()
      .find(|(id, _)| *id == update.focus)
      .ok_or("missing field")?;
    assert_eq!(input.role(), Role::PasswordInput);
    assert_eq!(input.value(), None);
    assert!(!format!("{update:?}").contains("this must stay private"));
    Ok(())
  }

  #[test]
  #[ignore = "synthetic focus benchmark through one million rows"]
  fn focus_scaling() -> Result<(), Failure> {
    use std::hint::black_box;
    use std::time::Instant;

    let size = Card {
      x: 0,
      y: 0,
      width: 800,
      height: 600,
    };
    let wearing = sample_palette()?;
    let Ok(fits) = fits_for(&size);
    let Ok(page) = page("Files", &[]);
    let Ok(row) = Row::text("A directory entry", Aside("File"));
    let mut state = state_of(vec![page])?;
    let Ok(wide) = wide_of(&size);
    let Ok(tabs) = measure_tabs(&state.pages, wide);
    let Ok(marks) = super::measure_marks();

    for count in [100_u32, 1_000, 10_000, 100_000, 1_000_000] {
      let Ok(count_index) = index(count);
      let rows = (0..count_index)
        .map(|entry| {
          let Ok(row_2) = row.clone().selectable(&format!("directory-entry-{entry}"));
          row_2
        })
        .collect();
      state.rows = rows;
      let measurements = super::RefCell::new(super::BTreeMap::new());
      let middle = ::core::primitive::u32::wrapping_div(count, 2);
      let Ok(scroll) = console_core_number_conversion::fitted::<u32, i32>(middle);
      state.scroll = scroll.saturating_mul(crate::fitting::ROW);
      let mut samples = Vec::new();
      let mut visible = 0u32;

      for _ in 0..9 {
        state.at = Some(middle);
        let started = Instant::now();
        let Ok(driving) = driving(&state);
        let Ok(meaning) = crate::keys::meaning(Keysym::Down, driving);
        let Ok(_outcome) = pressed_here(
          &mut state.model,
          &state.rows,
          Keysym::Down,
          meaning,
          driving,
        );
        let Ok(measured) = super::measured_now(&state, &tabs, &measurements, marks);
        let Ok(panel) = shapes(&state, &size, fits, &measured, &wearing);
        let elapsed = started.elapsed();
        assert_eq!(
          state.at,
          Some(::core::primitive::u32::wrapping_add(middle, 1))
        );
        let Ok(drawn_rows) = fitted::<_, u32>(
          panel
            .touching
            .iter()
            .filter(|hit| matches!(hit.lands, Lands::Row(_)))
            .count(),
        );
        visible = drawn_rows;
        let ids = std::hash::RandomState::new();
        let empty = super::accessibility::Snapshot::default();
        let Ok(snapshot) =
          super::accessibility::Snapshot::from_panel(&ids, "Files", &state, &panel);
        let Ok(tree) = snapshot.changes(&empty, super::accessibility::TreeChange::InitialTree);
        let Ok(nodes) = fitted::<_, u32>(tree.nodes.len());
        assert!(nodes <= visible.saturating_add(4));
        black_box(panel);
        samples.push(elapsed);
      }

      samples.sort_unstable();
      let median = samples.get(4).ok_or("nine focus samples were recorded")?;
      let visible_measurements = measurements.try_borrow()?;
      let Ok(measured) = fitted::<_, u32>(visible_measurements.len());
      println!(
        "{count} rows, {visible} complete rows visible, {measured} measured, focus median {median:?}"
      );
    }

    Ok(())
  }

  fn wide_of(card: &Card) -> Result<u32, Never> {
    console_core_number_conversion::fitted::<i32, u32>(card.width)
  }

  fn fits_for(card: &Card) -> Result<u32, Never> {
    let Ok(room) = crate::strip::room(crate::strip::Card {
      width: card.width,
      spent: 0,
    });

    crate::strip::fits(room, crate::strip::Cell(80))
  }

  fn state_of(pages: Vec<Page>) -> Result<State, Failure> {
    let Ok(rows) = first_rows(&pages);
    let booted = console_actor::boot::<crate::tasks::Tasks>(())?;

    Ok(State {
      rows,
      loading: false,
      model: super::CardModel {
        leaving: super::Leaving::No,
        pages,
        here: 0,
        at: None,
        opened: Opened::No,
        from_tab: 0,
        scroll: 0,
        note: None,
        typed: TextField::default(),
        preedit: None,
        prompt_identity: 0,
        beside: Beside::No,
        asking: None,
        sure: None,
        pressing: None,
        zoom: None,
        shown: None,
        selected: Vec::new(),
        framing: super::Framing::Card,
        tasks: booted.actor,
        live: None,
      },
    })
  }

  fn page(title: &str, says: &[&str]) -> Result<Page, Never> {
    let rows: Vec<Row> = says
      .iter()
      .map(|text| {
        let Ok(row) = Row::text(text, Aside(""));

        row
      })
      .collect();

    Page::new(title, Rows::Fixed(rows))
  }

  fn card() -> Result<Card, Never> {
    super::card_on(&SCREEN, Opened::No, super::Framing::Card)
  }

  fn first_rows(pages: &[Page]) -> Result<Vec<Row>, Never> {
    match pages.first() {
      Some(page) => page.rows.read(),
      None => Ok(Vec::new()),
    }
  }

  fn drawn(state: &State, card: &Card, wearing: &Wearing) -> Result<Panelled, Never> {
    let Ok(wide) = wide_of(card);
    let Ok(tabs) = measure_tabs(&state.pages, wide);
    let Ok(marks) = super::measure_marks();
    let rows = super::RefCell::new(super::BTreeMap::new());
    let Ok(measured) = super::measured_now(state, &tabs, &rows, marks);
    let Ok(fits) = fits_for(card);

    shapes(state, card, fits, &measured, wearing)
  }

  fn painted(state: &State, into: &std::path::Path) -> Result<(), Failure> {
    let Ok(spent) = palette::read(include_str!(
      "../../../files/usr/local/lib/console/palette.sh"
    ));
    let wearing = Wearing::out_of(&spent)?;
    let Ok(card) = card();
    let Ok(wide) = wide_of(&card);
    let Ok(measured_tabs) = measure_tabs(&state.pages, wide);
    let measured_rows = super::RefCell::new(super::BTreeMap::new());
    let Ok(marks) = super::measure_marks();
    let Ok(measured) = super::measured_now(state, &measured_tabs, &measured_rows, marks);
    let Ok(fits) = fits_for(&card);
    let Ok(panelled) = shapes(state, &card, fits, &measured, &wearing);
    let device = console_core_geometry::Size {
      width: 2560_u32,
      height: 1600_u32,
    };
    let points = console_core_geometry::Size {
      width: 1024_u32,
      height: 640_u32,
    };
    let mut pixels = vec![0; 16_384_000];

    console_draw_painting::onto(
      &mut pixels,
      console_draw_painting::Frame { device, points },
      &panelled.shapes,
    )?;
    console_core_atomic_writes::whole(into, &pixels)?;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
      explicit026_env_read_once,
      reason = "the picture is asked for by whoever runs this by hand, and nothing else reads the name"
    )
  )]
  #[test]
  fn a_picture_of_a_panel_is_written_when_someone_asks_for_one() -> Result<(), Failure> {
    let into = match std::env::var("CONSOLE_PANEL_PICTURE") {
      Ok(into) => PathBuf::from(into),
      Err(_no_one_wants_to_look_at_one) => return Ok(()),
    };
    let Ok(output) = Row::naming("Output", Aside(""));
    let Ok(volume) = Row::text("Volume", Aside("40%"));
    let Ok(volume_2) = volume.adjusting(|_step| {});
    let Ok(speakers) = Row::text("Speakers", Aside("In use"));
    let Ok(bluetooth) = Row::text("Bluetooth", Aside(""));
    let Ok(bluetooth_2) = bluetooth.opening();
    let Ok(ferry) = Row::text("Ferry.jpg", Aside("2.1 MB"));
    let Ok(ferry_2) = ferry.offering(|_showing| ActionOutcome::Stay);
    let rows = vec![output, volume_2, speakers, bluetooth_2, ferry_2];
    let Ok(sound) = Page::new("Sound", Rows::Fixed(rows.clone()));
    let Ok(network) = Page::new("Network", Rows::Fixed(Vec::new()));
    let Ok(display) = Page::new("Display", Rows::Fixed(Vec::new()));
    let mut state = state_of(vec![sound, network, display])?;

    state.at = Some(1);
    painted(&state, &into.join("tabs.bgra"))?;

    let Ok(single) = Page::new("Downloads", Rows::Fixed(rows.clone()));
    let mut state_2 = state_of(vec![single])?;

    state_2.at = Some(4);
    state_2.beside = Beside::Yes;
    painted(&state_2, &into.join("single.bgra"))?;

    let Ok(sought) = Page::new("Menu", Rows::Fixed(rows));
    let Ok(sought_2) = sought.searching("Type to narrow the list", |_showing, _word| {});
    let mut state_3 = state_of(vec![sought_2])?;

    let Ok(typed) = TextField::holding("fir");

    state_3.typed = typed;
    state_3.at = Some(2);
    painted(&state_3, &into.join("search.bgra"))?;

    let (turns, _turned) = mpsc::sync_channel(8);
    let (steps, _heard) = mpsc::sync_channel(8);
    let Ok(wifi) = switched("Wi-Fi", console_core_controls::Switch::On, &turns);
    let Ok(bluetooth_3) = switched("Bluetooth", console_core_controls::Switch::Off, &turns);
    let Ok(speakers_2) = slid(6, &steps);
    let Ok(night) = switched("Night Shift", console_core_controls::Switch::On, &turns);
    let Ok(settings) = Page::new(
      "Settings",
      Rows::Fixed(vec![wifi, bluetooth_3, speakers_2, night]),
    );
    let mut state_4 = state_of(vec![settings])?;

    state_4.at = Some(0);
    painted(&state_4, &into.join("controls.bgra"))?;

    state_4.at = Some(2);
    painted(&state_4, &into.join("controls-slider.bgra"))?;

    paint_choice_example(&into)
  }

  fn paint_choice_example(into: &std::path::Path) -> Result<(), Failure> {
    let (choices, _chosen) = mpsc::sync_channel(8);
    let Ok(nothing) = Handler::and_stay(|_| {});
    let Ok(scrub) = Row::new("0:25", Aside("1:40"), nothing);
    let Ok(scrub_2) = scrub.picturing(Picture::Bar(crate::page::Bar { at: 25, of: 100 }));
    let Ok(scrub_3) = scrub_2.seeking(|_, _fraction| {});
    let Ok(power) = picking(1, &choices);
    let Ok(choose_zoom) = ignored_choice();
    let zoom = crate::page::Choices {
      said: ["Smallest", "Smaller", "Default", "Larger", "Largest"]
        .iter()
        .map(|said| (*said).to_string())
        .collect(),
      chosen: 2,
      choose: choose_zoom,
    };
    let Ok(zoom_2) = Row::picking("Display Zoom", zoom);
    let Ok(choose_rotation) = ignored_choice();
    let ways = crate::page::Choices {
      said: ["Rotated Left", "Standard", "Rotated Right", "Upside Down"]
        .iter()
        .map(|said| (*said).to_string())
        .collect(),
      chosen: 1,
      choose: choose_rotation,
    };
    let Ok(ways_2) = Row::picking("Rotation", ways);
    let Ok(chosen_page) = Page::new(
      "Settings",
      Rows::Fixed(vec![scrub_3, power, zoom_2, ways_2]),
    );
    let mut state_5 = state_of(vec![chosen_page])?;

    state_5.at = Some(1);
    painted(&state_5, &into.join("choices.bgra"))?;

    Ok(())
  }

  #[test]
  fn every_visible_row_gets_a_touching_entry() -> Result<(), Failure> {
    let Ok(test) = page("test", &["alpha", "bravo", "charlie"]);
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![test])?;
    let Ok(panelled) = drawn(&state, &card, &wearing);
    let Ok(rows) = landing_on_rows(&panelled);

    assert_eq!(rows.len(), 3, "one touching per row");

    Ok(())
  }

  #[test]
  fn a_row_scrolled_half_out_of_view_is_drawn_and_cut_at_the_edge() -> Result<(), Failure> {
    let says: Vec<String> = (0..30).map(|n| format!("row {n}")).collect();
    let said: Vec<&str> = says.iter().map(String::as_str).collect();
    let Ok(test) = page("test", &said);
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let mut state = state_of(vec![test])?;

    state.scroll = crate::fitting::ROW.saturating_div(2);

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let drawn: Vec<&str> = panelled
      .shapes
      .iter()
      .filter_map(|shape| match shape {
        Shape::Text(words) => Some(words.said.as_str()),
        Shape::Panel(_)
        | Shape::Picture(_)
        | Shape::Cropped(_)
        | Shape::Line(_)
        | Shape::Clip(_) => None,
      })
      .collect();

    assert!(
      drawn.contains(&"row 0"),
      "the first row is half in view and was not drawn: {drawn:?}"
    );

    let clips: Vec<&Clip> = panelled
      .shapes
      .iter()
      .filter_map(|shape| match shape {
        Shape::Clip(clip) => Some(clip),
        Shape::Panel(_)
        | Shape::Text(_)
        | Shape::Picture(_)
        | Shape::Cropped(_)
        | Shape::Line(_) => None,
      })
      .collect();

    let first_top = clips.first().and_then(|clip| match clip {
      Clip::To { at, .. } => Some(at.y),
      Clip::Lifted => None,
    });
    let Ok(start) = rows_start(&card);

    assert_eq!(
      first_top,
      Some(start),
      "the rows are cut to the list: {clips:?}"
    );
    assert_eq!(
      clips.last(),
      Some(&&Clip::Lifted),
      "the clip is lifted before whatever stands over the rows"
    );

    let Ok(rows) = landing_on_rows(&panelled);
    let card_bottom = card.y.saturating_add(card.height);

    for row in &rows {
      let top = row.panel.at.y;
      let Ok(tall) = fitted::<u32, i32>(row.panel.size.height);
      let bottom = top.saturating_add(tall);

      assert!(
        top >= start && bottom <= card_bottom,
        "a row cut by the edge answers a finger: {top}..{bottom}"
      );
    }

    let Ok(rows_drawn) = fitted::<_, u32>(
      drawn
        .iter()
        .filter(|said_2| said_2.starts_with("row "))
        .count(),
    );
    let Ok(rows_touched) = fitted::<_, u32>(rows.len());

    assert_eq!(
      rows_drawn,
      rows_touched.saturating_add(2),
      "the rows cut at the top and at the bottom are both drawn: {drawn:?}"
    );

    Ok(())
  }

  #[test]
  fn rows_are_placed_at_expected_positions() -> Result<(), Failure> {
    let Ok(test) = page("test", &["one", "two", "three"]);
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let rows_start = card
      .y
      .saturating_add(crate::fitting::STRIP)
      .saturating_add(10);
    let row_tall = crate::fitting::ROW;
    let state = state_of(vec![test])?;
    let Ok(panelled) = drawn(&state, &card, &wearing);
    let Ok(rows) = landing_on_rows(&panelled);

    for (index, touching) in rows.iter().enumerate() {
      let Ok(step) = console_core_number_conversion::fitted::<_, i32>(index);
      let expected_y = rows_start.saturating_add(step.saturating_mul(row_tall));

      assert_eq!(
        touching.panel.at.y, expected_y,
        "row {index} should be at y={expected_y}"
      );
    }

    Ok(())
  }

  #[test]
  fn highlight_selects_the_right_row() -> Result<(), Failure> {
    let Ok(test) = page("test", &["a", "b", "c"]);
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let mut state = state_of(vec![test])?;

    state.at = Some(1);

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let Ok(highlights) = highlighted_rows(&panelled, &card, &wearing);

    assert_eq!(
      highlights.len(),
      1,
      "exactly one row is standing under the highlight"
    );

    let under_the_second_row = highlights
      .first()
      .ok_or("the highlight was counted and then could not be found")?;
    let Ok(start) = rows_start(&card);

    assert_eq!(
      *under_the_second_row,
      start
        .saturating_add(crate::fitting::ROW)
        .saturating_add(super::BETWEEN_ROWS.saturating_div(2)),
      "the highlight is behind the row the pad is standing on"
    );

    Ok(())
  }

  #[test]
  fn no_highlight_when_at_is_none() -> Result<(), Failure> {
    let Ok(test) = page("test", &["x", "y"]);
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![test])?;
    let Ok(panelled) = drawn(&state, &card, &wearing);
    let Ok(highlights) = highlighted_rows(&panelled, &card, &wearing);

    assert_eq!(
      highlights.len(),
      0,
      "nothing is standing under a highlight until something is standing on a row"
    );

    Ok(())
  }

  #[test]
  fn the_dpad_lands_on_a_row_that_does_something_and_steps_over_the_heading_over_it()
  -> Result<(), Failure> {
    let Ok(heading) = Row::placeholder("Sound");
    let Ok(volume) = Handler::run(&["console-volume"]);
    let Ok(balance) = Handler::run(&["console-balance"]);
    let Ok(first) = Row::new("Volume", Aside(""), volume);
    let Ok(second) = Row::new("Balance", Aside(""), balance);
    let rows = vec![heading, first, second];
    let Ok(page) = Page::new("Settings", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let Ok(down) = apply_meaning(&mut state, Meaning::Step(1), &rows);

    assert_eq!(down, Outcome::Redrawn);
    assert_eq!(state.at, Some(1), "the first press skips the heading");

    let Ok(_again) = apply_meaning(&mut state, Meaning::Step(1), &rows);

    assert_eq!(state.at, Some(2));

    let Ok(_at_the_end) = apply_meaning(&mut state, Meaning::Step(1), &rows);

    assert_eq!(
      state.at,
      Some(2),
      "the list does not wrap past its last row"
    );

    Ok(())
  }

  #[test]
  fn a_step_is_heard_only_when_it_lands_somewhere_else() -> Result<(), Failure> {
    let Ok(volume) = Handler::run(&["console-volume"]);
    let Ok(balance) = Handler::run(&["console-balance"]);
    let Ok(first) = Row::new("Volume", Aside(""), volume);
    let Ok(second) = Row::new("Balance", Aside(""), balance);
    let rows = vec![first, second];
    let Ok(page) = Page::new("Settings", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let before = state.at;
    let Ok(_down) = apply_meaning(&mut state, Meaning::Step(1), &rows);

    assert_eq!(stepped_onto(Meaning::Step(1), before, &state), Ok(Some(0)));

    let before_2 = state.at;
    let Ok(_down) = apply_meaning(&mut state, Meaning::Step(1), &rows);

    assert_eq!(
      stepped_onto(Meaning::Step(1), before_2, &state),
      Ok(Some(1))
    );

    let before_3 = state.at;
    let Ok(_at_the_end) = apply_meaning(&mut state, Meaning::Step(1), &rows);

    assert_eq!(
      stepped_onto(Meaning::Step(1), before_3, &state),
      Ok(None),
      "the end of the list is silent"
    );
    assert_eq!(
      stepped_onto(Meaning::Tab(1), Some(0), &state),
      Ok(None),
      "a tab is not a step"
    );

    Ok(())
  }

  #[test]
  fn a_is_the_row_doing_what_it_says_it_does() -> Result<(), Failure> {
    let (report, pressed) = mpsc::sync_channel(1);
    let Ok(does) = Handler::and_stay(move |_showing| {
      let _test_collects_the_press = report.try_send(());
    });
    let Ok(row) = Row::new("Rename", Aside(""), does);
    let rows = vec![row];
    let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let Ok(_down) = apply_meaning(&mut state, Meaning::Step(1), &rows);
    let Ok(chose) = apply_meaning(&mut state, Meaning::Choose, &rows);

    assert_eq!(chose, Outcome::Chose(0));

    let Ok(gone) = carried_out(&mut state, &rows, chose);

    assert_eq!(
      gone,
      ActionOutcome::Stay,
      "a row that stays leaves the panel up"
    );
    assert_eq!(
      pressed.try_recv(),
      Ok(()),
      "the row was chosen and nothing ran"
    );

    Ok(())
  }

  #[test]
  fn primary_secondary_and_button_actions_can_close_the_panel() -> Result<(), Failure> {
    let Ok(closes) = Handler::call(|_| ActionOutcome::Close);
    let Ok(row) = Row::new("Close", Aside(""), closes);
    let Ok(offering) = row.offering(|_| ActionOutcome::Close);
    let Ok(press) = crate::page::ButtonPress::labelled("Close", crate::page::Active::No, |_| {});
    let Ok(close_button) =
      crate::page::retained(|_: &dyn crate::page::Showing| ActionOutcome::Close);
    let Ok(button) = Row::pressing(
      vec![crate::page::ButtonPress {
        does: close_button,
        ..press
      }],
      0,
    );
    let rows = vec![offering, button];
    let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    for picked in [Outcome::Chose(0), Outcome::Else(0), Outcome::Chose(1)] {
      let Ok(outcome) = carried_out(&mut state, &rows, picked);

      assert_eq!(outcome, ActionOutcome::Close);
    }

    Ok(())
  }

  struct Door(PathBuf);

  impl Drop for Door {
    #[cfg_attr(
      dylint_lib = "explicit040_no_torn_write",
      allow(
        explicit040_no_torn_write,
        reason = "a fifo is opened for writing so the reader held at it is let past, and nothing is written into it"
      )
    )]
    fn drop(&mut self) {
      let at = self.0.clone();
      let opening = std::thread::spawn(move || {
        let opened = std::fs::OpenOptions::new().write(true).open(&at);

        drop(opened);
      });
      let Ok(()) = console_program_lifetime::threads::let_go(opening);
    }
  }

  fn ended_quietly(
    door: Door,
    tasks: &console_actor::Actor<crate::tasks::Tasks>,
  ) -> Result<(), Never> {
    let Ok(_started) = crate::frames::wake_state();

    drop(door);

    let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(10));
    let Ok(_ended) = console_waiting::until(patience, || {
      let Ok(running) = crate::tasks::running_now(tasks);

      Ok(
        match ::core::primitive::bool::then_some(running.started.is_empty(), ()) {
          ::core::option::Option::Some(()) => console_waiting::Ready::Yes,
          ::core::option::Option::None => console_waiting::Ready::NotYet,
        },
      )
    });
    let Ok(patience_2) = console_waiting::Schedule::of(Duration::from_secs(10));
    let Ok(_told) = console_waiting::until(patience_2, || {
      let Ok(woken) = crate::frames::wake_state();

      Ok(match woken.rows {
        crate::frames::FrameReceived::Yes => console_waiting::Ready::Yes,
        crate::frames::FrameReceived::No => console_waiting::Ready::NotYet,
      })
    });

    Ok(())
  }

  fn a_door_held_shut(named: &str) -> Result<Door, Box<dyn std::error::Error>> {
    let room = console_core_temporary_directories::fresh(named)?;
    let at = room.join("held");
    let Ok(mut making) = console_core_external_programs::Program::Sh.command();
    let made = making
      .args(["-c", "mkfifo \"$1\"", "sh"])
      .arg(&at)
      .status()?;

    match ::core::primitive::bool::then_some(made.success(), ()) {
      ::core::option::Option::Some(()) => Ok(Door(at)),
      ::core::option::Option::None => Err(Failure::from(format!("no fifo at {}", at.display()))),
    }
  }

  fn waiting_at(
    door: &std::path::Path,
    word: &str,
  ) -> Result<console_program_contract::Command, Failure> {
    let at = door.to_str().ok_or("the fixture door path must be UTF-8")?;

    let Ok(command) = console_program_contract::Command::external(
      console_core_external_programs::Program::Sh,
      &["-c", "cat \"$1\" >/dev/null", "sh", at, word],
    );

    Ok(command)
  }

  fn pairing_row(
    names: &console_program_contract::Command,
    runs: &console_program_contract::Command,
  ) -> Result<Row, Never> {
    let running = runs.clone();
    let Ok(does) = Handler::and_stay(move |showing| {
      let Ok(task) = crate::tasks::Task::of(running.clone());

      showing.run(task);
    });
    let Ok(row) = Row::new("Pair", Aside(""), does);

    row.while_running(names, "Pairing")
  }

  #[test]
  fn a_row_pressed_again_while_what_it_started_runs_starts_nothing()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let Ok(_before) = crate::frames::wake_state();
    let door = a_door_held_shut("pressed-twice")?;
    let pairing = waiting_at(&door.0, "")?;
    let Ok(row) = pairing_row(&pairing, &pairing);
    let rows = vec![row];
    let Ok(page) = Page::new("Bluetooth", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;
    let Ok(_gone) = carried_out(&mut state, &rows, Outcome::Chose(0));
    let Ok(_again) = carried_out(&mut state, &rows, Outcome::Chose(0));
    let Ok(running) = crate::tasks::running_now(&state.tasks);
    let started: Vec<_> = running
      .started
      .iter()
      .map(|started| (&started.command, &started.row))
      .collect();

    assert_eq!(
      started,
      vec![(&pairing, &pairing)],
      "a second press started a second pairing"
    );

    let Ok(shown) = crate::tasks::shown(rows, &running);
    let lit = shown.first().ok_or("no row")?;

    assert_eq!(
      (lit.says.as_str(), lit.aside.as_str(), lit.does.is_none()),
      ("Pairing", crate::page::YET, true)
    );

    let Ok(()) = ended_quietly(door, &state.tasks);

    Ok(())
  }

  #[test]
  fn a_row_in_progress_asks_whether_to_stop_and_stopping_ends_what_it_started()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let Ok(_before) = crate::frames::wake_state();
    let door = a_door_held_shut("stopped-from-its-row")?;
    let pairing = waiting_at(&door.0, "")?;
    let Ok(row) = pairing_row(&pairing, &pairing);
    let rows = vec![row];
    let Ok(page) = Page::new("Bluetooth", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let Ok(_gone) = carried_out(&mut state, &rows, Outcome::Chose(0));
    let Ok(running) = crate::tasks::running_now(&state.tasks);
    let Ok(lit) = crate::tasks::shown(rows, &running);
    let Ok(_asked) = carried_out(&mut state, &lit, Outcome::Else(0));
    let asked = state
      .sure
      .as_ref()
      .map(|sure| (sure.question.clone(), sure.answers.clone()));

    assert_eq!(
      asked,
      Some((
        "Stop Pairing?".to_string(),
        vec![
          crate::marks::CANCEL.to_string(),
          crate::tasks::STOP.to_string()
        ]
      )),
      "the row in progress did not ask whether to stop"
    );

    let Ok(()) = super::took(&mut state.model, &state.rows, 1);
    let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(10));
    let Ok(stopped) = console_waiting::until(patience, || {
      let Ok(running_2) = crate::tasks::running_now(&state.tasks);

      Ok(
        match ::core::primitive::bool::then_some(running_2.started.is_empty(), ()) {
          ::core::option::Option::Some(()) => console_waiting::Ready::Yes,
          ::core::option::Option::None => console_waiting::Ready::NotYet,
        },
      )
    });

    assert_eq!(
      stopped,
      console_waiting::Outcome::Happened,
      "the pairing ran on behind a door nobody opened"
    );

    let Ok(()) = ended_quietly(door, &state.tasks);

    Ok(())
  }

  #[test]
  fn what_a_question_from_a_row_starts_is_that_row_in_progress()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let Ok(_before) = crate::frames::wake_state();
    let door = a_door_held_shut("asked-from-a-row")?;
    let plain = waiting_at(&door.0, "")?;
    let typed = waiting_at(&door.0, "1234")?;
    let answering = typed.clone();
    let Ok(asks) = Handler::and_stay(move |showing| {
      let answering_2 = answering.clone();

      let Ok(answer) =
        crate::page::retained(move |confirmed: &dyn crate::page::Showing, _: &str| {
          let Ok(task) = crate::tasks::Task::of(answering_2.clone());
          confirmed.run(task);
        });

      showing.ask("PIN", answer);
    });
    let Ok(question) = Row::new("Pair", Aside(""), asks);
    let Ok(row) = question.while_running(&plain, "Pairing");
    let rows = vec![row];
    let Ok(page) = Page::new("Bluetooth", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let Ok(_gone) = carried_out(&mut state, &rows, Outcome::Chose(0));

    state.at = Some(0);

    let Ok(()) = super::submit_answer(&mut state.model, &state.rows);
    let Ok(running) = crate::tasks::running_now(&state.tasks);
    let Ok(shown) = crate::tasks::shown(rows, &running);
    let lit = shown.first().ok_or("no row")?;

    let started: Vec<_> = running
      .started
      .iter()
      .map(|started| (&started.command, &started.row))
      .collect();

    assert_eq!(started, vec![(&typed, &plain)]);
    assert_eq!(
      lit.progress(),
      Ok(crate::page::Progress::Running),
      "the row the PIN was typed for is not in progress"
    );

    let Ok(()) = ended_quietly(door, &state.tasks);

    Ok(())
  }

  #[test]
  fn what_a_row_asks_for_while_it_runs_is_what_the_panel_does_next() -> Result<(), Failure> {
    let Ok(does) = Handler::and_stay(|showing| {
      showing.note("Copied");
      showing.turn_to(1);
    });
    let Ok(row) = Row::new("Copy", Aside(""), does);
    let rows = vec![row];
    let Ok(here) = Page::new("Files", Rows::Fixed(rows.clone()));
    let Ok(there) = Page::new("Places", Rows::Fixed(Vec::new()));
    let mut state = state_of(vec![here, there])?;

    let Ok(_gone) = carried_out(&mut state, &rows, Outcome::Chose(0));

    assert_eq!(state.note.as_deref(), Some("Copied"));
    assert_eq!(state.here, 1, "the row asked for the tab beside it");

    Ok(())
  }

  #[test]
  fn entering_a_page_and_backing_out_of_it_apply_their_callbacks() -> Result<(), Failure> {
    let Ok(page) = page("Settings", &["First", "Second"]);
    let Ok(page) = page.on_arriving(|showing| showing.note("Arrived"));
    let Ok(page) = page.on_back(|showing| {
      showing.note("Back");
      showing.replace(1);
      ActionOutcome::Stay
    });
    let mut state = state_of(vec![page])?;
    let Ok(outcome) = super::entered(&mut state.model);
    assert_eq!(outcome, ActionOutcome::Stay);
    assert_eq!(state.note.as_deref(), Some("Arrived"));

    let rows = state.rows.clone();
    let Ok(outcome) = pressed(&mut state, &rows, Keysym::Escape);
    assert_eq!(outcome, ActionOutcome::Stay);
    assert_eq!(state.note.as_deref(), Some("Back"));
    assert_eq!(state.at, Some(1));

    state.note = None;
    let Ok(card) = card();
    let back = landing(&state, &card, Lands::Back)?;
    let Ok(outcome) = super::tapped(&mut state, Some(back.lands), back.panel.at, &card);
    assert_eq!(outcome, ActionOutcome::Stay);
    assert_eq!(state.note.as_deref(), Some("Back"));
    Ok(())
  }

  #[test]
  fn tapping_back_dismisses_the_prompt_without_running_the_pages_back_action()
  -> Result<(), Failure> {
    let Ok(page) = page("Settings", &["First"]);
    let Ok(page) = page.on_back(|_| ActionOutcome::Close);
    let mut state = state_of(vec![page])?;
    let Ok(answer) = empty_answer();
    let Ok(()) = super::effects::dispatch(
      &mut state.model,
      &[],
      super::effects::Source::Page,
      |showing, _| showing.ask("Password", answer),
    );
    let Ok(card) = card();
    let back = landing(&state, &card, Lands::Back)?;

    let Ok(outcome) = super::tapped(&mut state, Some(back.lands), back.panel.at, &card);
    assert_eq!(outcome, ActionOutcome::Stay);
    assert!(state.asking.is_none());
    Ok(())
  }

  #[test]
  fn b_over_a_picture_puts_the_picture_away_and_not_the_panel() -> Result<(), Failure> {
    let Ok(one) = page("One", &["a"]);
    let mut state = state_of(vec![one])?;

    state.opened = Opened::Expanded;

    let Ok(back) = apply_meaning(&mut state, Meaning::Close, &[]);

    assert_eq!(back, Outcome::Redrawn);
    assert_eq!(state.opened, Opened::No);

    let Ok(away) = apply_meaning(&mut state, Meaning::Close, &[]);

    assert_eq!(away, Outcome::Closing, "the second press is the panel's");

    Ok(())
  }

  #[test]
  fn b_puts_the_panel_away_and_the_shoulders_turn_the_page() -> Result<(), Failure> {
    let Ok(one) = page("One", &["a"]);
    let Ok(two) = page("Two", &["b"]);
    let mut state = state_of(vec![one, two])?;

    let Ok(along) = apply_meaning(&mut state, Meaning::Tab(1), &[]);

    assert_eq!(along, Outcome::Redrawn);
    assert_eq!(state.here, 1);

    let Ok(away) = apply_meaning(&mut state, Meaning::Close, &[]);

    assert_eq!(away, Outcome::Closing);

    Ok(())
  }

  #[test]
  fn a_highlight_under_the_card_brings_the_list_up_to_it() {
    let room = crate::fitting::ROW.saturating_mul(4);

    let top = Viewport { room, y: 0 };

    assert_eq!(scrolled(None, top), Ok(0));
    assert_eq!(
      scrolled(Some(2), top),
      Ok(0),
      "a row already in the room does not move it"
    );
    assert_eq!(
      scrolled(Some(5), top),
      Ok(crate::fitting::ROW.saturating_mul(2)),
      "the sixth row of four is two rows down"
    );
    assert_eq!(
      scrolled(
        Some(1),
        Viewport {
          room,
          y: crate::fitting::ROW.saturating_mul(3)
        }
      ),
      Ok(crate::fitting::ROW),
      "stepping back up brings the row to the top"
    );
  }

  #[test]
  fn a_page_that_can_be_searched_is_driven_as_a_search_line() -> Result<(), Failure> {
    let Ok(plain) = Page::new("Settings", Rows::Fixed(Vec::new()));
    let Ok(sought) = Page::new("Menu", Rows::Fixed(Vec::new()));
    let Ok(sought_2) = sought.searching("Type to narrow the list", |_showing, _word| {});
    let plain_2 = state_of(vec![plain])?;
    let sought_3 = state_of(vec![sought_2])?;

    assert_eq!(driving(&plain_2), Ok(Driving::Panel));
    assert_eq!(driving(&sought_3), Ok(Driving::Search));

    Ok(())
  }

  #[test]
  fn what_a_film_says_is_drawn_over_the_foot_of_its_picture_and_goes_when_it_stops()
  -> Result<(), Failure> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let film = PathBuf::from("/nowhere/said-over.mkv");
    let Ok(playing) = Row::picture(Picture::Playing(Some(film.clone())));
    let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![playing]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![page])?;
    let frame = super::Pixels {
      width: 4,
      height: 3,
      stride: 16,
      bytes: vec![9; 48],
    };
    let Ok(_first_look) = drawn(&state, &card, &wearing);
    let Ok(()) = crate::frames::insert(&film, frame.clone(), vec![String::from("Where are we?")]);
    let Ok(panelled) = drawn(&state, &card, &wearing);

    let picture = panelled.shapes.iter().find_map(|shape| match shape {
      Shape::Picture(picture) => Some(picture.clone()),
      Shape::Panel(_) | Shape::Text(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => {
        None
      }
    });
    let words = panelled.shapes.iter().find_map(|shape| match shape {
      Shape::Text(text) => {
        match ::core::primitive::bool::then_some(text.said == "Where are we?", ()) {
          ::core::option::Option::Some(()) => Some(text.clone()),
          ::core::option::Option::None => None,
        }
      }
      Shape::Panel(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    });
    let picture_drawn = picture.as_ref().ok_or("the frame was never drawn")?;
    let caption_drawn = words.as_ref().ok_or("what the film says was never drawn")?;
    let Ok(picture_tall) = fitted::<u32, i32>(picture_drawn.size.height);
    let middle = picture_drawn
      .at
      .y
      .saturating_add(picture_tall.saturating_div(2));

    assert!(
      caption_drawn.at.y > middle
        && caption_drawn.at.y < picture_drawn.at.y.saturating_add(picture_tall),
      "the words are not over the foot of the picture: {words:?} on {picture:?}"
    );

    let Ok(()) = crate::frames::insert(&film, frame, Vec::new());
    let Ok(panelled_2) = drawn(&state, &card, &wearing);
    let still_said = panelled_2.shapes.iter().any(|shape| match shape {
      Shape::Text(text) => text.said == "Where are we?",
      Shape::Panel(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => false,
    });

    assert!(
      !still_said,
      "the words stayed after the film stopped saying them"
    );

    Ok(())
  }

  #[test]
  fn a_picture_being_looked_at_is_drawn_across_the_card_with_its_rows_still_under_it()
  -> Result<(), Failure> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let folder = console_core_temporary_directories::fresh("panel-looked-at")?;
    let at = folder.join("panel-looked-at.png");
    crate::frames::a_red_picture(
      &at,
      console_core_geometry::Size {
        width: 400,
        height: 300,
      },
    )?;

    let Ok(looked) = Row::picture(Picture::Showing(Some(at.clone())));
    let Ok(name) = said_row("red.png");
    let Ok(place) = said_row("1 of 3");
    let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![looked, name, place]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![page])?;
    let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(20));

    let Ok(looked_at) = console_waiting::until_some(patience, || {
      let Ok(panelled) = drawn(&state, &card, &wearing);

      Ok(panelled.shapes.iter().find_map(|shape| match shape {
        Shape::Picture(picture) => Some(picture.size),
        Shape::Panel(_)
        | Shape::Text(_)
        | Shape::Cropped(_)
        | Shape::Line(_)
        | Shape::Clip(_) => None,
      }))
    });
    std::fs::remove_file(&at)?;

    let drawn_picture = looked_at.ok_or("the picture being looked at was never drawn")?;
    let Ok(row_tall) = fitted::<i32, u32>(crate::fitting::ROW);

    assert!(
      drawn_picture.height > row_tall.saturating_mul(3),
      "a picture looked at is drawn the size of a row: {looked_at:?}"
    );
    assert!(
      drawn_picture
        .width
        .saturating_mul(3)
        .abs_diff(drawn_picture.height.saturating_mul(4))
        <= 4,
      "the picture is stretched: {looked_at:?}"
    );

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let Ok(rows) = landing_on_rows(&panelled);
    let bottom = card.y.saturating_add(card.height);

    assert_eq!(
      rows.len(),
      3,
      "every row under the picture is still on the card"
    );

    for row in rows {
      let Ok(tall) = fitted::<u32, i32>(row.panel.size.height);

      assert!(
        row.panel.at.y.saturating_add(tall) <= bottom,
        "a row fell off the card: {row:?}"
      );
    }

    Ok(())
  }

  #[test]
  fn a_film_on_the_card_says_where_its_frames_go_and_a_frame_of_any_shape_fits_there()
  -> Result<(), Failure> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let Ok(playing) = Row::picture(Picture::Playing(Some(PathBuf::from("/nowhere/a-film.mkv"))));
    let Ok(name) = said_row("a-film.mkv");
    let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![playing, name]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![page])?;

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let moving = panelled
      .moving
      .ok_or("a film on the card says where its frames go")?;
    let Ok(row_tall) = fitted::<i32, u32>(crate::fitting::ROW);

    assert!(
      moving.room.height > row_tall.saturating_mul(3),
      "{moving:?}"
    );

    for (wide, tall) in [(1920, 1080), (1080, 1920), (16, 16)] {
      let pixels = console_core_shapes::Pixels {
        width: wide,
        height: tall,
        stride: 4,
        bytes: Vec::new(),
      };
      let Ok(picture) = super::place_picture(&moving, pixels);
      let Ok(room_right) = fitted::<u32, i32>(moving.room.width);
      let Ok(room_bottom) = fitted::<u32, i32>(moving.room.height);
      let Ok(right) = fitted::<u32, i32>(picture.size.width);
      let Ok(bottom) = fitted::<u32, i32>(picture.size.height);

      assert!(
        picture.at.x >= moving.at.x && picture.at.y >= moving.at.y,
        "{picture:?}"
      );
      assert!(
        picture.at.x.saturating_add(right) <= moving.at.x.saturating_add(room_right),
        "{picture:?} spills out of {moving:?}"
      );
      assert!(
        picture.at.y.saturating_add(bottom) <= moving.at.y.saturating_add(room_bottom),
        "{picture:?} spills out of {moving:?}"
      );
      assert!(
        u64::from(picture.size.width)
          .saturating_mul(u64::from(tall))
          .abs_diff(u64::from(picture.size.height).saturating_mul(u64::from(wide)))
          <= u64::from(wide.max(tall)),
        "{wide}x{tall} drawn {:?}",
        picture.size
      );
    }

    Ok(())
  }

  #[test]
  fn only_a_page_that_opens_on_a_picture_gives_its_first_row_the_room() {
    let Ok(looked) = Row::picture(Picture::Showing(None));
    let Ok(playing) = Row::picture(Picture::Playing(None));
    let Ok(a) = said_row("a");
    let Ok(b) = said_row("b");
    let room = crate::fitting::ROW.saturating_mul(10);

    assert_eq!(
      stage(&[looked, a.clone(), b.clone()], room),
      Ok(crate::fitting::ROW.saturating_mul(7))
    );
    assert_eq!(
      stage(&[playing], room),
      Ok(crate::fitting::ROW.saturating_mul(9))
    );
    assert_eq!(stage(&[a, b], room), Ok(0));
    assert_eq!(stage(&[], room), Ok(0));
  }

  #[test]
  fn the_line_to_type_in_stands_over_the_rows_and_pushes_them_down() -> Result<(), Failure> {
    let Ok(aether) = said_row("Aether");
    let Ok(page) = Page::new("Menu", Rows::Fixed(vec![aether]));
    let Ok(page_2) = page.searching("Type to narrow the list", |_showing, _word| {});
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![page_2])?;

    let Ok(panelled) = drawn(&state, &card, &wearing);

    let Ok(rows) = landing_on_rows(&panelled);
    let first = rows.first().ok_or("a page with one row drew no row")?;
    let Ok(start) = rows_start(&card);

    assert_eq!(
      first.panel.at.y,
      start.saturating_add(crate::fitting::ROW),
      "the rows begin a line below where they would without one to type in"
    );

    Ok(())
  }

  #[test]
  fn a_row_of_cells_draws_every_cell_and_lights_the_one_that_is_now() -> Result<(), Failure> {
    let Ok(before) = crate::page::Cell::new("8", crate::page::Active::No);
    let Ok(today) = crate::page::Cell::new("9", crate::page::Active::Yes);
    let Ok(row) = Row::celled(vec![before, today]);
    let Ok(page) = Page::new("Calendar", Rows::Fixed(vec![row]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![page])?;

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let said: Vec<&super::Text> = panelled
      .shapes
      .iter()
      .filter_map(|shape| match shape {
        super::Shape::Text(text) => Some(text),
        super::Shape::Panel(_)
        | super::Shape::Picture(_)
        | super::Shape::Cropped(_)
        | super::Shape::Line(_)
        | super::Shape::Clip(_) => None,
      })
      .collect();
    let eight = said
      .iter()
      .find(|text| text.said == "8")
      .ok_or("the eighth was not drawn")?;
    let nine = said
      .iter()
      .find(|text| text.said == "9")
      .ok_or("the ninth was not drawn")?;

    assert!(
      eight.at.x < nine.at.x,
      "the cells stand in the order they were given"
    );
    assert_eq!(nine.ink, wearing.night, "today is not lit");
    assert_eq!(eight.ink, wearing.text);

    Ok(())
  }

  fn transport(taken: &mpsc::SyncSender<u32>) -> Result<Row, Never> {
    let presses: Vec<crate::page::ButtonPress> = [
      crate::icons::Icon::Previous,
      crate::icons::Icon::Play,
      crate::icons::Icon::Next,
    ]
    .into_iter()
    .zip(0u32..)
    .map(|(icon, at)| {
      let report = taken.clone();
      let Ok(press) = crate::page::ButtonPress::new(icon, crate::page::Active::No, move |_| {
        let _test_reads_the_chosen_button = report.try_send(at);
      });

      press
    })
    .collect();

    Row::pressing(presses, 1)
  }

  #[test]
  fn a_strip_of_buttons_draws_each_one_where_a_finger_can_press_it() -> Result<(), Failure> {
    let (record, taken) = mpsc::sync_channel(8);
    let Ok(transport) = transport(&record);
    let rows = vec![transport];
    let Ok(page) = Page::new("Viewing", Rows::Fixed(rows.clone()));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let mut state = state_of(vec![page])?;

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let presses: Vec<&HitRegion> = panelled
      .touching
      .iter()
      .filter(|touching| matches!(touching.lands, Lands::ButtonPress { .. }))
      .collect();

    assert_eq!(
      presses.len(),
      3,
      "a transport with three buttons drew {} of them",
      presses.len()
    );
    assert!(
      presses
        .iter()
        .zip(presses.iter().skip(1))
        .all(|(one, next)| one.panel.at.x < next.panel.at.x),
      "the buttons stand in order"
    );

    let Ok(play) = crate::icons::Icon::Play.glyph();
    let drew_play = panelled.shapes.iter().any(|shape| match shape {
      super::Shape::Text(text) => text.said == play,
      super::Shape::Panel(_)
      | super::Shape::Picture(_)
      | super::Shape::Cropped(_)
      | super::Shape::Line(_)
      | super::Shape::Clip(_) => false,
    });

    assert!(drew_play, "nothing on the glass says where play is");

    let next = presses.get(2).ok_or("no third button")?;
    let Ok(half_width) = fitted::<u32, i32>(::core::primitive::u32::wrapping_div(
      next.panel.size.width,
      2,
    ));
    let Ok(half_height) = fitted::<u32, i32>(::core::primitive::u32::wrapping_div(
      next.panel.size.height,
      2,
    ));
    let at = super::Point {
      x: next.panel.at.x.saturating_add(half_width),
      y: next.panel.at.y.saturating_add(half_height),
    };
    let Ok(target) = super::hit_test(&panelled.touching, at);
    assert_eq!(target, Some(next.lands));
    let Ok(gone) = super::tapped(&mut state, target, at, &card);

    assert_eq!(gone, ActionOutcome::Stay);
    assert_eq!(
      taken.try_recv(),
      Ok(2),
      "a tap on next pressed something else"
    );
    let Ok(gone_2) = pressed(&mut state, &rows, Keysym::Return);
    assert_eq!(gone_2, ActionOutcome::Stay);
    assert_eq!(taken.try_recv(), Ok(2));

    Ok(())
  }

  #[test]
  fn a_strip_of_buttons_stands_on_the_card_with_no_row_behind_it() -> Result<(), Failure> {
    let (record, _taken) = mpsc::sync_channel(8);
    let Ok(transport) = transport(&record);
    let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![transport]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let mut state = state_of(vec![page])?;

    state.at = Some(0);

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let Ok(button_wide) = fitted::<i32, u32>(super::PRESS_WIDE);
    let behind: Vec<&super::ShapePanel> = panelled
      .shapes
      .iter()
      .filter_map(|shape| match shape {
        super::Shape::Panel(panel) => Some(panel),
        super::Shape::Text(_)
        | super::Shape::Picture(_)
        | super::Shape::Cropped(_)
        | super::Shape::Line(_)
        | super::Shape::Clip(_) => None,
      })
      .filter(|panel| panel.fill == wearing.ground || panel.fill == wearing.pink)
      .filter(|panel| panel.size.width > button_wide)
      .collect();

    assert!(
      behind.is_empty(),
      "the buttons sit on a row: {:?}",
      behind
        .iter()
        .map(|panel| panel.size.width)
        .collect::<Vec<_>>()
    );

    Ok(())
  }

  fn keypad_page() -> Result<Page, Never> {
    let Ok(number) = Row::headline(
      Picture::None,
      crate::page::Headline {
        title: "12 +".to_string(),
        big: "34".to_string(),
        alignment: crate::page::Alignment::Trailing,
        ..crate::page::Headline::default()
      },
    );
    let mut rows = vec![number];

    for _ in 0..5 {
      let presses: Vec<crate::page::ButtonPress> = ["7", "8", "9", "+"]
        .into_iter()
        .map(|says| {
          let Ok(press) =
            crate::page::ButtonPress::labelled(says, crate::page::Active::No, |_| ());

          press
        })
        .collect();
      let Ok(row) = Row::pressing(presses, 0);

      rows.push(row);
    }

    Page::new("Calculator", Rows::Fixed(rows))
  }

  #[test]
  fn a_row_of_written_keys_fills_the_card_from_edge_to_edge() -> Result<(), Failure> {
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let Ok(keypad) = keypad_page();
    let state = state_of(vec![keypad])?;
    let Ok(panelled) = drawn(&state, &card, &wearing);
    let keys: Vec<&super::ShapePanel> = panelled
      .touching
      .iter()
      .filter(|region| matches!(region.lands, super::Lands::ButtonPress { row: 1, which: _ }))
      .map(|region| &region.panel)
      .collect();
    let Ok(left) = super::inside_left(&card);
    let Ok(right) = super::inside_right(&card);
    let (first, last) = keys
      .first()
      .zip(keys.last())
      .ok_or("the keypad drew no keys")?;
    let Ok(last_wide) = fitted::<u32, i32>(last.size.width);
    let reaches = last.at.x.saturating_add(last_wide);

    assert_eq!(keys.len(), 4);
    assert!(
      first.at.x.saturating_sub(left).abs() <= 4,
      "the first key stands at {}, off the edge at {left}",
      first.at.x
    );
    assert!(
      right.saturating_sub(reaches).abs() <= 4,
      "the last key ends at {reaches}, short of the edge at {right}"
    );

    Ok(())
  }

  #[test]
  fn a_trailing_headline_draws_its_number_at_the_right_and_under_the_title_strip()
  -> Result<(), Failure> {
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let Ok(keypad) = keypad_page();
    let state = state_of(vec![keypad])?;
    let Ok(panelled) = drawn(&state, &card, &wearing);
    let number = panelled
      .shapes
      .iter()
      .find_map(|shape| match shape {
        super::Shape::Text(text) => {
          match ::core::primitive::bool::then_some(text.said == "34", ()) {
            ::core::option::Option::Some(()) => Some(text),
            ::core::option::Option::None => None,
          }
        }
        super::Shape::Panel(_)
        | super::Shape::Picture(_)
        | super::Shape::Cropped(_)
        | super::Shape::Line(_)
        | super::Shape::Clip(_) => None,
      })
      .ok_or("the number was not drawn")?;
    let key = panelled
      .touching
      .iter()
      .find(|region| matches!(region.lands, super::Lands::ButtonPress { row: 1, which: 0 }))
      .ok_or("the keypad drew no keys")?;
    let Ok(right) = super::inside_right(&card);
    let Ok(number_wide) = fitted::<u32, i32>(number.width);
    let big = number.font.height;
    let Ok(big_tall) = fitted::<u32, i32>(big);

    assert!(
      number.at.x > right.saturating_div(2),
      "the number stands at {} rather than at the right",
      number.at.x
    );
    assert!(
      number.at.x.saturating_add(number_wide) <= right.saturating_add(4),
      "the number runs past the edge"
    );
    assert!(
      number.at.y.saturating_add(big_tall) <= key.panel.at.y,
      "the number runs into the keys"
    );
    assert!(
      number.at.y >= card.y.saturating_add(super::strip::EDGE),
      "the number rises out of the card"
    );
    assert!(
      big > super::BIG,
      "the number is drawn at {big}, no larger than a headline with a title beside it"
    );

    Ok(())
  }

  #[test]
  fn left_and_right_walk_a_strip_of_buttons_and_a_press_takes_the_one_stood_on()
  -> Result<(), Failure> {
    let (record, taken) = mpsc::sync_channel(8);
    let Ok(transport) = transport(&record);
    let rows = vec![transport];
    let Ok(page) = Page::new("Viewing", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    state.at = Some(0);

    let Ok(_gone) = pressed(&mut state, &rows, Keysym::Return);

    assert_eq!(
      taken.try_recv(),
      Ok(1),
      "A before moving is the button the program stood on"
    );

    let Ok(_gone) = pressed(&mut state, &rows, Keysym::Left);
    let Ok(_gone) = pressed(&mut state, &rows, Keysym::Left);
    let Ok(_gone) = pressed(&mut state, &rows, Keysym::Return);

    assert_eq!(
      taken.try_recv(),
      Ok(0),
      "left twice from play is previous, and no further"
    );

    let Ok(_gone) = pressed(&mut state, &rows, Keysym::Right);
    let Ok(_gone) = pressed(&mut state, &rows, Keysym::Right);
    let Ok(_gone) = pressed(&mut state, &rows, Keysym::Return);

    assert_eq!(taken.try_recv(), Ok(2));

    Ok(())
  }

  #[test]
  fn a_finger_on_a_card_that_has_gone_quiet_wakes_it_rather_than_pressing_what_is_under_it()
  -> Result<(), Failure> {
    let Ok(row) = Row::text("a.png", Aside(""));
    let Ok(asleep) = Page::new("Viewing", Rows::Fixed(vec![row.clone()]))
      .and_then(|page| page.stirring(|| crate::page::WakeOutcome::Woke));
    let Ok(awake) = Page::new("Viewing", Rows::Fixed(vec![row]))
      .and_then(|page| page.stirring(|| crate::page::WakeOutcome::AlreadyAwake));
    let nothing = super::Panelled {
      shapes: Vec::new(),
      touching: Vec::new(),
      moving: None,
    };
    let asleep_2 = state_of(vec![asleep])?;
    let awake_state = state_of(vec![awake])?;

    let Ok(woke) = super::finger_down(&asleep_2, &nothing, (10.0, 10.0));
    let Ok(held) = super::finger_down(&awake_state, &nothing, (10.0, 10.0));

    assert!(
      matches!(woke, super::Down::Woke),
      "a tap on a quiet card pressed what was under it and the controls never came back"
    );
    assert!(
      matches!(held, super::Down::Touched(_)),
      "a tap on an awake card did not press"
    );

    Ok(())
  }

  #[test]
  fn a_press_put_in_the_corner_stands_at_the_far_end_and_the_rest_stay_in_the_middle()
  -> Result<(), Failure> {
    let Ok(a) =
      crate::page::ButtonPress::new(crate::icons::Icon::ZoomOut, crate::page::Active::No, |_| {});
    let Ok(b) =
      crate::page::ButtonPress::new(crate::icons::Icon::ZoomIn, crate::page::Active::No, |_| {});
    let Ok(whole) = crate::page::ButtonPress::new(
      crate::icons::Icon::FullScreen,
      crate::page::Active::No,
      |_| {},
    );
    let Ok(whole_2) = whole.cornered();
    let across = crate::page::Across {
      presses: vec![a, b, whole_2],
      at: 0,
    };
    let measured = super::MeasuredRow {
      says: super::NO_ROOM,
      aside: super::NO_ROOM,
      less: super::NO_ROOM,
      more: super::NO_ROOM,
      cells: Vec::new(),
      presses: Vec::new(),
    };
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
    let wearing = sample_palette()?;
    let Ok((_, touching)) = super::press_shapes(&across, &measured, strip, &wearing);
    let far = |which: u32| {
      touching
        .iter()
        .find(|region| region.lands == super::Lands::ButtonPress { row: 0, which })
        .map(|region| {
          let Ok(wide) = fitted::<u32, i32>(region.panel.size.width);

          region.panel.at.x.saturating_add(wide)
        })
    };

    assert_eq!(
      far(2),
      Some(1000),
      "the corner press is not at the end of the row"
    );
    assert!(
      far(1).is_some_and(|end| end < 600),
      "the presses in the middle made room for the one in the corner"
    );
    assert!(
      far(0).is_some_and(|end| end > 400),
      "the presses in the middle were pushed to the start"
    );

    Ok(())
  }

  #[test]
  fn an_app_is_the_whole_screen_and_a_panel_is_a_card_on_it() {
    let Ok(app) = super::card_on(&SCREEN, Opened::No, super::Framing::Window);
    let Ok(panel) = super::card_on(&SCREEN, Opened::No, super::Framing::Card);

    assert_eq!(
      app,
      Card {
        x: 0,
        y: 0,
        width: 1024,
        height: 640
      },
      "an app left a margin round itself, which is a panel"
    );
    assert!(
      panel.width < 1024 && panel.height < 640,
      "a panel took the whole screen, which is an app"
    );
  }

  #[test]
  fn opened_out_is_the_whole_screen_and_the_picture_runs_to_its_edges() -> Result<(), Failure> {
    let Ok(whole) = super::card_on(&SCREEN, Opened::Expanded, super::Framing::Card);
    let Ok(card) = super::card_on(&SCREEN, Opened::No, super::Framing::Card);

    assert_eq!(
      whole,
      Card {
        x: 0,
        y: 0,
        width: 1024,
        height: 640
      },
      "opened out left a margin round the card"
    );
    assert!(
      card.width < 1024 && card.height < 640,
      "a card that is not opened out is still a card"
    );

    let Ok(row) = Row::picture(Picture::Playing(Some(PathBuf::from("/nowhere/a-film.mkv"))));
    let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![row]));
    let wearing = sample_palette()?;
    let mut state = state_of(vec![page])?;

    state.opened = Opened::Expanded;
    state.at = Some(0);

    let Ok(panelled) = drawn(&state, &whole, &wearing);
    let moving = panelled.moving.ok_or("the film has nowhere to be drawn")?;

    assert_eq!(
      moving.at,
      super::Point { x: 0, y: 0 },
      "the picture does not start at the corner of the screen"
    );
    assert_eq!(
      moving.room.width, 1024,
      "the picture is not given the width of the screen"
    );

    let lit = panelled.shapes.iter().any(|shape| match shape {
      super::Shape::Panel(panel) => panel.fill == wearing.pink,
      super::Shape::Text(_)
      | super::Shape::Picture(_)
      | super::Shape::Cropped(_)
      | super::Shape::Line(_)
      | super::Shape::Clip(_) => false,
    });

    assert!(!lit, "something pink stands round a picture opened out");

    Ok(())
  }

  fn a_folder_that_selects(
    acted_on: mpsc::SyncSender<Vec<String>>,
    opened: mpsc::SyncSender<()>,
  ) -> Result<State, Failure> {
    let rows: Vec<Row> = ["beach.jpg", "dune.jpg", "sea.jpg"]
      .iter()
      .map(|name| {
        let count = opened.clone();
        let Ok(opens) = Handler::and_stay(move |_| {
          let _test_counts_successful_opens = count.try_send(());
        });
        let Ok(row) = Row::new(name, crate::page::Aside(""), opens);
        let Ok(row_2) = row.selectable(name);

        row_2
      })
      .collect();
    let Ok(page) = Page::new("Pictures", Rows::Fixed(rows));
    let Ok(page_2) = page.selecting("Delete", move |_, keys| {
      let _test_checks_the_selected_paths = acted_on.try_send(keys.to_vec());
    });

    state_of(vec![page_2])
  }

  #[test]
  fn once_something_is_selected_a_press_marks_rather_than_opens_and_y_acts_on_every_mark()
  -> Result<(), Failure> {
    let (selected, acted) = mpsc::sync_channel(4);
    let (opens, opened) = mpsc::sync_channel(4);
    let mut state = a_folder_that_selects(selected.clone(), opens)?;
    let rows = state.rows.clone();

    let Ok(_opens) = carried_out(&mut state, &rows, Outcome::Chose(0));

    assert_eq!(
      opened.try_recv(),
      Ok(()),
      "with nothing selected a press opens"
    );

    let Ok(()) = super::effects::dispatch(
      &mut state.model,
      &state.rows,
      super::effects::Source::Focused,
      |showing, _| showing.select(vec!["beach.jpg".to_string()]),
    );
    let Ok(_marks) = carried_out(&mut state, &rows, Outcome::Chose(2));
    let Ok(_marks) = carried_out(&mut state, &rows, Outcome::Chose(1));
    let Ok(_unmarks) = carried_out(&mut state, &rows, Outcome::Chose(1));

    assert_eq!(
      opened.try_recv(),
      Err(mpsc::TryRecvError::Empty),
      "a press while selecting opened the thing"
    );
    assert_eq!(
      state.selected,
      vec!["beach.jpg".to_string(), "sea.jpg".to_string()]
    );

    let Ok(_asks) = carried_out(&mut state, &rows, Outcome::Else(0));
    let Ok(()) = super::took(&mut state.model, &state.rows, 1);

    assert_eq!(
      acted.try_recv(),
      Ok(vec!["beach.jpg".to_string(), "sea.jpg".to_string()])
    );
    assert!(
      state.selected.is_empty(),
      "the selection outlived what was done with it"
    );

    Ok(())
  }

  fn landing(state: &State, card: &Card, lands: Lands) -> Result<HitRegion, Failure> {
    let wearing = sample_palette()?;
    let Ok(panelled) = drawn(state, card, &wearing);
    let region = panelled
      .touching
      .into_iter()
      .find(|region| region.lands == lands)
      .ok_or(format!("nothing a finger can reach lands on {lands:?}"))?;

    Ok(region)
  }

  fn marks_drawn(state: &State, card: &Card) -> Result<u32, Failure> {
    let wearing = sample_palette()?;
    let Ok(panelled) = drawn(state, card, &wearing);
    let Ok(side) = fitted::<i32, u32>(super::SELECTED_MARK);

    let Ok(marks) = fitted::<_, u32>(
      panelled
        .shapes
        .iter()
        .filter(|shape| match shape {
          super::Shape::Panel(panel) => {
            panel.size
              == console_core_geometry::Size {
                width: side,
                height: side,
              }
          }
          super::Shape::Text(_)
          | super::Shape::Picture(_)
          | super::Shape::Cropped(_)
          | super::Shape::Line(_)
          | super::Shape::Clip(_) => false,
        })
        .count(),
    );

    Ok(marks)
  }

  #[test]
  fn a_finger_alone_can_select_act_on_and_put_down_a_selection() -> Result<(), Failure> {
    let (selected, acted) = mpsc::sync_channel(4);
    let (opens, opened) = mpsc::sync_channel(4);
    let mut state = a_folder_that_selects(selected.clone(), opens)?;
    let rows: Vec<Row> = state
      .rows
      .iter()
      .cloned()
      .map(|row| {
        let key = match row.key.clone() {
          Some(key) => key,
          None => String::new(),
        };
        let Ok(row_2) = row.offering(move |showing| {
          showing.select(vec![key.clone()]);

          ActionOutcome::Stay
        });

        row_2
      })
      .collect();
    let Ok(page) = Page::new("Pictures", Rows::Fixed(rows.clone()));
    let Ok(page_2) = page.selecting("Delete", move |_, keys| {
      let _test_checks_the_selected_paths = selected.try_send(keys.to_vec());
    });

    state.pages = vec![page_2];
    state.rows = rows;

    let Ok(card) = card();

    let unmarked = marks_drawn(&state, &card)?;

    assert_eq!(unmarked, 0, "a mark is drawn before anything is selected");

    let dots = landing(&state, &card, Lands::Else(0))?;
    let Ok(_selects) = super::tapped(&mut state, Some(dots.lands), dots.panel.at, &card);
    let row = landing(&state, &card, Lands::Row(2))?;
    let Ok(_marks) = super::tapped(&mut state, Some(row.lands), row.panel.at, &card);

    let marked = marks_drawn(&state, &card)?;

    assert_eq!(
      state.selected,
      vec!["beach.jpg".to_string(), "sea.jpg".to_string()]
    );
    assert_eq!(marked, 2, "what is selected is not marked on the glass");
    assert_eq!(
      opened.try_recv(),
      Err(mpsc::TryRecvError::Empty),
      "a tap while selecting opened the thing"
    );

    let dots_2 = landing(&state, &card, Lands::Else(1))?;
    let Ok(_asks) = super::tapped(&mut state, Some(dots_2.lands), dots_2.panel.at, &card);
    let delete = landing(&state, &card, Lands::Answer(1))?;
    let Ok(_deletes) = super::tapped(&mut state, Some(delete.lands), delete.panel.at, &card);

    assert_eq!(
      acted.try_recv(),
      Ok(vec!["beach.jpg".to_string(), "sea.jpg".to_string()])
    );
    assert!(state.selected.is_empty());

    state.selected = vec!["dune.jpg".to_string()];

    let shut = landing(&state, &card, Lands::Back)?;
    let Ok(gone) = super::tapped(&mut state, Some(shut.lands), shut.panel.at, &card);

    assert_eq!(
      gone,
      ActionOutcome::Stay,
      "the \u{d7} closed the panel instead of putting the selection down"
    );
    assert!(state.selected.is_empty());

    Ok(())
  }

  #[test]
  fn b_puts_a_selection_down_before_it_closes_anything() -> Result<(), Failure> {
    let (selected, _acted) = mpsc::sync_channel(4);
    let (opens, _opened) = mpsc::sync_channel(4);
    let mut state = a_folder_that_selects(selected, opens)?;
    let rows = state.rows.clone();

    state.selected = vec!["dune.jpg".to_string()];

    assert_eq!(
      apply_meaning(&mut state, Meaning::Close, &rows),
      Ok(Outcome::Redrawn)
    );
    assert!(state.selected.is_empty());
    assert_eq!(
      apply_meaning(&mut state, Meaning::Close, &rows),
      Ok(Outcome::Closing)
    );

    Ok(())
  }

  #[test]
  fn select_all_marks_every_row_that_can_be_marked() -> Result<(), Failure> {
    let (selected, _acted) = mpsc::sync_channel(4);
    let (opens, _opened) = mpsc::sync_channel(4);
    let mut state = a_folder_that_selects(selected, opens)?;
    let rows = state.rows.clone();

    state.selected = vec!["dune.jpg".to_string()];

    let Ok(_asks) = carried_out(&mut state, &rows, Outcome::Else(1));
    let Ok(()) = super::took(&mut state.model, &state.rows, 2);

    assert_eq!(state.selected.len(), 3, "{:?}", state.selected);

    Ok(())
  }

  #[test]
  fn a_choice_made_over_a_zoomed_picture_is_told_what_was_on_the_screen() -> Result<(), Failure> {
    let still = PathBuf::from("/nowhere/a.png");
    let room = console_core_geometry::Size {
      width: 1280,
      height: 800,
    };
    let mut state = state_of(Vec::new())?;
    let Ok(closer) = crate::zoom::Zoom::default().times(crate::zoom::STEP);

    let Ok(inks) = plain_inks();

    state.shown = Some(super::Moving {
      of: still.clone(),
      at: console_core_geometry::Point { x: 0, y: 0 },
      room,
      inks,
    });
    state.zoom = Some((still.clone(), closer));

    let Ok(framed) = super::effects::dispatch(
      &mut state.model,
      &state.rows,
      super::effects::Source::Focused,
      |showing, _| showing.framed(),
    );

    assert_eq!(
      framed,
      Ok(Some(crate::zoom::Framed {
        of: still.clone(),
        zoom: closer,
        room
      })),
      "the viewer cannot crop to what it cannot see"
    );

    state.zoom = Some((PathBuf::from("/nowhere/b.png"), closer));
    let Ok(framed) = super::effects::dispatch(
      &mut state.model,
      &state.rows,
      super::effects::Source::Focused,
      |showing, _| showing.framed(),
    );

    assert_eq!(
      framed.map(|framed| framed.map(|framed_2| framed_2.zoom)),
      Ok(Some(crate::zoom::Zoom::default())),
      "another picture's zoom was handed on as this one's"
    );

    Ok(())
  }

  #[test]
  fn the_zoom_buttons_and_the_full_screen_button_reach_the_picture_on_the_screen()
  -> Result<(), Failure> {
    let Ok(row) = Row::picture(Picture::Showing(Some(PathBuf::from("/nowhere/a.png"))));
    let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![row]));
    let mut state = state_of(vec![page])?;
    let Ok(()) = super::effects::dispatch(
      &mut state.model,
      &state.rows,
      super::effects::Source::Page,
      |showing, _| {
        showing.zoom_by(crate::zoom::STEP);
        showing.toggle_full_screen();
      },
    );
    let rows = state.rows.clone();
    let row_2 = rows.first().ok_or("the page lost its picture")?;
    let Ok(zoom) = super::zoom_on(&state, row_2);

    assert_eq!(
      zoom.by,
      crate::zoom::STEP,
      "the zoom button did not zoom the picture"
    );
    assert_eq!(
      state.opened,
      Opened::Expanded,
      "the full screen button did not open the picture out"
    );

    let Ok(()) = super::effects::dispatch(
      &mut state.model,
      &state.rows,
      super::effects::Source::Page,
      |showing, _| showing.toggle_full_screen(),
    );

    assert_eq!(
      state.opened,
      Opened::No,
      "pressed again, full screen did not give the card back"
    );

    Ok(())
  }

  #[test]
  fn a_finger_drags_a_zoomed_picture_and_scrolls_a_whole_one() -> Result<(), Failure> {
    let still = PathBuf::from("/nowhere/a.png");
    let Ok(inks) = plain_inks();
    let moving = super::Moving {
      of: still.clone(),
      at: super::Point { x: 100, y: 100 },
      room: super::Size {
        width: 800,
        height: 400,
      },
      inks,
    };
    let Ok(row) = Row::picture(Picture::Showing(Some(still.clone())));
    let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![row]));
    let mut state = state_of(vec![page])?;
    let on_it = super::Point { x: 500, y: 300 };
    let Ok(whole) = super::panning(&state, Some(&moving), on_it);

    assert_eq!(
      whole, None,
      "a whole picture was dragged rather than the page scrolled"
    );

    let Ok(closer) = crate::zoom::Zoom::default().times(crate::zoom::STEP);

    state.zoom = Some((still.clone(), closer));

    let Ok(zoomed) = super::panning(&state, Some(&moving), on_it);
    let Ok(beside) = super::panning(&state, Some(&moving), super::Point { x: 50, y: 300 });

    assert_eq!(
      zoomed,
      Some((still, closer, moving.room)),
      "a zoomed picture does not follow the finger"
    );
    assert_eq!(beside, None, "a finger beside the picture dragged it");

    Ok(())
  }

  #[test]
  fn a_bar_draws_how_far_along_it_is_and_a_tap_on_it_goes_there() -> Result<(), Failure> {
    let (seeked, landed) = mpsc::sync_channel(1);
    let Ok(nothing) = Handler::and_stay(|_| {});
    let Ok(row) = Row::new("0:25", Aside("1:40"), nothing);
    let Ok(row_2) = row.picturing(Picture::Bar(crate::page::Bar { at: 25, of: 100 }));
    let Ok(row_3) = row_2.seeking(move |_, fraction| {
      let _test_checks_the_seek_fraction = seeked.try_send(fraction);
    });
    let Ok(page) = Page::new("Viewing", Rows::Fixed(vec![row_3]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let mut state = state_of(vec![page])?;

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let seek = panelled
      .touching
      .iter()
      .find(|touching| matches!(touching.lands, Lands::Seek { .. }))
      .ok_or("a bar that seeks has nowhere to tap")?;
    let rail = panelled
      .shapes
      .iter()
      .find_map(|shape| match shape {
        super::Shape::Panel(panel) => {
          match ::core::primitive::bool::then_some(panel.fill == wearing.soft, ()) {
            ::core::option::Option::Some(()) => Some(*panel),
            ::core::option::Option::None => None,
          }
        }
        super::Shape::Text(_)
        | super::Shape::Picture(_)
        | super::Shape::Cropped(_)
        | super::Shape::Line(_)
        | super::Shape::Clip(_) => None,
      })
      .ok_or("the bar has no rail")?;
    let filled = panelled.shapes.iter().any(|shape| match shape {
      super::Shape::Panel(panel) => {
        panel.fill == wearing.pink
          && panel.at.x == rail.at.x
          && panel.size.width == rail.size.width.saturating_div(4)
      }
      super::Shape::Text(_)
      | super::Shape::Picture(_)
      | super::Shape::Cropped(_)
      | super::Shape::Line(_)
      | super::Shape::Clip(_) => false,
    });

    assert!(
      filled,
      "a quarter of the way through is not a quarter of the bar lit"
    );

    let Ok(quarter) =
      console_core_number_conversion::fitted::<u32, i32>(rail.size.width.saturating_div(4));
    let Ok(marker) = knob_of(&panelled, wearing.text);
    let knob = marker.ok_or("the bar has no knob")?;
    let knob_wide = panelled
      .shapes
      .iter()
      .find_map(|shape| match shape {
        super::Shape::Panel(panel) => match (panel.fill == wearing.text, panel.at.x == knob) {
          (true, true) => Some(panel.size.width),
          (true, false) | (false, _) => None,
        },
        super::Shape::Text(_)
        | super::Shape::Picture(_)
        | super::Shape::Cropped(_)
        | super::Shape::Line(_)
        | super::Shape::Clip(_) => None,
      })
      .ok_or("the knob has no size")?;
    let Ok(half_a_knob) =
      console_core_number_conversion::fitted::<u32, i32>(knob_wide.saturating_div(2));

    assert_eq!(
      knob.saturating_add(half_a_knob),
      rail.at.x.saturating_add(quarter),
      "the knob is not where the lit part ends"
    );

    let Ok(along) = console_core_number_conversion::fitted::<u32, i32>(
      rail.size.width.saturating_mul(3).saturating_div(4),
    );
    let hit = super::Point {
      x: rail.at.x.saturating_add(along),
      y: seek.panel.at.y,
    };
    let Ok(_gone) = super::tapped(&mut state, Some(seek.lands), hit, &card);
    let fraction = landed.try_recv()?;

    assert!((fraction - 0.75).abs() < 0.01, "{fraction:?}");

    Ok(())
  }

  fn switched(
    says: &str,
    now: console_core_controls::Switch,
    turned: &mpsc::SyncSender<()>,
  ) -> Result<Row, Never> {
    let counting = turned.clone();
    let Ok(turning) = Handler::and_stay(move |_| {
      let _test_counts_switching = counting.try_send(());
    });
    let Ok(row) = Row::new(says, Aside(""), turning);

    row.switched(now)
  }

  fn slid(at: u32, heard: &mpsc::SyncSender<i32>) -> Result<Row, Never> {
    let hearing = heard.clone();
    let Ok(nothing) = Handler::and_stay(|_| {});
    let Ok(row) = Row::new("Speakers", Aside("20%"), nothing);
    let Ok(row_2) = row.adjusting(move |step| {
      let _test_collects_level_steps = hearing.try_send(step);
    });
    let Ok(slider) =
      console_core_controls::Slider::new(at, std::num::NonZeroU32::MIN.saturating_add(9));

    row_2.sliding(slider)
  }

  fn knob_of(panelled: &Panelled, ink: console_core_color::Oklch) -> Result<Option<i32>, Never> {
    Ok(panelled.shapes.iter().rev().find_map(|shape| match shape {
      Shape::Panel(panel) => match ::core::primitive::bool::then_some(panel.fill == ink, ()) {
        ::core::option::Option::Some(()) => Some(panel.at.x),
        ::core::option::Option::None => None,
      },
      Shape::Text(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    }))
  }

  #[test]
  fn a_switch_is_drawn_where_its_word_was_with_the_knob_on_the_side_it_is_set_to()
  -> Result<(), Failure> {
    let (turns, _turned) = mpsc::sync_channel(8);
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let Ok(on) = switched("Wi-Fi", console_core_controls::Switch::On, &turns);
    let Ok(off) = switched("Wi-Fi", console_core_controls::Switch::Off, &turns);
    let page = Page::new("Settings", Rows::Fixed(vec![on]))?;
    let on_state = state_of(vec![page])?;
    let wifi_off = Page::new("Settings", Rows::Fixed(vec![off]))?;
    let off_state = state_of(vec![wifi_off])?;
    let Ok(drawn_on) = drawn(&on_state, &card, &wearing);
    let Ok(drawn_off) = drawn(&off_state, &card, &wearing);
    let Ok(on_knob) = knob_of(&drawn_on, wearing.text);
    let Ok(off_knob) = knob_of(&drawn_off, wearing.text);
    let words: Vec<&str> = drawn_on
      .shapes
      .iter()
      .filter_map(|shape| match shape {
        Shape::Text(words) => Some(words.said.as_str()),
        Shape::Panel(_)
        | Shape::Picture(_)
        | Shape::Cropped(_)
        | Shape::Line(_)
        | Shape::Clip(_) => None,
      })
      .collect();

    assert!(
      on_knob
        .zip(off_knob)
        .is_some_and(|(on_position, off_position)| on_position > off_position),
      "{on_knob:?} is not right of {off_knob:?}"
    );
    assert!(
      !words.iter().any(|said| ["On", "Off"].contains(said)),
      "the switch was written as well as drawn: {words:?}"
    );

    Ok(())
  }

  #[test]
  fn right_turns_a_switch_on_and_right_on_one_already_on_does_nothing() -> Result<(), Failure> {
    let (turns, turned) = mpsc::sync_channel(8);
    let Ok(off) = switched("Wi-Fi", console_core_controls::Switch::Off, &turns);
    let Ok(on) = switched("Bluetooth", console_core_controls::Switch::On, &turns);
    let page = Page::new("Settings", Rows::Fixed(vec![off, on]))?;
    let mut state = state_of(vec![page])?;
    let rows = state.rows.clone();

    state.at = Some(0);

    let Ok(turning_on) = apply_meaning(&mut state, Meaning::Nudge(1), &rows);
    let Ok(_gone) = carried_out(&mut state, &rows, turning_on);

    state.at = Some(1);

    let Ok(already) = apply_meaning(&mut state, Meaning::Nudge(1), &rows);
    let Ok(_gone) = carried_out(&mut state, &rows, already);
    let Ok(turning_off) = apply_meaning(&mut state, Meaning::Nudge(-1), &rows);

    assert_eq!(turning_on, Outcome::Chose(0));
    assert_eq!(already, Outcome::None);
    assert_eq!(turning_off, Outcome::Chose(1));
    assert_eq!(
      turned.try_iter().take(8).count(),
      1,
      "right on a switch that was on turned it over"
    );

    Ok(())
  }

  #[test]
  fn a_tap_along_a_slider_moves_the_level_by_the_steps_between_the_knob_and_the_finger()
  -> Result<(), Failure> {
    let (record, heard) = mpsc::sync_channel(8);
    let Ok(row) = slid(2, &record);
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let page = Page::new("Sound", Rows::Fixed(vec![row]))?;
    let mut state = state_of(vec![page])?;
    let Ok(panelled) = drawn(&state, &card, &wearing);
    let slider = panelled
      .touching
      .iter()
      .find(|touching| matches!(touching.lands, Lands::Slider { .. }))
      .ok_or("the slider has nowhere to be pressed")?;
    let frame = match slider.lands {
      Lands::Slider { row: _, frame } => frame,
      Lands::Row(_)
      | Lands::Answer(_)
      | Lands::Nudge { .. }
      | Lands::Else(_)
      | Lands::ButtonPress { .. }
      | Lands::Seek { .. }
      | Lands::Picker { .. }
      | Lands::Tab(_)
      | Lands::More(_)
      | Lands::Back
      | Lands::Reveal => return Err(Failure::from("the slider landed somewhere else")),
    };
    let knob = frame.size.height.saturating_mul(2).saturating_div(3);
    let span = frame.size.width.saturating_sub(knob);
    let along = frame
      .origin
      .x
      .saturating_add(knob.saturating_div(2))
      .saturating_add(span.saturating_mul(7).saturating_div(10));
    let hit = super::Point {
      x: along,
      y: slider.panel.at.y,
    };
    let Ok(_gone) = super::tapped(&mut state, Some(slider.lands), hit, &card);
    let Ok(stepped) = collected(&heard);

    assert!(span > 0, "the slider was drawn with no track");
    assert_eq!(
      stepped,
      vec![5],
      "seven tenths along from two is five steps on"
    );

    Ok(())
  }

  #[test]
  fn a_finger_dragging_a_slider_asks_for_the_steps_from_where_it_last_left_the_knob()
  -> Result<(), Failure> {
    let (record, heard) = mpsc::sync_channel(8);
    let Ok(row) = slid(2, &record);
    let rows = vec![row];
    let frame = console_core_geometry::Rectangle {
      origin: super::Point { x: 0, y: 0 },
      size: console_core_geometry::Size {
        width: 230,
        height: 60,
      },
    };
    let Ok(first) = super::slid(
      &rows,
      0,
      (
        console_core_controls::Press::Drag(super::Point { x: 120, y: 30 }),
        frame,
      ),
      None,
    );
    let Ok(second) = super::slid(
      &rows,
      0,
      (
        console_core_controls::Press::Drag(super::Point { x: 9000, y: 30 }),
        frame,
      ),
      first,
    );
    let Ok(stepped) = collected(&heard);

    assert_eq!((first, second), (Some(5), Some(10)));
    assert_eq!(
      stepped,
      vec![3, 5],
      "the second step was counted from the knob the panel last read rather than the finger"
    );

    Ok(())
  }

  fn picking(chosen: u32, heard: &mpsc::SyncSender<u32>) -> Result<Row, Never> {
    let hearing = heard.clone();
    let Ok(report) = crate::page::retained(move |showing: &dyn crate::page::Showing, which| {
      let _test_collects_choices = hearing.try_send(which);
      showing.note("Choice applied");
    });
    let choices = crate::page::Choices {
      said: vec![
        "Low".to_string(),
        "Balanced".to_string(),
        "High".to_string(),
      ],
      chosen,
      choose: report,
    };

    Row::picking("Power", choices)
  }

  #[test]
  fn choosing_a_picker_option_starts_its_task() -> Result<(), Failure> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let Ok(_before) = crate::frames::wake_state();
    let door = a_door_held_shut("picked-task")?;
    let command = waiting_at(&door.0, "right")?;
    let Ok(task) = crate::tasks::Task::of(command.clone());
    let Ok(choose) = crate::page::retained(move |showing: &dyn crate::page::Showing, which| {
      if which == 1 {
        showing.run(task.clone());
      }
    });
    let Ok(row) = Row::picking(
      "Rotation",
      crate::page::Choices {
        said: vec!["Standard".to_owned(), "Rotated Right".to_owned()],
        chosen: 0,
        choose,
      },
    );
    let rows = vec![row];
    let Ok(page) = Page::new("Display", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;
    state.at = Some(0);
    let Ok(outcome) = pressed(&mut state, &rows, Keysym::Right);
    let Ok(running) = crate::tasks::running_now(&state.tasks);

    assert_eq!(outcome, ActionOutcome::Stay);
    assert_eq!(running.started.len(), 1);
    assert_eq!(
      running.started.first().map(|task| &task.command),
      Some(&command)
    );
    let Ok(()) = ended_quietly(door, &state.tasks);

    Ok(())
  }

  #[test]
  fn left_and_right_on_a_row_of_choices_choose_the_one_beside_and_stop_at_the_ends()
  -> Result<(), Failure> {
    let (record, heard) = mpsc::sync_channel(8);
    let Ok(middle) = picking(1, &record);
    let Ok(last) = picking(2, &record);
    let page = Page::new("Battery", Rows::Fixed(vec![middle, last]))?;
    let mut state = state_of(vec![page])?;
    let rows = state.rows.clone();

    state.at = Some(0);

    let Ok(on) = apply_meaning(&mut state, Meaning::Nudge(1), &rows);
    let Ok(_gone) = carried_out(&mut state, &rows, on);

    state.at = Some(1);

    let Ok(past_the_end) = apply_meaning(&mut state, Meaning::Nudge(1), &rows);
    let Ok(chosen) = collected(&heard);

    assert_eq!(on, Outcome::Picked { row: 0, which: 2 });
    assert_eq!(past_the_end, Outcome::None);
    assert_eq!(chosen, vec![2]);
    assert_eq!(state.note.as_deref(), Some("Choice applied"));

    Ok(())
  }

  #[test]
  fn choices_that_fit_stand_side_by_side_and_answer_taps_and_drags() -> Result<(), Failure> {
    let (record, heard) = mpsc::sync_channel(8);
    let Ok(row) = picking(1, &record);
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let page = Page::new("Battery", Rows::Fixed(vec![row]))?;
    let mut state = state_of(vec![page])?;
    let Ok(panelled) = drawn(&state, &card, &wearing);
    let words: Vec<&str> = panelled
      .shapes
      .iter()
      .filter_map(|shape| match shape {
        Shape::Text(words) => Some(words.said.as_str()),
        Shape::Panel(_)
        | Shape::Picture(_)
        | Shape::Cropped(_)
        | Shape::Line(_)
        | Shape::Clip(_) => None,
      })
      .collect();
    let picker = panelled
      .touching
      .iter()
      .find(|touching| matches!(touching.lands, Lands::Picker { .. }))
      .ok_or("the choices have nowhere to be pressed")?;
    let Ok(sixth) = fitted::<u32, i32>(picker.panel.size.width.saturating_div(6));
    let hit = super::Point {
      x: picker.panel.at.x.saturating_add(sixth),
      y: picker.panel.at.y,
    };
    let Ok(_gone) = super::tapped(&mut state, Some(picker.lands), hit, &card);
    let Ok(chosen) = collected(&heard);

    assert!(
      ["Low", "Balanced", "High"]
        .iter()
        .all(|said| words.contains(said)),
      "{words:?}"
    );
    assert_eq!(
      chosen,
      vec![0],
      "a tap on the first of three did not choose it"
    );
    assert_eq!(state.note.as_deref(), Some("Choice applied"));

    let Ok(room) = super::scroll_room(&state, &card);
    let mut reported = super::Reported {
      front: None,
      first_frame: super::FirstFrame::Reported,
      tells: None,
      presses: 0,
      told_presses: 0,
      keyboard: super::KeyboardFocus::Here,
      told_keyboard: super::KeyboardFocus::Here,
    };
    let mut dirty = super::Dirty::No;
    let mut stale = super::Stale::No;
    let mut finger = None;
    let mut input = super::PanelInput {
      state: &mut state,
      view: &panelled,
      card: &card,
      room,
      finger: &mut finger,
      reported: &mut reported,
      dirty: &mut dirty,
      stale: &mut stale,
    };
    let y = f64::from(hit.y.saturating_add(10));
    let start = f64::from(picker.panel.at.x);
    let Ok(_) = input.pointer(super::PointerEvent::Down {
      at: (f64::from(hit.x), y),
    });
    let Ok(_) = input.pointer(super::PointerEvent::Moved {
      at: (start + f64::from(sixth.saturating_mul(5)), y),
    });
    assert_eq!(heard.try_recv(), Err(mpsc::TryRecvError::Empty));
    let Ok(_) = input.pointer(super::PointerEvent::Up);
    assert_eq!(heard.try_recv(), Ok(2));
    assert_eq!(heard.try_recv(), Err(mpsc::TryRecvError::Empty));

    Ok(())
  }

  #[test]
  fn accessibility_can_choose_a_label_and_set_a_slider_value() -> Result<(), Failure> {
    use super::accessibility::{Snapshot, TreeChange, perform};
    use accesskit::{Action, ActionData, ActionRequest, Role, Toggled, TreeId};
    use std::hash::RandomState;

    let (record, heard) = mpsc::sync_channel(8);
    let (levels, adjusted) = mpsc::sync_channel(8);
    let Ok(picker) = picking(1, &record);
    let Ok(slider) = slid(2, &levels);
    let Ok(page) = Page::new("Settings", Rows::Fixed(vec![picker, slider]));
    let mut state = state_of(vec![page])?;
    let Ok(card) = card();
    let wearing = sample_palette()?;
    let Ok(view) = drawn(&state, &card, &wearing);
    let Ok(snapshot) = Snapshot::from_panel(&RandomState::new(), "Settings", &state, &view);
    let Ok(tree) = snapshot.changes(&Snapshot::default(), TreeChange::InitialTree);
    let Ok(room) = super::scroll_room(&state, &card);
    assert!(tree.nodes.iter().any(|(_, node)| {
      node.role() == Role::RadioButton
        && node.label() == Some("Balanced")
        && node.toggled() == Some(Toggled::True)
    }));
    let (high, _) = tree
      .nodes
      .iter()
      .find(|(_, node)| node.role() == Role::RadioButton && node.label() == Some("High"))
      .ok_or("the High choice is missing from the accessibility tree")?;
    let (slider, _) = tree
      .nodes
      .iter()
      .find(|(_, node)| node.role() == Role::Slider)
      .ok_or("the slider is missing from the accessibility tree")?;

    let Ok(choice) = snapshot.action(
      ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: *high,
        data: None,
      },
      &view,
    );
    let Ok(outcome) = perform(
      &mut state,
      &card,
      room,
      choice.ok_or("High cannot be clicked")?,
    );
    assert_eq!(outcome, ActionOutcome::Stay);
    assert_eq!(heard.try_recv(), Ok(2));
    assert_eq!(state.note.as_deref(), Some("Choice applied"));

    let Ok(value) = snapshot.action(
      ActionRequest {
        action: Action::SetValue,
        target_tree: TreeId::ROOT,
        target_node: *slider,
        data: Some(ActionData::NumericValue(7.0)),
      },
      &view,
    );
    let Ok(outcome) = perform(
      &mut state,
      &card,
      room,
      value.ok_or("the slider value cannot be set")?,
    );
    assert_eq!(outcome, ActionOutcome::Stay);
    assert_eq!(adjusted.try_recv(), Ok(5));
    Ok(())
  }

  #[test]
  fn right_on_a_slider_at_its_most_asks_nothing_of_the_level() -> Result<(), Failure> {
    let (record, heard) = mpsc::sync_channel(8);
    let Ok(full) = slid(10, &record);
    let Ok(half) = slid(5, &record);
    let page = Page::new("Sound", Rows::Fixed(vec![full, half]))?;
    let mut state = state_of(vec![page])?;
    let rows = state.rows.clone();

    state.at = Some(0);

    let Ok(at_the_most) = apply_meaning(&mut state, Meaning::Nudge(1), &rows);

    state.at = Some(1);

    let Ok(in_the_middle) = apply_meaning(&mut state, Meaning::Nudge(1), &rows);
    let Ok(stepped) = collected(&heard);

    assert_eq!(at_the_most, Outcome::None);
    assert_eq!(in_the_middle, Outcome::Redrawn);
    assert_eq!(stepped, vec![1]);

    Ok(())
  }

  #[test]
  fn a_bar_that_steps_holds_its_buttons_at_its_own_ends_and_its_length_past_them()
  -> Result<(), Failure> {
    let Ok(nothing) = Handler::and_stay(|_| {});
    let Ok(row) = Row::new("0:50", Aside("2:37"), nothing);
    let Ok(row_2) = row.picturing(Picture::Bar(crate::page::Bar { at: 50, of: 157 }));
    let Ok(row_3) = row_2.adjusting(|_step| {});
    let Ok(row_4) = row_3.seeking(|_, _fraction| {});
    let Ok(page) = Page::new("Now Playing", Rows::Fixed(vec![row_4]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![page])?;

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let landing = |wanted: fn(&Lands) -> bool| {
      panelled
        .touching
        .iter()
        .find(|touching| wanted(&touching.lands))
        .map(|touching| touching.panel)
    };
    let seek =
      landing(|lands| matches!(lands, Lands::Seek { .. })).ok_or("the bar is missing")?;
    let less = landing(|lands| matches!(lands, Lands::Nudge { step: -1, .. }))
      .ok_or("the minus is missing")?;
    let more = landing(|lands| matches!(lands, Lands::Nudge { step: 1, .. }))
      .ok_or("the plus is missing")?;
    let length = panelled.shapes.iter().find_map(|shape| match shape {
      super::Shape::Text(text) => (text.said == "2:37").then_some(text.at.x),
      super::Shape::Panel(_)
      | super::Shape::Picture(_)
      | super::Shape::Cropped(_)
      | super::Shape::Line(_)
      | super::Shape::Clip(_) => None,
    });
    let Ok(bar_wide) = console_core_number_conversion::fitted::<u32, i32>(seek.size.width);
    let bar_ends = seek.at.x.saturating_add(bar_wide);

    assert!(
      less.at.x < seek.at.x,
      "the minus is not before the bar: {less:?} {seek:?}"
    );
    assert!(
      more.at.x >= bar_ends,
      "the plus is not after the bar: {more:?} {seek:?}"
    );
    assert!(
      length.is_some_and(|x| x > more.at.x),
      "the length is not past the plus: {length:?} {more:?}"
    );

    Ok(())
  }

  #[test]
  fn a_bar_on_the_lit_row_fills_in_the_dark_rather_than_pink_on_pink() -> Result<(), Failure> {
    let Ok(nothing) = Handler::and_stay(|_| {});
    let Ok(entry) = Row::new("0:25", Aside("1:40"), nothing);
    let Ok(with_bar) = entry.picturing(Picture::Bar(crate::page::Bar { at: 25, of: 100 }));
    let Ok(row) = with_bar.seeking(|_, _fraction| {});
    let Ok(page) = Page::new("Now Playing", Rows::Fixed(vec![row]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let mut state = state_of(vec![page])?;

    state.at = Some(0);

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let rail = panelled
      .shapes
      .iter()
      .find_map(|shape| match shape {
        super::Shape::Panel(panel) => {
          match ::core::primitive::bool::then_some(panel.fill == wearing.soft, ()) {
            ::core::option::Option::Some(()) => Some(*panel),
            ::core::option::Option::None => None,
          }
        }
        super::Shape::Text(_)
        | super::Shape::Picture(_)
        | super::Shape::Cropped(_)
        | super::Shape::Line(_)
        | super::Shape::Clip(_) => None,
      })
      .ok_or("the bar has no rail")?;
    let filled = panelled.shapes.iter().any(|shape| match shape {
      super::Shape::Panel(panel) => {
        panel.fill == wearing.night
          && panel.at.x == rail.at.x
          && panel.size.width == rail.size.width.saturating_div(4)
      }
      super::Shape::Text(_)
      | super::Shape::Picture(_)
      | super::Shape::Cropped(_)
      | super::Shape::Line(_)
      | super::Shape::Clip(_) => false,
    });

    assert!(filled, "the lit row's bar is not filled in the dark");

    Ok(())
  }

  #[test]
  fn what_a_card_says_it_drew_names_every_part_a_hand_could_land_on() -> Result<(), Failure> {
    let Ok(entry) = Row::text("Ferry.jpg", Aside("2.1 MB"));
    let Ok(row) = entry.offering(|_showing| ActionOutcome::Stay);
    let rows = vec![row];
    let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let mut state = state_of(vec![page])?;

    state.at = Some(0);

    let Ok(panelled) = drawn(&state, &card, &wearing);
    let Ok(told) = what_it_drew(&state, &panelled, &rows, (1024, 640));

    assert_eq!(told.tab, "Files");

    let Ok(worn) = told.spot("shut");

    assert!(worn.is_some(), "a card with no way out drawn on it");

    let Ok(tabs) = told.spot("tab");

    assert!(
      tabs.is_none(),
      "one tab is the panel's title, not a strip with nothing to turn to"
    );

    let Ok(line) = told.line_saying("Ferry.jpg");
    let line_2 = line.ok_or("the row")?;

    assert_eq!(line_2.standing, crate::description::Standing::On);
    assert_eq!(line_2.offers, crate::description::Offers::Yes);

    let Ok(extra_action) = line_2.spot("else");

    assert!(
      extra_action.is_some(),
      "a row offering something and drawing no mark for it"
    );

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

    Ok(())
  }

  fn landing_on_rows(panelled: &Panelled) -> Result<Vec<&HitRegion>, Never> {
    Ok(
      panelled
        .touching
        .iter()
        .filter(|touching| matches!(touching.lands, Lands::Row(_)))
        .collect(),
    )
  }

  fn pressed(state: &mut State, rows: &[Row], key: Keysym) -> Result<ActionOutcome, Never> {
    let Ok(driving) = driving(state);
    let Ok(meaning) = crate::keys::meaning(key, driving);

    pressed_here(state, rows, key, meaning, driving)
  }

  #[test]
  fn a_row_that_asks_whether_somebody_is_sure_is_answered_left_and_right() -> Result<(), Failure>
  {
    let (answering, taken) = mpsc::sync_channel(1);
    let Ok(does) = Handler::and_stay(move |showing| {
      let told_which = answering.clone();

      showing.sure(
        "Throw away",
        crate::page::Subject("Ferry.jpg"),
        &["Throw Away"],
        Box::new(move |_showing, which| {
          let _test_checks_the_answer = told_which.try_send(which);
        }),
      );
    });
    let Ok(row) = Row::new("Throw away", Aside(""), does);
    let rows = vec![row];
    let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let Ok(_up) = carried_out(&mut state, &rows, Outcome::Chose(0));

    assert_eq!(
      driving(&state),
      Ok(Driving::Sure),
      "a question is what is being driven"
    );

    let Ok(_cancelled) = pressed(&mut state, &rows, Keysym::Return);

    assert_eq!(
      taken.try_recv(),
      Err(mpsc::TryRecvError::Empty),
      "the answer standing first is the one that undoes the asking"
    );
    assert_eq!(driving(&state), Ok(Driving::Panel), "and the list is back");

    let Ok(_again) = carried_out(&mut state, &rows, Outcome::Chose(0));
    let Ok(_leant) = pressed(&mut state, &rows, Keysym::Right);
    let Ok(_took) = pressed(&mut state, &rows, Keysym::Return);

    assert_eq!(
      taken.try_recv(),
      Ok(0),
      "the first thing it offers to do is done"
    );
    assert_eq!(driving(&state), Ok(Driving::Panel));

    Ok(())
  }

  fn answered(initial: &'static str, keys: &[Keysym]) -> Result<(String, Driving), Failure> {
    let (report, heard) = mpsc::sync_channel(1);
    let Ok(does) = Handler::and_stay(move |showing| {
      let answer = report.clone();
      let Ok(accept) = crate::page::retained(move |_: &dyn crate::page::Showing, word: &str| {
        let _test_checks_the_typed_word = answer.try_send(word.to_owned());
      });
      showing.ask_aloud(
        "What should it be called",
        crate::page::Initial(initial),
        accept,
      );
    });
    let Ok(row) = Row::new("Rename", Aside(""), does);
    let rows = vec![row];
    let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let Ok(_up) = carried_out(&mut state, &rows, Outcome::Chose(0));

    assert_eq!(driving(&state), Ok(Driving::Question));

    for key in keys {
      let Ok(_typed) = pressed(&mut state, &rows, *key);
    }

    let Ok(_answered) = pressed(&mut state, &rows, Keysym::Return);
    let said = heard.try_recv()?;
    let Ok(now) = driving(&state);

    Ok((said, now))
  }

  #[test]
  fn a_row_that_asks_a_question_is_typed_into_and_answered_by_return() -> Result<(), Failure> {
    let (said, now) = answered("", &[Keysym::h, Keysym::i])?;

    assert_eq!(said, "hi", "what was typed is what the row was told");
    assert_eq!(now, Driving::Panel, "and the question is gone");

    Ok(())
  }

  #[test]
  fn the_arrows_step_back_into_what_was_typed() -> Result<(), Failure> {
    let (said, _now) = answered(
      "",
      &[
        Keysym::parenleft,
        Keysym::parenright,
        Keysym::Left,
        Keysym::x,
      ],
    )?;

    assert_eq!(said, "(x)");

    Ok(())
  }

  #[test]
  fn a_question_can_start_out_holding_what_is_there_already() -> Result<(), Failure> {
    let (said, _now) = answered(
      "milk",
      &[Keysym::Home, Keysym::space, Keysym::End, Keysym::BackSpace],
    )?;

    assert_eq!(said, " mil", "the caret starts after what the line holds");

    Ok(())
  }

  fn asked(
    ask: impl Fn(&dyn crate::page::Showing) + Send + Sync + 'static,
  ) -> Result<(State, Vec<Row>), Failure> {
    let Ok(does) = Handler::and_stay(ask);
    let Ok(row) = Row::new("Home", Aside(""), does);
    let rows = vec![row];
    let Ok(page) = Page::new("Wi-Fi", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let Ok(_up) = carried_out(&mut state, &rows, Outcome::Chose(0));

    for key in [Keysym::a, Keysym::b] {
      let Ok(_typed) = pressed(&mut state, &rows, key);
    }

    Ok((state, rows))
  }

  fn line(state: &State) -> Result<String, Never> {
    let Ok((said, _before, _filled)) = super::in_the_line(state);

    Ok(said)
  }

  #[test]
  fn a_password_shows_its_letters_when_asked_and_hides_them_again() -> Result<(), Failure> {
    let (mut state, rows) = asked(|showing| {
      let Ok(answer) = empty_answer();
      showing.ask("Password for Home", answer);
    })?;
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let dots = crate::marks::HIDDEN.repeat(2);

    assert_eq!(
      line(&state),
      Ok(dots.clone()),
      "a password is drawn as dots"
    );

    let Ok(_shown) = pressed(&mut state, &rows, Keysym::F18);

    assert_eq!(line(&state), Ok("ab".to_string()), "y shows what was typed");

    let reveal = landing(&state, &card, Lands::Reveal)?;
    let Ok(_hidden) = super::tapped(&mut state, Some(Lands::Reveal), reveal.panel.at, &card);

    assert_eq!(
      line(&state),
      Ok(dots),
      "a thumb on the button hides it again"
    );

    let Ok(panelled) = drawn(&state, &card, &wearing);

    assert_eq!(
      driving(&state),
      Ok(Driving::Question),
      "showing it does not answer the question"
    );
    assert!(
      panelled
        .touching
        .iter()
        .any(|touching| touching.lands == Lands::Reveal),
      "the button is still there to show it again"
    );

    Ok(())
  }

  #[test]
  fn password_masks_and_the_caret_count_graphemes() -> Result<(), Failure> {
    let (mut state, _rows) = asked(|showing| {
      let Ok(answer) = empty_answer();
      showing.ask("Password", answer);
    })?;
    let Ok(mut typed) = TextField::holding("e\u{301}👩‍👩‍👧‍👦");
    let Ok(_moved) = typed.moved_to(1);
    let prompt = state
      .asking
      .as_mut()
      .ok_or_else(|| std::io::Error::other("question missing"))?;
    prompt.typed = typed;

    let Ok((said, before, _filled)) = super::in_the_line(&state);

    assert_eq!(said, crate::marks::HIDDEN.repeat(2));
    assert_eq!(before, crate::marks::HIDDEN);

    Ok(())
  }

  #[test]
  fn a_question_that_is_not_a_password_has_nothing_to_show() -> Result<(), Failure> {
    let (mut state, rows) = asked(|showing| {
      let Ok(answer) = empty_answer();
      showing.ask_aloud("What should it be called", crate::page::Initial(""), answer)
    })?;
    let wearing = sample_palette()?;
    let Ok(card) = card();

    let Ok(_pressed) = pressed(&mut state, &rows, Keysym::F18);
    let Ok(panelled) = drawn(&state, &card, &wearing);

    assert_eq!(line(&state), Ok("ab".to_string()), "y does not hide a name");
    assert!(
      !panelled
        .touching
        .iter()
        .any(|touching| touching.lands == Lands::Reveal),
      "no button offers to"
    );

    Ok(())
  }

  #[test]
  fn a_question_standing_over_the_list_is_what_is_drawn() -> Result<(), Failure> {
    let Ok(ferry) = said_row("Ferry.jpg");
    let Ok(page) = Page::new("Files", Rows::Fixed(vec![ferry]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let mut state = state_of(vec![page])?;

    state.sure = Some(super::Sure {
      question: "Throw away".to_string(),
      about: "Ferry.jpg".to_string(),
      answers: vec![crate::marks::CANCEL.to_string(), "Throw Away".to_string()],
      at: 1,
      then: Box::new(|_showing, _which| {}),
    });

    let Ok(panelled) = drawn(&state, &card, &wearing);

    let said: Vec<String> = panelled
      .shapes
      .iter()
      .filter_map(|shape| match shape {
        console_core_shapes::Shape::Text(text) => Some(text.said.clone()),
        console_core_shapes::Shape::Panel(_)
        | console_core_shapes::Shape::Picture(_)
        | console_core_shapes::Shape::Cropped(_)
        | console_core_shapes::Shape::Line(_)
        | console_core_shapes::Shape::Clip(_) => None,
      })
      .collect();

    assert!(
      said.contains(&"Throw away".to_string()),
      "the question: {said:?}"
    );
    assert!(
      said.contains(&"Ferry.jpg".to_string()),
      "what it is about: {said:?}"
    );
    assert!(
      said.contains(&crate::marks::CANCEL.to_string()),
      "the way out: {said:?}"
    );
    assert!(
      !said.contains(&"Ferry.jpg".to_string().repeat(2)),
      "the rows under it are not drawn as well"
    );
    assert_eq!(
      panelled
        .touching
        .iter()
        .filter(|touching| matches!(touching.lands, Lands::Answer(_)))
        .count(),
      2,
      "both answers can be pressed by a thumb"
    );

    Ok(())
  }

  #[test]
  fn right_on_a_row_that_offers_something_stands_beside_it_and_a_runs_it() -> Result<(), Failure>
  {
    let (report, offered) = mpsc::sync_channel(1);
    let Ok(label) = Row::text("Ferry.jpg", Aside("2.1 MB"));
    let Ok(row) = label.offering(move |_showing| {
      let _test_checks_the_offer = report.try_send(());

      ActionOutcome::Stay
    });
    let rows = vec![row];
    let Ok(page) = Page::new("Files", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    let Ok(_down) = apply_meaning(&mut state, Meaning::Step(1), &rows);
    let Ok(stood) = apply_meaning(&mut state, Meaning::Nudge(1), &rows);

    assert_eq!(stood, Outcome::Redrawn);
    assert_eq!(
      state.beside,
      Beside::Yes,
      "the highlight is on what else the row offers"
    );

    let Ok(chose) = apply_meaning(&mut state, Meaning::Choose, &rows);

    assert_eq!(chose, Outcome::Else(0));
    assert_eq!(state.beside, Beside::No, "and it comes back to the row");

    let Ok(_ran) = carried_out(&mut state, &rows, chose);

    assert_eq!(
      offered.try_recv(),
      Ok(()),
      "what else the row offers was never run"
    );

    Ok(())
  }

  #[test]
  fn right_on_a_row_that_holds_a_level_moves_the_level_instead() -> Result<(), Failure> {
    let (report, moved) = mpsc::sync_channel(1);
    let Ok(label) = Row::text("Volume", Aside("40%"));
    let Ok(row) = label.adjusting(move |step| {
      let _test_checks_the_level_delta = report.try_send(step);
    });
    let rows = vec![row];
    let Ok(page) = Page::new("Sound", Rows::Fixed(rows.clone()));
    let mut state = state_of(vec![page])?;

    state.at = Some(0);

    let Ok(nudged) = apply_meaning(&mut state, Meaning::Nudge(1), &rows);

    assert_eq!(nudged, Outcome::Redrawn);
    assert_eq!(
      state.beside,
      Beside::No,
      "a row with a level has nowhere to stand beside"
    );
    assert_eq!(moved.try_recv(), Ok(1), "the level was moved on");

    Ok(())
  }

  #[test]
  fn a_row_that_has_a_square_at_its_front_starts_its_words_after_it() {
    let Ok(room) = beside_the_words(&crate::page::Picture::Space);
    let Ok(none) = beside_the_words(&crate::page::Picture::None);
    let Ok(at) = beside_the_words(&crate::page::Picture::At(PathBuf::from("/x.png")));

    assert!(
      room > crate::strip::PICTURE,
      "the square and the gap after it"
    );
    assert_eq!(
      at, room,
      "a picture takes the room a blank square holds open"
    );
    assert_eq!(
      none, 0,
      "and a row with nothing at its front gives none of it away"
    );
  }

  #[test]
  fn a_row_whose_picture_is_not_made_yet_asks_for_it_rather_than_drawing_it() {
    let at = crate::page::Picture::At(PathBuf::from("/usr/share/x.png"));
    let Ok(wanted) = wanted_of(&at);
    let Ok(sleeve) = wanted_of(&crate::page::Picture::Sleeve(None));
    let Ok(named) = wanted_of(&crate::page::Picture::Named(crate::icons::Icon::Folder));

    assert_eq!(wanted.and_then(Path::to_str), Some("/usr/share/x.png"));
    assert_eq!(
      sleeve, None,
      "a sleeve nobody found is nothing to ask the maker for"
    );
    assert_eq!(
      named, None,
      "and an icon is a name the store is not keyed by"
    );
  }

  #[test]
  fn a_row_with_a_level_draws_its_two_marks_where_a_thumb_can_find_them() -> Result<(), Failure> {
    let Ok(label) = Row::text("Volume", Aside("40%"));
    let Ok(row) = label.adjusting(|_step| {});
    let Ok(page) = Page::new("Sound", Rows::Fixed(vec![row]));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![page])?;

    let Ok(panelled) = drawn(&state, &card, &wearing);

    let marks: Vec<Lands> = panelled
      .touching
      .iter()
      .map(|touching| touching.lands)
      .filter(|lands| matches!(lands, Lands::Nudge { row: _, step: _ }))
      .collect();

    assert_eq!(
      marks,
      vec![
        Lands::Nudge { row: 0, step: 1 },
        Lands::Nudge { row: 0, step: -1 }
      ],
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
        | console_core_shapes::Shape::Line(_)
        | console_core_shapes::Shape::Clip(_) => None,
      })
      .collect();

    assert!(
      said.contains(&crate::marks::LESS.to_string()),
      "the end that takes away: {said:?}"
    );
    assert!(
      said.contains(&crate::marks::MORE.to_string()),
      "the end that adds: {said:?}"
    );
    assert!(
      said.contains(&"40%".to_string()),
      "and what it is now: {said:?}"
    );

    Ok(())
  }

  #[test]
  fn a_tabs_watch_runs_while_the_tab_is_in_front_and_not_after() -> Result<(), Failure> {
    let Ok(watch) = crate::page::Watch::command(&["sleep", "60"]);
    let Ok(looking) = page("Bluetooth", &["Search for Devices"]);
    let Ok(looking_2) = looking.with_watch(watch);
    let Ok(wifi) = page("Wi-Fi", &["Home"]);
    let mut state = state_of(vec![looking_2, wifi])?;
    let mut watching = None;

    let Ok(()) = super::sync_watch(&mut watching, &state);

    let held_watch = match &watching {
      Some((0, running)) => Some(running),
      Some(_) | None => None,
    };
    let running =
      held_watch.ok_or("the Bluetooth tab is in front and nothing it watches is running")?;
    let Ok(id) = running.id();
    let at = PathBuf::from(format!("/proc/{id}"));

    let _watch_is_running = std::fs::metadata(&at)?;

    state.here = 1;

    let Ok(()) = super::sync_watch(&mut watching, &state);

    assert!(
      watching.is_none(),
      "a tab with nothing to watch is holding a watch"
    );
    let observed = std::fs::metadata(&at);
    assert!(matches!(observed, Err(fault) if fault.kind() == std::io::ErrorKind::NotFound));

    Ok(())
  }

  fn a_pool_watching(folder: &str) -> Result<(PathBuf, PathBuf), Failure> {
    let socket = console_core_temporary_directories::fresh(&format!("panel-{folder}-socket"))?;
    let at = socket.join("panel.sock");
    let into = console_core_temporary_directories::fresh(&format!("panel-{folder}"))?;

    let serving = at.clone();
    let Ok(()) = console_program_lifetime::threads::let_go(std::thread::spawn(move || {
      let _unserved = console_events::serving::serve(&serving, console_events::sources::hold);
    }));
    let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(5));
    let Ok(_up) = console_waiting::until(patience, || {
      Ok(match std::fs::metadata(&at) {
        Ok(_socket_is_named) => console_waiting::Ready::Yes,
        Err(_the_listener_is_starting) => console_waiting::Ready::NotYet,
      })
    });

    Ok((at, into))
  }

  fn woke_within(within: rustix::event::Timespec) -> Result<crate::frames::WakeState, Failure> {
    let _ready = crate::frames::with_waking(|waking| {
      let waking = waking.ok_or("a socket the loop is woken on")?;
      let mut watch = [rustix::event::PollFd::from_borrowed_fd(
        waking,
        rustix::event::PollFlags::IN,
      )];
      Ok::<_, Failure>(rustix::event::poll(&mut watch, Some(&within))?)
    })?;
    let Ok(woken) = crate::frames::wake_state();

    Ok(woken)
  }

  fn woke_at_all(
    within: rustix::event::Timespec,
  ) -> Result<crate::frames::FrameReceived, Failure> {
    let ready = crate::frames::with_waking(|waking| {
      let waking = waking.ok_or("a socket the loop is woken on")?;
      let mut watch = [rustix::event::PollFd::from_borrowed_fd(
        waking,
        rustix::event::PollFlags::IN,
      )];
      Ok::<_, Failure>(rustix::event::poll(&mut watch, Some(&within)))
    })?;
    let woke = match ready {
      Ok(0) => crate::frames::FrameReceived::No,
      Err(_interrupted) => crate::frames::FrameReceived::No,
      Ok(_ready) => crate::frames::FrameReceived::Yes,
    };
    let Ok(_drained) = crate::frames::wake_state();

    Ok(woke)
  }

  fn told_after_writing(
    at: &std::path::Path,
    into: &std::path::Path,
    worth: console_events::again::Worthwhile,
  ) -> Result<crate::frames::FrameReceived, Failure> {
    let Ok(subscriber) = console_events::subscription::connect_at(at, &[]);
    let Ok(mut front) = super::InFront::on(subscriber);
    let event_group = console_program_contract::EventGroup::Path(into.to_path_buf());
    let Ok(looking) = page("Sound", &["Speakers"]);
    let Ok(looking_2) = looking.with_subscription(event_group, worth);
    let Ok(_before) = crate::frames::wake_state();

    let Ok(()) = front.listen(Some(&looking_2));
    let joined = woke_at_all(rustix::event::Timespec {
      tv_sec: 2,
      tv_nsec: 0,
    })?;

    assert_eq!(joined, crate::frames::FrameReceived::Yes);
    assert_eq!(
      front.changed(),
      Ok(console_events::again::Worth::Querying),
      "getting into the pool did not wake the loop to read the tab again"
    );

    let _replayed = woke_within(rustix::event::Timespec {
      tv_sec: 0,
      tv_nsec: 100_000_000,
    })?;
    let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(2));
    let Ok(told) = console_waiting::until_some(patience, || {
      let written = console_core_atomic_writes::whole(&into.join("changed"), b"");
      let woke = woke_within(rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: 50_000_000,
      });

      Ok(match (written, woke) {
        (Ok(()), Ok(_woken)) => match front.changed() {
          Ok(console_events::again::Worth::Querying) => Some(()),
          Ok(console_events::again::Worth::Ignoring) => None,
        },
        (Err(_unwritten), _) => None,
        (Ok(()), Err(_unwaited)) => None,
      })
    });

    Ok(match told {
      Some(()) => crate::frames::FrameReceived::Yes,
      None => crate::frames::FrameReceived::No,
    })
  }

  #[test]
  fn a_tab_in_front_is_read_again_when_the_pool_says_its_event_group_changed()
  -> Result<(), Failure> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let (at, into) = a_pool_watching("listening")?;
    let told = told_after_writing(&at, &into, console_events::again::always)?;
    std::fs::remove_dir_all(&into)?;
    std::fs::remove_file(&at)?;

    assert_eq!(
      told,
      crate::frames::FrameReceived::Yes,
      "the pool said the event group changed and the loop was never woken to read the tab again"
    );

    Ok(())
  }

  #[test]
  fn a_change_the_tab_does_not_care_about_does_not_read_it_again() -> Result<(), Failure> {
    fn ignore_all(_line: &str) -> Result<console_events::again::Worth, Never> {
      Ok(console_events::again::Worth::Ignoring)
    }

    let Ok(_turn) = crate::frames::serial_fixture();
    let (at, into) = a_pool_watching("ignoring")?;
    let told = told_after_writing(&at, &into, ignore_all)?;
    std::fs::remove_dir_all(&into)?;
    std::fs::remove_file(&at)?;

    assert_eq!(
      told,
      crate::frames::FrameReceived::No,
      "a line the tab said was not worth asking after read it again"
    );

    Ok(())
  }

  #[test]
  fn a_panel_told_to_shut_wakes_a_loop_asleep_with_no_timeout() -> Result<(), Failure> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let Ok(_before) = crate::frames::wake_state();
    let Ok((close, watch)) = super::CloseWatch::pair();
    let Ok(()) = close.shut();
    let woke = woke_at_all(rustix::event::Timespec {
      tv_sec: 0,
      tv_nsec: 0,
    })?;

    assert!(
      matches!(watch.stop(), Ok(super::StopRequest::Requested)),
      "shutting did not say so"
    );
    assert_eq!(
      woke,
      crate::frames::FrameReceived::Yes,
      "a panel was told to shut and the loop asleep in poll was never woken to read it"
    );

    Ok(())
  }

  #[test]
  fn a_reading_that_lands_wakes_the_loop_waiting_for_it() -> Result<(), Failure> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let Ok(_before) = crate::frames::wake_state();
    let Ok(now) = page("Now", &["one"]);
    let Ok(reading) = super::reading(&now, 0);
    let woke = woke_at_all(rustix::event::Timespec {
      tv_sec: 5,
      tv_nsec: 0,
    })?;

    assert_eq!(
      woke,
      crate::frames::FrameReceived::Yes,
      "the rows were read and the loop asleep in poll was never woken to draw them"
    );
    assert_eq!(
      reading.arrived.try_recv().map(|rows| rows.len()),
      Ok(1),
      "the loop was woken before the rows it was woken for"
    );

    Ok(())
  }

  fn written_on(rows: Vec<Row>) -> Result<Vec<console_core_shapes::Text>, Failure> {
    let Ok(page) = Page::new("Now", Rows::Fixed(rows));
    let wearing = sample_palette()?;
    let Ok(card) = card();
    let state = state_of(vec![page])?;
    let Ok(panelled) = drawn(&state, &card, &wearing);

    Ok(
      panelled
        .shapes
        .into_iter()
        .filter_map(|shape| match shape {
          console_core_shapes::Shape::Text(text) => Some(text),
          console_core_shapes::Shape::Panel(_)
          | console_core_shapes::Shape::Picture(_)
          | console_core_shapes::Shape::Cropped(_)
          | console_core_shapes::Shape::Line(_)
          | console_core_shapes::Shape::Clip(_) => None,
        })
        .collect(),
    )
  }

  fn glyph_of(icon: crate::icons::Icon) -> Result<String, Never> {
    let Ok(glyph) = icon.glyph();

    Ok(glyph.to_string())
  }

  #[test]
  fn an_icon_beside_a_row_is_drawn_rather_than_left_as_a_gap() -> Result<(), Failure> {
    let Ok(entry) = said_row("Tomorrow  Rain");
    let Ok(row) = entry.picturing(Picture::Named(crate::icons::Icon::Rain));
    let written = written_on(vec![row])?;
    let said: Vec<String> = written.into_iter().map(|text| text.said).collect();
    let Ok(rain) = glyph_of(crate::icons::Icon::Rain);

    assert!(said.contains(&rain), "the rain is not on the row: {said:?}");

    Ok(())
  }

  #[test]
  fn a_cell_with_an_icon_draws_the_icon_before_its_words() -> Result<(), Failure> {
    let Ok(cell) = crate::page::Cell::new("Wind  10 km/h", crate::page::Active::No);
    let Ok(cell_2) = cell.with_icon(crate::icons::Icon::Wind);
    let Ok(row) = Row::celled(vec![cell_2]);
    let written = written_on(vec![row])?;
    let Ok(wind) = glyph_of(crate::icons::Icon::Wind);
    let glyph = written.iter().find(|text| text.said == wind);
    let words = written.iter().find(|text| text.said == "Wind  10 km/h");
    let (icon, title) = glyph
      .zip(words)
      .ok_or(format!("the icon or its words are missing: {written:?}"))?;

    assert!(icon.at.x < title.at.x, "the icon comes after its words");

    Ok(())
  }

  #[test]
  fn a_headline_draws_the_temperature_large_with_its_sky_beside_an_icon() -> Result<(), Failure> {
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
    let Ok(under) = said_row("under it");
    let written = written_on(vec![row, under])?;
    let said: Vec<&str> = written.iter().map(|text| text.said.as_str()).collect();
    let big = written.iter().find(|text| text.said == "17\u{b0}");
    let body = written.iter().find(|text| text.said == "under it");
    let Ok(sky) = glyph_of(crate::icons::Icon::CloudSun);

    for wanted in [
      "Brussels",
      "Thursday, updated 02:30",
      "Partly Cloudy",
      "H:21\u{b0}  L:15\u{b0}",
    ] {
      assert!(said.contains(&wanted), "{wanted} is missing: {said:?}");
    }

    assert!(
      said.contains(&sky.as_str()),
      "the sky has no icon: {said:?}"
    );

    let (temperature, below) = big.zip(body).ok_or(format!(
      "the temperature or the row under it is missing: {said:?}"
    ))?;

    assert!(
      temperature.font.height > below.font.height.saturating_mul(3),
      "the temperature is not large"
    );

    Ok(())
  }
}
