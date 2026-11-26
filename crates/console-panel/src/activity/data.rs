//! Data and actions for the activity dashboard.

use console_core_never::Never;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Page {
  Overview,
  Timeline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
  Today,
  Week,
  Month,
}

impl Scope {
  pub(super) const CHOICES: [(Self, &str); 3] = [
    (Self::Today, "Today"),
    (Self::Week, "7 days"),
    (Self::Month, "30 days"),
  ];

  pub(super) fn days(self) -> Result<u32, Never> {
    Ok(match self {
      Self::Today => 1,
      Self::Week => 7,
      Self::Month => 30,
    })
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
  Apps,
  Websites,
}

impl Kind {
  pub(super) const CHOICES: [(Self, &str); 2] =
    [(Self::Apps, "Apps"), (Self::Websites, "Websites")];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Period {
  pub start: u64,
  pub end: u64,
}

impl Period {
  pub fn seconds(self) -> Result<u64, Never> {
    Ok(self.end.saturating_sub(self.start))
  }
}

#[derive(Clone, Copy)]
pub struct Day<'a> {
  pub period: Period,
  pub weekday: &'a str,
  pub month: &'a str,
  pub number: &'a str,
  pub title: &'a str,
}

impl Day<'_> {
  pub(super) fn short(self) -> Result<String, Never> {
    Ok(format!("{} {}", self.month, self.number))
  }
}

#[derive(Default)]
pub struct Lane<'a> {
  pub label: &'a str,
  pub total: u64,
  pub periods: Vec<Period>,
}

pub enum LaneView {
  Overview,
  Timeline,
}

pub trait Source {
  fn day(&self, position: u32) -> Result<Option<Day<'_>>, Never>;

  fn total(&self, period: Period) -> Result<u64, Never>;

  fn lanes(&self, period: Period, kind: Kind, view: LaneView) -> Result<Vec<Lane<'_>>, Never>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
  Page(Page),
  PageArrow(Page),
  Scope(Scope),
  Kind(Kind),
  Chart { day: u32, slot: u32 },
  Step(i32),
  More,
  Segment { lane: u32, period: Period },
  Close,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Refresh {
  Unchanged,
  Changed,
}

#[derive(Clone, Copy)]
pub enum Rows {
  Summary,
  Expanded,
}

#[derive(Clone, Copy)]
pub enum Recorder<'a> {
  Current { status: &'a str, interval: u64 },
  Stale { interval: u64 },
  Waiting { interval: u64 },
}

pub struct View<'a> {
  pub title: &'a str,
  pub source: &'a dyn Source,
  pub recorder: Recorder<'a>,
  pub day: u32,
  pub current_day: u32,
  pub today_ticks: &'a [(u64, String)],
  pub ticks: &'a [(u64, String)],
  pub scope: Scope,
  pub kind: Kind,
  pub page: Page,
  pub scroll: i32,
  pub more: Rows,
  pub focus: Option<Action>,
  pub hover: Option<Action>,
  pub details: Option<&'a (Period, String)>,
  pub problem: Option<&'a str>,
}

pub fn duration(seconds: u64) -> Result<String, Never> {
  match seconds {
    0 => return Ok("0m".to_owned()),
    _nonzero => {}
  }

  let minutes = u64::wrapping_div(seconds, 60);

  Ok(match minutes {
    0 => format!("{seconds}s"),
    1..=59 => format!("{minutes}m"),
    _hours => format!(
      "{}h {:02}m",
      u64::wrapping_div(minutes, 60),
      u64::wrapping_rem(minutes, 60)
    ),
  })
}
