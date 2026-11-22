//! The first row of a panel can include a taller stage. The viewport reports
//! only rows intersecting its visible bounds, including that stage.

use std::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::fitted;

pub(crate) struct RowViewport {
  pub count: u64,
  pub height: i32,
  pub scroll: i32,
  pub stage: i32,
  pub row: i32,
}

#[must_use]
pub(crate) fn visible_rows(viewport: RowViewport) -> Result<Range<u64>, Never> {
  match (viewport.height <= 0 || viewport.row <= 0).then_some(()) {
    Some(()) => return Ok(0..0),
    None => {}
  }

  let start = i64::from(viewport.scroll);
  let end = start.saturating_add(i64::from(viewport.height));
  let stage = i64::from(viewport.stage.max(0));
  let row = i64::from(viewport.row);
  let regular_start = start.saturating_sub(stage).div_euclid(row).max(1);
  let regular_end = end
    .saturating_sub(stage)
    .saturating_sub(1)
    .div_euclid(row)
    .saturating_add(1)
    .max(1);
  let first = match (end > 0 && start < stage.saturating_add(row)).then_some(()) {
    Some(()) => 0,
    None => regular_start,
  };
  let Ok(first_row) = fitted::<_, u64>(first);
  let Ok(end_row) = fitted::<_, u64>(regular_end);
  let upper = end_row.min(viewport.count);

  Ok(first_row.min(upper)..upper)
}

#[cfg(kani)]
#[path = "../proofs/row_window.rs"]
mod proofs;
