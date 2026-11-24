//! The [Font Variations](https://docs.microsoft.com/en-us/typography/opentype/spec/fvar) table

include!("../../generated/generated_fvar.rs");

#[path = "./instance_record.rs"]
mod instance_record;

use super::{avar::Avar, variations::DeltaSetIndex};
use alloc::vec::Vec;

pub use instance_record::InstanceRecord;

type Avar2InlineAxes<T> = [T; 64];

#[inline]
fn apply_avar2_delta(
  coord: Fixed,
  item_delta: super::variations::FloatItemDelta,
) -> Result<Fixed, console_core_never::Never> {
  let fractional_bits = item_delta.to_f64() * 4.0;
  let biased_bits = match crate::Presence::from(fractional_bits >= 0.0) {
    crate::Presence::Present => fractional_bits + 0.5,
    crate::Presence::Absent => fractional_bits - 0.5,
  };
  let Ok(delta_bits) = console_core_number_conversion::toward_zero_i32(biased_bits);
  let Ok(coord_bits) = coord.to_bits();
  Fixed::from_bits(coord_bits.wrapping_add(delta_bits))
}

fn normalize_user_coords<T>(
  axes: &[VariationAxisRecord],
  user_coords: impl IntoIterator<Item = (Tag, Fixed)>,
  coords: &mut [T],
  convert: impl Fn(Fixed) -> T,
) -> core::result::Result<(), console_core_never::Never> {
  {
    for user_coord in user_coords {
      for (axis, coord) in axes.iter().zip(coords.iter_mut()).filter(|(axis, _)| {
        let Ok(axis_tag) = axis.axis_tag();

        axis_tag == user_coord.0
      }) {
        *coord = convert(axis.normalize(user_coord.1));
      }
    }
  };

  Ok(())
}

fn apply_avar_mappings<T>(
  avar: Option<&Avar>,
  coords: &mut [T],
  to_fixed: impl Fn(&T) -> Fixed,
  from_fixed: impl Fn(Fixed) -> T,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let avar_mappings = avar.map(|avar_2| {
      let Ok(maps) = avar_2.axis_segment_maps();

      maps
    });

    for (i, coord) in coords.iter_mut().enumerate() {
      match avar_mappings
        .as_ref()
        .and_then(|mappings| {
          let Ok(value_9) = console_core_number_conversion::fitted::<_, u64>(i);

          match mappings.get(value_9) {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }
        })
        .flatten()
      {
        Some(mapping) => {
          *coord = from_fixed(mapping.apply(to_fixed(coord)));
        }
        None => {}
      }
    }
  };

  Ok(())
}

fn to_normalized_coords(
  fixed_coords: &[Fixed],
  normalized_coords: &mut [F2Dot14],
) -> core::result::Result<(), console_core_never::Never> {
  {
    for (target_coord, coord) in normalized_coords.iter_mut().zip(fixed_coords.iter()) {
      let Ok(value_2) = coord.to_f2dot14();

      *target_coord = value_2;
    }
  };

  Ok(())
}

impl<'a> Fvar<'a> {
  pub fn axes(&self) -> Result<&'a [VariationAxisRecord], ReadError> {
    let axis_instance_arrays = self.axis_instance_arrays()?;
    let Ok(explicit_value) = axis_instance_arrays.axes();

