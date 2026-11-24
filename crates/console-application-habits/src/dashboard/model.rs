//! The calendar ranges, totals, and uninterrupted runs shown in the window.

use std::collections::BTreeMap;

use console_core_never::Never;

use crate::activity::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
  Today,
  Week,
  Month,
}

impl Scope {
  pub const CHOICES: [(Self, &str); 3] = [
    (Self::Today, "Today"),
    (Self::Week, "7 days"),
    (Self::Month, "30 days"),
  ];

  pub fn days(self) -> Result<u32, Never> {
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
  pub const CHOICES: [(Self, &str); 2] = [(Self::Apps, "Apps"), (Self::Websites, "Websites")];

  fn label(self, span: &Span) -> Result<Option<&str>, Never> {
    Ok(match self {
      Self::Apps => Some(&span.activity.app),
      Self::Websites => span.activity.site.as_deref(),
    })
  }
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

  fn clip(self, span: &Span) -> Result<Self, Never> {
    Ok(Self {
      start: self.start.max(span.start),
      end: self.end.min(span.end),
    })
  }

  fn spans(self, history: &[Span]) -> Result<&[Span], Never> {
    let recent = match history.get(history.partition_point(|span| span.end <= self.start)..) {
      Some(recent) => recent,
      None => return Ok(&[]),
    };
    Ok(
      match recent.get(..recent.partition_point(|span| span.start < self.end)) {
        Some(spans) => spans,
        None => &[],
      },
    )
  }

  pub fn total(self, history: &[Span]) -> Result<u64, Never> {
    let Ok(spans) = self.spans(history);
    Ok(
      spans
        .iter()
        .map(|span| {
          let Ok(period) = self.clip(span);
          let Ok(seconds) = period.seconds();
          seconds
        })
        .fold(0, u64::wrapping_add),
    )
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

pub fn lanes(
  history: &[Span],
  range: Period,
  kind: Kind,
  view: LaneView,
) -> Result<Vec<Lane<'_>>, Never> {
  let mut found = BTreeMap::<&str, Lane>::new();
  let Ok(spans) = range.spans(history);

  for span in spans {
    let Ok(optional_label) = kind.label(span);
    let label = match optional_label {
      Some(label) => label,
      None => continue,
    };

    let Ok(period) = range.clip(span);
    let seconds = match period.seconds() {
      Ok(0) => continue,
      Ok(seconds) => seconds,
    };

    let lane = found.entry(label).or_insert_with(|| Lane {
      label,
      ..Lane::default()
    });
    lane.total = lane.total.wrapping_add(seconds);

    match view {
      LaneView::Overview => {}
      LaneView::Timeline => match lane.periods.last_mut().filter(|last| last.end == period.start) {
        Some(last) => last.end = period.end,
        None => lane.periods.push(period),
      },
    }
  }

  let mut result: Vec<_> = found.into_values().collect();
  result.sort_by(|left, right| {
    right
      .total
      .cmp(&left.total)
      .then_with(|| left.label.cmp(right.label))
  });
  Ok(result)
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::activity::Activity;

  #[test]
  fn clips_midnight_and_keeps_gaps_while_merging_tab_switches_in_an_app_lane() {
    let span = |start, end, site: &str| Span {
      start,
      end,
      activity: Activity {
        app: String::from("Browser"),
        site: Some(String::from(site)),
      },
    };
    let history = [
      span(10, 20, "alpha"),
      span(20, 30, "beta"),
      span(40, 50, "alpha"),
    ];
    let range = Period { start: 15, end: 45 };
    assert_eq!(range.total(&history), Ok(20));
    let Ok(apps) = lanes(&history, range, Kind::Apps, LaneView::Timeline);
    assert_eq!(
      apps.first().map(|lane| lane.periods.as_slice()),
      Some([Period { start: 15, end: 30 }, Period { start: 40, end: 45 }].as_slice())
    );
    let Ok(websites) = lanes(&history, range, Kind::Websites, LaneView::Timeline);
    assert_eq!(websites.len(), 2);
  }
}
