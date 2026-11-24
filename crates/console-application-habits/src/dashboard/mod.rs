//! A standalone Wayland window over the recorder's existing local history.

mod calendar;
mod model;
mod scene;
mod theme;

use std::cmp::Ordering;
use std::time::{Duration, Instant};

use console_core_geometry::Size;
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_i32};
use console_draw_surface::{Closed, Keysym, PointerEvent, Scale, Surface, Window};

use crate::activity::{Span, read_history};
use crate::report::Recorder;
use crate::{APP_ID, Paths, Result, now};
use calendar::{CURRENT_DAY, Day, local_days, time_range};
use model::{Kind, Period, Scope, duration};
use scene::{Art, FocusDirection, Frame};

const WINDOW_SIZE: Size<u32> = Size {
  width: 1080,
  height: 760,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
  Overview,
  Timeline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
  Page(Page),
  Scope(Scope),
  Kind(Kind),
  Chart { day: u32, slot: u32 },
  Step(i32),
  More,
  Segment { lane: u32, period: Period },
  Close,
}

#[derive(Clone, Copy, PartialEq)]
enum RecorderState {
  Current,
  Stale,
}

#[derive(Clone, Copy, PartialEq)]
enum Refresh {
  Unchanged,
  Changed,
}

#[derive(Clone, Copy)]
enum WindowChange {
  StayOpen,
  Close,
}

#[derive(Clone, Copy)]
enum Rows {
  Summary,
  Expanded,
}

struct Dashboard {
  history: Vec<Span>,
  calendar: Vec<Day>,
  recorder: Option<Recorder>,
  recorder_state: RecorderState,
  scope: Scope,
  kind: Kind,
  page: Page,
  day: u32,
  today_ticks: Vec<(u64, String)>,
  ticks: Vec<(u64, String)>,
  scroll: i32,
  more: Rows,
  focus: Option<Action>,
  hover: Option<Action>,
  details: Option<(Period, String)>,
  problem: Option<&'static str>,
}

impl Dashboard {
  fn new(paths: &Paths) -> Result<Self> {
    let Ok(at) = now();
    let calendar = local_days(at)?;
    let day = CURRENT_DAY;
    let today = calendar.last().ok_or("calendar has no current day")?;
    let today_ticks = today.ticks((0..24).step_by(3))?;
    let ticks = timeline_ticks(today)?;
    let recorder = Recorder::read(paths)?;
    Ok(Self {
      history: {
        let checked_value = read_history(paths, at)?;
        checked_value
      },
      calendar,
      recorder_state: match recorder_state(recorder.as_ref(), at) {
        Ok(state) => state,
      },
      recorder,
      scope: Scope::Week,
      kind: Kind::Apps,
      page: Page::Overview,
      day,
      today_ticks,
      ticks,
      scroll: 0,
      more: Rows::Summary,
      focus: None,
      hover: None,
      details: None,
      problem: None,
    })
  }

  fn refresh(&mut self, paths: &Paths) -> Result<Refresh> {
    let Ok(at) = now();
    let recorder = Recorder::read(paths)?;
    let today = self.calendar.last().ok_or("calendar has no current day")?;
    let new_day = !(today.start..today.end).contains(&at);
    let Ok(state) = recorder_state(recorder.as_ref(), at);
    let changed = match bool::then_some(self.recorder_state != state, ()) {
      Some(()) => Refresh::Changed,
      None => Refresh::Unchanged,
    };
    self.recorder_state = state;

    match bool::then_some(
      !new_day && self.recorder == recorder && self.problem.is_none(),
      (),
    ) {
      Some(()) => {
        return Ok(changed);
      }
      None => {}
    }

    let history = read_history(paths, at)?;

    match bool::then_some(new_day, ()) {
      Some(()) => {
        let live_day = self.day == CURRENT_DAY;
        let Ok(previous) = index(self.day);
        let previous_day = self
          .calendar
          .get(previous)
          .ok_or("selected calendar day is unavailable")?;
        let iso = &previous_day.iso;
        let calendar = local_days(at)?;
        let selected = match bool::then_some(live_day, ()) {
          Some(()) => CURRENT_DAY,
          None => match calendar.iter().position(|day| &day.iso == iso) {
            Some(present) => {
              let Ok(day) = fitted::<_, u32>(present);
              day
            }
            None => 0,
          },
        };
        let today_2 = calendar.last().ok_or("calendar has no current day")?;
        let today_ticks = today_2.ticks((0..24).step_by(3))?;
        let Ok(position) = index(selected);
        let day = calendar
          .get(position)
          .ok_or("selected calendar day is unavailable")?;
        let ticks = timeline_ticks(day)?;
        self.calendar = calendar;
        self.day = selected;
        self.today_ticks = today_ticks;
        self.ticks = ticks;
        self.details = None;
      }
      None => {}
    }

    self.history = history;
    self.recorder = recorder;
    self.problem = None;
    Ok(Refresh::Changed)
  }

