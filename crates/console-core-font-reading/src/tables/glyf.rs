//! The [glyf (Glyph Data)](https://docs.microsoft.com/en-us/typography/opentype/spec/glyf) table

pub mod bytecode;

use bytemuck::AnyBitPattern;
use core::ops::{Add, AddAssign, Div, Mul, MulAssign, Sub};
use types::{F26Dot6, Point};

include!("../../generated/generated_glyf.rs");

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct PointMarker(u8);

impl PointMarker {
  pub const HAS_DELTA: Self = Self(0x4);

  pub const TOUCHED_X: Self = Self(0x10);

  pub const TOUCHED_Y: Self = Self(0x20);

  pub const TOUCHED: Self = Self(Self::TOUCHED_X.0 | Self::TOUCHED_Y.0);

  pub const WEAK_INTERPOLATION: Self = Self(0x2);

  pub const NEAR: PointMarker = Self(0x8);
}

impl core::ops::BitOr for PointMarker {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    Self(self.0 | rhs.0)
  }
}

#[derive(
  Copy, Clone, PartialEq, Eq, Default, Debug, bytemuck::AnyBitPattern, bytemuck::NoUninit,
)]
#[repr(transparent)]
pub struct PointFlags(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointControl {
  None,
  Quadratic,
  Cubic,
}

impl PointFlags {
  const ON_CURVE: u8 = SimpleGlyphFlags::ON_CURVE_POINT.bits;
  const OFF_CURVE_CUBIC: u8 = SimpleGlyphFlags::CUBIC.bits;
  const CURVE_MASK: u8 = Self::ON_CURVE | Self::OFF_CURVE_CUBIC;

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::on_curve` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub const fn on_curve() -> Self {
    Self(Self::ON_CURVE)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::off_curve_quad` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub const fn off_curve_quad() -> Self {
    Self(0)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::off_curve_cubic` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub const fn off_curve_cubic() -> Self {
    Self(Self::OFF_CURVE_CUBIC)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::from_bits` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub const fn from_bits(bits: u8) -> Self {
    Self(bits & Self::CURVE_MASK)
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::curve_membership` method returns `CurveMembership` directly in the existing parser interface"
    )
  )]
  pub const fn curve_membership(self) -> CurveMembership {
    match self.0 & Self::ON_CURVE {
      0 => CurveMembership::OffCurve,
      _ => CurveMembership::OnCurve,
    }
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::point_control` method returns `PointControl` directly in the existing parser interface"
    )
  )]
  pub const fn point_control(self) -> PointControl {
    match self.0 & Self::CURVE_MASK {
      0 => PointControl::Quadratic,
      Self::ON_CURVE => PointControl::None,
      _cubic => PointControl::Cubic,
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::flip_on_curve` method is called through its existing unit-returning signature"
    )
  )]
  pub fn flip_on_curve(&mut self) {
    self.0 ^= 1;
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::set_on_curve` method is called through its existing unit-returning signature"
    )
  )]
  pub fn set_on_curve(&mut self) {
    self.0 |= Self::ON_CURVE;
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::clear_on_curve` method is called through its existing unit-returning signature"
    )
  )]
  pub fn clear_on_curve(&mut self) {
    self.0 &= !Self::ON_CURVE;
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::marker_presence` method returns `Presence` directly in the existing parser interface"
    )
  )]
  pub fn marker_presence(self, marker: PointMarker) -> Presence {
    match self.0 & marker.0 {
      0 => Presence::Absent,
      _ => Presence::Present,
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::set_marker` method is called through its existing unit-returning signature"
    )
  )]
  pub fn set_marker(&mut self, marker: PointMarker) {
    self.0 |= marker.0;
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::clear_marker` method is called through its existing unit-returning signature"
    )
  )]
  pub fn clear_marker(&mut self, marker: PointMarker) {
    self.0 &= !marker.0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::without_markers` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub const fn without_markers(self) -> Self {
    Self(self.0 & Self::CURVE_MASK)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PointFlags::to_bits` method returns `u8` directly in the existing parser interface"
    )
  )]
  pub const fn to_bits(self) -> u8 {
    self.0
  }
}

pub trait PointCoord:
  Copy
  + Default
  + AnyBitPattern
  + PartialEq
  + PartialOrd
  + Add<Output = Self>
  + AddAssign
  + Sub<Output = Self>
  + Div<Output = Self>
  + Mul<Output = Self>
  + MulAssign
{
  fn from_fixed(x: Fixed) -> Self;

  fn from_i32(x: i32) -> Self;

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "external PointCoord implementations return an f32 glyph coordinate through this published trait"
    )
  )]
  fn to_f32(self) -> f32;

  fn midpoint(self, other: Self) -> Self;
}

