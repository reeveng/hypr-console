//! the [VARC (Variable Composite/Component)](https://github.com/harfbuzz/boring-expansion-spec/blob/main/VARC.md) table

pub use super::layout::{Condition, CoverageTable};
use super::variations::PackedDeltas;
pub use crate::ps::cff::index::Index2;
use crate::types::{F2Dot14, Matrix};

include!("../../generated/generated_varc.rs");

trait Get<'a> {
  fn get(self, nth: u64) -> Result<&'a [u8], ReadError>;
}

impl<'a> Get<'a> for Result<Option<Index2<'a>>, ReadError> {
  fn get(self, nth: u64) -> Result<&'a [u8], ReadError> {
    {
      let maybe_index = self?;
      maybe_index
    }
    .ok_or(ReadError::NullOffset)
    .and_then(|index| index.get(nth).map_err(|_| ReadError::OutOfBounds))
  }
}

impl Varc<'_> {
  pub fn axis_indices(&self, nth: u64) -> Result<PackedDeltas<'_>, ReadError> {
    let raw = self.axis_indices_list().get(nth)?;
    Ok(PackedDeltas::consume_all(
      <crate::font_data::FontData<'_> as ::core::convert::From<&[::core::primitive::u8]>>::from(
        raw,
      ),
    ))
  }

  pub fn glyph(&self, nth: u64) -> Result<VarcGlyph<'_>, ReadError> {
    let var_composite_glyphs = self.var_composite_glyphs()?;
    let raw = var_composite_glyphs
      .get(nth)
      .map_err(|_| ReadError::OutOfBounds)?;
    Ok(VarcGlyph {
      table: self,
      data:
        <crate::font_data::FontData<'_> as ::core::convert::From<&[::core::primitive::u8]>>::from(
          raw,
        ),
    })
  }
}

impl SparseVariationRegion<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `SparseVariationRegion::compute_scalar_f32` method returns `f32` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the sparse-region API returns a dimensionless binary32 interpolation weight"
    )
  )]
  pub fn compute_scalar_f32(&self, coords: &[F2Dot14]) -> f32 {
    let mut scalar = 1.0f32;

    let Ok(region_axes) = self.region_axes();

    for axis in region_axes {
      let Ok(peak) = axis.peak();

      match ::core::primitive::bool::then_some(peak == F2Dot14::ZERO, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let axis_index = {
        let Ok(axis_axis_index) = axis.axis_index();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(axis_axis_index);
        migrated_value
      };
      let Ok(value_26) = console_core_number_conversion::index(axis_index);

      let coord = match coords.get(value_26).copied() {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => F2Dot14::ZERO,
      };

      match ::core::primitive::bool::then_some(coord == peak, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(coord == F2Dot14::ZERO, ()) {
        ::core::option::Option::Some(()) => {
          return 0.0;
        }
        ::core::option::Option::None => {}
      }

      let Ok(start) = axis.start();
      let Ok(end) = axis.end();

      match ::core::primitive::bool::then_some(
        start > peak || peak > end || (start < F2Dot14::ZERO && end > F2Dot14::ZERO),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      match (start..=end).contains(&coord).then_some(()) {
        Some(()) => {}
        None => return 0.0,
      }

      let Ok(coordinate) = coord.to_bits();
      let Ok(peak_bits) = peak.to_bits();
      let (numerator, denominator) = match coord.cmp(&peak) {
        core::cmp::Ordering::Less => {
          let Ok(start_bits) = start.to_bits();
          (
            coordinate.wrapping_sub(start_bits),
            peak_bits.wrapping_sub(start_bits),
          )
        }
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
          let Ok(end_bits) = end.to_bits();
          (
            end_bits.wrapping_sub(coordinate),
            end_bits.wrapping_sub(peak_bits),
          )
        }
      };

      match numerator {
        0 => return 0.0,
        _nonzero => scalar *= f32::from(numerator) / f32::from(denominator),
      }
    }

    scalar
  }
}

pub struct VarcGlyph<'a> {
  table: &'a Varc<'a>,
  data: FontData<'a>,
}

impl<'a> VarcGlyph<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcGlyph::components` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn components(
    &self,
  ) -> impl Iterator<Item = Result<VarcComponent<'a>, ReadError>> + use<'a> {
    let Ok(value_9) = self.data.cursor();

    VarcComponentIter {
      table: self.table,
      cursor: value_9,
    }
  }
}

struct VarcComponentIter<'a> {
  table: &'a Varc<'a>,
  cursor: Cursor<'a>,
}

impl<'a> Iterator for VarcComponentIter<'a> {
  type Item = Result<VarcComponent<'a>, ReadError>;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(remaining) = self.cursor.is_empty();

    match remaining {
      crate::Emptiness::Empty => return None,
      crate::Emptiness::NonEmpty => {}
    }

    Some(VarcComponent::parse(self.table, &mut self.cursor))
  }
}

pub struct VarcComponent<'a> {
  flags: VarcFlags,
  gid: GlyphId,
  condition_index: Option<u32>,
  axis_indices_index: Option<u32>,
  axis_values: Option<PackedDeltas<'a>>,
  axis_values_var_index: Option<u32>,
  transform_var_index: Option<u32>,
  transform: DecomposedTransform,
}

fn read_component_deltas<'a>(
  cursor: &mut Cursor<'a>,
  count: u64,
) -> Result<Option<PackedDeltas<'a>>, ReadError> {
  match count {
    0 => return Ok(None),
    _positive => {}
  }

  let Ok(remaining) = cursor.remaining();
  let data = remaining.ok_or(ReadError::OutOfBounds)?;
  let Ok(deltas) = PackedDeltas::new(data, count);
  let Ok(end) = deltas.iter().end();
  *cursor = end;

  Ok(Some(deltas))
}