  fn select_day(&mut self, number: u32) -> Result<()> {
    let Ok(position) = index(number);
    let day = self
      .calendar
      .get(position)
      .ok_or("selected calendar day is unavailable")?;
    self.ticks = {
      let checked_value_2 = timeline_ticks(day)?;
      checked_value_2
    };
    self.page = Page::Timeline;
    self.day = number;
    self.details = None;
    self.scroll = 0;
    Ok(())
  }

  fn activate(&mut self, action: Action) -> Result<WindowChange> {
    match action {
      Action::Page(page) => {
        self.page = page;
        self.scroll = 0;
      }
      Action::Scope(scope) => {
        self.scope = scope;
        self.scroll = 0;
      }
      Action::Kind(kind) => {
        self.kind = kind;
        self.details = None;
        self.scroll = 0;
      }
      Action::Chart { day, .. } => self.select_day(day)?,
      Action::Step(by) => {
        let day = self.day.saturating_add_signed(by).min(CURRENT_DAY);

        match bool::then_some(day != self.day, ()) {
          Some(()) => {
            self.select_day(day)?;
          }
          None => {}
        }
      }
      Action::More => match self.more {
        Rows::Summary => self.more = Rows::Expanded,
        Rows::Expanded => {
          self.more = Rows::Summary;
          self.scroll = 0;
        }
      },
      Action::Segment { period, .. } => {
        let range = time_range(period)?;
        let Ok(seconds) = period.seconds();
        let Ok(time_spent) = duration(seconds);
        self.details = Some((period, format!("{range}  ·  {time_spent}")));
      }
      Action::Close => return Ok(WindowChange::Close),
    }

    self.hover = None;
    Ok(WindowChange::StayOpen)
  }

  fn keyboard(&mut self, key: Keysym, frame: &Frame) -> Result<Option<WindowChange>> {
    let action = match key {
      Keysym::Escape | Keysym::q => Some(Action::Close),
      Keysym::o => Some(Action::Page(Page::Overview)),
      Keysym::t => Some(Action::Page(Page::Timeline)),
      Keysym::_1 => Some(Action::Scope(Scope::Today)),
      Keysym::_7 => Some(Action::Scope(Scope::Week)),
      Keysym::_3 => Some(Action::Scope(Scope::Month)),
      Keysym::Left | Keysym::Right => match (key, self.page) {
        (Keysym::Left, Page::Timeline) => Some(Action::Step(-1)),
        (Keysym::Right, Page::Timeline) => Some(Action::Step(1)),
        _ => None,
      },
      Keysym::Return | Keysym::KP_Enter | Keysym::space => self.focus,
      Keysym::Tab | Keysym::ISO_Left_Tab => {
        let direction = match key {
          Keysym::ISO_Left_Tab => FocusDirection::Backward,
          _forward => FocusDirection::Forward,
        };
        let Ok(focus) = frame.tab(self.focus, direction);
        self.focus = focus;
        return Ok(Some(WindowChange::StayOpen));
      }
      Keysym::Down | Keysym::Page_Down | Keysym::Up | Keysym::Page_Up => {
        let by = match key {
          Keysym::Down => 45,
          Keysym::Up => -45,
          Keysym::Page_Down => 270,
          _ => -270,
        };
        self.scroll = i32::wrapping_add(self.scroll, by).clamp(0, frame.max_scroll);
        return Ok(Some(WindowChange::StayOpen));
      }
      _ => None,
    };

    match action {
      Some(requested) => self.activate(requested).map(Some),
      None => Ok(None),
    }
  }
}

fn recorder_state(
  recorder: Option<&Recorder>,
  at: u64,
) -> std::result::Result<RecorderState, Never> {
  let stale = recorder.is_some_and(|value| {
    at.saturating_sub(value.updated) > value.interval.wrapping_mul(2).wrapping_add(10)
  });
  Ok(match bool::then_some(stale, ()) {
    Some(()) => RecorderState::Stale,
    None => RecorderState::Current,
  })
}

fn timeline_ticks(day: &Day) -> Result<Vec<(u64, String)>> {
  day.ticks((0..24).step_by(4))
}

pub fn open(paths: &Paths) -> Result<()> {
  let app = Dashboard::new(paths)?;
  let art = Art::new()?;
  let mut surface = Surface::connect()?;
  surface.show_window_sized(
    &Window {
      app_id: String::from(APP_ID),
      title: String::from("Habits"),
    },
    WINDOW_SIZE,
  )?;
  let starting = WindowSession {
    app,
    art,
    surface,
    frame: Frame::default(),
    drawing: Drawing::Needed,
    last_refresh: Instant::now(),
    last_problem: None,
  };
  let outcome = iterate(starting, |mut window| {
    Ok(match window.turn(paths) {
      Ok(WindowChange::StayOpen) => Step::Again(window),
      Ok(WindowChange::Close) => Step::Halt(Ok(())),
      Err(fault) => Step::Halt(Err(fault)),
    })
  })?;
  outcome
}

#[derive(Clone, Copy, PartialEq)]
enum Drawing {
  Needed,
  Shown { room: Size<u32>, scale: Scale },
}