impl<'a> SimpleGlyph<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `SimpleGlyph::num_points` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn num_points(&self) -> u64 {
    match ({
      let Ok(explicit_value) = self.end_pts_of_contours();
      explicit_value
    })
    .last()
    .map(|last| {
      let Ok(value_1) = last.get();

      ::core::primitive::u64::wrapping_add(
        {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_1);
          migrated_value
        },
        1,
      )
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `SimpleGlyph::overlap_flag` method returns `Presence` directly in the existing parser interface"
    )
  )]
  pub fn overlap_flag(&self) -> Presence {
    let Ok(glyph_data) = self.glyph_data();
    let Ok(data) = FontData::new(glyph_data);

    match data.read_at::<SimpleGlyphFlags>(0) {
      Ok(flag) => {
        let Ok(overlap) = flag.contains(SimpleGlyphFlags::OVERLAP_SIMPLE);

        overlap
      }
      Err(_unreadable) => Presence::Absent,
    }
  }

  pub fn read_points_fast<C: PointCoord>(
    &self,
    points: &mut [Point<C>],
    flags: &mut [PointFlags],
  ) -> Result<(), ReadError> {
    let Ok(count) = console_core_number_conversion::index(self.num_points());

    match crate::Presence::from(points.len() == count && flags.len() == count) {
      crate::Presence::Present => {}
      crate::Presence::Absent => return Err(ReadError::InvalidArrayLen),
    }

    let Ok(glyph_data) = self.glyph_data();
    let consumed = read_point_flags(glyph_data, flags)?;
    let Ok(data) = FontData::new(glyph_data);
    let Ok(mut cursor) = data.cursor();
    let Ok(()) = cursor.advance_by(consumed);
    let mut x = 0i32;

    for (&point_flags, point) in flags.iter().zip(points.iter_mut()) {
      let Ok(bits) = SimpleGlyphFlags::from_bits_truncate(point_flags.0);
      let Ok(encoding) = CoordinateAxis::X.encoding(bits);
      let delta = encoding.read_delta(&mut cursor)?;
      x = x.wrapping_add(delta);
      point.x = C::from_i32(x);
    }

    let mut y = 0i32;
    #[cfg(feature = "spec_next")]
    let preserved_flags = PointFlags::CURVE_MASK;
    #[cfg(not(feature = "spec_next"))]
    let preserved_flags = PointFlags::ON_CURVE;

    for (point_flags, point) in flags.iter_mut().zip(points.iter_mut()) {
      let Ok(bits) = SimpleGlyphFlags::from_bits_truncate(point_flags.0);
      let Ok(encoding) = CoordinateAxis::Y.encoding(bits);
      let delta = encoding.read_delta(&mut cursor)?;
      y = y.wrapping_add(delta);
      point.y = C::from_i32(y);
      point_flags.0 &= preserved_flags;
    }

    Ok(())
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `SimpleGlyph::points` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn points(&self) -> impl Iterator<Item = CurvePoint> + 'a + Clone + use<'a> {
    let Ok(points_impl) = self.points_impl();

    match points_impl {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => {
        let Ok(point_iter) = PointIter::new(&[], &[], &[]);

        point_iter
      }
    }
  }

  #[must_use]
  fn points_impl(
    &self,
  ) -> core::result::Result<Option<PointIter<'a>>, console_core_never::Never> {
    let end_points = {
      let Ok(explicit_value) = self.end_pts_of_contours();
      explicit_value
    };
    let end_point = match end_points.last() {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(value_5) = end_point.get();

    let n_points = match value_5.checked_add(1) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let data = {
      let Ok(explicit_value) = self.glyph_data();
      explicit_value
    };
    let lens = match match resolve_coords_len(data, n_points) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(flag_bytes) = console_core_number_conversion::index(lens.flags);
    let Ok(x_bytes) = console_core_number_conversion::index(lens.x_coords);
    let Ok(y_bytes) = console_core_number_conversion::index(lens.y_coords);
    let (flags, coordinates) = match data.split_at_checked(flag_bytes) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let (x_coords, remaining) = match coordinates.split_at_checked(x_bytes) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let y_coords = match remaining.get(..y_bytes) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let Ok(point_iter) = PointIter::new(flags, x_coords, y_coords);

    Ok(Some(point_iter))
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveMembership {
  OnCurve,
  OffCurve,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurvePoint {
  pub x: i16,
  pub y: i16,
  pub membership: CurveMembership,
}

impl CurvePoint {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CurvePoint::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the published `CurvePoint::new` method retains the positional argument order for `x`, `y`, `membership`"
    )
  )]
  pub fn new(x: i16, y: i16, membership: CurveMembership) -> Self {
    Self { x, y, membership }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CurvePoint::on_curve` method supports direct construction or chaining with its existing Self return"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the published `CurvePoint::on_curve` method retains the positional argument order for `x`, `y`"
    )
  )]
  pub fn on_curve(x: i16, y: i16) -> Self {
    Self::new(x, y, CurveMembership::OnCurve)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CurvePoint::off_curve` method supports direct construction or chaining with its existing Self return"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the published `CurvePoint::off_curve` method retains the positional argument order for `x`, `y`"
    )
  )]
  pub fn off_curve(x: i16, y: i16) -> Self {
    Self::new(x, y, CurveMembership::OffCurve)
  }
}

#[derive(Clone)]
struct PointIter<'a> {
  flags: Cursor<'a>,
  x_coords: Cursor<'a>,
  y_coords: Cursor<'a>,
  flag_repeats: u8,
  cur_flags: SimpleGlyphFlags,
  cur_x: i16,
  cur_y: i16,
}

impl Iterator for PointIter<'_> {
  type Item = CurvePoint;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(advance_flags) = self.advance_flags();
    advance_flags?;
    let Ok(()) = self.advance_points();
    let Ok(on_curve) = self.cur_flags.contains(SimpleGlyphFlags::ON_CURVE_POINT);
    let membership = match on_curve {
      Presence::Present => CurveMembership::OnCurve,
      Presence::Absent => CurveMembership::OffCurve,
    };

    Some(CurvePoint::new(self.cur_x, self.cur_y, membership))
  }
}

impl<'a> PointIter<'a> {
  #[must_use]
  fn new(
    flags: &'a [u8],
    x_coords: &'a [u8],
    y_coords: &'a [u8],
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(value_6) = FontData::new(flags);
    let Ok(value_7) = FontData::new(x_coords);
    let Ok(value_8) = FontData::new(y_coords);

    let Ok(value_26) = value_6.cursor();
    let Ok(value_27) = value_7.cursor();
    let Ok(value_28) = value_8.cursor();

    Ok(Self {
      flags: value_26,
      x_coords: value_27,
      y_coords: value_28,
      flag_repeats: 0,
      cur_flags: ({
        let Ok(explicit_value) = SimpleGlyphFlags::empty();
        explicit_value
      }),
      cur_x: 0,
      cur_y: 0,
    })
  }