impl<'a> VarcComponent<'a> {
  fn parse(table: &Varc, cursor: &mut Cursor<'a>) -> Result<Self, ReadError> {
    let raw_flags = cursor.read_u32_var()?;
    let Ok(flags) = VarcFlags::from_bits_truncate(raw_flags);

    let Ok(gid_is_24bit) = flags.contains(VarcFlags::GID_IS_24BIT);

    let gid = match gid_is_24bit {
      Presence::Present => {
        let read = cursor.read::<Uint24>()?;
        let Ok(value_11) = read.to_u32();

        let Ok(value_25) = GlyphId::new(value_11);

        value_25
      }
      Presence::Absent => {
        let read = cursor.read::<u16>()?;

        GlyphId::from(read)
      }
    };

    let Ok(have_condition) = flags.contains(VarcFlags::HAVE_CONDITION);

    let condition_index = match have_condition {
      Presence::Present => {
        let u32_var = cursor.read_u32_var()?;

        Some(u32_var)
      }
      Presence::Absent => None,
    };

    let Ok(have_axes) = flags.contains(VarcFlags::HAVE_AXES);

    let (axis_indices_index, axis_values) = match have_axes {
      Presence::Present => {
        let axis_indices_index = cursor.read_u32_var()?;
        let axis_indices = table.axis_indices({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(axis_indices_index);
          migrated_value
        })?;
        let num_axis_values = axis_indices.count_or_compute();

        let deltas = read_component_deltas(cursor, num_axis_values)?;
        (Some(axis_indices_index), deltas)
      }
      Presence::Absent => (None, None),
    };

    let Ok(axis_values_have_variation) = flags.contains(VarcFlags::AXIS_VALUES_HAVE_VARIATION);

    let axis_values_var_index = match axis_values_have_variation {
      Presence::Present => {
        let u32_var = cursor.read_u32_var()?;

        Some(u32_var)
      }
      Presence::Absent => None,
    };

    let Ok(transform_has_variation) = flags.contains(VarcFlags::TRANSFORM_HAS_VARIATION);

    let transform_var_index = match transform_has_variation {
      Presence::Present => {
        let u32_var = cursor.read_u32_var()?;

        Some(u32_var)
      }
      Presence::Absent => None,
    };

    let mut transform = DecomposedTransform::default();
    let Ok(intersecting) =
      flags.intersects(VarcFlags::HAVE_TRANSLATE_X | VarcFlags::HAVE_TRANSLATE_Y);

    match intersecting {
      Overlap::Intersecting => {
        let Ok(have_translate_x) = flags.contains(VarcFlags::HAVE_TRANSLATE_X);

        match have_translate_x {
          Presence::Present => {
            let read = cursor.read::<FWord>()?;
            let Ok(value_12) = read.to_i16();

            transform.translate_x = FontUnits(f32::from(value_12))
          }
          Presence::Absent => {}
        }

        let Ok(have_translate_y) = flags.contains(VarcFlags::HAVE_TRANSLATE_Y);

        match have_translate_y {
          Presence::Present => {
            let read = cursor.read::<FWord>()?;
            let Ok(value_13) = read.to_i16();

            transform.translate_y = FontUnits(f32::from(value_13))
          }
          Presence::Absent => {}
        }
      }
      Overlap::Disjoint => {}
    }

    let Ok(have_rotation) = flags.contains(VarcFlags::HAVE_ROTATION);

    match have_rotation {
      Presence::Present => {
        let read = cursor.read::<F4Dot12>()?;
        let Ok(value_14) = read.to_f32();

        transform.rotation = HalfTurns(value_14)
      }
      Presence::Absent => {}
    }

    let Ok(intersecting_flag) =
      flags.intersects(VarcFlags::HAVE_SCALE_X | VarcFlags::HAVE_SCALE_Y);

    match intersecting_flag {
      Overlap::Intersecting => {
        let Ok(have_scale_x) = flags.contains(VarcFlags::HAVE_SCALE_X);

        match have_scale_x {
          Presence::Present => {
            let read = cursor.read::<F6Dot10>()?;
            let Ok(value_15) = read.to_f32();

            transform.scale_x = TransformScale(value_15)
          }
          Presence::Absent => {}
        }

        let Ok(have_scale_y) = flags.contains(VarcFlags::HAVE_SCALE_Y);

        transform.scale_y = match have_scale_y {
          Presence::Present => {
            let read = cursor.read::<F6Dot10>()?;
            let Ok(value_16) = read.to_f32();

            TransformScale(value_16)
          }
          Presence::Absent => transform.scale_x,
        };
      }
      Overlap::Disjoint => {}
    }

    let Ok(intersecting_flag_2) =
      flags.intersects(VarcFlags::HAVE_SKEW_X | VarcFlags::HAVE_SKEW_Y);

    match intersecting_flag_2 {
      Overlap::Intersecting => {
        let Ok(have_skew_x) = flags.contains(VarcFlags::HAVE_SKEW_X);

        match have_skew_x {
          Presence::Present => {
            let read = cursor.read::<F4Dot12>()?;
            let Ok(value_17) = read.to_f32();

            transform.skew_x = HalfTurns(value_17)
          }
          Presence::Absent => {}
        }

        let Ok(have_skew_y) = flags.contains(VarcFlags::HAVE_SKEW_Y);

        match have_skew_y {
          Presence::Present => {
            let read = cursor.read::<F4Dot12>()?;
            let Ok(value_18) = read.to_f32();

            transform.skew_y = HalfTurns(value_18)
          }
          Presence::Absent => {}
        }
      }
      Overlap::Disjoint => {}
    }

    let Ok(intersecting_2) =
      flags.intersects(VarcFlags::HAVE_TCENTER_X | VarcFlags::HAVE_TCENTER_Y);

    match intersecting_2 {
      Overlap::Intersecting => {
        let Ok(have_tcenter_x) = flags.contains(VarcFlags::HAVE_TCENTER_X);

        match have_tcenter_x {
          Presence::Present => {
            let read = cursor.read::<FWord>()?;
            let Ok(value_19) = read.to_i16();

            transform.center_x = FontUnits(f32::from(value_19))
          }
          Presence::Absent => {}
        }

        let Ok(have_tcenter_y) = flags.contains(VarcFlags::HAVE_TCENTER_Y);

        match have_tcenter_y {
          Presence::Present => {
            let read = cursor.read::<FWord>()?;
            let Ok(value_20) = read.to_i16();

            transform.center_y = FontUnits(f32::from(value_20))
          }
          Presence::Absent => {}
        }
      }
      Overlap::Disjoint => {}
    }

    let reserved = raw_flags & VarcFlags::RESERVED_MASK.bits;

    match ::core::primitive::bool::then_some(reserved != 0, ()) {
      ::core::option::Option::Some(()) => {
        let num_reserved = reserved.count_ones();

        for _ in 0..num_reserved {
          cursor.read_u32_var()?;
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(VarcComponent {
      flags,
      gid,
      condition_index,
      axis_indices_index,
      axis_values,
      axis_values_var_index,
      transform_var_index,
      transform,
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcComponent::flags` method returns `VarcFlags` directly in the existing parser interface"
    )
  )]
  pub fn flags(&self) -> VarcFlags {
    self.flags
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcComponent::gid` method returns `GlyphId` directly in the existing parser interface"
    )
  )]
  pub fn gid(&self) -> GlyphId {
    self.gid
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcComponent::condition_index` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn condition_index(&self) -> Option<u32> {
    self.condition_index
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcComponent::transform` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn transform(&self) -> &DecomposedTransform {
    &self.transform
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcComponent::axis_indices_index` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn axis_indices_index(&self) -> Option<u32> {
    self.axis_indices_index
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcComponent::axis_values` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn axis_values(&self) -> Option<&PackedDeltas<'a>> {
    self.axis_values.as_ref()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcComponent::axis_values_var_index` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn axis_values_var_index(&self) -> Option<u32> {
    self.axis_values_var_index
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VarcComponent::transform_var_index` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn transform_var_index(&self) -> Option<u32> {
    self.transform_var_index
  }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the transparent representation stores fractional OpenType design units in binary32"
  )
)]
struct FontUnits(f32);

#[derive(Clone, Copy)]
#[repr(transparent)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the transparent representation stores angles as multiples of π without converting their precision"
  )
)]
struct HalfTurns(f32);

#[derive(Clone, Copy)]
#[repr(transparent)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the scale factor is a dimensionless binary32 ratio"
  )
)]
struct TransformScale(f32);

