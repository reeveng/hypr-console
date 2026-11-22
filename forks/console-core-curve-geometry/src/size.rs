// Copyright 2019 the Kurbo Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A 2D size.

#[cfg(feature = "schemars")]
use alloc::string::String;
use console_core_size::Size as Dimensions;
use core::fmt;
use core::ops::{Add, AddAssign, Deref, DerefMut, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

use crate::common::FloatExt;
use crate::{Axis, Rect, RoundedRect, RoundedRectRadii, Vec2};

#[cfg(not(feature = "std"))]
use crate::common::FloatFuncs;

/// A 2D size with operations for curves and rectangles.
///
/// Construct it with [`Size::new`] or convert it from a [`Dimensions<f64>`].
#[repr(transparent)]
#[derive(Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "SizeFields", into = "SizeFields"))]
pub struct Size(Dimensions<f64>);

impl Deref for Size {
    type Target = Dimensions<f64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Size {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Dimensions<f64>> for Size {
    fn from(size: Dimensions<f64>) -> Self {
        Self(size)
    }
}

impl From<Size> for Dimensions<f64> {
    fn from(size: Size) -> Self {
        size.0
    }
}

/// A 2D size.
#[cfg(any(feature = "serde", feature = "schemars"))]
#[cfg_attr(
    feature = "schemars",
    derive(schemars::JsonSchema),
    schemars(rename = "Size")
)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename = "Size")
)]
struct SizeFields {
    /// The width.
    width: f64,
    /// The height.
    height: f64,
}

#[cfg(feature = "serde")]
impl From<SizeFields> for Size {
    fn from(fields: SizeFields) -> Self {
        Self::new(fields.width, fields.height)
    }
}

