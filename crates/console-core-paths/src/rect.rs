use crate::StrictNum;
use console_core_rectangle::Bounds;
use core::convert::TryFrom;

use crate::{FiniteF32, IntSize, LengthU32, Point, SaturateRound, Size, Transform};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use crate::NoStdFloat;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct IntRect {
  x: i32,
  y: i32,
  width: LengthU32,
  height: LengthU32,
}

impl IntRect {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rectangle constructor takes x, y, width and height in that order"
    )
  )]
  pub fn from_xywh(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    match x.checked_add(
      match match i32::try_from(width) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      } {
        Some(value) => value,
        None => return Ok(None),
      },
    ) {
      Some(value) => value,
      None => return Ok(None),
    };

    match y.checked_add(
      match match i32::try_from(height) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      } {
        Some(value) => value,
        None => return Ok(None),
      },
    ) {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(Some(IntRect {
      x,
      y,
      width: match LengthU32::new(width) {
        Some(value) => value,
        None => return Ok(None),
      },
      height: match LengthU32::new(height) {
        Some(value) => value,
        None => return Ok(None),
      },
    }))
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rectangle constructor takes left, top, right and bottom in that order"
    )
  )]
  pub fn from_ltrb(
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let width = match match u32::try_from(match right.checked_sub(left) {
      Some(value) => value,
      None => return Ok(None),
    }) {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    } {
      Some(value) => value,
      None => return Ok(None),
    };
    let height = match match u32::try_from(match bottom.checked_sub(top) {
      Some(value) => value,
      None => return Ok(None),
    }) {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    } {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(int_rect) = IntRect::from_xywh(left, top, width, height);

    Ok(int_rect)
  }

  pub fn x(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(self.x)
  }

  pub fn y(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(self.y)
  }

  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.width.get())
  }

  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.height.get())
  }

  pub fn left(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(self.x)
  }

  pub fn top(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(self.y)
  }

  pub fn right(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(i32::wrapping_add(self.x, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(self.width.get());
      numeric_value
    }))
  }

  pub fn bottom(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(i32::wrapping_add(self.y, {
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped::<_, i32>(self.height.get());
      numeric_value
    }))
  }

  pub fn size(&self) -> core::result::Result<IntSize, console_core_never::Never> {
    let Ok(int_size) = IntSize::from_wh_safe(self.width, self.height);

    Ok(int_size)
  }

  pub fn contains(
    &self,
    other: &Self,
  ) -> core::result::Result<crate::ContainsAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self.x <= other.x
          && self.y <= other.y
          && ({
            let Ok(explicit_value) = self.right();
            explicit_value
          }) >= ({
            let Ok(explicit_value) = other.right();
            explicit_value
          })
          && ({
            let Ok(explicit_value) = self.bottom();
            explicit_value
          }) >= ({
            let Ok(explicit_value) = other.bottom();
            explicit_value
          }),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::ContainsAnswer::Yes,
        ::core::option::Option::None => crate::ContainsAnswer::No,
      },
    )
  }

  pub fn intersect(
    &self,
    other: &Self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let left = self.x.max(other.x);
    let top = self.y.max(other.y);

    let Ok(own_right) = self.right();
    let Ok(other_right) = other.right();
    let right = own_right.min(other_right);
    let Ok(own_bottom) = self.bottom();
    let Ok(other_bottom) = other.bottom();
    let bottom = own_bottom.min(other_bottom);

    let w = match match u32::try_from(match right.checked_sub(left) {
      Some(value) => value,
      None => return Ok(None),
    }) {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    } {
      Some(value) => value,
      None => return Ok(None),
    };
    let h = match match u32::try_from(match bottom.checked_sub(top) {
      Some(value) => value,
      None => return Ok(None),
    }) {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    } {
      Some(value) => value,
      None => return Ok(None),
    };

    let Ok(int_rect) = IntRect::from_xywh(left, top, w, h);

    Ok(int_rect)
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public inset takes the horizontal then the vertical extent"
    )
  )]
  pub fn inset(
    &self,
    dx: i32,
    dy: i32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(left) = self.left();
    let Ok(int_rect) = IntRect::from_ltrb(
      i32::wrapping_add(left, dx),
      i32::wrapping_add(
        {
          let Ok(explicit_value) = self.top();
          explicit_value
        },
        dy,
      ),
      i32::wrapping_sub(
        {
          let Ok(explicit_value) = self.right();
          explicit_value
        },
        dx,
      ),
      i32::wrapping_sub(
        {
          let Ok(explicit_value) = self.bottom();
          explicit_value
        },
        dy,
      ),
    );

    Ok(int_rect)
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public outset takes the horizontal then the vertical pixel extent"
    )
  )]
  pub fn make_outset(
    &self,
    dx: i32,
    dy: i32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(left) = self.left();
    let Ok(int_rect) = IntRect::from_ltrb(
      left.saturating_sub(dx),
      ({
        let Ok(explicit_value) = self.top();
        explicit_value
      })
      .saturating_sub(dy),
      ({
        let Ok(explicit_value) = self.right();
        explicit_value
      })
      .saturating_add(dx),
      ({
        let Ok(explicit_value) = self.bottom();
        explicit_value
      })
      .saturating_add(dy),
    );

    Ok(int_rect)
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public translation takes the horizontal then the vertical offset"
    )
  )]
  pub fn translate(
    &self,
    tx: i32,
    ty: i32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(int_rect) = IntRect::from_xywh(
      i32::wrapping_add(
        {
          let Ok(explicit_value) = self.x();
          explicit_value
        },
        tx,
      ),
      i32::wrapping_add(
        {
          let Ok(explicit_value) = self.y();
          explicit_value
        },
        ty,
      ),
      {
        let Ok(explicit_value) = self.width();
        explicit_value
      },
      {
        let Ok(explicit_value) = self.height();
        explicit_value
      },
    );

    Ok(int_rect)
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public position takes x then y"
    )
  )]
  pub fn translate_to(
    &self,
    x: i32,
    y: i32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(int_rect) = IntRect::from_xywh(x, y, width, height);

    Ok(int_rect)
  }

  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "Increasing signed 32-bit bounds remain ordered and finite in f32, and Rect permits bounds that round to the same coordinate"
    )
  )]
  pub fn to_rect(&self) -> core::result::Result<Rect, console_core_never::Never> {
    let Ok(rect) = Rect::from_ltrb(
      match ::console_core_number_conversion::nearest_f32_from_i64(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(self.x),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      match ::console_core_number_conversion::nearest_f32_from_i64(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(self.y),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      (match ::console_core_number_conversion::nearest_f32_from_i64(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(self.x),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      }) + match ::console_core_number_conversion::nearest_f32_from_u64(
        <::core::primitive::u64 as ::core::convert::From<::core::primitive::u32>>::from(
          self.width.get(),
        ),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      (match ::console_core_number_conversion::nearest_f32_from_i64(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(self.y),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      }) + match ::console_core_number_conversion::nearest_f32_from_u64(
        <::core::primitive::u64 as ::core::convert::From<::core::primitive::u32>>::from(
          self.height.get(),
        ),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
    );

    Ok(rect.unwrap())
  }
}

#[cfg(test)]
mod int_rect_tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};

  #[test]
  fn tests() -> Result<(), TestFailure> {
    assert_eq!(IntRect::from_xywh(0, 0, 0, 0), Ok(None));
    assert_eq!(IntRect::from_xywh(0, 0, 1, 0), Ok(None));
    assert_eq!(IntRect::from_xywh(0, 0, 0, 1), Ok(None));

    assert_eq!(IntRect::from_xywh(0, 0, u32::MAX, u32::MAX), Ok(None));
    assert_eq!(IntRect::from_xywh(0, 0, 1, u32::MAX), Ok(None));
    assert_eq!(IntRect::from_xywh(0, 0, u32::MAX, 1), Ok(None));

    assert_eq!(IntRect::from_xywh(i32::MAX, 0, 1, 1), Ok(None));
    assert_eq!(IntRect::from_xywh(0, i32::MAX, 1, 1), Ok(None));

    let Ok(made_apart) = IntRect::from_xywh(1, 2, 3, 4);
    let apart = made_apart.required()?;
    let Ok(made_far) = IntRect::from_xywh(11, 12, 13, 14);
    let far = made_far.required()?;
    assert_eq!(apart.intersect(&far), Ok(None));

    let Ok(made_around) = IntRect::from_xywh(1, 2, 30, 40);
    let around = made_around.required()?;
    assert_eq!(around.intersect(&far), IntRect::from_xywh(11, 12, 13, 14));

    let Ok(made_overlapping) = IntRect::from_xywh(11, 12, 50, 60);
    let overlapping = made_overlapping.required()?;
    assert_eq!(
      around.intersect(&overlapping),
      IntRect::from_xywh(11, 12, 20, 30)
    );

    Ok(())
  }
}

#[derive(Copy, Clone, PartialEq, Hash, Eq)]
pub struct Rect {
  bounds: Bounds<FiniteF32>,
}

impl core::fmt::Debug for Rect {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Rect")
      .field("left", &self.bounds.x0.get())
      .field("top", &self.bounds.y0.get())
      .field("right", &self.bounds.x1.get())
      .field("bottom", &self.bounds.y1.get())
      .finish()
  }
}

