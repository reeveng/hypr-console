//! Distinct quantities at text layout boundaries.

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "a vertical scroll offset is stored as fractional layout pixels"
  )
)]
pub struct ScrollOffset(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "a line height stores fractional layout pixels"
  )
)]
pub struct LineHeight(pub f32);

impl From<f32> for LineHeight {
  fn from(value: f32) -> Self {
    Self(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the line-height scale is a dimensionless font-size multiplier"
  )
)]
pub struct LineHeightScale(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the vertical coordinate stores fractional layout pixels"
  )
)]
pub struct VerticalPosition(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ByteIndex(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutIndex(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphIndex(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the horizontal offset stores fractional layout pixels"
  )
)]
pub struct HorizontalOffset(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupportedPosition(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnsupportedPosition(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunEnd(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the horizontal coordinate stores fractional layout pixels"
  )
)]
pub struct HorizontalPosition(pub f32);

impl From<f32> for HorizontalPosition {
  fn from(value: f32) -> Self {
    Self(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "a glyph width stores fractional layout pixels"
  )
)]
pub struct GlyphWidth(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordIndex(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the width already laid out is measured in fractional layout pixels"
  )
)]
pub struct UsedWidth(pub f32);

impl From<f32> for UsedWidth {
  fn from(value: f32) -> Self {
    Self(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the available width is measured in fractional layout pixels"
  )
)]
pub struct AvailableWidth(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlankCount(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordCount(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstWordIndex(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstSpanIndex(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpanCount(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the threshold is measured in fractional layout pixels"
  )
)]
pub struct WidthThreshold(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the ellipsis advance is measured in fractional layout pixels"
  )
)]
pub struct EllipsisWidth(pub f32);

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the line width is measured in fractional layout pixels"
  )
)]
pub struct LineWidth(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "a font size is measured in fractional layout pixels"
  )
)]
pub struct FontSize(pub f32);

impl From<f32> for FontSize {
  fn from(value: f32) -> Self {
    Self(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is a dimensionless device-to-layout scale ratio"
  )
)]
pub struct ScaleFactor(pub f32);

impl From<f32> for ScaleFactor {
  fn from(value: f32) -> Self {
    Self(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "a glyph raster coordinate is measured in fractional device pixels"
  )
)]
pub struct SubpixelPosition(pub f32);

impl From<f32> for SubpixelPosition {
  fn from(value: f32) -> Self {
    Self(value)
  }
}