  fn advance_flags(&mut self) -> core::result::Result<Option<()>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(self.flag_repeats == 0, ()) {
      ::core::option::Option::Some(()) => {
        self.cur_flags = {
          let read = match match self.flags.read() {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          } {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          };
          let Ok(explicit_value) = SimpleGlyphFlags::from_bits_truncate(read);
          explicit_value
        };
        let Ok(repeat) = self.cur_flags.contains(SimpleGlyphFlags::REPEAT_FLAG);
        let repeats = match repeat {
          Presence::Present => match self.flags.read::<u8>() {
            Ok(repeats) => repeats,
            Err(_unreadable) => 0,
          },
          Presence::Absent => 0,
        };

        self.flag_repeats = repeats.wrapping_add(1);
      }
      ::core::option::Option::None => {}
    }

    match (1, &mut self.flag_repeats) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u8::wrapping_sub(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    Ok(Some(()))
  }

  fn advance_points(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(x_short) = self.cur_flags.contains(SimpleGlyphFlags::X_SHORT_VECTOR);
      let Ok(x_same_or_pos) = self
        .cur_flags
        .contains(SimpleGlyphFlags::X_IS_SAME_OR_POSITIVE_X_SHORT_VECTOR);
      let Ok(y_short) = self.cur_flags.contains(SimpleGlyphFlags::Y_SHORT_VECTOR);
      let Ok(y_same_or_pos) = self
        .cur_flags
        .contains(SimpleGlyphFlags::Y_IS_SAME_OR_POSITIVE_Y_SHORT_VECTOR);

      let delta_x = match (x_short, x_same_or_pos) {
        (Presence::Present, Presence::Absent) => ::core::primitive::i16::wrapping_neg({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i16>(match self.x_coords.read::<u8>() {
              Ok(value) => value,
              Err(_truncated_x_coordinates) => 0,
            });
          migrated_value
        }),
        (Presence::Present, Presence::Present) => {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i16>(match self.x_coords.read::<u8>() {
              Ok(value) => value,
              Err(_truncated_x_coordinates) => 0,
            });
          migrated_value
        }
        (Presence::Absent, Presence::Absent) => match self.x_coords.read::<i16>() {
          Ok(value) => value,
          Err(_truncated_x_coordinates) => 0,
        },
        (Presence::Absent, Presence::Present) => 0,
      };

      let delta_y = match (y_short, y_same_or_pos) {
        (Presence::Present, Presence::Absent) => ::core::primitive::i16::wrapping_neg({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i16>(match self.y_coords.read::<u8>() {
              Ok(value) => value,
              Err(_truncated_y_coordinates) => 0,
            });
          migrated_value
        }),
        (Presence::Present, Presence::Present) => {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i16>(match self.y_coords.read::<u8>() {
              Ok(value) => value,
              Err(_truncated_y_coordinates) => 0,
            });
          migrated_value
        }
        (Presence::Absent, Presence::Absent) => match self.y_coords.read::<i16>() {
          Ok(value) => value,
          Err(_truncated_y_coordinates) => 0,
        },
        (Presence::Absent, Presence::Present) => 0,
      };

      self.cur_x = self.cur_x.wrapping_add(delta_x);
      self.cur_y = self.cur_y.wrapping_add(delta_y);
    };
    Ok(())
  }
}

fn read_point_flags(data: &[u8], flags: &mut [PointFlags]) -> Result<u64, ReadError> {
  let Ok(point_count) = console_core_number_conversion::fitted::<_, u64>(flags.len());
  let mut payload = data.iter().copied();
  let mut decoded = 0u64;
  let mut consumed = 0u64;

  for _ in 0..=point_count {
    let bits = match payload.next() {
      Some(bits) => bits,
      None => break,
    };
    consumed = consumed.wrapping_add(1);
    let Ok(encoded) = SimpleGlyphFlags::from_bits_truncate(bits);
    let count = match encoded.contains(SimpleGlyphFlags::REPEAT_FLAG) {
      Ok(Presence::Present) => {
        let repetitions = payload.next().ok_or(ReadError::OutOfBounds)?;
        consumed = consumed.wrapping_add(1);
        u64::from(repetitions)
          .wrapping_add(1)
          .min(point_count.wrapping_sub(decoded))
      }
      Ok(Presence::Absent) => 1,
    };
    let next = decoded.wrapping_add(count);
    let Ok(range) = console_core_number_conversion::indices(decoded..next);
    let target = flags.get_mut(range).ok_or(ReadError::OutOfBounds)?;
    target.fill(PointFlags(bits));
    decoded = next;

    match crate::Presence::from(decoded == point_count) {
      crate::Presence::Present => break,
      crate::Presence::Absent => {}
    }
  }

  Ok(consumed)
}

#[derive(Clone, Copy)]
enum CoordinateAxis {
  X,
  Y,
}

impl CoordinateAxis {
  #[inline]
  fn encoding(
    self,
    flags: SimpleGlyphFlags,
  ) -> Result<CoordinateEncoding, console_core_never::Never> {
    let (short_vector, same_or_positive) = match self {
      Self::X => (
        SimpleGlyphFlags::X_SHORT_VECTOR,
        SimpleGlyphFlags::X_IS_SAME_OR_POSITIVE_X_SHORT_VECTOR,
      ),
      Self::Y => (
        SimpleGlyphFlags::Y_SHORT_VECTOR,
        SimpleGlyphFlags::Y_IS_SAME_OR_POSITIVE_Y_SHORT_VECTOR,
      ),
    };
    let Ok(short) = flags.contains(short_vector);
    let Ok(same) = flags.contains(same_or_positive);
    Ok(match (short, same) {
      (Presence::Present, Presence::Present) => CoordinateEncoding::PositiveByte,
      (Presence::Present, Presence::Absent) => CoordinateEncoding::NegativeByte,
      (Presence::Absent, Presence::Present) => CoordinateEncoding::Unchanged,
      (Presence::Absent, Presence::Absent) => CoordinateEncoding::SignedWord,
    })
  }
}