impl Rect {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rectangle constructor takes left, top, right and bottom in that order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "Floating rectangle bounds are in path coordinate units"
    )
  )]
  pub fn from_ltrb(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let left_2 = match FiniteF32::new(left) {
      Some(value) => value,
      None => return Ok(None),
    };
    let top_2 = match FiniteF32::new(top) {
      Some(value) => value,
      None => return Ok(None),
    };
    let right_2 = match FiniteF32::new(right) {
      Some(value) => value,
      None => return Ok(None),
    };
    let bottom_2 = match FiniteF32::new(bottom) {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(
      match ::core::primitive::bool::then_some(
        left_2.get() <= right_2.get() && top_2.get() <= bottom_2.get(),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(width) = checked_f32_sub(self::CheckedF32SubArguments {
            end: right_2,
            start: left_2,
          });
          let Ok(height) = checked_f32_sub(self::CheckedF32SubArguments {
            end: bottom_2,
            start: top_2,
          });

          match width {
            Some(_finite) => {}
            None => return Ok(None),
          };

          match height {
            Some(_finite) => {}
            None => return Ok(None),
          };

          Some(Rect {
            bounds: Bounds {
              x0: left_2,
              y0: top_2,
              x1: right_2,
              y1: bottom_2,
            },
          })
        }
        ::core::option::Option::None => None,
      },
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rectangle constructor takes x, y, width and height in that order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "Floating rectangle dimensions are in path coordinate units"
    )
  )]
  pub fn from_xywh(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(rect) = Rect::from_ltrb(x, y, w + x, h + y);

    Ok(rect)
  }

  pub fn left(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.x0.get())
  }

  pub fn top(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.y0.get())
  }

  pub fn right(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.x1.get())
  }

  pub fn bottom(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.y1.get())
  }

  pub fn x(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.x0.get())
  }

  pub fn y(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.y0.get())
  }

  #[inline]
  pub fn width(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.x1.get() - self.bounds.x0.get())
  }

  #[inline]
  pub fn height(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.y1.get() - self.bounds.y0.get())
  }

  #[inline]
  pub fn is_empty(&self) -> core::result::Result<crate::Emptiness, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self.bounds.x0 == self.bounds.x1 || self.bounds.y0 == self.bounds.y1,
        (),
      ) {
        ::core::option::Option::Some(()) => crate::Emptiness::Empty,
        ::core::option::Option::None => crate::Emptiness::NonEmpty,
      },
    )
  }

  pub fn round(&self) -> core::result::Result<Option<IntRect>, console_core_never::Never> {
    let Ok(own_left) = self.left();
    let left = i32::saturate_round(own_left);
    let Ok(own_top) = self.top();
    let top = i32::saturate_round(own_top);
    let Ok(own_right) = self.right();
    let right = i32::saturate_round(own_right);
    let Ok(own_bottom) = self.bottom();
    let bottom = i32::saturate_round(own_bottom);
    let Ok(int_rect) = IntRect::from_xywh(
      left,
      top,
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
          core::cmp::max(1, right.saturating_sub(left)),
        );
        numeric_value
      },
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
          core::cmp::max(1, bottom.saturating_sub(top)),
        );
        numeric_value
      },
    );

    Ok(int_rect)
  }

  pub fn round_out(&self) -> core::result::Result<Option<IntRect>, console_core_never::Never> {
    let Ok(own_left) = self.left();
    let left = i32::saturate_floor(own_left);
    let Ok(own_top) = self.top();
    let top = i32::saturate_floor(own_top);
    let Ok(own_right) = self.right();
    let right = i32::saturate_ceil(own_right);
    let Ok(own_bottom) = self.bottom();
    let bottom = i32::saturate_ceil(own_bottom);
    let Ok(int_rect) = IntRect::from_xywh(
      left,
      top,
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
          core::cmp::max(1, right.saturating_sub(left)),
        );
        numeric_value
      },
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
          core::cmp::max(1, bottom.saturating_sub(top)),
        );
        numeric_value
      },
    );

    Ok(int_rect)
  }

  pub fn intersect(
    &self,
    other: &Self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let left = ({
      let Ok(explicit_value) = self.x();
      explicit_value
    })
    .max({
      let Ok(explicit_value) = other.x();
      explicit_value
    });
    let top = ({
      let Ok(explicit_value) = self.y();
      explicit_value
    })
    .max({
      let Ok(explicit_value) = other.y();
      explicit_value
    });

    let Ok(own_right) = self.right();
    let Ok(other_right) = other.right();
    let right = own_right.min(other_right);
    let Ok(own_bottom) = self.bottom();
    let Ok(other_bottom) = other.bottom();
    let bottom = own_bottom.min(other_bottom);

    let Ok(rect) = Rect::from_ltrb(left, top, right, bottom);

    Ok(rect)
  }

  pub fn join(
    &self,
    other: &Self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(is_empty) = other.is_empty();

    match is_empty {
      crate::Emptiness::Empty => {
        return Ok(Some(*self));
      }
      crate::Emptiness::NonEmpty => {}
    }

    let Ok(own_is_empty) = self.is_empty();

    match own_is_empty {
      crate::Emptiness::Empty => {
        return Ok(Some(*other));
      }
      crate::Emptiness::NonEmpty => {}
    }

    let left = ({
      let Ok(explicit_value) = self.x();
      explicit_value
    })
    .min({
      let Ok(explicit_value) = other.x();
      explicit_value
    });
    let top = ({
      let Ok(explicit_value) = self.y();
      explicit_value
    })
    .min({
      let Ok(explicit_value) = other.y();
      explicit_value
    });

    let Ok(own_right) = self.right();
    let Ok(other_right) = other.right();
    let right = own_right.max(other_right);
    let Ok(own_bottom) = self.bottom();
    let Ok(other_bottom) = other.bottom();
    let bottom = own_bottom.max(other_bottom);

    let Ok(rect) = Rect::from_ltrb(left, top, right, bottom);

    Ok(rect)
  }

  pub fn from_points(
    points: &[Point],
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    use crate::f32x4_t::f32x4;

    let (initial, paired) = match points {
      [] => return Ok(None),
      [Point { x, y }] => return Self::from_xywh(*x, *y, 0.0, 0.0),
      [first, second] => {
        let (left, right) = match ::core::primitive::bool::then_some(first.x < second.x, ()) {
          ::core::option::Option::Some(()) => (first.x, second.x),
          ::core::option::Option::None => (second.x, first.x),
        };
        let (top, bottom) = match ::core::primitive::bool::then_some(first.y < second.y, ()) {
          ::core::option::Option::Some(()) => (first.y, second.y),
          ::core::option::Option::None => (second.y, first.y),
        };

        return Self::from_ltrb(left, top, right, bottom);
      }
      [first, second, rest @ ..] => match points.chunks_exact(2).remainder() {
        [] => (f32x4([first.x, first.y, second.x, second.y]), rest),
        _ => {
          let remaining = match points.split_first() {
            Some((_, remaining)) => remaining,
            None => return Ok(None),
          };

          (f32x4([first.x, first.y, first.x, first.y]), remaining)
        }
      },
    };
    let mut min = initial;
    let mut max = initial;
    let mut accum = f32x4::default();

    for [first, second] in paired.as_chunks::<2>().0 {
      let xy = f32x4([first.x, first.y, second.x, second.y]);
      accum *= xy;
      let Ok(minimum) = min.min(xy);
      let Ok(maximum) = max.max(xy);
      min = minimum;
      max = maximum;
    }

    match ::core::primitive::bool::then_some(accum * f32x4::default() != f32x4::default(), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        },
        ::core::option::Option::None => {},
    }

    let [min_x0, min_y0, min_x1, min_y1] = min.0;
    let [max_x0, max_y0, max_x1, max_y1] = max.0;

    Self::from_ltrb(
      min_x0.min(min_x1),
      min_y0.min(min_y1),
      max_x0.max(max_x1),
      max_y0.max(max_y1),
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public inset takes the horizontal then the vertical extent"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The inset extents are in path coordinate units"
    )
  )]
  pub fn inset(
    &self,
    dx: f32,
    dy: f32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(left) = self.left();
    let Ok(top) = self.top();
    let Ok(right) = self.right();
    let Ok(bottom) = self.bottom();
    let Ok(rect) = Rect::from_ltrb(left + dx, top + dy, right - dx, bottom - dy);

    Ok(rect)
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public outset takes the horizontal then the vertical extent"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The outset extents are in path coordinate units"
    )
  )]
  pub fn outset(
    &self,
    dx: f32,
    dy: f32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(inset) = self.inset(-dx, -dy);

    Ok(inset)
  }

  pub fn transform(
    &self,
    ts: Transform,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(is_identity) = ts.is_identity();

    Ok(match is_identity {
      crate::IsIdentityAnswer::Yes => Some(*self),
      crate::IsIdentityAnswer::No => {
        let Ok(has_skew) = ts.has_skew();

        match has_skew {
          crate::HasSkewAnswer::Yes => {
            let Ok(left) = self.left();
            let Ok(top) = self.top();
            let Ok(lt) = Point::from_xy(left, top);
            let Ok(right) = self.right();
            let Ok(own_top) = self.top();
            let Ok(rt) = Point::from_xy(right, own_top);
            let Ok(own_left) = self.left();
            let Ok(bottom) = self.bottom();
            let Ok(lb) = Point::from_xy(own_left, bottom);
            let Ok(own_right) = self.right();
            let Ok(own_bottom) = self.bottom();
            let Ok(rb) = Point::from_xy(own_right, own_bottom);
            let mut pts = [lt, rt, lb, rb];
            let Ok(()) = ts.map_points(&mut pts);
            let Ok(rect) = Self::from_points(&pts);

            rect
          }
          crate::HasSkewAnswer::No => {
            let Ok(left) = self.left();
            let Ok(top) = self.top();
            let Ok(lt) = Point::from_xy(left, top);
            let Ok(right) = self.right();
            let Ok(bottom) = self.bottom();
            let Ok(rb) = Point::from_xy(right, bottom);
            let mut pts = [lt, rb];
            let Ok(()) = ts.map_points(&mut pts);
            let Ok(rect) = Self::from_points(&pts);

            rect
          }
        }
      }
    })
  }

  pub fn bbox_transform(
    &self,
    bbox: NonZeroRect,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(bbox_width) = bbox.width();
    let Ok(bbox_height) = bbox.height();

    Self::from_xywh(
      self.bounds.x0.get() * bbox_width + bbox.bounds.x0.get(),
      self.bounds.y0.get() * bbox_height + bbox.bounds.y0.get(),
      width * bbox_width,
      height * bbox_height,
    )
  }

  pub fn to_non_zero_rect(
    &self,
  ) -> core::result::Result<Option<NonZeroRect>, console_core_never::Never> {
    let Ok(non_zero_rect) = NonZeroRect::from_xywh(
      {
        let Ok(explicit_value) = self.x();
        explicit_value
      },
      {
        let Ok(explicit_value) = self.y();
        explicit_value
      },
      {
        let Ok(explicit_value) = self.width();
        explicit_value
      },
      {
        let Ok(explicit_value) = self.height();
        explicit_value
      },
    );

    Ok(non_zero_rect)
  }
}

