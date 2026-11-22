use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_paths::{IntRect, IntSize, Rect};
use console_core_size::Size;

pub type PixelPosition = console_core_point::Point<u32>;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ScreenIntRect(IntRect);

impl ScreenIntRect {
  pub fn new(position: PixelPosition, extent: Size<u32>) -> Result<Option<Self>, Never> {
    let (x, y) = match (i32::try_from(position.x), i32::try_from(position.y)) {
      (Ok(x), Ok(y)) => (x, y),
      (Err(_x_outside_the_coordinate_range), _) => return Ok(None),
      (_, Err(_y_outside_the_coordinate_range)) => return Ok(None),
    };
    let Ok(rect) = IntRect::from_xywh(x, y, extent.width, extent.height);

    Ok(rect.map(Self))
  }

  pub fn x(&self) -> Result<u32, Never> {
    let Ok(x) = self.0.x();

    fitted::<i32, u32>(x)
  }

  pub fn y(&self) -> Result<u32, Never> {
    let Ok(y) = self.0.y();

    fitted::<i32, u32>(y)
  }

  pub fn width(&self) -> Result<u32, Never> {
    self.0.width()
  }

  pub fn height(&self) -> Result<u32, Never> {
    self.0.height()
  }

  pub fn left(&self) -> Result<u32, Never> {
    self.x()
  }

  pub fn top(&self) -> Result<u32, Never> {
    self.y()
  }

  pub fn right(&self) -> Result<u32, Never> {
    let Ok(right) = self.0.right();

    fitted::<i32, u32>(right)
  }

  pub fn bottom(&self) -> Result<u32, Never> {
    let Ok(bottom) = self.0.bottom();

    fitted::<i32, u32>(bottom)
  }

  pub fn size(&self) -> Result<IntSize, Never> {
    self.0.size()
  }

  pub fn contains(&self, other: &Self) -> Result<crate::ContainsAnswer, Never> {
    let Ok(contains) = self.0.contains(&other.0);

    Ok(match contains {
      console_core_paths::ContainsAnswer::Yes => crate::ContainsAnswer::Yes,
      console_core_paths::ContainsAnswer::No => crate::ContainsAnswer::No,
    })
  }

  pub fn to_int_rect(self) -> Result<IntRect, Never> {
    Ok(self.0)
  }

  pub fn to_rect(self) -> Result<Rect, Never> {
    self.0.to_rect()
  }
}

pub trait IntSizeExt {
  fn at_origin(&self) -> Result<Option<ScreenIntRect>, Never>;
}

impl IntSizeExt for IntSize {
  fn at_origin(&self) -> Result<Option<ScreenIntRect>, Never> {
    let Ok(rect) = self.to_int_rect(0, 0);

    Ok(rect.map(ScreenIntRect))
  }
}

pub trait IntRectExt {
  fn to_screen_int_rect(&self) -> Option<ScreenIntRect>;
}

impl IntRectExt for IntRect {
  fn to_screen_int_rect(&self) -> Option<ScreenIntRect> {
    let Ok(x) = self.x();
    let Ok(y) = self.y();

    (x >= 0 && y >= 0).then_some(ScreenIntRect(*self))
  }
}

#[cfg(test)]
mod screen_int_rect_tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};

  #[test]
  fn tests() -> Result<(), TestFailure> {
    let Ok(limit) = fitted::<i32, u32>(i32::MAX);

    for [x, y, width, height] in [
      [0, 0, 0, 0],
      [0, 0, 1, 0],
      [0, 0, 0, 1],
      [0, 0, u32::MAX, u32::MAX],
      [0, 0, 1, u32::MAX],
      [0, 0, u32::MAX, 1],
      [u32::MAX, 0, 1, 1],
      [0, u32::MAX, 1, 1],
      [u32::MAX, u32::MAX, u32::MAX, u32::MAX],
      [limit, 0, 1, 1],
      [0, limit, 1, 1],
    ] {
      assert_eq!(
        ScreenIntRect::new(PixelPosition { x, y }, Size { width, height }),
        Ok(None)
      );
    }

    let Ok(made) = ScreenIntRect::new(
      PixelPosition { x: 1, y: 2 },
      Size {
        width: 3,
        height: 4,
      },
    );
    let r = made.required()?;
    assert_eq!(r.x(), Ok(1));
    assert_eq!(r.y(), Ok(2));
    assert_eq!(r.width(), Ok(3));
    assert_eq!(r.height(), Ok(4));
    assert_eq!(r.right(), Ok(4));
    assert_eq!(r.bottom(), Ok(6));

    Ok(())
  }
}

pub(crate) fn visible_rect(size: IntSize, rect: &IntRect) -> Result<Option<IntRect>, Never> {
  let Ok(bounds) = size.to_int_rect(0, 0);

  match bounds {
    Some(base) => base.intersect(rect),
    None => Ok(None),
  }
}

pub(crate) fn offset_in_rect(width: u32, rect: &IntRect) -> Result<u64, Never> {
  let Ok(left) = rect.left();
  let Ok(top) = rect.top();
  let Ok(column) = console_core_number_conversion::fitted::<_, u64>(left);
  let Ok(row) = console_core_number_conversion::fitted::<_, u64>(top);

  Ok(row.saturating_mul(u64::from(width)).saturating_add(column))
}