enum CoordinateEncoding {
  Unchanged,
  PositiveByte,
  NegativeByte,
  SignedWord,
}

impl CoordinateEncoding {
  #[inline]
  fn read_delta(self, cursor: &mut Cursor<'_>) -> Result<i32, ReadError> {
    match self {
      Self::Unchanged => Ok(0),
      Self::PositiveByte => cursor.read::<u8>().map(i32::from),
      Self::NegativeByte => cursor
        .read::<u8>()
        .map(|value| i32::from(value).wrapping_neg()),
      Self::SignedWord => cursor.read::<i16>().map(i32::from),
    }
  }

  #[inline]
  fn byte_width(self) -> Result<u32, console_core_never::Never> {
    Ok(match self {
      Self::Unchanged => 0,
      Self::PositiveByte | Self::NegativeByte => 1,
      Self::SignedWord => 2,
    })
  }
}

fn resolve_coords_len(data: &[u8], points_total: u16) -> Result<FieldLengths, ReadError> {
  let Ok(data_2) = FontData::new(data);
  let Ok(mut cursor) = data_2.cursor();
  let mut flags_left = u32::from(points_total);
  let mut x_coords = 0u32;
  let mut y_coords = 0u32;

  for _ in 0..points_total {
    match flags_left {
      0 => break,
      _unread_points => {}
    }

    let flags: SimpleGlyphFlags = cursor.read()?;
    let repeats = match flags.contains(SimpleGlyphFlags::REPEAT_FLAG) {
      Ok(Presence::Present) => {
        let count: u8 = cursor.read()?;
        u32::from(count).wrapping_add(1)
      }
      Ok(Presence::Absent) => 1,
    };
    let checked_value = flags_left
      .checked_sub(repeats)
      .ok_or(ReadError::MalformedData("repeat count too large in glyf"))?;
    flags_left = checked_value;
    let Ok(x_encoding) = CoordinateAxis::X.encoding(flags);
    let Ok(y_encoding) = CoordinateAxis::Y.encoding(flags);
    let Ok(x_width) = x_encoding.byte_width();
    let Ok(y_width) = y_encoding.byte_width();
    x_coords = x_coords.wrapping_add(x_width.wrapping_mul(repeats));
    y_coords = y_coords.wrapping_add(y_width.wrapping_mul(repeats));
  }

  let position = cursor.position()?;
  let Ok(flags) = console_core_number_conversion::wrapped::<_, u32>(position);
  Ok(FieldLengths {
    flags,
    x_coords,
    y_coords,
  })
}

