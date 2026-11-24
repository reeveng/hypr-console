use super::{AxisSpan, Fit};

#[kani::proof]
fn a_layer_extent_is_nonempty_and_inside_its_parent_without_wrapping() {
  let parent: u32 = kani::any();
  let start: i32 = kani::any();
  let length: u32 = kani::any();
  let end = i64::from(start).saturating_add(i64::from(length));
  let region = AxisSpan {
    parent,
    start,
    length,
  };
  let Ok(fit) = region.fit();
  let inside = fit == Fit::Inside;
  assert_eq!(inside, start >= 0 && length > 0 && end <= i64::from(parent));
  kani::cover!(inside);
  kani::cover!(end > i64::from(u32::MAX));
}