#[cfg(feature = "serde")]
impl From<Size> for SizeFields {
    fn from(size: Size) -> Self {
        Self {
            width: size.width,
            height: size.height,
        }
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for Size {
    fn schema_name() -> String {
        SizeFields::schema_name()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SizeFields::json_schema(generator)
    }
}

impl Size {
    /// A size with zero width or height.
    pub const ZERO: Size = Size::new(0., 0.);

    /// A size with width and height set to `f64::INFINITY`.
    pub const INFINITY: Size = Size::new(f64::INFINITY, f64::INFINITY);

    /// Create a new `Size` with the provided `width` and `height`.
    #[inline(always)]
    pub const fn new(width: f64, height: f64) -> Self {
        Self(Dimensions { width, height })
    }

    /// Returns the max of `width` and `height`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    /// let size = Size::new(-10.5, 42.0);
    /// assert_eq!(size.max_side(), 42.0);
    /// ```
    pub const fn max_side(self) -> f64 {
        let Ok(answer) = self.0.max_side();
        answer
    }

    /// Returns the min of `width` and `height`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    /// let size = Size::new(-10.5, 42.0);
    /// assert_eq!(size.min_side(), -10.5);
    /// ```
    pub const fn min_side(self) -> f64 {
        let Ok(answer) = self.0.min_side();
        answer
    }

    /// The area covered by this size.
    #[inline]
    pub const fn area(self) -> f64 {
        let Ok(answer) = self.0.area();
        answer
    }

    /// Whether this size has zero area.
    #[doc(alias = "is_empty")]
    #[inline]
    pub const fn is_zero_area(self) -> bool {
        self.area() == 0.0
    }

    /// Returns the component-wise minimum of `self` and `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    ///
    /// let this = Size::new(0., 100.);
    /// let other = Size::new(10., 10.);
    ///
    /// assert_eq!(this.min(other), Size::new(0., 10.));
    /// ```
    pub const fn min(self, other: Size) -> Self {
        Self::new(
            self.0.width.min(other.0.width),
            self.0.height.min(other.0.height),
        )
    }

    /// Returns the component-wise maximum of `self` and `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    ///
    /// let this = Size::new(0., 100.);
    /// let other = Size::new(10., 10.);
    ///
    /// assert_eq!(this.max(other), Size::new(10., 100.));
    /// ```
    pub const fn max(self, other: Size) -> Self {
        Self::new(
            self.0.width.max(other.0.width),
            self.0.height.max(other.0.height),
        )
    }

    /// Returns a new size bounded by `min` and `max.`
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    ///
    /// let this = Size::new(0., 100.);
    /// let min = Size::new(10., 10.,);
    /// let max = Size::new(50., 50.);
    /// assert_eq!(this.clamp(min, max), Size::new(10., 50.))
    /// ```
    pub const fn clamp(self, min: Size, max: Size) -> Self {
        self.max(min).min(max)
    }

    /// Convert this size into a [`Vec2`], with `width` mapped to `x` and `height`
    /// mapped to `y`.
    #[inline(always)]
    pub const fn to_vec2(self) -> Vec2 {
        Vec2::new(self.0.width, self.0.height)
    }

    /// Returns a new `Size`,
    /// with `width` and `height` [rounded] to the nearest integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    /// let size_pos = Size::new(3.3, 3.6).round();
    /// assert_eq!(size_pos.width, 3.0);
    /// assert_eq!(size_pos.height, 4.0);
    /// let size_neg = Size::new(-3.3, -3.6).round();
    /// assert_eq!(size_neg.width, -3.0);
    /// assert_eq!(size_neg.height, -4.0);
    /// ```
    ///
    /// [rounded]: f64::round
    #[inline]
    pub fn round(self) -> Size {
        Size::new(self.0.width.round(), self.0.height.round())
    }

    /// Returns a new `Size`,
    /// with `width` and `height` [rounded up] to the nearest integer,
    /// unless they are already an integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    /// let size_pos = Size::new(3.3, 3.6).ceil();
    /// assert_eq!(size_pos.width, 4.0);
    /// assert_eq!(size_pos.height, 4.0);
    /// let size_neg = Size::new(-3.3, -3.6).ceil();
    /// assert_eq!(size_neg.width, -3.0);
    /// assert_eq!(size_neg.height, -3.0);
    /// ```
    ///
    /// [rounded up]: f64::ceil
    #[inline]
    pub fn ceil(self) -> Size {
        Size::new(self.0.width.ceil(), self.0.height.ceil())
    }

    /// Returns a new `Size`,
    /// with `width` and `height` [rounded down] to the nearest integer,
    /// unless they are already an integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    /// let size_pos = Size::new(3.3, 3.6).floor();
    /// assert_eq!(size_pos.width, 3.0);
    /// assert_eq!(size_pos.height, 3.0);
    /// let size_neg = Size::new(-3.3, -3.6).floor();
    /// assert_eq!(size_neg.width, -4.0);
    /// assert_eq!(size_neg.height, -4.0);
    /// ```
    ///
    /// [rounded down]: f64::floor
    #[inline]
    pub fn floor(self) -> Size {
        Size::new(self.0.width.floor(), self.0.height.floor())
    }

    /// Returns a new `Size`,
    /// with `width` and `height` [rounded away] from zero to the nearest integer,
    /// unless they are already an integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    /// let size_pos = Size::new(3.3, 3.6).expand();
    /// assert_eq!(size_pos.width, 4.0);
    /// assert_eq!(size_pos.height, 4.0);
    /// let size_neg = Size::new(-3.3, -3.6).expand();
    /// assert_eq!(size_neg.width, -4.0);
    /// assert_eq!(size_neg.height, -4.0);
    /// ```
    ///
    /// [rounded away]: FloatExt::expand
    #[inline]
    pub fn expand(self) -> Size {
        Size::new(self.0.width.expand(), self.0.height.expand())
    }

    /// Returns a new `Size`,
    /// with `width` and `height` [rounded towards] zero to the nearest integer,
    /// unless they are already an integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kurbo::Size;
    /// let size_pos = Size::new(3.3, 3.6).trunc();
    /// assert_eq!(size_pos.width, 3.0);
    /// assert_eq!(size_pos.height, 3.0);
    /// let size_neg = Size::new(-3.3, -3.6).trunc();
    /// assert_eq!(size_neg.width, -3.0);
    /// assert_eq!(size_neg.height, -3.0);
    /// ```
    ///
    /// [rounded towards]: f64::trunc
    #[inline]
    pub fn trunc(self) -> Size {
        Size::new(self.0.width.trunc(), self.0.height.trunc())
    }

    /// Returns the aspect ratio of a rectangle with this size.
    ///
    /// The aspect ratio is the ratio of the width to the height.
    ///
    /// If the height is `0`, the output will be `sign(self.0.width) * infinity`. If the width and
    /// height are both `0`, then the output will be `NaN`.
    #[inline]
    pub const fn aspect_ratio_width(self) -> f64 {
        // ratio is determined by width / height
        // https://en.wikipedia.org/wiki/Aspect_ratio_(image)
        // https://en.wikipedia.org/wiki/Ratio
        self.0.width / self.0.height
    }

    /// Returns **the inverse** of the aspect ratio of a rectangle with this size.
    ///
    /// Aspect ratios are usually defined as the ratio of the width to the height, but
    /// this method incorrectly returns the ratio of height to width.
    /// You should generally prefer [`aspect_ratio_width`](Self::aspect_ratio_width).
    ///
    /// If the width is `0`, the output will be `sign(self.0.height) * infinity`. If the width and
    /// height are both `0`, then the output will be `NaN`.
    #[deprecated(
        note = "You should use `aspect_ratio_width` instead, as this method returns a potentially unexpected value.",
        since = "0.12.0"
    )]
    #[inline]
    // TODO: When we remove this, we should also work out what to do with aspect_ratio_width
    // The tentatitive plan is to rename `aspect_ratio_width` back to this name
    pub const fn aspect_ratio(self) -> f64 {
        self.0.height / self.0.width
    }

    /// Convert this `Size` into a [`Rect`] with origin `(0.0, 0.0)`.
    #[inline(always)]
    pub const fn to_rect(self) -> Rect {
        Rect::new(0., 0., self.0.width, self.0.height)
    }

