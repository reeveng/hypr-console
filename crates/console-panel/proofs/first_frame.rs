//! `cargo kani -p console-panel --lib --harness a_preview_can_draw_the_first_frame_while_rows_are_loading`.

use super::{Dirty, DrawFrame, RowsForFrame, Visible, draw_frame};

#[kani::proof]
fn a_preview_can_draw_the_first_frame_while_rows_are_loading() {
  let dirty = kani::any::<bool>();
  let painted = kani::any::<bool>();
  let loading = kani::any::<bool>();
  let cached = kani::any::<bool>();

  let dirt = match dirty {
    true => Dirty::Yes,
    false => Dirty::No,
  };
  let visibility = match painted {
    true => Visible::Yes,
    false => Visible::NotYet,
  };
  let rows = match (loading, cached) {
    (false, _) => RowsForFrame::Current,
    (true, true) => RowsForFrame::Preview,
    (true, false) => RowsForFrame::Pending,
  };
  let Ok(draw) = draw_frame(dirt, visibility, rows);

  assert_eq!(
    draw == DrawFrame::Now,
    dirty && (!loading || !painted && cached)
  );
  kani::cover!(dirty && !painted && loading && cached && draw == DrawFrame::Now);
}
