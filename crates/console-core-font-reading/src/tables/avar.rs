//! The [Axis Variations](https://docs.microsoft.com/en-us/typography/opentype/spec/avar) table

use super::variations::{DeltaSetIndexMap, ItemVariationStore};

include!("../../generated/generated_avar.rs");

impl SegmentMaps<'_> {
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public axis remapper returns a Fixed coordinate directly to existing variable-font callers"
    )
  )]
  #[must_use]
  pub fn apply(&self, coord: Fixed) -> Fixed {
    let Ok(maps) = self.axis_value_maps();
    let Ok(adjusted) = apply_segment_map(maps, coord);

    adjusted
  }
}

#[derive(Clone, Copy)]
struct AxisMapping {
  from: Fixed,
  to: Fixed,
}

impl AxisMapping {
  fn read(record: &AxisValueMap) -> Result<Self, Never> {
    let Ok(from) = record.from_coordinate();
    let Ok(to) = record.to_coordinate();
    let Ok(from_fixed) = from.to_fixed();
    let Ok(to_fixed) = to.to_fixed();

    Ok(Self {
      from: from_fixed,
      to: to_fixed,
    })
  }

  fn extrapolate(self, coordinate: Fixed) -> Result<Fixed, Never> {
    Ok(coordinate - self.from + self.to)
  }

  fn interpolate(self, next: Self, coordinate: Fixed) -> Result<Fixed, Never> {
    let Ok(delta) = (next.to - self.to).mul_div(coordinate - self.from, next.from - self.from);

    Ok(self.to + delta)
  }
}

fn apply_segment_map(maps: &[AxisValueMap], coordinate: Fixed) -> Result<Fixed, Never> {
  let Ok(negative_limit) = Fixed::from_i32(-1);
  let Ok(positive_limit) = Fixed::from_i32(1);

  let (before, without_lower_marker) = match maps.split_first() {
    None => return Ok(coordinate),
    Some((first, rest)) => {
      let Ok(lower) = AxisMapping::read(first);
      let second = match rest.first() {
        Some(record) => record,
        None => return lower.extrapolate(coordinate),
      };
      let Ok(second_coordinate) = AxisMapping::read(second);

      let duplicates_lower_marker =
        lower.from == negative_limit && lower.to == negative_limit && second_coordinate.from == negative_limit;

      match duplicates_lower_marker.then_some(lower) {
        Some(marker) => (Some(marker), rest),
        None => (None, maps),
      }
    }
  };
  let upper_boundary = without_lower_marker
    .split_last()
    .and_then(|(last, rest)| rest.last().map(|previous| (last, previous, rest)));
  let candidates = match upper_boundary {
    Some((last, previous, rest)) => {
      let Ok(upper) = AxisMapping::read(last);
      let Ok(previous_coordinate) = AxisMapping::read(previous);

      let duplicates_upper_marker =
        upper.from == positive_limit && upper.to == positive_limit && previous_coordinate.from == positive_limit;

      match duplicates_upper_marker.then_some(rest) {
        Some(without_upper_marker) => without_upper_marker,
        None => without_lower_marker,
      }
    }
    None => without_lower_marker,
  };
  let Ok(exact) = exact_coordinate(candidates, coordinate);

  match exact {
    Some(mapped) => Ok(mapped),
    None => interpolate_coordinate(candidates, before, coordinate),
  }
}

fn exact_coordinate(maps: &[AxisValueMap], coordinate: Fixed) -> Result<Option<Fixed>, Never> {
  let matches_coordinate = |mapping: &AxisMapping| mapping.from == coordinate;
  let mut matching = maps
    .iter()
    .map(|record| {
      let Ok(mapping) = AxisMapping::read(record);

      mapping
    })
    .skip_while(|mapping| !matches_coordinate(mapping))
    .take_while(matches_coordinate);
  let first = match matching.next() {
    Some(first) => first,
    None => return Ok(None),
  };
  let second = match matching.next() {
    Some(second) => second,
    None => return Ok(Some(first.to)),
  };
  let last = match matching.next() {
    None => second,
    Some(_third) => match matching.next() {
      None => return Ok(Some(second.to)),
      Some(fourth) => match matching.last() {
        Some(last) => last,
        None => fourth,
      },
    },
  };

  use core::cmp::Ordering;
  Ok(Some(match coordinate.cmp(&Fixed::ZERO) {
    Ordering::Less => last.to,
    Ordering::Greater => first.to,
    Ordering::Equal => {
      let Ok(first_abs) = first.to.abs();
      let Ok(last_abs) = last.to.abs();

      match first_abs.cmp(&last_abs) {
        Ordering::Less => first.to,
        Ordering::Equal | Ordering::Greater => last.to,
      }
    }
  }))
}