struct FieldLengths {
  flags: u32,
  x_coords: u32,
  y_coords: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transform {
  pub xx: F2Dot14,
  pub yx: F2Dot14,
  pub xy: F2Dot14,
  pub yy: F2Dot14,
}

impl Default for Transform {
  fn default() -> Self {
    let Ok(value_10) = F2Dot14::from_f32(1.0);
    let Ok(value_11) = F2Dot14::from_f32(0.0);
    let Ok(value_12) = F2Dot14::from_f32(0.0);
    let Ok(value_13) = F2Dot14::from_f32(1.0);

    Self {
      xx: value_10,
      yx: value_11,
      xy: value_12,
      yy: value_13,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
  pub flags: CompositeGlyphFlags,
  pub glyph: GlyphId16,
  pub anchor: Anchor,
  pub transform: Transform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
  Offset { x: i16, y: i16 },
  Point { base: u16, component: u16 },
}

impl<'a> CompositeGlyph<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CompositeGlyph::components` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn components(&self) -> impl Iterator<Item = Component> + 'a + Clone + use<'a> {
    let Ok(value_14) = FontData::new({
      let Ok(explicit_value) = self.component_data();
      explicit_value
    });

    let Ok(value_29) = value_14.cursor();

    ComponentIter {
      cur_flags: ({
        let Ok(explicit_value) = CompositeGlyphFlags::empty();
        explicit_value
      }),
      done: false,
      cursor: value_29,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CompositeGlyph::component_glyphs_and_flags` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn component_glyphs_and_flags(
    &self,
  ) -> impl Iterator<Item = (GlyphId16, CompositeGlyphFlags)> + 'a + Clone + use<'a> {
    let Ok(value_15) = FontData::new({
      let Ok(explicit_value) = self.component_data();
      explicit_value
    });

    let Ok(value_30) = value_15.cursor();

    ComponentGlyphIdFlagsIter {
      cur_flags: ({
        let Ok(explicit_value) = CompositeGlyphFlags::empty();
        explicit_value
      }),
      done: false,
      cursor: value_30,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CompositeGlyph::count_and_instructions` method returns its tuple directly for existing callers to destructure"
    )
  )]
  pub fn count_and_instructions(&self) -> (u64, Option<&'a [u8]>) {
    let Ok(value_16) = FontData::new({
      let Ok(explicit_value) = self.component_data();
      explicit_value
    });

    let Ok(value_31) = value_16.cursor();

    let mut iter = ComponentGlyphIdFlagsIter {
      cur_flags: ({
        let Ok(explicit_value) = CompositeGlyphFlags::empty();
        explicit_value
      }),
      done: false,
      cursor: value_31,
    };
    let Ok(count) = console_core_number_conversion::fitted::<_, u64>(iter.by_ref().count());

    let Ok(we_have_instructions) = iter
      .cur_flags
      .contains(CompositeGlyphFlags::WE_HAVE_INSTRUCTIONS);

    let instructions = match we_have_instructions {
      Presence::Present => {
        let length = match iter.cursor.read::<u16>() {
          Ok(length) => length,
          Err(_truncated_length) => return (count, None),
        };

        match iter.cursor.read_array(u64::from(length)) {
          Ok(bytes) => Some(bytes),
          Err(_truncated_instructions) => None,
        }
      }
      Presence::Absent => None,
    };
    (count, instructions)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CompositeGlyph::instructions` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn instructions(&self) -> Option<&'a [u8]> {
    self.count_and_instructions().1
  }
}

#[derive(Clone)]
struct ComponentIter<'a> {
  cur_flags: CompositeGlyphFlags,
  done: bool,
  cursor: Cursor<'a>,
}

impl Iterator for ComponentIter<'_> {
  type Item = Component;

  fn next(&mut self) -> Option<Self::Item> {
    match ::core::primitive::bool::then_some(self.done, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let flags: CompositeGlyphFlags = match self.cursor.read() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    }?;
    self.cur_flags = flags;
    let glyph = match self.cursor.read::<GlyphId16>() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    }?;
    let Ok(args_are_words) = flags.contains(CompositeGlyphFlags::ARG_1_AND_2_ARE_WORDS);
    let Ok(args_are_xy_values) = flags.contains(CompositeGlyphFlags::ARGS_ARE_XY_VALUES);
    let anchor = match (args_are_xy_values, args_are_words) {
      (Presence::Present, Presence::Present) => {
        let x = match self.cursor.read() {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;
        let y = match self.cursor.read() {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;

        Anchor::Offset { x, y }
      }
      (Presence::Present, Presence::Absent) => Anchor::Offset {
        x: ({
          let read = match self.cursor.read::<i8>() {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(read);
          migrated_value
        }),
        y: ({
          let read = match self.cursor.read::<i8>() {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(read);
          migrated_value
        }),
      },
      (Presence::Absent, Presence::Present) => {
        let base = match self.cursor.read() {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;
        let component = match self.cursor.read() {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;

        Anchor::Point { base, component }
      }
      (Presence::Absent, Presence::Absent) => Anchor::Point {
        base: ({
          let read = match self.cursor.read::<u8>() {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(read);
          migrated_value
        }),
        component: ({
          let read = match self.cursor.read::<u8>() {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(read);
          migrated_value
        }),
      },
    };
    let mut transform = Transform::default();
    let Ok(we_have_a_scale_flag) = flags.contains(CompositeGlyphFlags::WE_HAVE_A_SCALE);
    let Ok(we_have_an_x_and_y_scale) =
      flags.contains(CompositeGlyphFlags::WE_HAVE_AN_X_AND_Y_SCALE);
    let Ok(we_have_a_two_by_two) = flags.contains(CompositeGlyphFlags::WE_HAVE_A_TWO_BY_TWO);

    match we_have_a_scale_flag {
      Presence::Present => {
        let read = match self.cursor.read() {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;
        transform.xx = read;
        transform.yy = transform.xx;
      }
      Presence::Absent => match we_have_an_x_and_y_scale {
        Presence::Present => {
          let read = match self.cursor.read() {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;
          transform.xx = read;
          let own_read = match self.cursor.read() {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;
          transform.yy = own_read;
        }
        Presence::Absent => match we_have_a_two_by_two {
          Presence::Present => {
            let read = match self.cursor.read() {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }?;
            transform.xx = read;
            let own_read = match self.cursor.read() {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }?;
            transform.yx = own_read;
            let checked_value = match self.cursor.read() {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }?;
            transform.xy = checked_value;
            let checked_value_2 = match self.cursor.read() {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }?;
            transform.yy = checked_value_2;
          }
          Presence::Absent => {}
        },
      },
    }

    let Ok(more_components) = flags.contains(CompositeGlyphFlags::MORE_COMPONENTS);

    self.done = matches!(more_components, Presence::Absent);

    Some(Component {
      flags,
      glyph,
      anchor,
      transform,
    })
  }
}

#[derive(Clone)]
struct ComponentGlyphIdFlagsIter<'a> {
  cur_flags: CompositeGlyphFlags,
  done: bool,
  cursor: Cursor<'a>,
}

impl Iterator for ComponentGlyphIdFlagsIter<'_> {
  type Item = (GlyphId16, CompositeGlyphFlags);

  fn next(&mut self) -> Option<Self::Item> {
    match ::core::primitive::bool::then_some(self.done, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let flags: CompositeGlyphFlags = match self.cursor.read() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    }?;
    self.cur_flags = flags;
    let glyph = match self.cursor.read::<GlyphId16>() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    }?;
    let args_are_words = {
      let Ok(explicit_value) = flags.contains(CompositeGlyphFlags::ARG_1_AND_2_ARE_WORDS);
      explicit_value
    };

    match args_are_words {
      Presence::Present => {
        let ::core::result::Result::Ok(()) = self.cursor.advance_by(4);
      }
      Presence::Absent => {
        let ::core::result::Result::Ok(()) = self.cursor.advance_by(2);
      }
    }

    let Ok(we_have_a_scale) = flags.contains(CompositeGlyphFlags::WE_HAVE_A_SCALE);
    let Ok(we_have_an_x_and_y_scale) =
      flags.contains(CompositeGlyphFlags::WE_HAVE_AN_X_AND_Y_SCALE);
    let Ok(we_have_a_two_by_two) = flags.contains(CompositeGlyphFlags::WE_HAVE_A_TWO_BY_TWO);

    match we_have_a_scale {
      Presence::Present => {
        let ::core::result::Result::Ok(()) = self.cursor.advance_by(2);
      }
      Presence::Absent => match we_have_an_x_and_y_scale {
        Presence::Present => {
          let ::core::result::Result::Ok(()) = self.cursor.advance_by(4);
        }
        Presence::Absent => match we_have_a_two_by_two {
          Presence::Present => {
            let ::core::result::Result::Ok(()) = self.cursor.advance_by(8);
          }
          Presence::Absent => {}
        },
      },
    }

    let Ok(more_components) = flags.contains(CompositeGlyphFlags::MORE_COMPONENTS);

    self.done = matches!(more_components, Presence::Absent);
    Some((glyph, flags))
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Component {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("Component")
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    Ok(match idx {
      0 => Some({
        let Ok(explicit_value) = Field::new("flags", self.flags.bits());
        explicit_value
      }),
      1 => Some({
        let Ok(explicit_value) = Field::new("glyph", self.glyph);
        explicit_value
      }),
      2 => match self.anchor {
        Anchor::Point { base, .. } => Some({
          let Ok(explicit_value) = Field::new("base", base);
          explicit_value
        }),
        Anchor::Offset { x, .. } => Some({
          let Ok(explicit_value) = Field::new("x", x);
          explicit_value
        }),
      },
      3 => match self.anchor {
        Anchor::Point { component, .. } => Some({
          let Ok(explicit_value) = Field::new("component", component);
          explicit_value
        }),
        Anchor::Offset { y, .. } => Some({
          let Ok(explicit_value) = Field::new("y", y);
          explicit_value
        }),
      },
      _ => None,
    })
  }
}

impl Anchor {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Anchor::compute_flags` method returns `CompositeGlyphFlags` directly in the existing parser interface"
    )
  )]
  pub fn compute_flags(&self) -> CompositeGlyphFlags {
    const I8_RANGE: Range<i16> = ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i8 => ::core::primitive::i16, i8::MIN)
      ..::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i8 => ::core::primitive::i16, i8::MAX)
        + 1;
    const U8_MAX: u16 = ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => ::core::primitive::u16, u8::MAX);

    let mut flags = {
      let Ok(explicit_value) = CompositeGlyphFlags::empty();
      explicit_value
    };

    match self {
      Anchor::Offset { x, y } => {
        flags |= CompositeGlyphFlags::ARGS_ARE_XY_VALUES;

        match ::core::primitive::bool::then_some(
          !I8_RANGE.contains(x) || !I8_RANGE.contains(y),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            flags |= CompositeGlyphFlags::ARG_1_AND_2_ARE_WORDS;
          }
          ::core::option::Option::None => {}
        }
      }
      Anchor::Point { base, component } => {
        match ::core::primitive::bool::then_some(base > &U8_MAX || component > &U8_MAX, ()) {
          ::core::option::Option::Some(()) => {
            flags |= CompositeGlyphFlags::ARG_1_AND_2_ARE_WORDS;
          }
          ::core::option::Option::None => {}
        }
      }
    }

    flags
  }
}