#[derive(Clone, Copy)]
pub struct DecomposedTransform {
  translate_x: FontUnits,
  translate_y: FontUnits,
  rotation: HalfTurns,
  scale_x: TransformScale,
  scale_y: TransformScale,
  skew_x: HalfTurns,
  skew_y: HalfTurns,
  center_x: FontUnits,
  center_y: FontUnits,
}

impl Default for DecomposedTransform {
  fn default() -> Self {
    Self {
      translate_x: FontUnits(0.0),
      translate_y: FontUnits(0.0),
      rotation: HalfTurns(0.0),
      scale_x: TransformScale(1.0),
      scale_y: TransformScale(1.0),
      skew_x: HalfTurns(0.0),
      skew_y: HalfTurns(0.0),
      center_x: FontUnits(0.0),
      center_y: FontUnits(0.0),
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "font-metrics applies VARC deltas through the existing scalar accessors and setters"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "font-metrics consumes the published raw VARC coordinate, half-turn, and scale accessors"
  )
)]
impl DecomposedTransform {
  #[must_use]
  pub fn translate_x(&self) -> f32 {
    self.translate_x.0
  }

  #[must_use]
  pub fn translate_y(&self) -> f32 {
    self.translate_y.0
  }

  #[must_use]
  pub fn rotation(&self) -> f32 {
    self.rotation.0
  }

  #[must_use]
  pub fn scale_x(&self) -> f32 {
    self.scale_x.0
  }

  #[must_use]
  pub fn scale_y(&self) -> f32 {
    self.scale_y.0
  }

  #[must_use]
  pub fn skew_x(&self) -> f32 {
    self.skew_x.0
  }

  #[must_use]
  pub fn skew_y(&self) -> f32 {
    self.skew_y.0
  }

  #[must_use]
  pub fn center_x(&self) -> f32 {
    self.center_x.0
  }

  #[must_use]
  pub fn center_y(&self) -> f32 {
    self.center_y.0
  }

  pub fn set_translate_x(&mut self, value: f32) {
    self.translate_x = FontUnits(value);
  }

  pub fn set_translate_y(&mut self, value: f32) {
    self.translate_y = FontUnits(value);
  }

  pub fn set_rotation(&mut self, value: f32) {
    self.rotation = HalfTurns(value);
  }

  pub fn set_scale_x(&mut self, value: f32) {
    self.scale_x = TransformScale(value);
  }

  pub fn set_scale_y(&mut self, value: f32) {
    self.scale_y = TransformScale(value);
  }

  pub fn set_skew_x(&mut self, value: f32) {
    self.skew_x = HalfTurns(value);
  }

  pub fn set_skew_y(&mut self, value: f32) {
    self.skew_y = HalfTurns(value);
  }

  pub fn set_center_x(&mut self, value: f32) {
    self.center_x = FontUnits(value);
  }

  pub fn set_center_y(&mut self, value: f32) {
    self.center_y = FontUnits(value);
  }
}

impl DecomposedTransform {
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published VARC transform API hands font-metrics its affine matrix directly"
    )
  )]
  #[must_use]
  pub fn matrix(&self) -> Matrix<f32> {
    let Self {
      translate_x: FontUnits(translate_x),
      translate_y: FontUnits(translate_y),
      rotation: HalfTurns(rotation),
      scale_x: TransformScale(scale_x),
      scale_y: TransformScale(scale_y),
      skew_x: HalfTurns(skew_x),
      skew_y: HalfTurns(skew_y),
      center_x: FontUnits(center_x),
      center_y: FontUnits(center_y),
    } = *self;

    let mut transform = Matrix::IDENTITY;
    transform.dx = translate_x + center_x;
    transform.dy = translate_y + center_y;

    use core::num::FpCategory;

    match rotation.classify() {
      FpCategory::Zero => {}
      FpCategory::Normal | FpCategory::Subnormal | FpCategory::Infinite | FpCategory::Nan => {
        let angle = rotation * core::f32::consts::PI;
        #[cfg(feature = "libm")]
        let (s, c) = (libm::sinf(angle), libm::cosf(angle));
        #[cfg(not(feature = "libm"))]
        let (s, c) = angle.sin_cos();

        let Ok(value_21) = Matrix::from_elements([c, s, -s, c, 0.0, 0.0]);

        transform *= value_21;
      }
    }

    match ::core::primitive::bool::then_some((scale_x, scale_y) != (1.0, 1.0), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_22) = Matrix::from_elements([scale_x, 0.0, 0.0, scale_y, 0.0, 0.0]);

        transform *= value_22;
      }
      ::core::option::Option::None => {}
    }

    match (skew_x.classify(), skew_y.classify()) {
      (FpCategory::Zero, FpCategory::Zero) => {}
      (_, _) => {
        let skew_x_radians = -skew_x * core::f32::consts::PI;
        let skew_y_radians = skew_y * core::f32::consts::PI;
        #[cfg(feature = "libm")]
        let (tan_x, tan_y) = (libm::tanf(skew_x_radians), libm::tanf(skew_y_radians));
        #[cfg(not(feature = "libm"))]
        let (tan_x, tan_y) = (skew_x_radians.tan(), skew_y_radians.tan());

        let Ok(value_23) = Matrix::from_elements([1.0, tan_y, tan_x, 1.0, 0.0, 0.0]);

        transform *= value_23
      }
    }

    match ::core::primitive::bool::then_some((center_x, center_y) != (0.0, 0.0), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_24) = Matrix::from_elements([1.0, 0.0, 0.0, 1.0, -center_x, -center_y]);

        transform *= value_24;
      }
      ::core::option::Option::None => {}
    }

    transform
  }
}

impl<'a> MultiItemVariationData<'a> {
  pub fn delta_sets(&self) -> Result<Index2<'a>, ReadError> {
    let Ok(raw_delta_sets) = self.raw_delta_sets();

    Index2::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(raw_delta_sets))
  }

  pub fn delta_set(&self, i: u64) -> Result<PackedDeltas<'a>, ReadError> {
    let index = self.delta_sets()?;
    let raw_deltas = index.get(i).map_err(|_| ReadError::OutOfBounds)?;
    Ok(PackedDeltas::consume_all(
      <crate::font_data::FontData<'_> as ::core::convert::From<&[::core::primitive::u8]>>::from(
        raw_deltas,
      ),
    ))
  }
}

#[cfg(test)]
mod tests {
  use types::GlyphId16;

  use crate::FontData;
  use crate::types::{F2Dot14, Matrix};
  use crate::{FontRef, ReadError, TableProvider};

  use super::{Condition, DecomposedTransform, FontUnits, HalfTurns, TransformScale, Varc};

  impl Varc<'_> {
    fn conditions(&self) -> Result<impl Iterator<Item = Condition<'_>>, ReadError> {
      let condition_list = self.condition_list()?;
      let list = condition_list.ok_or(ReadError::NullOffset)?;
      let Ok(conditions) = list.conditions();
      let Ok(records) = conditions.iter();
      let checked = records.collect::<Result<Vec<_>, _>>()?;