fn checked_f32_sub(
  CheckedF32SubArguments { end, start }: CheckedF32SubArguments,
) -> core::result::Result<Option<f32>, console_core_never::Never> {
  let difference = f64::from(end.get()) - f64::from(start.get());

  match ::core::primitive::bool::then_some(difference <= f64::from(f32::MIN) || difference >= f64::from(f32::MAX), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      },
      ::core::option::Option::None => {},
  }

  let Ok(value) = console_core_number_conversion::nearest_f32_from_f64(difference);

  Ok(Some(value))
}

struct CheckedF32SubArguments {
  end: FiniteF32,
  start: FiniteF32,
}

#[cfg(test)]
mod rect_tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};

  #[test]
  fn tests() -> Result<(), TestFailure> {
    assert_eq!(Rect::from_ltrb(10.0, 10.0, 5.0, 10.0), Ok(None));
    assert_eq!(Rect::from_ltrb(10.0, 10.0, 10.0, 5.0), Ok(None));
    assert_eq!(Rect::from_ltrb(f32::NAN, 10.0, 10.0, 10.0), Ok(None));
    assert_eq!(Rect::from_ltrb(10.0, f32::NAN, 10.0, 10.0), Ok(None));
    assert_eq!(Rect::from_ltrb(10.0, 10.0, f32::NAN, 10.0), Ok(None));
    assert_eq!(Rect::from_ltrb(10.0, 10.0, 10.0, f32::NAN), Ok(None));
    assert_eq!(Rect::from_ltrb(10.0, 10.0, 10.0, f32::INFINITY), Ok(None));

    let Ok(made_positive) = Rect::from_ltrb(10.0, 20.0, 30.0, 40.0);
    let positive = made_positive.required()?;
    assert_eq!(positive.left(), Ok(10.0));
    assert_eq!(positive.top(), Ok(20.0));
    assert_eq!(positive.right(), Ok(30.0));
    assert_eq!(positive.bottom(), Ok(40.0));
    assert_eq!(positive.width(), Ok(20.0));
    assert_eq!(positive.height(), Ok(20.0));

    let Ok(made_negative) = Rect::from_ltrb(-30.0, 20.0, -10.0, 40.0);
    let negative = made_negative.required()?;
    assert_eq!(negative.width(), Ok(20.0));
    assert_eq!(negative.height(), Ok(20.0));

    Ok(())
  }

  #[test]
  fn transform() -> Result<(), TestFailure> {
    let Ok(made_original) = Rect::from_ltrb(1.0, 2.0, 3.0, 4.0);
    let original = made_original.required()?;

    let Ok(identity) = Transform::identity();
    assert_eq!(original.transform(identity), Ok(Some(original)));

    let Ok(scale) = Transform::from_scale_factors(crate::ScaleFactors { x: 1.0, y: 2.0 });
    assert_eq!(
      original.transform(scale),
      Rect::from_ltrb(1.0, 4.0, 3.0, 8.0)
    );

    let Ok(skew_x) = Transform::from_skew(1.0, 0.0);
    assert_eq!(
      original.transform(skew_x),
      Rect::from_ltrb(3.0, 2.0, 7.0, 4.0)
    );

    let Ok(skew_y) = Transform::from_skew(0.0, 2.0);
    assert_eq!(
      original.transform(skew_y),
      Rect::from_ltrb(1.0, 4.0, 3.0, 10.0)
    );

    Ok(())
  }
}