    /// Convert this `Size` into a [`RoundedRect`] with origin `(0.0, 0.0)` and
    /// the provided corner radius.
    #[inline]
    pub fn to_rounded_rect(self, radii: impl Into<RoundedRectRadii>) -> RoundedRect {
        self.to_rect().to_rounded_rect(radii)
    }

    /// Is this size [finite]?
    ///
    /// [finite]: f64::is_finite
    #[inline]
    pub const fn is_finite(self) -> bool {
        self.0.width.is_finite() && self.0.height.is_finite()
    }

    /// Is this size [NaN]?
    ///
    /// [NaN]: f64::is_nan
    #[inline]
    pub const fn is_nan(self) -> bool {
        self.0.width.is_nan() || self.0.height.is_nan()
    }

    /// Get the member matching the given axis.
    #[inline]
    pub const fn get_coord(self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.0.width,
            Axis::Vertical => self.0.height,
        }
    }

    /// Get a mutable reference to the member matching the given axis.
    #[inline]
    pub const fn get_coord_mut(&mut self, axis: Axis) -> &mut f64 {
        match axis {
            Axis::Horizontal => &mut self.0.width,
            Axis::Vertical => &mut self.0.height,
        }
    }

    /// Set the member matching the given axis to the given value.
    #[inline]
    pub const fn set_coord(&mut self, axis: Axis, value: f64) {
        match axis {
            Axis::Horizontal => self.0.width = value,
            Axis::Vertical => self.0.height = value,
        }
    }
}

impl fmt::Debug for Size {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}W×{:?}H", self.width, self.height)
    }
}

impl fmt::Display for Size {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "(")?;
        fmt::Display::fmt(&self.width, formatter)?;
        write!(formatter, "×")?;
        fmt::Display::fmt(&self.height, formatter)?;
        write!(formatter, ")")
    }
}

impl MulAssign<f64> for Size {
    #[inline]
    fn mul_assign(&mut self, other: f64) {
        *self = *self * other;
    }
}

impl Mul<Size> for f64 {
    type Output = Size;

    #[inline]
    fn mul(self, other: Size) -> Size {
        other * self
    }
}

impl Mul<f64> for Size {
    type Output = Size;

    #[inline]
    fn mul(self, other: f64) -> Size {
        Self::new(self.width * other, self.height * other)
    }
}

impl DivAssign<f64> for Size {
    #[inline]
    fn div_assign(&mut self, other: f64) {
        *self = *self / other;
    }
}

impl Div<f64> for Size {
    type Output = Size;

    #[inline]
    fn div(self, other: f64) -> Size {
        Self::new(self.width / other, self.height / other)
    }
}

impl Add<Size> for Size {
    type Output = Size;
    #[inline]
    fn add(self, other: Size) -> Size {
        Self::new(self.width + other.width, self.height + other.height)
    }
}

impl AddAssign<Size> for Size {
    #[inline]
    fn add_assign(&mut self, other: Size) {
        *self = *self + other;
    }
}

impl Sub<Size> for Size {
    type Output = Size;
    #[inline]
    fn sub(self, other: Size) -> Size {
        Self::new(self.width - other.width, self.height - other.height)
    }
}

impl SubAssign<Size> for Size {
    #[inline]
    fn sub_assign(&mut self, other: Size) {
        *self = *self - other;
    }
}

impl From<(f64, f64)> for Size {
    #[inline(always)]
    fn from(v: (f64, f64)) -> Size {
        Self::new(v.0, v.1)
    }
}

impl From<Size> for (f64, f64) {
    #[inline(always)]
    fn from(v: Size) -> (f64, f64) {
        (v.width, v.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_dimensions_can_be_read_and_changed_through_a_curve_size() {
        let original = Dimensions {
            width: 3.0,
            height: 5.0,
        };
        let mut size = Size::from(original);
        size.width = 7.0;

        assert_eq!(size.to_rect(), Rect::new(0.0, 0.0, 7.0, 5.0));
        assert_eq!(
            Dimensions::from(size),
            Dimensions {
                width: 7.0,
                height: 5.0
            }
        );
    }

    #[test]
    fn display() {
        let s = Size::new(-0.12345, 9.87654);
        assert_eq!(format!("{s}"), "(-0.12345×9.87654)");

        let s = Size::new(-0.12345, 9.87654);
        assert_eq!(format!("{s:+6.2}"), "( -0.12× +9.88)");
    }

    #[test]
    #[expect(deprecated, reason = "Testing deprecated function.")]
    fn aspect_ratio() {
        let s = Size::new(1.0, 1.0);
        assert!((s.aspect_ratio() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn aspect_ratio_width() {
        let s = Size::new(1.0, 1.0);
        assert!((s.aspect_ratio_width() - 1.0).abs() < 1e-6);

        // 3:2 film (mm)
        let s = Size::new(36.0, 24.0);
        assert!((s.aspect_ratio_width() - 1.5).abs() < 1e-6);
        // 4k screen
        let s = Size::new(3840.0, 2160.0);
        assert!((s.aspect_ratio_width() - (16. / 9.)).abs() < 1e-6);
    }
}
