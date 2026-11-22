use super::{RowViewport, visible_rows};

#[kani::proof]
fn the_window_contains_exactly_the_intersecting_rows() {
  let count = u64::from(kani::any::<u32>());
  let index = u64::from(kani::any::<u32>());
  let height: i32 = kani::any();
  let scroll: i32 = kani::any();
  let stage: i32 = kani::any();
  let row: i32 = 52;
  kani::assume(stage >= 0);

  let Ok(window) = visible_rows(RowViewport {
    count,
    height,
    scroll,
    stage,
    row,
  });
  let viewport_start = i64::from(scroll);
  let viewport_end = viewport_start + i64::from(height);
  let row_start = match index {
    0 => 0,
    _ => i64::from(stage) + index as i64 * i64::from(row),
  };
  let row_end = i64::from(stage) + (index as i64 + 1) * i64::from(row);
  assert!(window.start <= window.end && window.end <= count);
  assert_eq!(
    window.contains(&index),
    index < count && height > 0 && row_start < viewport_end && row_end > viewport_start,
  );
}
