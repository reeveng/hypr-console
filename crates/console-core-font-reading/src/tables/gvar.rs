//! The [gvar (Glyph Variations)](https://learn.microsoft.com/en-us/typography/opentype/spec/gvar)
//! table

include!("../../generated/generated_gvar.rs");

use super::{
  glyf::{CompositeGlyphFlags, Glyf, Glyph, PointCoord},
  loca::Loca,
  variations::{
    PackedPointNumbers, Tuple, TupleDelta, TupleVariationCount, TupleVariationData,
    TupleVariationHeader,
  },
};

pub type GlyphVariationData<'a> = TupleVariationData<'a, GlyphDelta>;

#[derive(Clone, Copy, Debug)]
pub struct U16Or32(u32);

impl ReadArgs for U16Or32 {
  type Arguments = GvarFlags;
}

impl ComputeSize for U16Or32 {
  fn compute_size(args: &GvarFlags) -> Result<u64, ReadError> {
    let Ok(long_offsets_flag) = args.contains(GvarFlags::LONG_OFFSETS);

    Ok(match long_offsets_flag {
      Presence::Present => 4,
      Presence::Absent => 2,
    })
  }
}

impl FontReadWithArgs<'_> for U16Or32 {
  fn read_with_args(data: FontData<'_>, args: &Self::Arguments) -> Result<Self, ReadError> {
    let Ok(long_offsets) = args.contains(GvarFlags::LONG_OFFSETS);

    match long_offsets {
      Presence::Present => data.read_at::<u32>(0).map(Self),
      Presence::Absent => data
        .read_at::<u16>(0)
        .map(|offset| Self(u32::from(offset).wrapping_mul(2))),
    }
  }
}

impl U16Or32 {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `U16Or32::get` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn get(self) -> u32 {
    self.0
  }
}

impl<'a> GlyphVariationDataHeader<'a> {
  fn raw_tuple_header_data(&self) -> Result<FontData<'a>, ReadError> {
    let Ok(range) = self.tuple_variation_headers_byte_range();
    let Ok(data) = self.data.split_off(range.start);

    data.ok_or(ReadError::OutOfBounds)
  }
}

impl<'a> Gvar<'a> {
  pub fn data_for_gid(&self, gid: GlyphId) -> Result<Option<FontData<'a>>, ReadError> {
    let range = self.data_range_for_gid(gid)?;

    match ::core::primitive::bool::then_some(range.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_10) = self.data.slice(range);

