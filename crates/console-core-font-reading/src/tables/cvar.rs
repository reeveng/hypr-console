//! The [cvar (CVT Variations)](https://learn.microsoft.com/en-us/typography/opentype/spec/cvar)
//! table.

include!("../../generated/generated_cvar.rs");

use super::variations::{
  PackedPointNumbers, TupleDelta, TupleVariationCount, TupleVariationData, TupleVariationHeader,
};

pub type CvtVariationData<'a> = TupleVariationData<'a, CvtDelta>;

impl<'a> Cvar<'a> {
  pub fn variation_data(&self, axis_count: u16) -> Result<CvtVariationData<'a>, ReadError> {
    let Ok(count) = self.tuple_variation_count();
    let data = self.data()?;
    let header_data = self.raw_tuple_header_data()?;

    let (shared_point_numbers, serialized_data) = match ::core::primitive::bool::then_some(count.shared_point_numbers(), ()) {
      ::core::option::Option::Some(()) => {
        let (packed, data_2) = PackedPointNumbers::split_off_front(data);
        (Some(packed), data_2)
      }
      ::core::option::Option::None => (None, data),
    };
    Ok(CvtVariationData {
      tuple_count: count,
      axis_count,
      shared_tuples: None,
      shared_point_numbers,
      header_data,
      serialized_data,
      _marker: std::marker::PhantomData,
    })
  }

  pub fn deltas(
    &self,
    axis_count: u16,
    coords: &[F2Dot14],
    deltas: &mut [i32],
  ) -> Result<(), ReadError> {
    let var_data = self.variation_data(axis_count)?;

    for (tuple, scalar) in var_data.active_tuples_at(coords) {
      for delta in tuple.deltas() {
        let ix = {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(delta.position);
          migrated_value
        };
        let Ok(value_2) = console_core_number_conversion::index(ix);

        match deltas.get_mut(value_2) {
          Some(value) => {
            let Ok(bits) = delta.apply_scalar(scalar).to_bits();
            *value = value.wrapping_add(bits);
          }
          None => {}
        }
      }
    }

    Ok(())
  }

  fn raw_tuple_header_data(&self) -> Result<FontData<'a>, ReadError> {
    let Ok(range) = self.tuple_variation_headers_byte_range();
    let Ok(data) = self.data.split_off(range.start);

    data.ok_or(ReadError::OutOfBounds)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CvtDelta {
  pub position: u16,
  pub value: i32,
}

impl CvtDelta {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `CvtDelta::apply_scalar` method returns `Fixed` directly in the existing parser interface")
  )]
  pub fn apply_scalar(self, scalar: Fixed) -> Fixed {
    let Ok(value_1) = Fixed::from_i32(self.value);

    value_1 * scalar
  }
}

impl TupleDelta for CvtDelta {
  fn is_point() -> bool {
    false
  }

  fn new(position: u16, x: i32, _y: i32) -> Self {
    Self { position, value: x }
  }
}

#[cfg(test)]
mod tests {
  use font_types::F2Dot14;

  use crate::{FontRef, TableProvider};

