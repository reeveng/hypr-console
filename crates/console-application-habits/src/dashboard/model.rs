//! The calendar ranges, totals, and uninterrupted runs shown in the window.

use std::collections::BTreeMap;

use console_core_never::Never;
use console_panel::activity::{Kind, Lane, LaneView, Period};

use crate::activity::Span;

fn clip(period: Period, span: &Span) -> Result<Period, Never> {
  Ok(Period {
    start: period.start.max(span.start),
    end: period.end.min(span.end),
  })
}

fn spans(period: Period, history: &[Span]) -> Result<&[Span], Never> {
  let recent = match history.get(history.partition_point(|span| span.end <= period.start)..) {
    Some(recent) => recent,
    None => return Ok(&[]),
  };
  Ok(
    match recent.get(..recent.partition_point(|span| span.start < period.end)) {
      Some(spans) => spans,
      None => &[],
    },
  )
}

pub fn total(period: Period, history: &[Span]) -> Result<u64, Never> {
  let Ok(in_range) = spans(period, history);
  Ok(
    in_range
      .iter()
      .map(|span| {
        let Ok(clipped) = clip(period, span);
        let Ok(seconds) = clipped.seconds();
        seconds
      })
      .fold(0, u64::wrapping_add),
  )
}

pub fn lanes(
  history: &[Span],
  range: Period,
  kind: Kind,
  view: LaneView,
) -> Result<Vec<Lane<'_>>, Never> {
  let mut found = BTreeMap::<&str, Lane>::new();
  let Ok(in_range) = spans(range, history);

  for span in in_range {
    let label = match kind {
      Kind::Apps => span.activity.app.as_str(),
      Kind::Websites => match span.activity.site.as_deref() {
        Some(site) => site,
        None => continue,
      },
    };

    let Ok(period) = clip(range, span);
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
      LaneView::Timeline => match lane
        .periods
        .last_mut()
        .filter(|last| last.end == period.start)
      {
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
    assert_eq!(total(range, &history), Ok(20));
    let Ok(apps) = lanes(&history, range, Kind::Apps, LaneView::Timeline);
    assert_eq!(
      apps.first().map(|lane| lane.periods.as_slice()),
      Some([Period { start: 15, end: 30 }, Period { start: 40, end: 45 }].as_slice())
    );
    let Ok(websites) = lanes(&history, range, Kind::Websites, LaneView::Timeline);
    assert_eq!(websites.len(), 2);
  }
}