struct WindowSession {
  app: Dashboard,
  art: Art,
  surface: Surface,
  frame: Frame,
  drawing: Drawing,
  last_refresh: Instant,
  last_problem: Option<String>,
}

impl WindowSession {
  fn turn(&mut self, paths: &Paths) -> Result<WindowChange> {
    self.draw()?;
    self.surface.wait(&[], Some(Duration::from_secs(1)))?;

    let closed = self.surface.closed()?;

    match closed {
      Closed::Yes => return Ok(WindowChange::Close),
      Closed::No => {}
    }

    let presses = self.surface.key_presses()?;

    for press in presses {
      let action = self.app.keyboard(press.key, &self.frame)?;

      match action {
        Some(WindowChange::Close) => return Ok(WindowChange::Close),
        Some(WindowChange::StayOpen) => self.drawing = Drawing::Needed,
        None => {}
      }
    }

    let pointers = self.surface.pointer_events()?;

    for pointer in pointers {
      let action = self.pointer(pointer)?;

      match action {
        WindowChange::Close => return Ok(WindowChange::Close),
        WindowChange::StayOpen => {}
      }
    }

    let Ok(()) = self.refresh(paths);
    Ok(WindowChange::StayOpen)
  }

  fn draw(&mut self) -> Result<()> {
    let Ok(logical) = self.surface.logical();
    let room = match logical {
      Some(room) => room,
      None => return Ok(()),
    };
    let Ok(scale) = self.surface.scale();
    let current = Drawing::Shown { room, scale };

    match (current == self.drawing).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(next_frame) = scene::draw(&self.app, room, &self.art);
    self.frame = next_frame;

    match self.app.scroll.cmp(&self.frame.max_scroll) {
      Ordering::Greater => {
        self.app.scroll = self.frame.max_scroll;
        let Ok(scrolled) = scene::draw(&self.app, room, &self.art);
        self.frame = scrolled;
      }
      Ordering::Less | Ordering::Equal => {}
    }

    self.surface.draw(|pixels, device, _| {
      let frame_size = console_draw_painting::Frame {
        device,
        points: room,
      };

      match console_draw_painting::onto(pixels, frame_size, &self.frame.shapes) {
        Ok(()) => {}
        Err(fault) => console_core_logging::warn!("Habits window: {fault}"),
      }

      Ok(())
    })?;
    self.drawing = current;
    Ok(())
  }

  fn pointer(&mut self, event: PointerEvent) -> Result<WindowChange> {
    match event {
      PointerEvent::Down { at } => return self.press(at),
      PointerEvent::Moved { at } => {
        let Ok(hover) = self.frame.action(at);

        match (hover != self.app.hover).then_some(()) {
          Some(()) => self.drawing = Drawing::Needed,
          None => {}
        }

        self.app.hover = hover;
      }
      PointerEvent::Left => match self.app.hover.take() {
        Some(_) => self.drawing = Drawing::Needed,
        None => {}
      },
      PointerEvent::Scrolled { by } => {
        let position =
          (f64::from(self.app.scroll) + by).clamp(0.0, f64::from(self.frame.max_scroll));
        let Ok(scroll) = toward_zero_i32(position);

        match (scroll != self.app.scroll).then_some(()) {
          Some(()) => self.drawing = Drawing::Needed,
          None => {}
        }

        self.app.scroll = scroll;
      }
      PointerEvent::Up | PointerEvent::Pinched { .. } => {}
    }

    Ok(WindowChange::StayOpen)
  }

  fn press(&mut self, at: (f64, f64)) -> Result<WindowChange> {
    self.app.focus = None;
    let Ok(action) = self.frame.action(at);
    let change = match action {
      Some(requested) => self.app.activate(requested)?,
      None => {
        let Ok(titlebar) = scene::titlebar(at);

        match titlebar {
          Some(()) => self.surface.move_window()?,
          None => {}
        }

        WindowChange::StayOpen
      }
    };

    match change {
      WindowChange::StayOpen => self.drawing = Drawing::Needed,
      WindowChange::Close => {}
    }

    Ok(change)
  }

  fn refresh(&mut self, paths: &Paths) -> core::result::Result<(), Never> {
    match self.last_refresh.elapsed().cmp(&Duration::from_secs(1)) {
      Ordering::Less => return Ok(()),
      Ordering::Equal | Ordering::Greater => self.last_refresh = Instant::now(),
    }

    match self.app.refresh(paths) {
      Ok(change) => {
        match change {
          Refresh::Changed => self.drawing = Drawing::Needed,
          Refresh::Unchanged => {}
        }

        self.last_problem = None;
      }
      Err(fault) => {
        let message = fault.to_string();

        match (self.last_problem.as_ref() != Some(&message)).then_some(()) {
          Some(()) => {
            console_core_logging::warn!("Habits history: {message}");
            self.last_problem = Some(message);
            self.app.problem = Some("Could not refresh activity");
            self.drawing = Drawing::Needed;
          }
          None => {}
        }
      }
    }

    let Ok(change) = self.art.refresh();

    match change {
      Refresh::Changed => self.drawing = Drawing::Needed,
      Refresh::Unchanged => {}
    }

    Ok(())
  }
}