  const SCALED_DELTA_CASES: &[([f32; 2], [i32; 120])] = &[
      (
        [0.5, 0.5],
        [
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 720896, 3276800, 1179648, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 622592, 0, 1179648, 0, 0, 0, 0, 0, 622592, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
      ),
      (
        [-0.5, 0.5],
        [
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, -1441792, -2162688, -1277952, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, -917504, 0, -1277952, 0, 0, 0, 0, 0, -720896, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
      ),
      (
        [0.5, -0.5],
        [
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 1900544, 2621440, 2129920, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 360448, 0, 1015808, 0, 0, 0, 0, 0, 524288, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
      ),
      (
        [-0.5, -0.5],
        [
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, -1212416, -2293760, -1130496, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, -1097728, 0, -1277952, 0, 0, 0, 0, 0, -737280, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
      ),
      (
        [-1.0, -1.0],
        [
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, -2490368, -4325376, -2490368, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, -1835008, 0, -2555904, 0, 0, 0, 0, 0, -1441792, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
      ),
      (
        [1.0, 1.0],
        [
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 1441792, 6553600, 2359296, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 1245184, 0, 2359296, 0, 0, 0, 0, 0, 1245184, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
      ),
      (
        [-1.0, 1.0],
        [
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, -2883584, -4325376, -2555904, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, -1835008, 0, -2555904, 0, 0, 0, 0, 0, -1441792, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
      ),
      (
        [1.0, -1.0],
        [
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 5636096, 4456448, 5636096, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 917504, 0, 1703936, 0, 0, 0, 0, 0, 917504, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
          0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
      ),
    ] ;

  #[test]
  fn scaled_deltas() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font =
      crate::test_support::fixture_value(FontRef::new(font_test_data::CVAR), "fixture value")?;


    let cvar = crate::test_support::fixture_value(font.cvar(), "fixture value")?;
    let fixture_value = crate::test_support::fixture_value(font.fvar(), "fixture value")?;
    let Ok(axis_count) =
      fixture_value.axis_count();
    let cvar_data =
      crate::test_support::fixture_value(cvar.variation_data(axis_count), "fixture value")?;

    for (coords, expected_deltas) in SCALED_DELTA_CASES {
      let coords_2 = coords.map(|argument| {
        let Ok(value_3) = F2Dot14::from_f32(argument);

        value_3
      });
      let mut deltas = vec![0i32; expected_deltas.len()];

      'over_tuple_2: for tuple in cvar_data.tuples() {
        let scalar = match tuple.compute_scalar(&coords_2) {
          Some(scalar) => scalar,
          None => {
            continue 'over_tuple_2;
          }
        };

        for delta in tuple.deltas() {
          let scaled_delta = delta.apply_scalar(scalar);

          let Ok(position) = console_core_number_conversion::index(u64::from(delta.position));
          let slot = crate::test_support::fixture_value(
            deltas.get_mut(position), "cvar destination delta",
          )?;
          let Ok(bits) = scaled_delta.to_bits();
          *slot = slot.wrapping_add(bits);
        }
      }

      assert_eq!(&deltas, expected_deltas);
    }

    Ok(())
  }

  #[test]
  fn raw_tuple_deltas() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::CVAR)?;
    let cvar = font.cvar()?;
    let Ok(axis_count) = match font.fvar() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(read_error),
    }
    .axis_count();
    let cvar_data = cvar.variation_data(axis_count)?;

    let expected = [
      &[(65, 8), (66, -8), (67, 8), (85, -11), (87, 0), (93, -1)],
      &[(65, -2), (66, 8), (67, -7), (85, 11), (87, 0), (93, 1)],
      &[(65, 56), (66, -24), (67, 42), (85, 6), (87, -10), (93, -4)],
      &[
        (65, -44),
        (66, -66),
        (67, -39),
        (85, -28),
        (87, -39),
        (93, -22),
      ],
      &[(65, 22), (66, 100), (67, 36), (85, 19), (87, 36), (93, 19)],
      &[(65, 8), (66, 0), (67, 8), (85, -43), (87, -49), (93, -32)],
      &[(65, -8), (66, 0), (67, -8), (85, 11), (87, 9), (93, 1)],
      &[(65, -80), (66, 0), (67, -90), (85, -6), (87, -47), (93, 4)],
      &[(65, -16), (66, 0), (67, -21), (85, 28), (87, 39), (93, 22)],
      &[
        (65, -46),
        (66, 0),
        (67, -22),
        (85, -19),
        (87, 35),
        (93, -19),
      ],
      &[(65, 2), (66, 0), (67, 7), (85, -11), (87, -9), (93, -1)],
    ];
    let mut count = 0u64;

    for (tuple, expected_2) in cvar_data.tuples().zip(&expected) {
      count = count.wrapping_add(1);
      let deltas = tuple
        .deltas()
        .map(|delta| (delta.position, delta.value))
        .collect::<Vec<_>>();
      assert_eq!(&deltas, expected_2);
    }

    let Ok(expected_count) = console_core_number_conversion::fixed_index(expected.len());
    assert_eq!(count, expected_count);
    ::core::result::Result::Ok(())
  }
}
