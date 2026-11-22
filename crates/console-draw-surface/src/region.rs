use console_core_geometry::Size;
use console_core_never::Never;

use crate::Part;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fit {
  Inside,
  Outside,
}

pub(crate) fn fit(parent: Size<u32>, part: Part) -> Result<Fit, Never> {
  let Ok(horizontal) = AxisSpan {
    parent: parent.width,
    start: part.at.x,
    length: part.size.width,
  }
  .fit();
  let Ok(vertical) = AxisSpan {
    parent: parent.height,
    start: part.at.y,
    length: part.size.height,
  }
  .fit();

  Ok(match (horizontal, vertical) {
    (Fit::Inside, Fit::Inside) => Fit::Inside,
    (Fit::Outside, _) | (_, Fit::Outside) => Fit::Outside,
  })
}

#[derive(Clone, Copy)]
struct AxisSpan {
  parent: u32,
  start: i32,
  length: u32,
}

impl AxisSpan {
  fn fit(self) -> Result<Fit, Never> {
    Ok(match (u32::try_from(self.start), self.length) {
      (Ok(offset), 1..) => match self.length.cmp(&self.parent.saturating_sub(offset)) {
        std::cmp::Ordering::Greater => Fit::Outside,
        std::cmp::Ordering::Equal | std::cmp::Ordering::Less => Fit::Inside,
      },
      (Err(_negative_origin), _) => Fit::Outside,
      (Ok(_offset), 0) => Fit::Outside,
    })
  }
}

#[cfg(kani)]
#[path = "../proofs/region.rs"]
mod proofs;

#[cfg(test)]
mod tests {
  use super::{AxisSpan, Fit};

  #[test]
  fn an_extent_must_be_nonempty_and_wholly_inside_the_parent() {
    for (region, expected) in [
      (
        AxisSpan {
          parent: 20,
          start: 5,
          length: 15,
        },
        Fit::Inside,
      ),
      (
        AxisSpan {
          parent: 20,
          start: 5,
          length: 16,
        },
        Fit::Outside,
      ),
      (
        AxisSpan {
          parent: 20,
          start: -1,
          length: 1,
        },
        Fit::Outside,
      ),
      (
        AxisSpan {
          parent: 20,
          start: 5,
          length: 0,
        },
        Fit::Outside,
      ),
      (
        AxisSpan {
          parent: 20,
          start: 25,
          length: 1,
        },
        Fit::Outside,
      ),
      (
        AxisSpan {
          parent: u32::MAX,
          start: i32::MAX,
          length: u32::MAX,
        },
        Fit::Outside,
      ),
    ] {
      assert_eq!(region.fit(), Ok(expected));
    }
  }
}