    Ok(explicit_value)
  }

  pub fn instances(&self) -> Result<ComputedArray<'a, InstanceRecord<'a>>, ReadError> {
    let axis_instance_arrays = self.axis_instance_arrays()?;
    let Ok(explicit_value) = axis_instance_arrays.instances();

    Ok(explicit_value)
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Fvar::user_to_normalized` method is called through its existing unit-returning signature"
    )
  )]
  pub fn user_to_normalized(
    &self,
    avar: Option<&Avar>,
    user_coords: impl IntoIterator<Item = (Tag, Fixed)>,
    normalized_coords: &mut [F2Dot14],
  ) {
    normalized_coords.fill(F2Dot14::ZERO);
    let axes = match self.axes() {
      Ok(axes) => axes,
      Err(_unreadable) => &[],
    };
    let Ok(axis_length) = console_core_number_conversion::fixed_index(axes.len());
    let Ok(output_length) = console_core_number_conversion::fixed_index(normalized_coords.len());
    let Ok(actual_len) = console_core_number_conversion::index(axis_length.min(output_length));
    let normalized = match normalized_coords.get_mut(..actual_len) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => &mut [],
    };
    let mut stack_fixed_coords: Avar2InlineAxes<Fixed> = [Fixed::ZERO; _];
    let mut heap_fixed_coords = Vec::new();
    let fixed_coords = match stack_fixed_coords.get_mut(..actual_len) {
      Some(coords) => coords,
      None => {
        heap_fixed_coords.resize(normalized.len(), Fixed::ZERO);
        heap_fixed_coords.as_mut_slice()
      }
    };
    let Ok(()) = normalize_user_coords(axes, user_coords, fixed_coords, core::convert::identity);
    let Ok(()) = apply_avar_mappings(avar, fixed_coords, |coord| *coord, core::convert::identity);

    let avar_3 = match avar {
      Some(avar_2) => avar_2,
      None => {
        let Ok(()) = to_normalized_coords(fixed_coords, normalized);
        return;
      }
    };

    let Ok(version) = avar_3.version();

    match ::core::primitive::bool::then_some(version == MajorMinor::VERSION_1_0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = to_normalized_coords(fixed_coords, normalized);
        return;
      }
      ::core::option::Option::None => {}
    }

    let var_store = avar_3.var_store();
    let var_index_map = avar_3.axis_index_map();

    let mut stack_coords_2dot14: Avar2InlineAxes<F2Dot14> = [F2Dot14::ZERO; _];
    let mut heap_coords_2dot14 = Vec::new();
    let coords_2dot14 = match stack_coords_2dot14.get_mut(..actual_len) {
      Some(coords) => coords,
      None => {
        heap_coords_2dot14.resize(normalized.len(), F2Dot14::ZERO);
        heap_coords_2dot14.as_mut_slice()
      }
    };

    for (coord_2dot14, coord) in coords_2dot14.iter_mut().zip(fixed_coords.iter()) {
      let Ok(value_3) = coord.to_f2dot14();

      *coord_2dot14 = value_3;
    }

    let usable_store = match var_store {
      Ok(store) => store,
      Err(_unreadable_variation_store) => None,
    };
    let varstore = match usable_store {
      Some(store) => store,
      None => {
        let Ok(()) = to_normalized_coords(fixed_coords, normalized);
        return;
      }
    };
    let index_map = match var_index_map {
      Ok(map) => map,
      Err(_unreadable_variation_index_map) => None,
    };

    for (position, coord) in fixed_coords.iter_mut().enumerate() {
      let Ok(axis) = console_core_number_conversion::wrapped::<_, u16>(position);
      let var_index = match &index_map {
        Some(map) => match map.get(u32::from(axis)) {
          Ok(index) => index,
          Err(_invalid_variation_map_index) => continue,
        },
        None => DeltaSetIndex {
          outer: 0,
          inner: axis,
        },
      };
      let delta = match varstore.compute_float_delta(var_index, coords_2dot14) {
        Ok(delta) => delta,
        Err(_invalid_variation_delta) => continue,
      };
      let Ok(adjusted) = apply_avar2_delta(*coord, delta);
      *coord = adjusted.clamp(Fixed::NEG_ONE, Fixed::ONE);
    }

    let Ok(()) = to_normalized_coords(fixed_coords, normalized);
  }
}

impl VariationAxisRecord {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `VariationAxisRecord::normalize` method returns `Fixed` directly in the existing parser interface"
    )
  )]
  pub fn normalize(&self, mut value: Fixed) -> Fixed {
    use core::cmp::Ordering::*;
    let Ok(min_value) = self.min_value();
    let Ok(default_value) = self.default_value();

    let Ok(own_max_value) = self.max_value();
    let max_value = own_max_value.max(min_value);
    value = value.clamp(min_value, max_value);
    value = match value.cmp(&default_value) {
      Less => {
        let Ok(value_4) = default_value.saturating_sub(value);
        let Ok(value_5) = default_value.saturating_sub(min_value);

        -(value_4 / value_5)
      }
      Greater => {
        let Ok(value_6) = value.saturating_sub(default_value);
        let Ok(value_7) = max_value.saturating_sub(default_value);

        value_6 / value_7
      }
      Equal => Fixed::ZERO,
    };
    value.clamp(Fixed::NEG_ONE, Fixed::ONE)
  }
}

#[cfg(test)]
mod tests {
  use crate::{FontRef, TableProvider};
  use types::{F2Dot14, Fixed, NameId, Tag};

  #[test]
  fn axes() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let fvar = font.fvar()?;
    assert_eq!(fvar.axis_count(), Ok(1));
    let fixture_value = crate::test_support::fixture_value(
      match fvar.axes() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      }
      .first(),
      "fixture value",
    )?;
    let wght = &fixture_value;
    assert_eq!(
      wght.axis_tag(),
      Ok({
        let Ok(value) = Tag::new(b"wght");
        value
      })
    );
    assert_eq!(
      wght.min_value(),
      Ok({
        let Ok(explicit_value) = Fixed::from_f64(100.0);
        explicit_value
      })
    );
    assert_eq!(
      wght.default_value(),
      Ok({
        let Ok(explicit_value) = Fixed::from_f64(400.0);
        explicit_value
      })
    );
    assert_eq!(
      wght.max_value(),
      Ok({
        let Ok(explicit_value) = Fixed::from_f64(900.0);
        explicit_value
      })
    );
    assert_eq!(wght.flags(), Ok(0));
    assert_eq!(
      wght.axis_name_id(),
      Ok({
        let Ok(explicit_value) = NameId::new(257);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn instances() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::VAZIRMATN_VAR),
      "fixture value",
    )?;
    let fvar = crate::test_support::fixture_value(font.fvar(), "fixture value")?;
    assert_eq!(fvar.instance_count(), Ok(9));

    let instances = crate::test_support::fixture_value(fvar.instances(), "fixture value")?;

    for i in 0..9 {
      let Ok(ordinal) = console_core_number_conversion::Float::float(u64::wrapping_add(i, 1));
      let value = 100.0 * ordinal;
      let Ok(name_id) = NameId::new(::core::primitive::u16::wrapping_add(258, {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(i);
        migrated_value
      }));
      let instance = crate::test_support::fixture_value(instances.get(i), "fixture value")?;
      assert_eq!(instance.coordinates.len(), 1);
      assert_eq!(
        ({
          let fixture_value =
            crate::test_support::fixture_value(instance.coordinates.first(), "fixture value")?;
          let Ok(value_2) = fixture_value.get();
          value_2
        }),
        {
          let Ok(value_3) = Fixed::from_f64(value);
          value_3
        }
      );
      assert_eq!(instance.subfamily_name_id, name_id);
      assert_eq!(instance.post_script_name_id, None);
    }

    Ok(())
  }

  #[test]
  fn normalize() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::VAZIRMATN_VAR),
      "fixture value",
    )?;
    let fvar = crate::test_support::fixture_value(font.fvar(), "fixture value")?;
    let fixture_value = crate::test_support::fixture_value(fvar.axes(), "fixture value")?;
    let axis = crate::test_support::fixture_value(fixture_value.first(), "fixture value")?;
    let values = [100.0, 220.0, 250.0, 400.0, 650.0, 900.0];
    let expected = [-1.0, -0.60001, -0.5, 0.0, 0.5, 1.0];

    for (value, expected_2) in values.into_iter().zip(expected) {
      assert_eq!(
        axis.normalize({
          let Ok(explicit_value) = Fixed::from_f64(value);
          explicit_value
        }),
        ({
          let Ok(explicit_value) = Fixed::from_f64(expected_2);
          explicit_value
        })
      );
    }

    Ok(())
  }

  #[test]
  fn normalize_overflow() -> ::core::result::Result<(), crate::read::ReadError> {
    let test_case = &[
      79, 84, 84, 79, 0, 1, 32, 32, 255, 32, 32, 32, 102, 118, 97, 114, 32, 32, 32, 32, 0, 0, 0,
      28, 0, 0, 0, 41, 32, 0, 0, 0, 0, 1, 32, 32, 0, 2, 32, 32, 32, 32, 0, 0, 32, 32, 32, 32, 32,
      0, 0, 0, 0, 153, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
    ];
    let font = FontRef::new(test_case)?;
    let fvar = font.fvar()?;
    let axis = *match <[_]>::get(
      &*match fvar.axes() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(read_error);
        }
      },
      1,
    ) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
      }
    };

    assert_eq!(
      axis.normalize({
        let Ok(explicit_value) = Fixed::from_f64(0.0);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = Fixed::from_f64(-0.2509765625);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn user_to_normalized() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::from_index(font_test_data::VAZIRMATN_VAR, 0),
      "fixture value",
    )?;
    let fvar = crate::test_support::fixture_value(font.fvar(), "fixture value")?;
    let avar = match font.avar() {
      Ok(value) => Some(value),
      Err(_unavailable_axis_variations) => None,
    };
    let wght = {
      let Ok(value) = Tag::new(b"wght");
      value
    };
    let fixture_value = crate::test_support::fixture_value(fvar.axes(), "fixture value")?;
    let axis = crate::test_support::fixture_value(fixture_value.first(), "weight axis")?;
    let mut normalized_coords = [F2Dot14::default(); 1];

    let avar_user = {
      let Ok(default_value) = axis.default_value();
      let Ok(value) = default_value.to_f32();
      value
    } + (({
      let Ok(max_value) = axis.max_value();
      let Ok(explicit_value) = max_value.to_f32();
      explicit_value
    }) - ({
      let Ok(default_value) = axis.default_value();
      let Ok(explicit_value) = default_value.to_f32();
      explicit_value
    }))
      * 0.8;
    let avar_normalized = 0.83875;

    #[rustfmt::skip]
        let cases = [

            (-1000.0, -1.0f32),
            (100.0, -1.0),
            (200.0, -0.5),
            (400.0, 0.0),
            (900.0, 1.0),
            (avar_user, avar_normalized),
            (1251.5, 1.0),
        ];
    for (user, normalized) in cases {
      fvar.user_to_normalized(
        avar.as_ref(),
        [(wght, {
          let Ok(value) = Fixed::from_f64(f64::from(user));
          value
        })],
        &mut normalized_coords,
      );
      assert_eq!(
        *match &normalized_coords {
          [element, ..] => element,
        },
        ({
          let Ok(explicit_value) = F2Dot14::from_f32(normalized);
          explicit_value
        })
      );
    }

    Ok(())
  }

  #[test]
  fn avar2() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::AVAR2_CHECKER),
      "fixture value",
    )?;
    let avar = match font.avar() {
      Ok(value) => Some(value),
      Err(_unavailable_axis_variations) => None,
    };
    let fvar = crate::test_support::fixture_value(font.fvar(), "fixture value")?;
    let avar_axis = {
      let Ok(value) = Tag::new(b"AVAR");
      value
    };
    let avwk_axis = {
      let Ok(value) = Tag::new(b"AVWK");
      value
    };
    let mut normalized_coords = [F2Dot14::default(); 2];

    let cases = [
      ((100.0, 0.0), (1.0, 1.0)),
      ((50.0, 0.0), (0.5, 0.5)),
      ((0.0, 50.0), (0.0, 0.5)),
    ];
    for (user, expected) in cases {
      fvar.user_to_normalized(
        avar.as_ref(),
        [
          (avar_axis, {
            let Ok(value) = Fixed::from_f64(user.0);
            value
          }),
          (avwk_axis, {
            let Ok(value) = Fixed::from_f64(user.1);
            value
          }),
        ],
        &mut normalized_coords,
      );
      assert_eq!(
        *match &normalized_coords {
          [element, ..] => element,
        },
        ({
          let Ok(explicit_value) = F2Dot14::from_f32(expected.0);
          explicit_value
        })
      );
      assert_eq!(
        *match &normalized_coords {
          [_, element, ..] => element,
        },
        ({
          let Ok(explicit_value) = F2Dot14::from_f32(expected.1);
          explicit_value
        })
      );
    }

    Ok(())
  }

  #[test]
  fn avar2_no_panic_with_wrong_size_coords_array()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::AVAR2_CHECKER)?;
    let avar = match font.avar() {
      Ok(value) => Some(value),
      Err(_unavailable_axis_variations) => None,
    };
    let fvar = font.fvar()?;

    let mut normalized_coords = [F2Dot14::default(); 1];
    fvar.user_to_normalized(avar.as_ref(), [], &mut normalized_coords);

    let mut normalized_coords_2 = [F2Dot14::default(); 4];
    fvar.user_to_normalized(avar.as_ref(), [], &mut normalized_coords_2);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn avar2_preserves_16_16_precision_until_final_rounding() {
    let Ok(coord) = Fixed::from_bits(3);
    assert_eq!(
      ({
        let Ok(apply_avar2_delta) = super::apply_avar2_delta(
          coord,
          crate::tables::variations::FloatItemDelta::FIXTURE_HALF_DELTA,
        );
        let Ok(value) = apply_avar2_delta.to_f2dot14();
        value
      }),
      {
        let Ok(value) = F2Dot14::from_bits(1);
        value
      }
    );
  }

  #[test]
  fn avar2_clamps_hidden_axis_for_amstelvar_repro()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::AMSTELVAR_AVAR2_A)?;
    let avar = match font.avar() {
      Ok(value) => Some(value),
      Err(_unavailable_axis_variations) => None,
    };
    let fvar = font.fvar()?;
    let mut normalized_coords = [F2Dot14::ZERO; 12];

    fvar.user_to_normalized(
      avar.as_ref(),
      [(
        {
          let Ok(value) = Tag::new(b"wght");
          value
        },
        {
          let Ok(value) = Fixed::from_f64(1000.0);
          value
        },
      )],
      &mut normalized_coords,
    );

    assert_eq!(
      *match &normalized_coords {
        [_, element, ..] => element,
      },
      ({
        let Ok(explicit_value) = F2Dot14::from_bits(-5370);
        explicit_value
      })
    );
    assert_eq!(
      *match &normalized_coords {
        [_, _, element, ..] => element,
      },
      F2Dot14::ONE
    );
    assert_eq!(
      *match &normalized_coords {
        [_, _, _, element, ..] => element,
      },
      ({
        let Ok(explicit_value) = F2Dot14::from_bits(2254);
        explicit_value
      })
    );
    assert_eq!(
      *match &normalized_coords {
        [_, _, _, _, _, _, _, _, _, _, _, element, ..] => element,
      },
      F2Dot14::ONE
    );
    ::core::result::Result::Ok(())
  }
}
