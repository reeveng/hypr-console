use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::num::NonZeroU16;
use core::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::color::AlphaU8;

pub type AlphaRun = Option<NonZeroU16>;

pub struct AlphaRuns {
  pub runs: Vec<AlphaRun>,
  pub alpha: Vec<AlphaU8>,
  width: NonZeroU16,
}

pub struct Coverage {
  pub first: AlphaU8,
  pub interior: u32,
  pub full_alpha: AlphaU8,
  pub last: AlphaU8,
}

impl AlphaRuns {
  pub fn new(width: NonZeroU16) -> Result<Self, Never> {
    let Ok(needed) = index(u32::from(width.get()).saturating_add(1));
    let mut line = Self {
      runs: vec![None; needed.min(65_536)],
      alpha: vec![0; needed.min(65_536)],
      width,
    };
    let Ok(()) = line.reset();

    Ok(line)
  }

  pub fn is_empty(&self) -> Result<crate::Emptiness, Never> {
    Ok(match (self.runs.first().copied().flatten(), self.alpha.first()) {
      (Some(run), Some(0)) => {
        let Ok(end) = index(run.get());

        match self.runs.get(end) {
          Some(None) => crate::Emptiness::Empty,
          Some(Some(_)) | None => crate::Emptiness::NonEmpty,
        }
      }
      (Some(_), Some(_)) => crate::Emptiness::NonEmpty,
      (None, _) | (_, None) => crate::Emptiness::Empty,
    })
  }

  pub fn reset(&mut self) -> Result<(), Never> {
    match (self.runs.first_mut(), self.alpha.first_mut()) {
      (Some(first_run), Some(first_alpha)) => {
        *first_run = Some(self.width);
        *first_alpha = 0;
      }
      (None, _) | (_, None) => {}
    }

    match self.runs.last_mut() {
      Some(last) => *last = None,
      None => {}
    }

    Ok(())
  }

  pub fn add(&mut self, x: u32, offset: RunOffset, coverage: Coverage) -> Result<RunOffset, Never> {
    let width = u32::from(self.width.get());
    let from = match offset.0.cmp(&x) {
      Ordering::Less | Ordering::Equal => offset.0,
      Ordering::Greater => 0,
    };
    let first_count = u32::from(coverage.first != 0);
    let last_count = u32::from(coverage.last != 0);
    let interior_from = x.saturating_add(first_count);
    let interior_to = interior_from.saturating_add(coverage.interior);

    match interior_to.saturating_add(last_count).cmp(&width) {
      Ordering::Less | Ordering::Equal => {}
      Ordering::Greater => return Ok(offset),
    }

    let mut row = RunLine {
      alpha: &mut self.alpha,
      runs: &mut self.runs,
    };

    let Ok(()) = row.split(from..x, Cut::AtRun);

    match coverage.first {
      0 => {}
      first => {
        let Ok(()) = row.split(x..interior_from, Cut::AtRun);
        let Ok(first_index) = index(x);

        match row.alpha.get_mut(first_index) {
          Some(alpha) => *alpha = alpha.saturating_add(first),
          None => return Ok(offset),
        }
      }
    }

    let mut last = offset;

    match coverage.interior {
      0 => {}
      _ => {
        let Ok(()) = row.split(interior_from..interior_to, Cut::AtRun);
        let Ok(()) = row.cover(interior_from..interior_to, coverage.full_alpha);
        last = RunOffset(interior_to);
      }
    }

    match coverage.last {
      0 => {}
      alpha => {
        let Ok(()) = row.split(interior_to..interior_to.saturating_add(1), Cut::AtRun);
        let Ok(stop) = index(interior_to);

        match row.alpha.get_mut(stop) {
          Some(found) => *found = found.wrapping_add(alpha),
          None => return Ok(last),
        }

        last = RunOffset(interior_to);
      }
    }

    Ok(last)
  }

  pub fn break_at(alpha: &mut [AlphaU8], runs: &mut [AlphaRun], x: u32) -> Result<(), Never> {
    RunLine { alpha, runs }.split(0..x, Cut::AtOrigin)
  }
}

#[derive(Clone, Copy, Default)]
pub struct RunOffset(u32);

struct RunLine<'a> {
  alpha: &'a mut [AlphaU8],
  runs: &'a mut [AlphaRun],
}

enum Cut {
  AtRun,
  AtOrigin,
}

struct LocatedRun {
  start: u32,
  length: NonZeroU16,
}

impl RunLine<'_> {
  fn at(&self, position: u32) -> Result<Option<LocatedRun>, Never> {
    let Ok(position_index) = index(position);

    Ok(self
      .runs
      .get(position_index)
      .copied()
      .flatten()
      .map(|length| LocatedRun {
        start: position,
        length,
      }))
  }

  fn covering(&self, cut: Range<u32>) -> Result<Option<LocatedRun>, Never> {
    let mut start = cut.start;

    for _ in &*self.runs {
      let Ok(found) = self.at(start);
      let run = match found {
        Some(run) => run,
        None => return Ok(None),
      };
      let end = start.saturating_add(u32::from(run.length.get()));

      match cut.end.cmp(&end) {
        Ordering::Less => return Ok(Some(run)),
        Ordering::Equal | Ordering::Greater => start = end,
      }
    }

    Ok(None)
  }

  fn split(&mut self, at: Range<u32>, cut: Cut) -> Result<(), Never> {
    let destination = at.end;
    let Ok(found) = self.covering(at);
    let run = match found {
      Some(run) => run,
      None => return Ok(()),
    };
    let before = destination.saturating_sub(run.start);

    match before {
      0 => return Ok(()),
      _ => {}
    }

    let Ok(before_2) = console_core_number_conversion::fitted::<u32, u16>(before);
    let after = run.length.get().saturating_sub(before_2);
    let Ok(source) = index(run.start);
    let Ok(write_at) = index(destination);
    let Ok(split_start) = index(match cut {
      Cut::AtRun => run.start,
      Cut::AtOrigin => 0,
    });
    let Ok(split_end) = index(match cut {
      Cut::AtRun => destination,
      Cut::AtOrigin => before,
    });

    match self.alpha.get(source).copied() {
      Some(value) => match (self.alpha.get_mut(write_at), self.runs.get_disjoint_mut([split_start, split_end])) {
        (Some(alpha), Ok([first, second])) => {
          *alpha = value;
          *first = NonZeroU16::new(before_2);
          *second = NonZeroU16::new(after);
        }
        (None, Ok(_runs)) => {}
        (_, Err(_cut_outside_the_available_runs)) => {}
      },
      None => {}
    }

    Ok(())
  }

  fn cover(&mut self, columns: Range<u32>, coverage: AlphaU8) -> Result<(), Never> {
    let mut start = columns.start;

    for _ in &*self.runs {
      match start.cmp(&columns.end) {
        Ordering::Less => {}
        Ordering::Equal | Ordering::Greater => break,
      }

      let Ok(found) = self.at(start);
      let run = match found {
        Some(run) => run,
        None => break,
      };
      let Ok(position) = index(start);

      match self.alpha.get_mut(position) {
        Some(alpha) => *alpha = alpha.saturating_add(coverage),
        None => break,
      }

      start = start.saturating_add(u32::from(run.length.get()));
    }

    Ok(())
  }
}