impl Transform {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Transform::compute_flags` method returns `CompositeGlyphFlags` directly in the existing parser interface"
    )
  )]
  pub fn compute_flags(&self) -> CompositeGlyphFlags {
    match ::core::primitive::bool::then_some(
      self.yx != F2Dot14::ZERO || self.xy != F2Dot14::ZERO,
      (),
    ) {
      ::core::option::Option::Some(()) => CompositeGlyphFlags::WE_HAVE_A_TWO_BY_TWO,
      ::core::option::Option::None => {
        match ::core::primitive::bool::then_some(self.xx != self.yy, ()) {
          ::core::option::Option::Some(()) => CompositeGlyphFlags::WE_HAVE_AN_X_AND_Y_SCALE,
          ::core::option::Option::None => {
            match ::core::primitive::bool::then_some(self.xx != F2Dot14::ONE, ()) {
              ::core::option::Option::Some(()) => CompositeGlyphFlags::WE_HAVE_A_SCALE,
              ::core::option::Option::None => {
                let Ok(explicit_value) = CompositeGlyphFlags::empty();
                explicit_value
              }
            }
          }
        }
      }
    }
  }
}

impl PointCoord for F26Dot6 {
  fn from_fixed(x: Fixed) -> Self {
    let Ok(value_17) = x.to_f26dot6();

    value_17
  }

  #[inline]
  fn from_i32(x: i32) -> Self {
    let Ok(value_18) = Self::from_i32(x);

    value_18
  }

  #[inline]
  fn to_f32(self) -> f32 {
    let Ok(value_19) = self.to_f32();

    value_19
  }

  #[inline]
  fn midpoint(self, other: Self) -> Self {
    let Ok(value_20) = Self::from_bits({
      let Ok(left) = self.to_bits();
      let Ok(right) = other.to_bits();

      {
        let Ok(explicit_value) = midpoint_i32([left, right]);
        explicit_value
      }
    });

    value_20
  }
}

impl PointCoord for Fixed {
  fn from_fixed(x: Fixed) -> Self {
    x
  }

  fn from_i32(x: i32) -> Self {
    let Ok(value_21) = Self::from_i32(x);

    value_21
  }

  fn to_f32(self) -> f32 {
    let Ok(value_22) = self.to_f32();

    value_22
  }

  fn midpoint(self, other: Self) -> Self {
    let Ok(value_23) = Self::from_bits({
      let Ok(left) = self.to_bits();
      let Ok(right) = other.to_bits();

      {
        let Ok(explicit_value) = midpoint_i32([left, right]);
        explicit_value
      }
    });

    value_23
  }
}

impl PointCoord for i32 {
  fn from_fixed(x: Fixed) -> Self {
    let Ok(value_24) = x.to_i32();

    value_24
  }

  fn from_i32(x: i32) -> Self {
    x
  }

  fn to_f32(self) -> f32 {
    match ::console_core_number_conversion::nearest_f32_from_i64(
      <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(self),
    ) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    }
  }

  fn midpoint(self, other: Self) -> Self {
    {
      let Ok(explicit_value) = midpoint_i32([self, other]);
      explicit_value
    }
  }
}

#[inline(always)]
#[must_use]
fn midpoint_i32(coordinates: [i32; 2]) -> Result<i32, console_core_never::Never> {
  let [left, right] = coordinates;
  Ok(left.wrapping_add(right).wrapping_div(2))
}

impl PointCoord for f32 {
  fn from_fixed(x: Fixed) -> Self {
    let Ok(value_25) = x.to_f32();

    value_25
  }

  fn from_i32(x: i32) -> Self {
    match ::console_core_number_conversion::nearest_f32_from_i64(
      <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(x),
    ) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    }
  }

  fn to_f32(self) -> f32 {
    self
  }

  fn midpoint(self, other: Self) -> Self {
    self + 0.5 * (other - self)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontRef, GlyphId, TableProvider};

  #[test]
  fn simple_glyph() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::COLR_GRADIENT_RECT)?;
    let loca = font.loca(None)?;
    let glyf = font.glyf()?;
    let glyph = crate::test_support::fixture_value(
      match loca.get_glyf(
        {
          let Ok(explicit_value) = GlyphId::new(0);
          explicit_value
        },
        &glyf,
      ) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    assert_eq!(glyph.number_of_contours(), Ok(2));
    let simple_glyph = match glyph {
      Glyph::Simple(simple) => simple,
      Glyph::Composite(_) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "expected simple glyph"
        )));
      }
    };
    assert_eq!(
      ({
        let Ok(explicit_value) = simple_glyph.end_pts_of_contours();
        explicit_value
      })
      .iter()
      .map(|x| {
        let Ok(value) = x.get();
        value
      })
      .collect::<Vec<_>>(),
      &[3, 7]
    );
    assert_eq!(
      simple_glyph
        .points()
        .map(|pt| (pt.x, pt.y, pt.membership))
        .collect::<Vec<_>>(),
      &[
        (5, 0, CurveMembership::OnCurve),
        (5, 100, CurveMembership::OnCurve),
        (45, 100, CurveMembership::OnCurve),
        (45, 0, CurveMembership::OnCurve),
        (10, 5, CurveMembership::OnCurve),
        (40, 5, CurveMembership::OnCurve),
        (40, 95, CurveMembership::OnCurve),
        (10, 95, CurveMembership::OnCurve),
      ]
    );
    ::core::result::Result::Ok(())
  }

  fn all_glyphs(font_data: &[u8]) -> Result<Vec<Option<Glyph<'_>>>, ReadError> {
    let font = FontRef::new(font_data)?;
    let loca = font.loca(None)?;
    let glyf = font.glyf()?;
    let maxp = font.maxp()?;
    let Ok(count) = maxp.num_glyphs();
    (0..u32::from(count))
      .map(|id| {
        let Ok(glyph) = GlyphId::new(id);
        loca.get_glyf(glyph, &glyf)
      })
      .collect()
  }

  #[test]
  fn simple_glyph_overlapping_contour_flag() -> Result<(), ReadError> {
    let glyphs = all_glyphs(font_test_data::VAZIRMATN_VAR)?;
    let gids_with_overlap: Vec<_> = glyphs
      .into_iter()
      .enumerate()
      .filter_map(|(gid, glyph)| match glyph {
        Some(Glyph::Simple(glyph_2)) => match glyph_2.overlap_flag() {
          Presence::Present => Some(gid),
          Presence::Absent => None,
        },
        Some(Glyph::Composite(_)) | None => None,
      })
      .collect();

    assert_eq!(gids_with_overlap, [3]);
    Ok(())
  }

  #[test]
  fn composite_glyph_overlapping_contour_flag() -> Result<(), ReadError> {
    let glyphs = all_glyphs(font_test_data::VAZIRMATN_VAR)?;
    let gids_components_with_overlap: Vec<_> = glyphs
      .into_iter()
      .enumerate()
      .filter_map(|(gid, glyph)| match glyph {
        Some(Glyph::Composite(glyph_2)) => Some((gid, glyph_2)),
        ::core::option::Option::None | ::core::option::Option::Some(_) => None,
      })
      .flat_map(|(gid, glyph)| {
        glyph
          .components()
          .enumerate()
          .filter_map(move |(comp_ix, comp)| {
            let Ok(overlap) = comp.flags.contains(CompositeGlyphFlags::OVERLAP_COMPOUND);

            match overlap {
              Presence::Present => Some((gid, comp_ix)),
              Presence::Absent => None,
            }
          })
      })
      .collect();

    assert_eq!(gids_components_with_overlap, [(2, 1)]);
    Ok(())
  }

  #[test]
  fn compute_anchor_flags() {
    let anchor = Anchor::Offset { x: -128, y: 127 };
    assert_eq!(
      anchor.compute_flags(),
      CompositeGlyphFlags::ARGS_ARE_XY_VALUES
    );

    let anchor_2 = Anchor::Offset { x: -129, y: 127 };
    assert_eq!(
      anchor_2.compute_flags(),
      CompositeGlyphFlags::ARGS_ARE_XY_VALUES | CompositeGlyphFlags::ARG_1_AND_2_ARE_WORDS
    );
    let anchor_3 = Anchor::Offset { x: -1, y: 128 };
    assert_eq!(
      anchor_3.compute_flags(),
      CompositeGlyphFlags::ARGS_ARE_XY_VALUES | CompositeGlyphFlags::ARG_1_AND_2_ARE_WORDS
    );

    let anchor_4 = Anchor::Point {
      base: 255,
      component: 20,
    };
    assert_eq!(
      anchor_4.compute_flags(),
      ({
        let Ok(explicit_value) = CompositeGlyphFlags::empty();
        explicit_value
      })
    );

    let anchor_5 = Anchor::Point {
      base: 256,
      component: 20,
    };

    assert_eq!(
      anchor_5.compute_flags(),
      CompositeGlyphFlags::ARG_1_AND_2_ARE_WORDS
    )
  }

  #[test]
  fn compute_transform_flags() {
    #[must_use]
    fn make_xform(elements: [f32; 4]) -> Result<Transform, console_core_never::Never> {
      let [xx, yx, xy, yy] = elements;
      Ok(Transform {
        xx: ({
          let Ok(explicit_value) = F2Dot14::from_f32(xx);
          explicit_value
        }),
        yx: ({
          let Ok(explicit_value) = F2Dot14::from_f32(yx);
          explicit_value
        }),
        xy: ({
          let Ok(explicit_value) = F2Dot14::from_f32(xy);
          explicit_value
        }),
        yy: ({
          let Ok(explicit_value) = F2Dot14::from_f32(yy);
          explicit_value
        }),
      })
    }

    assert_eq!(
      ({
        let Ok(explicit_value) = make_xform([1.0, 0., 0., 1.0]);
        explicit_value
      })
      .compute_flags(),
      ({
        let Ok(explicit_value) = CompositeGlyphFlags::empty();
        explicit_value
      })
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = make_xform([2.0, 0., 0., 2.0]);
        explicit_value
      })
      .compute_flags(),
      CompositeGlyphFlags::WE_HAVE_A_SCALE
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = make_xform([2.0, 0., 0., 1.0]);
        explicit_value
      })
      .compute_flags(),
      CompositeGlyphFlags::WE_HAVE_AN_X_AND_Y_SCALE
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = make_xform([2.0, 0., 1.0, 1.0]);
        explicit_value
      })
      .compute_flags(),
      CompositeGlyphFlags::WE_HAVE_A_TWO_BY_TWO
    );
  }

  #[test]
  fn point_flags_and_marker_bits() {
    let bits = [
      PointFlags::OFF_CURVE_CUBIC,
      PointFlags::ON_CURVE,
      PointMarker::HAS_DELTA.0,
      PointMarker::TOUCHED_X.0,
      PointMarker::TOUCHED_Y.0,
    ];

    for (i, a) in bits.iter().enumerate() {
      for b in bits.iter().skip(i.wrapping_add(1)) {
        assert_eq!(a & b, 0);
      }
    }
  }

  #[test]
  fn cubic_glyf() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CUBIC_GLYF)?;
    let loca = font.loca(None)?;
    let glyf = font.glyf()?;
    let glyph = crate::test_support::fixture_value(
      match loca.get_glyf(
        {
          let Ok(explicit_value) = GlyphId::new(2);
          explicit_value
        },
        &glyf,
      ) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    assert_eq!(glyph.number_of_contours(), Ok(1));
    let simple_glyph = match glyph {
      Glyph::Simple(simple) => simple,
      Glyph::Composite(_) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "expected simple glyph"
        )));
      }
    };
    assert_eq!(
      simple_glyph
        .points()
        .map(|pt| (pt.x, pt.y, pt.membership))
        .collect::<Vec<_>>(),
      &[
        (278, 710, CurveMembership::OnCurve),
        (278, 470, CurveMembership::OnCurve),
        (300, 500, CurveMembership::OffCurve),
        (800, 500, CurveMembership::OffCurve),
        (998, 470, CurveMembership::OnCurve),
        (998, 710, CurveMembership::OnCurve),
      ]
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn avoid_midpoint_overflow() {
    let Ok(a) = F26Dot6::from_bits(1084092352);
    let Ok(b) = F26Dot6::from_bits(1085243712);
    let expected = ::core::primitive::i32::wrapping_div(
      {
        let Ok(value) = (a + b).to_bits();
        value
      },
      2,
    );

    let midpoint = a.midpoint(b);
    assert_eq!(
      ({
        let Ok(value) = midpoint.to_bits();
        value
      }),
      expected
    );
  }

  #[test]
  fn simple_glyph_truncated_data() -> ::core::result::Result<(), crate::read::ReadError> {
    let buf =
      font_test_data::be_buffer! { (100_i16), (0_i16), (0_i16), (0_i16), (0_i16), (0_u16) };

    let glyph = SimpleGlyph::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    assert_eq!(glyph.number_of_contours(), Ok(100));

    assert_eq!(glyph.instruction_length(), Ok(0));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_points_fast_long_flags() -> ::core::result::Result<(), crate::read::ReadError> {
    let buf = font_test_data::be_buffer! { (1_i16), (..([0_i16; 4])), (2_u16), (0_u16), (..([0x39u8, 0x00, 0x39, 0x00, 0x39, 0x00])) };

    let glyph = SimpleGlyph::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    assert_eq!(glyph.num_points(), 3);

    let expected: Vec<_> = glyph
      .points()
      .map(|p| {
        (
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(p.x);
            migrated_value
          }),
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(p.y);
            migrated_value
          }),
        )
      })
      .collect();

    let mut points = vec![Point::default(); 3];
    let mut flags = vec![PointFlags::default(); 3];
    glyph.read_points_fast::<i32>(&mut points, &mut flags)?;
    let actual: Vec<_> = points.iter().map(|p| (p.x, p.y)).collect();

    assert_eq!(actual, expected);
    ::core::result::Result::Ok(())
  }
}