#[derive(Copy, Clone, PartialEq)]
pub struct NonZeroRect {
  bounds: Bounds<FiniteF32>,
}

impl core::fmt::Debug for NonZeroRect {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("NonZeroRect")
      .field("left", &self.bounds.x0.get())
      .field("top", &self.bounds.y0.get())
      .field("right", &self.bounds.x1.get())
      .field("bottom", &self.bounds.y1.get())
      .finish()
  }
}

impl NonZeroRect {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rectangle constructor takes left, top, right and bottom in that order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "Floating rectangle bounds are in path coordinate units"
    )
  )]
  pub fn from_ltrb(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let left_2 = match FiniteF32::new(left) {
      Some(value) => value,
      None => return Ok(None),
    };
    let top_2 = match FiniteF32::new(top) {
      Some(value) => value,
      None => return Ok(None),
    };
    let right_2 = match FiniteF32::new(right) {
      Some(value) => value,
      None => return Ok(None),
    };
    let bottom_2 = match FiniteF32::new(bottom) {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(
      match ::core::primitive::bool::then_some(
        left_2.get() < right_2.get() && top_2.get() < bottom_2.get(),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(width) = checked_f32_sub(self::CheckedF32SubArguments {
            end: right_2,
            start: left_2,
          });
          let Ok(height) = checked_f32_sub(self::CheckedF32SubArguments {
            end: bottom_2,
            start: top_2,
          });

          match width {
            Some(_finite) => {}
            None => return Ok(None),
          };

          match height {
            Some(_finite) => {}
            None => return Ok(None),
          };

          Some(Self {
            bounds: Bounds {
              x0: left_2,
              y0: top_2,
              x1: right_2,
              y1: bottom_2,
            },
          })
        }
        ::core::option::Option::None => None,
      },
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rectangle constructor takes x, y, width and height in that order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "Floating rectangle dimensions are in path coordinate units"
    )
  )]
  pub fn from_xywh(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(non_zero_rect) = Self::from_ltrb(x, y, w + x, h + y);

    Ok(non_zero_rect)
  }

  pub fn left(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.x0.get())
  }

  pub fn top(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.y0.get())
  }

  pub fn right(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.x1.get())
  }

  pub fn bottom(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.y1.get())
  }

  pub fn x(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.x0.get())
  }

  pub fn y(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.y0.get())
  }

  pub fn width(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.x1.get() - self.bounds.x0.get())
  }

  pub fn height(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bounds.y1.get() - self.bounds.y0.get())
  }

  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "NonZeroRect validates both dimensions as strictly positive finite differences at construction"
    )
  )]
  pub fn size(&self) -> core::result::Result<Size, console_core_never::Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(size) = Size::from_wh(width, height);

    Ok(size.unwrap())
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public position takes x then y"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "Floating positions are in path coordinate units"
    )
  )]
  pub fn translate_to(
    &self,
    x: f32,
    y: f32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(non_zero_rect) = Self::from_xywh(x, y, width, height);

    Ok(non_zero_rect)
  }

  pub fn transform(
    &self,
    ts: Transform,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(is_identity) = ts.is_identity();

    Ok(match is_identity {
      crate::IsIdentityAnswer::Yes => Some(*self),
      crate::IsIdentityAnswer::No => {
        let Ok(rect) = self.to_rect();
        let Ok(transform) = rect.transform(ts);
        let Ok(non_zero_rect) = match transform {
          Some(value) => value,
          None => return Ok(None),
        }
        .to_non_zero_rect();

        non_zero_rect
      }
    })
  }

  pub fn bbox_transform(
    &self,
    bbox: NonZeroRect,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(bbox_width) = bbox.width();
    let Ok(bbox_height) = bbox.height();

    Self::from_xywh(
      self.bounds.x0.get() * bbox_width + bbox.bounds.x0.get(),
      self.bounds.y0.get() * bbox_height + bbox.bounds.y0.get(),
      width * bbox_width,
      height * bbox_height,
    )
  }

  pub fn to_rect(&self) -> core::result::Result<Rect, console_core_never::Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(rect) = Rect::from_xywh(self.bounds.x0.get(), self.bounds.y0.get(), width, height);

    Ok(match (rect, Rect { bounds: self.bounds }) {
      (::core::option::Option::Some(present), _) => present,
      (::core::option::Option::None, alternate) => alternate,
    })
  }

  pub fn to_int_rect(&self) -> core::result::Result<Option<IntRect>, console_core_never::Never> {
    use console_core_number_conversion::{toward_zero_i32, toward_zero_u32};

    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(x) = toward_zero_i32(f64::from(self.bounds.x0.get().floor()));
    let Ok(y) = toward_zero_i32(f64::from(self.bounds.y0.get().floor()));
    let Ok(w) = toward_zero_u32(f64::from(width.ceil()));
    let Ok(h) = toward_zero_u32(f64::from(height.ceil()));

    IntRect::from_xywh(x, y, w.max(1), h.max(1))
  }
}