fn interpolate_coordinate(
  maps: &[AxisValueMap],
  previous: Option<AxisMapping>,
  coordinate: Fixed,
) -> Result<Fixed, Never> {
  let mut preceding = previous;

  for record in maps {
    let Ok(next) = AxisMapping::read(record);

    match coordinate.cmp(&next.from) {
      core::cmp::Ordering::Less => {
        return match preceding {
          Some(before) => before.interpolate(next, coordinate),
          None => next.extrapolate(coordinate),
        };
      }
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => preceding = Some(next),
    }
  }

  match preceding {
    Some(last) => last.extrapolate(coordinate),
    None => Ok(coordinate),
  }
}

impl VarSize for SegmentMaps<'_> {
  type Size = u16;

  fn read_len_at(data: FontData, pos: u64) -> Result<u64, ReadError> {
    let count = data.read_at::<u16>(pos)?;
    let size = u64::from(count)
      .checked_mul(AxisValueMap::RAW_BYTE_LEN)
      .ok_or(ReadError::OutOfBounds)?;

    size
      .checked_add(u16::RAW_BYTE_LEN)
      .ok_or(ReadError::OutOfBounds)
  }
}

impl<'a> FontRead<'a> for SegmentMaps<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let Ok(mut cursor) = data.cursor();
    let position_map_count: BigEndian<u16> = cursor.read_be()?;
    let Ok(value_12) = position_map_count.get();

    let axis_value_maps = cursor.read_array({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_12);
      migrated_value
    })?;
    Ok(SegmentMaps {
      position_map_count,
      axis_value_maps,
    })
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::{FontRef, TableProvider};

  fn value_map(values: [f32; 2]) -> Result<[F2Dot14; 2], Never> {
    Ok(values.map(|coord| {
      let Ok(normalized) = F2Dot14::from_f32(coord);
      normalized
    }))
  }

  impl PartialEq<[F2Dot14; 2]> for AxisValueMap {
    fn eq(&self, other: &[F2Dot14; 2]) -> bool {
      self.from_coordinate
        == *match &*other {
          [element, ..] => element,
        }
        && self.to_coordinate
          == *match &*other {
            [_, element, ..] => element,
          }
    }
  }

  #[test]
  fn segment_maps() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let avar = font.avar()?;
    assert_eq!(avar.axis_count(), Ok(1));
    let expected_segment_maps = &[vec![
      ({
        let Ok(explicit_value) = value_map([-1.0, -1.0]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([-0.6667, -0.5]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([-0.3333, -0.25]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([0.0, 0.0]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([0.2, 0.3674]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([0.4, 0.52246]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([0.6, 0.67755]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([0.8, 0.83875]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([1.0, 1.0]);
        explicit_value
      }),
    ]];
    let Ok(axis_segment_maps) = avar.axis_segment_maps();
    let Ok(value_35) = axis_segment_maps.iter();

    let segment_maps = value_35
      .map(|segment_map| segment_map.map(|map| {
        let Ok(axis_values) = map.axis_value_maps();
        axis_values.to_owned()
      }))
      .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(segment_maps, expected_segment_maps);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn segment_maps_multi_axis() -> ::core::result::Result<(), crate::test_support::Failure> {
    let segment_one_maps = [
      ({
        let Ok(explicit_value) = value_map([-1.0, -1.0]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([-0.6667, -0.5]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([-0.3333, -0.25]);
        explicit_value
      }),
    ];
    let segment_two_maps = [
      ({
        let Ok(explicit_value) = value_map([0.8, 0.83875]);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = value_map([1.0, 1.0]);
        explicit_value
      }),
    ];

    let data = font_test_data::be_buffer! { (MajorMinor::VERSION_1_0), (0u16), (2u16), (3u16), (..match segment_one_maps { [_explicit_element, ..] => _explicit_element }), (..match segment_one_maps { [_, _explicit_element, ..] => _explicit_element }), (..match segment_one_maps { [_, _, _explicit_element, ..] => _explicit_element }), (2u16), (..match segment_two_maps { [_explicit_element, ..] => _explicit_element }), (..match segment_two_maps { [_, _explicit_element, ..] => _explicit_element }) };

    let avar = super::Avar::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;
    let Ok(axis_segment_maps) = avar.axis_segment_maps();
    let Ok(value_36) = axis_segment_maps.iter();

    assert_eq!(value_36.count(), 2);
    let Ok(avar_axis_segment_maps) = avar.axis_segment_maps();
    let fixture_value = crate::test_support::fixture_value(
      match avar_axis_segment_maps.get(0) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
      },
      "fixture value"
    )?;
    assert_eq!(
      fixture_value
      .axis_value_maps,
      segment_one_maps,
    );
    assert_eq!(
      {
        let second_axis_map = crate::test_support::fixture_value(
        match ({
          let Ok(explicit_value) = avar.axis_segment_maps();
          explicit_value
        })
        .get(1)
        {
          ::core::result::Result::Ok(read_value) => read_value,
          ::core::result::Result::Err(read_error) =>
            return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
        },
        "fixture value"
      )?;
        second_axis_map
      }
      .axis_value_maps,
      segment_two_maps,
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn piecewise_linear() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let avar = font.avar()?;
    let Ok(axis_segment_maps) = avar.axis_segment_maps();
    let segment_map = crate::test_support::fixture_value(
      match axis_segment_maps.get(0) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    let coords = [-1.0, -0.5, 0.0, 0.5, 1.0];
    let expected_result = [-1.0, -0.375, 0.0, 0.600006103515625, 1.0];
    assert_eq!(
      {
        let [_explicit_part @ ..] = &expected_result;
        let _explicit_slice: &[_] = _explicit_part;
        _explicit_slice
      },
      &coords
        .iter()
        .map(|coord| {
          let Ok(value) = segment_map
            .apply({
              let Ok(explicit_value) = Fixed::from_f64(*coord);
              explicit_value
            })
            .to_f64();
          value
        })
        .collect::<Vec<_>>()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn avar2() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::AVAR2_CHECKER)?;
    let avar = font.avar()?;
    assert_eq!(avar.version(), Ok(MajorMinor::VERSION_2_0));
    assert!(matches!(avar.axis_index_map_offset(), Ok(Some(_))));
    assert!(matches!(avar.var_store_offset(), Ok(Some(_))));
    assert!(
      match avar.var_store() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      }
      .is_some()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn piecewise_linear_zero_tie_break_matches_harfbuzz() {
    let maps = [
      AxisValueMap {
        from_coordinate:
          <::font_types::BigEndian<::font_types::F2Dot14> as ::core::convert::From<
            ::font_types::F2Dot14,
          >>::from(F2Dot14::NEG_ONE),
        to_coordinate:
          <::font_types::BigEndian<::font_types::F2Dot14> as ::core::convert::From<
            ::font_types::F2Dot14,
          >>::from(F2Dot14::NEG_ONE),
      },
      AxisValueMap {
        from_coordinate:
          <::font_types::BigEndian<::font_types::F2Dot14> as ::core::convert::From<
            ::font_types::F2Dot14,
          >>::from(F2Dot14::ZERO),
        to_coordinate:
          <::font_types::BigEndian<::font_types::F2Dot14> as ::core::convert::From<
            ::font_types::F2Dot14,
          >>::from({
            let Ok(value) = F2Dot14::from_f32(-0.25);
            value
          }),
      },
      AxisValueMap {
        from_coordinate:
          <::font_types::BigEndian<::font_types::F2Dot14> as ::core::convert::From<
            ::font_types::F2Dot14,
          >>::from(F2Dot14::ZERO),
        to_coordinate:
          <::font_types::BigEndian<::font_types::F2Dot14> as ::core::convert::From<
            ::font_types::F2Dot14,
          >>::from({
            let Ok(value) = F2Dot14::from_f32(0.25);
            value
          }),
      },
      AxisValueMap {
        from_coordinate:
          <::font_types::BigEndian<::font_types::F2Dot14> as ::core::convert::From<
            ::font_types::F2Dot14,
          >>::from(F2Dot14::ONE),
        to_coordinate:
          <::font_types::BigEndian<::font_types::F2Dot14> as ::core::convert::From<
            ::font_types::F2Dot14,
          >>::from(F2Dot14::ONE),
      },
    ];
    let segment_map = SegmentMaps {
      position_map_count:
        <::font_types::BigEndian<::core::primitive::u16> as ::core::convert::From<
          ::core::primitive::u16,
        >>::from({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(maps.len());
          migrated_value
        }),
      axis_value_maps: &maps,
    };
    assert_eq!(
      segment_map.apply(Fixed::ZERO),
      ({
        let Ok(explicit_value) = Fixed::from_f64(0.25);
        explicit_value
      })
    );
  }
}