    match value_10 {
      Some(data) => Ok(Some(data)),
      None => Err(ReadError::OutOfBounds),
    }
  }

  pub fn glyph_variation_data_for_range(
    &self,
    offset_range: Range<u64>,
  ) -> Result<FontData<'a>, ReadError> {
    let base = {
      let Ok(glyph_variation_data_array_offset) = self.glyph_variation_data_array_offset();
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(glyph_variation_data_array_offset);
      migrated_value
    };
    let start = base
      .checked_add(offset_range.start)
      .ok_or(ReadError::OutOfBounds)?;
    let end = base
      .checked_add(offset_range.end)
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_1) = self.data.slice(start..end);

    value_1.ok_or(ReadError::OutOfBounds)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Gvar::as_bytes` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn as_bytes(&self) -> &[u8] {
    let Ok(value_2) = self.data.as_bytes();

    value_2
  }

  fn data_range_for_gid(&self, gid: GlyphId) -> Result<Range<u64>, ReadError> {
    let Ok(value_3) = gid.to_u32();

    let start_idx = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_3);
      migrated_value
    };
    let end_idx = ::core::primitive::u64::wrapping_add(start_idx, 1);
    let Ok(data_start) = self.glyph_variation_data_array_offset();
    let Ok(glyph_variation_data_offsets) = self.glyph_variation_data_offsets();
    let glyph_variation_data_offset = glyph_variation_data_offsets.get(start_idx)?;
    let value_6 = glyph_variation_data_offset.get();

    let start = data_start.checked_add(value_6);
    let Ok(own_glyph_variation_data_offsets) = self.glyph_variation_data_offsets();
    let own_glyph_variation_data_offset = own_glyph_variation_data_offsets.get(end_idx)?;
    let value_9 = own_glyph_variation_data_offset.get();

    let end = data_start.checked_add(value_9);

    let (start_2, end_2) = {
      let outcome_start_2 = (start, end);

      match outcome_start_2 {
        (Some(matched_start_2), Some(matched_end_2)) => (matched_start_2, matched_end_2),
        (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {
          return Err(ReadError::OutOfBounds);
        }
      }
    };

    Ok(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(start_2);
        migrated_value
      })..({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(end_2);
        migrated_value
      }),
    )
  }

  pub fn glyph_variation_data(
    &self,
    gid: GlyphId,
  ) -> Result<Option<GlyphVariationData<'a>>, ReadError> {
    let shared_tuples = self.shared_tuples()?;
    let Ok(axis_count) = self.axis_count();
    let data = self.data_for_gid(gid)?;
    data
      .map(|data_2| GlyphVariationData::new(data_2, axis_count, shared_tuples))
      .transpose()
  }

  pub fn phantom_point_deltas(
    &self,
    glyf: &Glyf,
    loca: &Loca,
    coords: &[F2Dot14],
    glyph_id: GlyphId,
  ) -> Result<Option<[Point<Fixed>; 4]>, ReadError> {
    let (glyph_id_2, point_count) = find_glyph_and_point_count(glyf, loca, glyph_id, 0)?;
    let mut phantom_deltas = [Point::default(); 4];
    let phantom_range = point_count..::core::primitive::u64::wrapping_add(point_count, 4);
    let glyph_variation_data = self.glyph_variation_data(glyph_id_2)?;
    let var_data = match glyph_variation_data {
      Some(var_data) => var_data,
      None => {
        return Ok(None);
      }
    };

    for (tuple, scalar) in var_data.active_tuples_at(coords) {
      for tuple_delta in tuple.deltas() {
        let ix = {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(tuple_delta.position);
          migrated_value
        };

        match ::core::primitive::bool::then_some(phantom_range.contains(&ix), ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_5) = console_core_number_conversion::index(
              ::core::primitive::u64::wrapping_sub(ix, phantom_range.start),
            );

            let delta = phantom_deltas
              .get_mut(value_5)
              .ok_or(ReadError::OutOfBounds)?;
            *delta += tuple_delta.apply_scalar(scalar);
          }
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(Some(phantom_deltas))
  }
}

impl<'a> GlyphVariationData<'a> {
  pub(crate) fn new(
    data: FontData<'a>,
    axis_count: u16,
    shared_tuples: SharedTuples<'a>,
  ) -> Result<Self, ReadError> {
    let header = GlyphVariationDataHeader::read(data)?;

    let header_data = header.raw_tuple_header_data()?;
    let Ok(count) = header.tuple_variation_count();
    let data_2 = header.serialized_data()?;

    let Ok(tuple_variation_count) = header.tuple_variation_count();
    let (shared_point_numbers, serialized_data) = match ::core::primitive::bool::then_some(
      tuple_variation_count.shared_point_numbers(),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let (packed, data_3) = PackedPointNumbers::split_off_front(data_2);
        (Some(packed), data_3)
      }
      ::core::option::Option::None => (None, data_2),
    };

    let Ok(tuples) = shared_tuples.tuples();

    Ok(GlyphVariationData {
      tuple_count: count,
      axis_count,
      shared_tuples: Some(tuples),
      shared_point_numbers,
      header_data,
      serialized_data,
      _marker: std::marker::PhantomData,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphDelta {
  pub position: u16,
  pub x_delta: i32,
  pub y_delta: i32,
}

impl GlyphDelta {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `GlyphDelta::apply_scalar` method returns `Point` directly in the existing parser interface"
    )
  )]
  pub fn apply_scalar<D: PointCoord>(self, scalar: Fixed) -> Point<D> {
    let scalar_2 = D::from_fixed(scalar);
    let Ok(point) = Point::new(self.x_delta, self.y_delta);
    let Ok(value_4) = point.map(D::from_i32);

    value_4 * scalar_2
  }
}

impl TupleDelta for GlyphDelta {
  fn is_point() -> bool {
    true
  }

  fn new(position: u16, x: i32, y: i32) -> Self {
    Self {
      position,
      x_delta: x,
      y_delta: y,
    }
  }
}

fn find_glyph_and_point_count(
  glyf: &Glyf,
  loca: &Loca,
  mut glyph_id: GlyphId,
  initial_depth: u64,
) -> Result<(GlyphId, u64), ReadError> {
  const COMPONENT_DEPTH_LIMIT: u64 = 64;

  for _depth in initial_depth..=COMPONENT_DEPTH_LIMIT {
    let glyph = loca.get_glyf(glyph_id, glyf)?;

    let composite = match glyph {
      None => return Ok((glyph_id, 0)),
      Some(Glyph::Simple(simple)) => return Ok((glyph_id, simple.num_points())),
      Some(Glyph::Composite(composite)) => composite,
    };
    let mut point_count = 0u64;
    let mut metrics_source = None;

    for (component, flags) in composite.component_glyphs_and_flags() {
      let Ok(inherit_metrics) = flags.contains(CompositeGlyphFlags::USE_MY_METRICS);
      point_count = point_count.wrapping_add(1);

      match inherit_metrics {
        Presence::Present => metrics_source = Some(component),
        Presence::Absent => {}
      }
    }

    match metrics_source {
      Some(component) => glyph_id = GlyphId::from(component),
      None => return Ok((glyph_id, point_count)),
    }
  }

  Err(ReadError::MalformedData(
    "nesting too deep in composite glyph",
  ))
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::*;
  use crate::{FontRef, TableProvider};

  static SKIA_GVAR_SHARED_TUPLES_DATA: FontData = {
    let Ok(explicit_value) = FontData::new(&[
      0x40, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0xC0,
      0x00, 0xC0, 0x00, 0xC0, 0x00, 0x40, 0x00, 0xC0, 0x00, 0x40, 0x00, 0x40, 0x00, 0xC0, 0x00,
      0x40, 0x00,
    ]);
    explicit_value
  };

  static SKIA_GVAR_I_DATA: FontData = {
    let Ok(explicit_value) = FontData::new(&[
      0x00, 0x08, 0x00, 0x24, 0x00, 0x33, 0x20, 0x00, 0x00, 0x15, 0x20, 0x01, 0x00, 0x1B, 0x20,
      0x02, 0x00, 0x24, 0x20, 0x03, 0x00, 0x15, 0x20, 0x04, 0x00, 0x26, 0x20, 0x07, 0x00, 0x0D,
      0x20, 0x06, 0x00, 0x1A, 0x20, 0x05, 0x00, 0x40, 0x01, 0x01, 0x01, 0x81, 0x80, 0x43, 0xFF,
      0x7E, 0xFF, 0x7E, 0xFF, 0x7E, 0xFF, 0x7E, 0x00, 0x81, 0x45, 0x01, 0x01, 0x01, 0x03, 0x01,
      0x04, 0x01, 0x04, 0x01, 0x04, 0x01, 0x02, 0x80, 0x40, 0x00, 0x82, 0x81, 0x81, 0x04, 0x3A,
      0x5A, 0x3E, 0x43, 0x20, 0x81, 0x04, 0x0E, 0x40, 0x15, 0x45, 0x7C, 0x83, 0x00, 0x0D, 0x9E,
      0xF3, 0xF2, 0xF0, 0xF0, 0xF0, 0xF0, 0xF3, 0x9E, 0xA0, 0xA1, 0xA1, 0xA1, 0x9F, 0x80, 0x00,
      0x91, 0x81, 0x91, 0x00, 0x0D, 0x0A, 0x0A, 0x09, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A,
      0x0A, 0x0A, 0x0A, 0x0B, 0x80, 0x00, 0x15, 0x81, 0x81, 0x00, 0xC4, 0x89, 0x00, 0xC4, 0x83,
      0x00, 0x0D, 0x80, 0x99, 0x98, 0x96, 0x96, 0x96, 0x96, 0x99, 0x80, 0x82, 0x83, 0x83, 0x83,
      0x81, 0x80, 0x40, 0xFF, 0x18, 0x81, 0x81, 0x04, 0xE6, 0xF9, 0x10, 0x21, 0x02, 0x81, 0x04,
      0xE8, 0xE5, 0xEB, 0x4D, 0xDA, 0x83, 0x00, 0x0D, 0xCE, 0xD3, 0xD4, 0xD3, 0xD3, 0xD3, 0xD5,
      0xD2, 0xCE, 0xCC, 0xCD, 0xCD, 0xCD, 0xCD, 0x80, 0x00, 0xA1, 0x81, 0x91, 0x00, 0x0D, 0x07,
      0x03, 0x04, 0x02, 0x02, 0x02, 0x03, 0x03, 0x07, 0x07, 0x08, 0x08, 0x08, 0x07, 0x80, 0x00,
      0x09, 0x81, 0x81, 0x00, 0x28, 0x40, 0x00, 0xA4, 0x02, 0x24, 0x24, 0x66, 0x81, 0x04, 0x08,
      0xFA, 0xFA, 0xFA, 0x28, 0x83, 0x00, 0x82, 0x02, 0xFF, 0xFF, 0xFF, 0x83, 0x02, 0x01, 0x01,
      0x01, 0x84, 0x91, 0x00, 0x80, 0x06, 0x07, 0x08, 0x08, 0x08, 0x08, 0x0A, 0x07, 0x80, 0x03,
      0xFE, 0xFF, 0xFF, 0xFF, 0x81, 0x00, 0x08, 0x81, 0x82, 0x02, 0xEE, 0xEE, 0xEE, 0x8B, 0x6D,
      0x00,
    ]);
    explicit_value
  };

  #[test]
  fn test_shared_tuples() -> ::core::result::Result<(), crate::read::ReadError> {
    const MINUS_ONE: F2Dot14 = {
      let Ok(explicit_value) = F2Dot14::from_bits(i16::from_be_bytes([0xC0, 0x00]));
      explicit_value
    };
    assert_eq!(
      MINUS_ONE,
      ({
        let Ok(explicit_value) = F2Dot14::from_f32(-1.0);
        explicit_value
      })
    );

    static EXPECTED: &[(F2Dot14, F2Dot14)] = &[
      (F2Dot14::ONE, F2Dot14::ZERO),
      (MINUS_ONE, F2Dot14::ZERO),
      (F2Dot14::ZERO, F2Dot14::ONE),
      (F2Dot14::ZERO, MINUS_ONE),
      (MINUS_ONE, MINUS_ONE),
      (F2Dot14::ONE, MINUS_ONE),
      (F2Dot14::ONE, F2Dot14::ONE),
      (MINUS_ONE, F2Dot14::ONE),
    ];

    const N_AXES: u16 = 2;

    let tuples = SharedTuples::read(
      SKIA_GVAR_SHARED_TUPLES_DATA,
      {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u16>(EXPECTED.len());
        migrated_value
      },
      N_AXES,
    )?;
    let Ok(shared_tuples) = tuples.tuples();
    let Ok(entries) = shared_tuples.iter();
    let mut tuple_vec = Vec::new();

    for entry in entries {
      let tuple = entry?;
      let Ok(values) = tuple.values();
      let (first, second) = match values {
        [first, second] => (first, second),
        _invalid_axes => {
          return Err(ReadError::MalformedData(
            "expected two shared tuple coordinates",
          ));
        }
      };
      let Ok(x) = first.get();
      let Ok(y) = second.get();
      tuple_vec.push((x, y));
    }

    assert_eq!(tuple_vec, EXPECTED);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn smoke_test() -> ::core::result::Result<(), crate::test_support::Failure> {
    let header = crate::test_support::fixture_value(
      GlyphVariationDataHeader::read(SKIA_GVAR_I_DATA),
      "fixture value",
    )?;
    let Ok(serialized_data_offset) = header.serialized_data_offset();
    assert_eq!(serialized_data_offset, 36);
    let Ok(tuple_variation_count) = header.tuple_variation_count();
    assert_eq!(tuple_variation_count.count(), 8);
    let shared_tuples = crate::test_support::fixture_value(
      SharedTuples::read(SKIA_GVAR_SHARED_TUPLES_DATA, 8, 2),
      "fixture value",
    )?;

    let vardata = crate::test_support::fixture_value(
      GlyphVariationData::new(SKIA_GVAR_I_DATA, 2, shared_tuples),
      "fixture value",
    )?;
    let Ok(tuple_count) = vardata.tuple_count();
    assert_eq!(tuple_count, 8);
    let fixture_value =
      crate::test_support::fixture_value(vardata.tuples().next(), "fixture value")?;
    let deltas = fixture_value.deltas().collect::<Vec<_>>();
    assert_eq!(deltas.len(), 18);
    static EXPECTED: &[(i32, i32)] = &[
      (257, 0),
      (-127, 0),
      (-128, 58),
      (-130, 90),
      (-130, 62),
      (-130, 67),
      (-130, 32),
      (-127, 0),
      (257, 0),
      (259, 14),
      (260, 64),
      (260, 21),
      (260, 69),
      (258, 124),
      (0, 0),
      (130, 0),
      (0, 0),
      (0, 0),
    ];
    let expected = EXPECTED
      .iter()
      .copied()
      .enumerate()
      .map(|(pos, (x_delta, y_delta))| GlyphDelta {
        position: ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(pos);
          migrated_value
        }),
        x_delta,
        y_delta,
      })
      .collect::<Vec<_>>();

    for (a, b) in deltas.iter().zip(expected.iter()) {
      assert_eq!(a, b);
    }

    Ok(())
  }

  #[test]
  fn vazirmatn_var_a() -> ::core::result::Result<(), crate::test_support::Failure> {
    let gvar = match FontRef::new(font_test_data::VAZIRMATN_VAR) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
      }
    }
    .gvar()?;
    let a_glyph_var = crate::test_support::fixture_value(
      match gvar.glyph_variation_data({
        let Ok(explicit_value) = GlyphId::new(1);
        explicit_value
      }) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    assert_eq!(a_glyph_var.axis_count, 1);
    let mut tuples = a_glyph_var.tuples();
    let tup1 = crate::test_support::fixture_value(tuples.next(), "fixture value")?;
    let Ok(values) = tup1.peak().values();
    assert_eq!(
      values,
      &[{
        let Ok(value) = F2Dot14::from_f32(-1.0);
        value
      }]
    );
    assert_eq!(tup1.deltas().count(), 18);
    let x_vals = &[
      -90, -134, 4, -6, -81, 18, -25, -33, -109, -121, -111, -111, -22, -22, 0, -113, 0, 0,
    ];
    let y_vals = &[
      83, 0, 0, 0, 0, 0, 83, 0, 0, 0, -50, 54, 54, -50, 0, 0, -21, 0,
    ];
    assert_eq!(tup1.deltas().map(|d| d.x_delta).collect::<Vec<_>>(), x_vals);
    assert_eq!(tup1.deltas().map(|d| d.y_delta).collect::<Vec<_>>(), y_vals);
    let tup2 = crate::test_support::fixture_value(tuples.next(), "fixture value")?;
    let Ok(tup2_values) = tup2.peak().values();
    assert_eq!(
      tup2_values,
      &[{
        let Ok(value) = F2Dot14::from_f32(1.0);
        value
      }]
    );
    let x_vals_2 = &[
      20, 147, -33, -53, 59, -90, 37, -6, 109, 90, -79, -79, -8, -8, 0, 59, 0, 0,
    ];
    let y_vals_2 = &[
      -177, 0, 0, 0, 0, 0, -177, 0, 0, 0, 4, -109, -109, 4, 0, 0, 9, 0,
    ];

    assert_eq!(
      tup2.deltas().map(|d| d.x_delta).collect::<Vec<_>>(),
      x_vals_2
    );
    assert_eq!(
      tup2.deltas().map(|d| d.y_delta).collect::<Vec<_>>(),
      y_vals_2
    );
    assert!(tuples.next().is_none());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn vazirmatn_var_agrave() -> ::core::result::Result<(), crate::test_support::Failure> {
    let gvar = match FontRef::new(font_test_data::VAZIRMATN_VAR) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
      }
    }
    .gvar()?;
    let agrave_glyph_var = crate::test_support::fixture_value(
      match gvar.glyph_variation_data({
        let Ok(explicit_value) = GlyphId::new(2);
        explicit_value
      }) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    let mut tuples = agrave_glyph_var.tuples();
    let tup1 = crate::test_support::fixture_value(tuples.next(), "fixture value")?;
    assert_eq!(
      tup1
        .deltas()
        .map(|d| (d.position, d.x_delta, d.y_delta))
        .collect::<Vec<_>>(),
      &[(1, -51, 8), (3, -113, 0)]
    );
    let tup2 = crate::test_support::fixture_value(tuples.next(), "fixture value")?;
    assert_eq!(
      tup2
        .deltas()
        .map(|d| (d.position, d.x_delta, d.y_delta))
        .collect::<Vec<_>>(),
      &[(1, -54, -1), (3, 59, 0)]
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn vazirmatn_var_grave() -> ::core::result::Result<(), crate::test_support::Failure> {
    let gvar = match FontRef::new(font_test_data::VAZIRMATN_VAR) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
      }
    }
    .gvar()?;
    let grave_glyph_var = crate::test_support::fixture_value(
      match gvar.glyph_variation_data({
        let Ok(explicit_value) = GlyphId::new(3);
        explicit_value
      }) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    let mut tuples = grave_glyph_var.tuples();
    let tup1 = crate::test_support::fixture_value(tuples.next(), "fixture value")?;
    let tup2 = crate::test_support::fixture_value(tuples.next(), "fixture value")?;
    assert!(tuples.next().is_none());
    assert_eq!(tup1.deltas().count(), 8);
    assert_eq!(
      tup2.deltas().map(|d| d.y_delta).collect::<Vec<_>>(),
      &[0, -20, -20, 0, 0, 0, 0, 0]
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn phantom_point_deltas() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::VAZIRMATN_VAR),
      "fixture value",
    )?;

    #[rustfmt::skip]
        let a_cases = [

            (&[0.0], [(0.0, 0.0); 4]),
            (&[1.0], [(0.0, 0.0), (59.0, 0.0), (0.0, 9.0), (0.0, 0.0)]),
            (&[-1.0], [(0.0, 0.0), (-113.0, 0.0), (0.0, -21.0), (0.0, 0.0)]),
            (&[0.5], [(0.0, 0.0), (29.5, 0.0), (0.0, 4.5), (0.0, 0.0)]),
            (&[-0.5], [(0.0, 0.0), (-56.5, 0.0), (0.0, -10.5), (0.0, 0.0)]),
        ];
    for (coords, deltas) in a_cases {
      assert_eq!(
        ({
          let deltas_2 = compute_phantom_deltas(&font, coords, {
            let Ok(explicit_value) = GlyphId::new(1);
            explicit_value
          })?;
          deltas_2
        }),
        deltas
      );

      assert_eq!(
        ({
          let deltas_3 = compute_phantom_deltas(&font, coords, {
            let Ok(explicit_value) = GlyphId::new(2);
            explicit_value
          })?;
          deltas_3
        }),
        deltas
      );
    }

    #[rustfmt::skip]
        let grave_cases = [

            (&[0.0], [(0.0, 0.0); 4]),
            (&[1.0], [(0.0, 0.0), (63.0, 0.0), (0.0, 0.0), (0.0, 0.0)]),
            (&[-1.0], [(0.0, 0.0), (-96.0, 0.0), (0.0, 0.0), (0.0, 0.0)]),
            (&[0.5], [(0.0, 0.0), (31.5, 0.0), (0.0, 0.0), (0.0, 0.0)]),
            (&[-0.5], [(0.0, 0.0), (-48.0, 0.0), (0.0, 0.0), (0.0, 0.0)]),
        ];

    for (coords, deltas) in grave_cases {
      assert_eq!(
        ({
          let deltas_4 = compute_phantom_deltas(&font, coords, {
            let Ok(explicit_value) = GlyphId::new(3);
            explicit_value
          })?;
          deltas_4
        }),
        deltas
      );
    }

    Ok(())
  }

  fn compute_phantom_deltas(
    font: &FontRef,
    coords: &[f32],
    glyph_id: GlyphId,
  ) -> Result<[(f32, f32); 4], crate::test_support::Failure> {
    let loca = font.loca(None)?;
    let glyf = font.glyf()?;
    let gvar = font.gvar()?;
    let scaled = coords
      .iter()
      .map(|&coord| {
        let Ok(normalized) = F2Dot14::from_f32(coord);
        normalized
      })
      .collect::<Vec<_>>();
    let maybe_deltas = gvar.phantom_point_deltas(&glyf, &loca, &scaled, glyph_id)?;
    let deltas = crate::test_support::fixture_value(maybe_deltas, "phantom point deltas")?;
    Ok(deltas.map(|point| {
      let Ok(x) = point.x.to_f32();
      let Ok(y) = point.y.to_f32();
      (x, y)
    }))
  }

  #[test]
  fn avoid_data_range_overflow() -> ::core::result::Result<(), crate::read::ReadError> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    {
      buf.extend([1u16]);
      buf.extend([0u16]);
    };

    buf.extend([0u16]);

    {
      buf.extend([0u16]);
      buf.extend([0u32]);
    };

    buf.extend([1u16]);

    buf.extend([1u16]);

    buf.extend([::core::primitive::u32::wrapping_sub(u32::MAX, 10)]);

    {
      buf.extend([0u32]);
      buf.extend([11u32]);
    };

    let gvar = Gvar::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;

    let Ok(glyph) = GlyphId::new(0);
    assert!(matches!(
      gvar.data_range_for_gid(glyph),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn follow_use_my_metrics() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::gvar::USE_MY_METRICS)?;
    let glyf = font.glyf()?;
    let loca = font.loca(None)?;
    let post = font.post()?;

    let Ok(num_glyphs) = post.num_glyphs();
    let fixture_value = crate::test_support::fixture_value(num_glyphs, "fixture value")?;
    let mut gids = HashMap::new();

    for index in 0..fixture_value {
      let Ok(glyph) = GlyphId16::new(index);
      let name =
        crate::test_support::fixture_value(post.glyph_name(glyph), "post fixture glyph name")?;
      gids.insert(name, glyph);
    }

    let named_glyph = |name| crate::test_support::fixture_value(gids.get(name).copied(), name);

    for (glyph_name, source_name, expectation) in [
      (
        "neither",
        "neither",
        "a composite without any USE_MY_METRICS components should use its own metrics",
      ),
      (
        "firstonly",
        "first",
        "a composite with a single USE_MY_METRICS component should use that component's metrics",
      ),
      (
        "both",
        "second",
        "a composite with multiple USE_MY_METRICS components should use the last flagged component's metrics",
      ),
    ] {
      let checked_value = named_glyph(glyph_name)?;
      let glyph = GlyphId::from(checked_value);
      let expected = named_glyph(source_name)?;
      let (source, _) = find_glyph_and_point_count(&glyf, &loca, glyph, 5)?;
      assert_eq!(source, expected, "{expectation}");
    }

    ::core::result::Result::Ok(())
  }
}