      Ok(checked.into_iter())
    }

    fn axis_indices_count(&self) -> Result<u64, ReadError> {
      let own_axis_indices_list = self.axis_indices_list()?;
      let axis_indices_list = match own_axis_indices_list {
        Some(axis_indices_list) => axis_indices_list,
        None => {
          return Ok(0);
        }
      };

      Ok({
        let Ok(count) = axis_indices_list.count();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(count);
        migrated_value
      })
    }
  }

  fn rounded_matrix(matrix: Matrix<f32>) -> Result<Matrix<f32>, console_core_never::Never> {
    matrix.map(|value| (value * 1_000_000.0).round() / 1_000_000.0)
  }

  fn assert_close(
    AssertCloseArguments {
      actual: TransformScale(actual),
      expected: TransformScale(expected),
    }: AssertCloseArguments,
  ) -> ::core::result::Result<(), ::console_core_never::Never> {
    let diff = (actual - expected).abs();
    assert!(
      diff <= 1e-6,
      "expected {expected}, got {actual}, diff {diff}"
    );
    ::core::result::Result::Ok(())
  }

  struct AssertCloseArguments {
    actual: TransformScale,
    expected: TransformScale,
  }

  fn sfnt_table_range(data: &[u8], tag: [u8; 4]) -> Result<(u64, u64), ReadError> {
    let font = FontRef::new(data)?;
    let Ok(directory) = font.table_directory();
    let Ok(records) = directory.table_records();
    let Ok(table_tag) = crate::types::Tag::new(&tag);
    let record = records
      .iter()
      .find(|record| record.tag() == Ok(table_tag))
      .ok_or(ReadError::TableIsMissing(table_tag))?;
    let Ok(start) = record.offset();
    let Ok(length) = record.length();
    Ok((u64::from(start), u64::from(length)))
  }

  fn patch_bytes<const N: usize>(
    dst: &mut [u8],
    offset: u64,
    value: [u8; N],
  ) -> Result<(), ReadError> {
    let Ok(start) = console_core_number_conversion::index(offset);
    let remaining = dst.get_mut(start..).ok_or(ReadError::OutOfBounds)?;
    let field = remaining
      .first_chunk_mut::<N>()
      .ok_or(ReadError::OutOfBounds)?;
    *field = value;

    Ok(())
  }

  fn font_with_empty_axis_indices() -> Result<Vec<u8>, ReadError> {
    let mut bytes = font_test_data::varc::CJK_6868.to_vec();
    let (varc_offset, _) = sfnt_table_range(&bytes, *b"VARC")?;
    let field = varc_offset.checked_add(16).ok_or(ReadError::OutOfBounds)?;
    let Ok(data) = FontData::new(&bytes);
    let relative = data.read_at::<u32>(field)?;
    let axis_indices = varc_offset
      .checked_add(u64::from(relative))
      .ok_or(ReadError::OutOfBounds)?;
    patch_bytes(&mut bytes, axis_indices, [0, 0, 0, 1, 1, 1, 1])?;

    Ok(bytes)
  }

  #[must_use]
  fn encode_u32_var(value: u32) -> Result<Vec<u8>, console_core_never::Never> {
    let [a, b, c, d] = value.to_be_bytes();
    Ok(match value {
      0..=0x7F => vec![d],
      0x80..=0x3FFF => vec![0x80 | c, d],
      0x4000..=0x1F_FFFF => vec![0xC0 | b, c, d],
      0x20_0000..=0xFFF_FFFF => vec![0xE0 | a, b, c, d],
      _long => vec![0xF0, a, b, c, d],
    })
  }

  #[test]
  fn read_cjk_0x6868() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;
    table.coverage()?;
    ::core::result::Result::Ok(())
  }

  #[test]
  fn identify_all_conditional_types() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;
    let conditions = table.conditions()?;

    assert_eq!(
      (1..=5).collect::<Vec<_>>(),
      conditions
        .map(|c| {
          let Ok(c_format) = c.format();

          c_format
        })
        .collect::<Vec<_>>()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_condition_format1_axis_range()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;

    let mut conditions = table.conditions()?;

    let record = crate::test_support::fixture_value(
      conditions.find(|condition| condition.format() == Ok(1)),
      "VARC condition format 1",
    )?;
    let condition = match record {
      Condition::Format1AxisRange(condition) => condition,
      Condition::Format2VariableValue(_)
      | Condition::Format3And(_)
      | Condition::Format4Or(_)
      | Condition::Format5Negate(_) => {
        return Err(crate::test_support::Failure::from(
          "expected VARC condition format 1",
        ));
      }
    };

    let Ok(axis_index) = condition.axis_index();
    assert_eq!(
      (0, 0.5, 1.0),
      (
        axis_index,
        ({
          let Ok(filter_range_min_value) = condition.filter_range_min_value();
          let Ok(value) = filter_range_min_value.to_f32();
          value
        }),
        ({
          let Ok(filter_range_max_value) = condition.filter_range_max_value();
          let Ok(value) = filter_range_max_value.to_f32();
          value
        }),
      )
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_condition_format2_variable_value()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;

    let mut conditions = table.conditions()?;

    let record = crate::test_support::fixture_value(
      conditions.find(|condition| condition.format() == Ok(2)),
      "VARC condition format 2",
    )?;
    let condition = match record {
      Condition::Format2VariableValue(condition) => condition,
      Condition::Format1AxisRange(_)
      | Condition::Format3And(_)
      | Condition::Format4Or(_)
      | Condition::Format5Negate(_) => {
        return Err(crate::test_support::Failure::from(
          "expected VARC condition format 2",
        ));
      }
    };

    assert_eq!(
      (Ok(1), Ok(2)),
      (condition.default_value(), condition.var_index(),)
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_condition_format3_and() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;

    let mut conditions = table.conditions()?;

    let record = crate::test_support::fixture_value(
      conditions.find(|condition| condition.format() == Ok(3)),
      "VARC condition format 3",
    )?;
    let condition = match record {
      Condition::Format3And(condition) => condition,
      Condition::Format1AxisRange(_)
      | Condition::Format2VariableValue(_)
      | Condition::Format4Or(_)
      | Condition::Format5Negate(_) => {
        return Err(crate::test_support::Failure::from(
          "expected VARC condition format 3",
        ));
      }
    };

    let Ok(child_conditions) = condition.conditions();
    let Ok(children) = child_conditions.iter();
    let mut formats = Vec::new();

    for child in children {
      let record_2 = child?;
      let Ok(format) = record_2.format();
      formats.push(format);
    }

    assert_eq!(formats, [1, 2]);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_condition_format4_or() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;

    let mut conditions = table.conditions()?;

    let record = crate::test_support::fixture_value(
      conditions.find(|condition| condition.format() == Ok(4)),
      "VARC condition format 4",
    )?;
    let condition = match record {
      Condition::Format4Or(condition) => condition,
      Condition::Format1AxisRange(_)
      | Condition::Format2VariableValue(_)
      | Condition::Format3And(_)
      | Condition::Format5Negate(_) => {
        return Err(crate::test_support::Failure::from(
          "expected VARC condition format 4",
        ));
      }
    };

    let Ok(child_conditions) = condition.conditions();
    let Ok(children) = child_conditions.iter();
    let mut formats = Vec::new();

    for child in children {
      let record_2 = child?;
      let Ok(format) = record_2.format();
      formats.push(format);
    }

    assert_eq!(formats, [1, 2]);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_condition_format5_negate() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;

    let mut conditions = table.conditions()?;

    let record = crate::test_support::fixture_value(
      conditions.find(|condition| condition.format() == Ok(5)),
      "VARC condition format 5",
    )?;
    let condition = match record {
      Condition::Format5Negate(condition) => condition,
      Condition::Format1AxisRange(_)
      | Condition::Format2VariableValue(_)
      | Condition::Format3And(_)
      | Condition::Format4Or(_) => {
        return Err(crate::test_support::Failure::from(
          "expected VARC condition format 5",
        ));
      }
    };

    assert_eq!(
      Ok(1),
      match condition.condition() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
      }
      .format(),
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_axis_indices_list() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;
    assert_eq!(table.axis_indices_count(), ::core::result::Result::Ok(2));
    assert_eq!(
      vec![2, 3, 4, 5, 6],
      match table.axis_indices(1) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      }
      .iter()
      .collect::<Vec<_>>()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn compute_sparse_region_scalar_handles_boundaries_and_products()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let varc = font.varc()?;
    let store = crate::test_support::fixture_value(
      match varc.multi_var_store() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    let regions = store.region_list()?;
    let Ok(region_list) = regions.regions();

    let axis0_region = region_list.get(0)?;
    let ::core::result::Result::Ok(()) = assert_close(self::AssertCloseArguments {
      actual: TransformScale(axis0_region.compute_scalar_f32(&[({
        let Ok(explicit_value) = F2Dot14::from_f32(1.0);
        explicit_value
      })])),
      expected: TransformScale(1.0),
    });
    let ::core::result::Result::Ok(()) = assert_close(self::AssertCloseArguments {
      actual: TransformScale(axis0_region.compute_scalar_f32(&[({
        let Ok(explicit_value) = F2Dot14::from_f32(0.5);
        explicit_value
      })])),
      expected: TransformScale(0.5),
    });
    let ::core::result::Result::Ok(()) = assert_close(self::AssertCloseArguments {
      actual: TransformScale(axis0_region.compute_scalar_f32(&[F2Dot14::ZERO])),
      expected: TransformScale(0.0),
    });
    let ::core::result::Result::Ok(()) = assert_close(self::AssertCloseArguments {
      actual: TransformScale(axis0_region.compute_scalar_f32(&[])),
      expected: TransformScale(0.0),
    });
    let ::core::result::Result::Ok(()) = assert_close(self::AssertCloseArguments {
      actual: TransformScale(axis0_region.compute_scalar_f32(&[({
        let Ok(explicit_value) = F2Dot14::from_f32(-0.25);
        explicit_value
      })])),
      expected: TransformScale(0.0),
    });

    let axis0_axis1_region = region_list.get(2)?;
    let ::core::result::Result::Ok(()) = assert_close(self::AssertCloseArguments {
      actual: TransformScale(axis0_axis1_region.compute_scalar_f32(&[
        ({
          let Ok(explicit_value) = F2Dot14::from_f32(0.5);
          explicit_value
        }),
        ({
          let Ok(explicit_value) = F2Dot14::from_f32(0.25);
          explicit_value
        }),
      ])),
      expected: TransformScale(0.125),
    });
    ::core::result::Result::Ok(())
  }

  #[test]
  fn axis_indices_offset_out_of_bounds_errors()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let mut bytes = font_test_data::varc::CJK_6868.to_vec();
    let (varc_offset, varc_len) = sfnt_table_range(&bytes, *b"VARC")?;

    let axis_indices_offset = varc_offset.checked_add(16).ok_or(ReadError::OutOfBounds)?;
    let length = u32::try_from(varc_len).map_err(|_| ReadError::OutOfBounds)?;
    let beyond_table = length.saturating_add(8);
    patch_bytes(&mut bytes, axis_indices_offset, beyond_table.to_be_bytes())?;

    let font = FontRef::new(&bytes)?;
    let table = font.varc()?;
    assert!(matches!(table.axis_indices(0), Err(ReadError::OutOfBounds)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn var_composite_glyphs_offset_out_of_bounds_errors()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let mut bytes = font_test_data::varc::CJK_6868.to_vec();
    let (varc_offset, varc_len) = sfnt_table_range(&bytes, *b"VARC")?;

    let glyphs_offset = varc_offset.checked_add(20).ok_or(ReadError::OutOfBounds)?;
    let length = u32::try_from(varc_len).map_err(|_| ReadError::OutOfBounds)?;
    let beyond_table = length.saturating_add(8);
    patch_bytes(&mut bytes, glyphs_offset, beyond_table.to_be_bytes())?;

    let font = FontRef::new(&bytes)?;
    let table = font.varc()?;
    assert!(matches!(table.glyph(0), Err(ReadError::OutOfBounds)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_with_missing_translate_data_errors()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;

    let Ok(data) = FontData::new(&[0x10, 0x00, 0x01]);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };
    assert!(matches!(
      super::VarcComponent::parse(&table, &mut cursor),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_with_invalid_axis_indices_index_errors()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;

    let Ok(data) = FontData::new(&[0x02, 0x00, 0x01, 0x7F]);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };
    assert!(matches!(
      super::VarcComponent::parse(&table, &mut cursor),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_reserved_fields_are_consumed()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;

    let flags = 0x0001_8010_u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(flags);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(&[0x00, 0x07]);
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(1);
        explicit_value
      }),
    );
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(2);
        explicit_value
      }),
    );
    bytes.push(0xAA);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    assert_eq!(component.transform().translate_x(), 7.0);
    assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_reserved_fields_truncation_errors()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;

    let flags = 0x0001_8010_u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(flags);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(&[0x00, 0x07]);
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(1);
        explicit_value
      }),
    );
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    assert!(matches!(
      super::VarcComponent::parse(&table, &mut cursor),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_gid_is_24bit_path() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;
    let mut bytes = Vec::new();
    let Ok(bits) = super::VarcFlags::GID_IS_24BIT.bits();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(bits);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x12, 0x34, 0x56]);
    bytes.push(0xAA);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    assert_eq!(
      ({
        let Ok(value) = component.gid().to_u32();
        value
      }),
      0x12_34_56
    );
    assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_gid_is_24bit_truncation_errors()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;
    let mut bytes = Vec::new();
    let Ok(bits) = super::VarcFlags::GID_IS_24BIT.bits();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(bits);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x12, 0x34]);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    assert!(matches!(
      super::VarcComponent::parse(&table, &mut cursor),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_with_axes_zero_count_does_not_consume_axis_values()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let bytes = font_with_empty_axis_indices()?;
    let font = FontRef::new(&bytes)?;
    let table = font.varc()?;
    assert_eq!(
      match table.axis_indices(0) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      }
      .count_or_compute(),
      0
    );

    let mut bytes_2 = Vec::new();
    bytes_2.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(0x12);
        explicit_value
      }),
    );
    bytes_2.extend_from_slice(&[0x00, 0x01]);
    bytes_2.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(0);
        explicit_value
      }),
    );
    bytes_2.extend_from_slice(&[0x00, 0x05]);
    bytes_2.push(0xAA);
    let Ok(data) = FontData::new(&bytes_2);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    assert_eq!(component.transform().translate_x(), 5.0);
    assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_with_axes_nonzero_count_consumes_axis_values()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;
    assert_eq!(
      match table.axis_indices(1) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      }
      .count_or_compute(),
      5
    );

    let mut bytes = Vec::new();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(0x12);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(1);
        explicit_value
      }),
    );
    bytes.push(0x04);
    bytes.extend_from_slice(&[1, 2, 3, 4, 5]);
    bytes.extend_from_slice(&[0x00, 0x09]);
    bytes.push(0xAA);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    assert_eq!(component.transform().translate_x(), 9.0);
    assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_scale_x_only_applies_to_scale_y()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;
    let mut bytes = Vec::new();
    let Ok(bits) = super::VarcFlags::HAVE_SCALE_X.bits();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(bits);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(&[0x08, 0x00]);
    bytes.push(0xAA);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    assert_eq!(component.transform().scale_x(), 2.0);
    assert_eq!(component.transform().scale_y(), 2.0);
    assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_scale_x_and_scale_y_are_independent()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;
    let Ok(bits) = super::VarcFlags::HAVE_SCALE_X.bits();
    let Ok(have_scale_y_bits) = super::VarcFlags::HAVE_SCALE_Y.bits();
    let flags = bits | have_scale_y_bits;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(flags);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(&[0x08, 0x00]);
    bytes.extend_from_slice(&[0x0C, 0x00]);
    bytes.push(0xAA);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    assert_eq!(component.transform().scale_x(), 2.0);
    assert_eq!(component.transform().scale_y(), 3.0);
    assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_multibyte_condition_and_var_indices()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;
    let Ok(bits) = super::VarcFlags::HAVE_CONDITION.bits();
    let Ok(axis_values_have_variation_bits) = super::VarcFlags::AXIS_VALUES_HAVE_VARIATION.bits();
    let Ok(transform_has_variation_bits) = super::VarcFlags::TRANSFORM_HAS_VARIATION.bits();
    let Ok(have_translate_x_bits) = super::VarcFlags::HAVE_TRANSLATE_X.bits();
    let flags = bits
      | axis_values_have_variation_bits
      | transform_has_variation_bits
      | have_translate_x_bits;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(flags);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(0x2345);
        explicit_value
      }),
    );
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(0x0123);
        explicit_value
      }),
    );
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(0x0222);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x07]);
    bytes.push(0xAA);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    assert_eq!(component.condition_index(), Some(0x2345));
    assert_eq!(component.axis_values_var_index(), Some(0x0123));
    assert_eq!(component.transform_var_index(), Some(0x0222));
    assert_eq!(component.transform().translate_x(), 7.0);
    assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_truncated_multibyte_condition_index_errors()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;
    let mut bytes = Vec::new();
    let Ok(bits) = super::VarcFlags::HAVE_CONDITION.bits();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(bits);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.push(0x81);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    assert!(matches!(
      super::VarcComponent::parse(&table, &mut cursor),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_axes_with_var_index_and_zero_axis_count()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let bytes = font_with_empty_axis_indices()?;
    let font = FontRef::new(&bytes)?;
    let table = font.varc()?;

    let Ok(bits) = super::VarcFlags::HAVE_AXES.bits();
    let Ok(axis_values_have_variation_bits) = super::VarcFlags::AXIS_VALUES_HAVE_VARIATION.bits();
    let Ok(have_translate_x_bits) = super::VarcFlags::HAVE_TRANSLATE_X.bits();
    let flags = bits | axis_values_have_variation_bits | have_translate_x_bits;
    let mut bytes_2 = Vec::new();
    bytes_2.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(flags);
        explicit_value
      }),
    );
    bytes_2.extend_from_slice(&[0x00, 0x01]);
    bytes_2.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(0);
        explicit_value
      }),
    );
    bytes_2.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(0x0123);
        explicit_value
      }),
    );
    bytes_2.extend_from_slice(&[0x00, 0x06]);
    bytes_2.push(0xAA);
    let Ok(data) = FontData::new(&bytes_2);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    assert_eq!(component.axis_indices_index(), Some(0));
    assert!(component.axis_values().is_none());
    assert_eq!(component.axis_values_var_index(), Some(0x0123));
    assert_eq!(component.transform().translate_x(), 6.0);
    assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_axes_truncated_i16_payload_detected_on_fetch()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;
    assert_eq!(
      match table.axis_indices(1) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
      }
      .count_or_compute(),
      5
    );

    let mut bytes = Vec::new();
    let Ok(bits) = super::VarcFlags::HAVE_AXES.bits();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(bits);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(1);
        explicit_value
      }),
    );
    bytes.push(0x44);
    bytes.extend_from_slice(&[0x00, 0x01, 0x00, 0x02, 0x00, 0x03]);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    let mut out = [0.0; 5];
    let fixture_value =
      crate::test_support::fixture_value(component.axis_values(), "fixture value")?;
    assert!(matches!(
      fixture_value.fetcher().add_to_f32_scaled(&mut out, 1.0),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_axes_truncated_i32_payload_detected_on_fetch()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;
    assert_eq!(
      match table.axis_indices(1) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
      }
      .count_or_compute(),
      5
    );

    let mut bytes = Vec::new();
    let Ok(bits) = super::VarcFlags::HAVE_AXES.bits();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(bits);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(1);
        explicit_value
      }),
    );
    bytes.push(0xC4);
    bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02]);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    let component = super::VarcComponent::parse(&table, &mut cursor)?;
    let mut out = [0.0; 5];
    let fixture_value =
      crate::test_support::fixture_value(component.axis_values(), "fixture value")?;
    assert!(matches!(
      fixture_value.fetcher().add_to_f32_scaled(&mut out, 1.0),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_component_axes_payload_overshoot_causes_following_field_error()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::varc::CONDITIONALS)?;
    let table = font.varc()?;
    assert_eq!(
      match table.axis_indices(1) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      }
      .count_or_compute(),
      5
    );

    let mut bytes = Vec::new();
    let Ok(bits) = super::VarcFlags::HAVE_AXES.bits();
    let Ok(have_translate_x_bits) = super::VarcFlags::HAVE_TRANSLATE_X.bits();
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(bits | have_translate_x_bits);
        explicit_value
      }),
    );
    bytes.extend_from_slice(&[0x00, 0x01]);
    bytes.extend_from_slice(
      &({
        let Ok(explicit_value) = encode_u32_var(1);
        explicit_value
      }),
    );
    bytes.push(0x44);
    bytes.extend_from_slice(&[0x00, 0x01, 0x00, 0x02]);
    bytes.extend_from_slice(&[0x00, 0x09]);
    let Ok(data) = FontData::new(&bytes);
    let mut cursor = {
      let Ok(value) = data.cursor();
      value
    };

    assert!(matches!(
      super::VarcComponent::parse(&table, &mut cursor),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn generated_condition_varints_roundtrip()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::varc::CJK_6868),
      "fixture value",
    )?;
    let table = crate::test_support::fixture_value(font.varc(), "fixture value")?;
    let mut state = 0x1234_5678_u32;

    for _ in 0..256 {
      state ^= ::core::primitive::u32::wrapping_shl(state, 13_u32);
      state ^= ::core::primitive::u32::wrapping_shr(state, 17_u32);
      state ^= ::core::primitive::u32::wrapping_shl(state, 5_u32);
      let value = state;

      let mut bytes = Vec::new();
      let Ok(bits) = super::VarcFlags::HAVE_CONDITION.bits();
      bytes.extend_from_slice(
        &({
          let Ok(explicit_value) = encode_u32_var(bits);
          explicit_value
        }),
      );
      bytes.extend_from_slice(&[0x00, 0x01]);
      bytes.extend_from_slice(
        &({
          let Ok(explicit_value) = encode_u32_var(value);
          explicit_value
        }),
      );
      bytes.push(0xAA);
      let Ok(data) = FontData::new(&bytes);
      let mut cursor = {
        let Ok(value_2) = data.cursor();
        value_2
      };

      let component = crate::test_support::fixture_value(
        super::VarcComponent::parse(&table, &mut cursor),
        "fixture value",
      )?;
      assert_eq!(component.condition_index(), Some(value));
      assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    }

    Ok(())
  }

  #[test]
  fn generated_axis_values_single_run_roundtrip()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::varc::CONDITIONALS),
      "fixture value",
    )?;
    let table = crate::test_support::fixture_value(font.varc(), "fixture value")?;
    let fixture_value =
      crate::test_support::fixture_value(table.axis_indices(1), "fixture value")?;
    assert_eq!(fixture_value.count_or_compute(), 5);

    let mut state = 0xA5A5_5A5A_u32;

    for _ in 0..128 {
      state ^= ::core::primitive::u32::wrapping_shl(state, 13_u32);
      state ^= ::core::primitive::u32::wrapping_shr(state, 17_u32);
      state ^= ::core::primitive::u32::wrapping_shl(state, 5_u32);
      let run_kind = state & 3;

      let mut payload = Vec::new();
      let mut expected = [0f32; 5];

      match run_kind {
        0 => {
          payload.push(0x04);

          for out in &mut expected {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = ::core::primitive::i8::wrapping_rem(
              {
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i8>(
                  ::core::primitive::u32::wrapping_shr(state, 24_u32),
                );
                migrated_value
              },
              64,
            );
            payload.push({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(v);
              migrated_value
            });
            *out = f32::from(v);
          }
        }
        1 => {
          payload.push(0x44);

          for out in &mut expected {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = ::core::primitive::i16::wrapping_rem(
              {
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(
                  ::core::primitive::u32::wrapping_shr(state, 16_u32),
                );
                migrated_value
              },
              2048,
            );
            payload.extend_from_slice(&v.to_be_bytes());
            *out = f32::from(v);
          }
        }
        2 => {
          payload.push(0xC4);

          for out in &mut expected {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = ::core::primitive::i32::wrapping_rem(
              {
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(state);
                migrated_value
              },
              1_000_000,
            );
            payload.extend_from_slice(&v.to_be_bytes());
            *out = match ::console_core_number_conversion::nearest_f32_from_i64(
              <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(v),
            ) {
              ::core::result::Result::Ok(_converted_number) => _converted_number,
            };
          }
        }
        _ => {
          payload.push(0x84);
        }
      }

      let mut bytes = Vec::new();
      let Ok(bits) = super::VarcFlags::HAVE_AXES.bits();
      let Ok(have_translate_x_bits) = super::VarcFlags::HAVE_TRANSLATE_X.bits();
      bytes.extend_from_slice(
        &({
          let Ok(explicit_value) = encode_u32_var(bits | have_translate_x_bits);
          explicit_value
        }),
      );
      bytes.extend_from_slice(&[0x00, 0x01]);
      bytes.extend_from_slice(
        &({
          let Ok(explicit_value) = encode_u32_var(1);
          explicit_value
        }),
      );
      bytes.extend_from_slice(&payload);
      bytes.extend_from_slice(&[0x00, 0x07]);
      bytes.push(0xAA);

      let Ok(data) = FontData::new(&bytes);
      let mut cursor = {
        let Ok(value) = data.cursor();
        value
      };
      let component = crate::test_support::fixture_value(
        super::VarcComponent::parse(&table, &mut cursor),
        "fixture value",
      )?;
      let mut out = [0.0f32; 5];
      crate::test_support::fixture_value(
        {
          let axis_coordinates =
            crate::test_support::fixture_value(component.axis_values(), "fixture value")?;
          axis_coordinates
        }
        .fetcher()
        .add_to_f32_scaled(&mut out, 1.0),
        "fixture value",
      )?;
      assert_eq!(out, expected);
      assert_eq!(component.transform().translate_x(), 7.0);
      assert_eq!((cursor).read::<u8>(), ::core::result::Result::Ok(0xAA));
    }

    Ok(())
  }

  #[test]
  fn parse_component_truncated_condition_varint_boundaries_error()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::varc::CJK_6868),
      "fixture value",
    )?;
    let table = crate::test_support::fixture_value(font.varc(), "fixture value")?;
    let values = [0x80_u32, 0x4000_u32, 0x20_0000_u32, 0x1000_0000_u32];

    for value in values {
      let Ok(encoded) = encode_u32_var(value);
      assert!(encoded.len() > 1);
      let (_last_byte, truncated) = crate::test_support::fixture_value(
        encoded.split_last(),
        "VARC component condition bytes",
      )?;

      let mut bytes = Vec::new();
      let Ok(bits) = super::VarcFlags::HAVE_CONDITION.bits();
      bytes.extend_from_slice(
        &({
          let Ok(explicit_value) = encode_u32_var(bits);
          explicit_value
        }),
      );
      bytes.extend_from_slice(&[0x00, 0x01]);
      bytes.extend_from_slice(truncated);
      let Ok(data) = FontData::new(&bytes);
      let mut cursor = {
        let Ok(value_2) = data.cursor();
        value_2
      };

      assert!(matches!(
        super::VarcComponent::parse(&table, &mut cursor),
        Err(ReadError::OutOfBounds)
      ));
    }

    Ok(())
  }

  #[test]
  fn read_glyph_6868() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let gid = crate::test_support::fixture_value(
      match font.cmap() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      }
      .map_codepoint(0x6868_u32),
      "fixture value",
    )?;
    let table = font.varc()?;
    let idx = crate::test_support::fixture_value(
      match table.coverage() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      }
      .get(gid),
      "fixture value",
    )?;

    let glyph = table.glyph({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
      migrated_value
    })?;
    let component_ids = glyph
      .components()
      .map(|entry| entry.map(|component| component.gid))
      .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
      vec![
        {
          let Ok(value) = GlyphId16::new(2);
          value
        },
        {
          let Ok(value) = GlyphId16::new(5);
          value
        },
        {
          let Ok(value) = GlyphId16::new(7);
          value
        }
      ],
      component_ids
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn decomposed_scale_to_matrix() {
    let scale_x = 2.0;
    let scale_y = 3.0;
    assert_eq!(
      [scale_x, 0.0, 0.0, scale_y, 0.0, 0.0],
      ({
        let Ok(value) = ({
          let Ok(explicit_value) = rounded_matrix(
            DecomposedTransform {
              scale_x: TransformScale(scale_x),
              scale_y: TransformScale(scale_y),
              ..Default::default()
            }
            .matrix(),
          );
          explicit_value
        })
        .elements();
        value
      })
    );
  }

  #[test]
  fn decomposed_rotate_to_matrix() {
    assert_eq!(
      [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
      ({
        let Ok(value) = ({
          let Ok(explicit_value) = rounded_matrix(
            DecomposedTransform {
              rotation: HalfTurns(0.5),
              ..Default::default()
            }
            .matrix(),
          );
          explicit_value
        })
        .elements();
        value
      })
    );
  }

  #[test]
  fn decomposed_skew_to_matrix() {
    let skew_x: f32 = 1.0 / 6.0;
    let skew_y: f32 = -1.0 / 3.0;
    let Ok(expected) = Matrix::from_elements([
      1.0,
      (skew_y * core::f32::consts::PI).tan(),
      (-skew_x * core::f32::consts::PI).tan(),
      1.0,
      0.0,
      0.0,
    ]);
    let Ok(rounded_expected) = rounded_matrix(expected);
    let Ok(expected_elements) = rounded_expected.elements();
    assert_eq!(
      expected_elements,
      ({
        let Ok(value) = ({
          let Ok(explicit_value) = rounded_matrix(
            DecomposedTransform {
              skew_x: HalfTurns(skew_x),
              skew_y: HalfTurns(skew_y),
              ..Default::default()
            }
            .matrix(),
          );
          explicit_value
        })
        .elements();
        value
      })
    );
  }

  #[test]
  fn decomposed_scale_rotate_to_matrix() {
    let scale_x = 2.0;
    let scale_y = 3.0;
    assert_eq!(
      [0.0, scale_x, -scale_y, 0.0, 0.0, 0.0],
      ({
        let Ok(value) = ({
          let Ok(explicit_value) = rounded_matrix(
            DecomposedTransform {
              scale_x: TransformScale(scale_x),
              scale_y: TransformScale(scale_y),

              rotation: HalfTurns(0.5),
              ..Default::default()
            }
            .matrix(),
          );
          explicit_value
        })
        .elements();
        value
      })
    );
  }

  #[test]
  fn decomposed_scale_rotate_translate_to_matrix() {
    assert_eq!(
      [0.0, 2.0, -1.0, 0.0, 10.0, 20.0],
      ({
        let Ok(value) = ({
          let Ok(explicit_value) = rounded_matrix(
            DecomposedTransform {
              scale_x: TransformScale(2.0),

              rotation: HalfTurns(0.5),
              translate_x: FontUnits(10.0),
              translate_y: FontUnits(20.0),
              ..Default::default()
            }
            .matrix(),
          );
          explicit_value
        })
        .elements();
        value
      })
    );
  }

  #[test]
  fn decomposed_scale_skew_translate_to_matrix() {
    assert_eq!(
      [-0.866026, 5.5, -2.5, 2.020726, 10.0, 20.0],
      ({
        let Ok(value) = ({
          let Ok(explicit_value) = rounded_matrix(
            DecomposedTransform {
              scale_x: TransformScale(2.0),
              scale_y: TransformScale(3.0),

              rotation: HalfTurns(1.0 / 6.0),
              skew_x: HalfTurns(1.0 / 6.0),
              skew_y: HalfTurns(1.0 / 3.0),
              translate_x: FontUnits(10.0),
              translate_y: FontUnits(20.0),
              ..Default::default()
            }
            .matrix(),
          );
          explicit_value
        })
        .elements();
        value
      })
    );
  }

  #[test]
  fn decomposed_rotate_around_to_matrix() {
    assert_eq!(
      [1.732051, 1.0, -0.5, 0.866025, 10.267949, 19.267949],
      ({
        let Ok(value) = ({
          let Ok(explicit_value) = rounded_matrix(
            DecomposedTransform {
              scale_x: TransformScale(2.0),

              rotation: HalfTurns(1.0 / 6.0),
              translate_x: FontUnits(10.0),
              translate_y: FontUnits(20.0),
              center_x: FontUnits(1.0),
              center_y: FontUnits(2.0),
              ..Default::default()
            }
            .matrix(),
          );
          explicit_value
        })
        .elements();
        value
      })
    );
  }

  #[test]
  fn read_multivar_store_region_list() -> ::core::result::Result<(), crate::test_support::Failure>
  {
    let font = FontRef::new(font_test_data::varc::CJK_6868)?;
    let table = font.varc()?;
    let varstore = crate::test_support::fixture_value(
      match table.multi_var_store() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    let Ok(regions) = match varstore.region_list() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
      }
    }
    .regions();

    let Ok(value_27) = regions.iter();

    let sparse_regions = value_27
      .map(|entry| -> Result<_, ReadError> {
        let region = entry?;
        let Ok(region_axes) = region.region_axes();
        let axes = region_axes
          .iter()
          .map(|a| {
            let Ok(axis_index) = a.axis_index();

            (
              axis_index,
              ({
                let Ok(start) = a.start();
                let Ok(value) = start.to_f32();
                value
              }),
              ({
                let Ok(peak) = a.peak();
                let Ok(value) = peak.to_f32();
                value
              }),
              ({
                let Ok(end) = a.end();
                let Ok(value) = end.to_f32();
                value
              }),
            )
          })
          .collect::<Vec<_>>();
        Ok(axes)
      })
      .collect::<Result<Vec<_>, _>>()?;
    let regions_to_compare = [0, 2, 38]
      .into_iter()
      .map(|index| {
        crate::test_support::fixture_value(
          sparse_regions.get(index).cloned(),
          "fixture sparse region",
        )
      })
      .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(
      vec![
        vec![(0, 0.0, 1.0, 1.0),],
        vec![(0, 0.0, 1.0, 1.0), (1, 0.0, 1.0, 1.0),],
        vec![(6, -1.0, -1.0, 0.0),],
      ],
      regions_to_compare
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_multivar_store_delta_sets() -> ::core::result::Result<(), crate::test_support::Failure>
  {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::varc::CJK_6868),
      "fixture value",
    )?;
    let table = crate::test_support::fixture_value(font.varc(), "fixture value")?;
    let fixture_value =
      crate::test_support::fixture_value(table.multi_var_store(), "fixture value")?;
    let varstore = crate::test_support::fixture_value(fixture_value, "fixture value")?;
    let Ok(data_offsets) = varstore.variation_data();
    let Ok(records) = data_offsets.iter();
    let mut counts = Vec::new();

    for entry in records {
      let data = entry?;
      let delta_sets = data.delta_sets()?;
      counts.push((data.region_index_count(), delta_sets.count()));
    }

    assert_eq!(
      counts,
      [
        (Ok(3), Ok(6)),
        (Ok(33), Ok(6)),
        (Ok(10), Ok(5)),
        (Ok(25), Ok(8))
      ]
    );

    assert_eq!(
      vec![-1, 33, 0, 0, 0, 0],
      {
        let delta_set = crate::test_support::fixture_value(
          {
            let variation_data_record = crate::test_support::fixture_value(
              ({
                let Ok(explicit_value) = varstore.variation_data();
                explicit_value
              })
              .get(0),
              "fixture value",
            )?;
            variation_data_record
          }
          .delta_set(5),
          "fixture value",
        )?;
        delta_set
      }
      .iter()
      .collect::<Vec<_>>()
    );
    Ok(())
  }
}
