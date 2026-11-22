//! The [tracking (trak)](https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6trak.html) table.

include!("../../generated/generated_trak.rs");

impl<'a> TrackData<'a> {
  pub fn size_table(
    &self,
    offset_data: FontData<'a>,
  ) -> Result<&'a [BigEndian<Fixed>], ReadError> {
    let Ok(value_1) = offset_data.split_off({
      let Ok(size_table_offset) = self.size_table_offset();
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(size_table_offset);
      migrated_value
    });

    let horizontal_track_data = value_1.ok_or(ReadError::OutOfBounds)?;
    let Ok(mut cursor) = horizontal_track_data.cursor();
    cursor.read_array({
      let Ok(n_sizes) = self.n_sizes();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(n_sizes);
      migrated_value
    })
  }
}

impl TrackTableEntry {
  pub fn per_size_values<'a>(
    &self,
    offset_data: FontData<'a>,
    n_sizes: u16,
  ) -> Result<&'a [BigEndian<i16>], ReadError> {
    let Ok(value_2) = offset_data.split_off({
      let Ok(offset) = self.offset();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(offset);
      migrated_value
    });

    let vertical_track_data = value_2.ok_or(ReadError::OutOfBounds)?;
    let Ok(mut cursor) = vertical_track_data.cursor();
    cursor.read_array({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(n_sizes);
      migrated_value
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_header() -> ::core::result::Result<(), crate::test_support::Failure> {
    let ::core::result::Result::Ok(table_data) = example_track_table();
    let trak = Trak::read({
      let Ok(explicit_value) = FontData::new(&table_data);
      explicit_value
    })?;
    assert_eq!(trak.version(), Ok(MajorMinor::VERSION_1_0));
    let _ = crate::test_support::fixture_value(match trak.horiz() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }, "fixture value")?;
    assert!(
      match trak.vert() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
      }
      .is_none()
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_tracks() -> Result<(), crate::test_support::Failure> {
    let Ok(table_data) = example_track_table();
    let Ok(data) = FontData::new(&table_data);
    let trak = Trak::read(data)?;
    let horizontal = trak.horiz()?;
    let horiz = crate::test_support::fixture_value(horizontal, "horizontal track data")?;
    let Ok(track_table) = horiz.track_table();
    let expected = [
      (-1, 256, 52, [-15i16, -7]),
      (0, 258, 60, [0, 0]),
      (1, 257, 56, [50, 20]),
    ];
    assert_eq!(track_table.len(), expected.len());
    let Ok(offset_data) = trak.offset_data();
    let Ok(size_count) = horiz.n_sizes();

    for (track, (adjustment, name, offset, per_size)) in track_table.iter().zip(expected) {
      let Ok(expected_adjustment) = Fixed::from_i32(adjustment);
      let Ok(expected_name) = NameId::new(name);
      assert_eq!(track.track(), Ok(expected_adjustment));
      assert_eq!(track.name_index(), Ok(expected_name));
      assert_eq!(track.offset(), Ok(offset));
      let values = track.per_size_values(offset_data, size_count)?;
      assert!(values.iter().map(|value| {
        let Ok(per_size_delta) = value.get();
        per_size_delta
      }).eq(per_size));
    }

    Ok(())
  }

  #[test]
  fn parse_per_size_values() -> ::core::result::Result<(), crate::test_support::Failure> {
    let ::core::result::Result::Ok(table_data) = example_track_table();
    let trak = crate::test_support::fixture_value(Trak::read({
      let Ok(explicit_value) = FontData::new(&table_data);
      explicit_value
    }), "fixture value")?;
    let fixture_value = crate::test_support::fixture_value(trak.horiz(), "fixture value")?;
    let horiz = crate::test_support::fixture_value(fixture_value, "fixture value")?;
    let Ok(track_table) = horiz.track_table();
    let expected_per_size_tracking_values = [[-15i16, -7], [0, 0], [50, 20]];

    for (track, expected_values) in track_table.iter().zip(expected_per_size_tracking_values) {
      let values = {
        let vertical_sizes = crate::test_support::fixture_value(track
        .per_size_values(
          {
            let Ok(explicit_value) = trak.offset_data();
            explicit_value
          },
          {
            let Ok(explicit_value) = horiz.n_sizes();
            explicit_value
          },
        ), "fixture value")?;
        vertical_sizes
      }
        .iter()
        .map(|v| {
          let Ok(value) = v.get();
          value
        })
        .collect::<Vec<_>>();
      assert_eq!(values, expected_values);
    }
  ;

    Ok(())
  }

  #[test]
  fn parse_sizes() -> ::core::result::Result<(), crate::test_support::Failure> {
    let ::core::result::Result::Ok(table_data) = example_track_table();
    let trak = Trak::read({
      let Ok(explicit_value) = FontData::new(&table_data);
      explicit_value
    })?;
    let horiz = crate::test_support::fixture_value(match trak.horiz() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }, "fixture value")?;
    let size_table = match horiz.size_table({
      let Ok(explicit_value) = trak.offset_data();
      explicit_value
    }) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }
    .iter()
    .map(|v| {
      let Ok(value) = v.get();
      value
    })
    .collect::<Vec<_>>();
    let expected_sizes = [
      {
        let Ok(value) = Fixed::from_i32(12);
        value
      },
      {
        let Ok(value) = Fixed::from_i32(24);
        value
      },
    ];
    assert_eq!(size_table, expected_sizes);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn insufficient_data() -> ::core::result::Result<(), crate::test_support::Failure> {
    let ::core::result::Result::Ok(mut table_data) = example_track_table();

    table_data.pop();
    let trak = crate::test_support::fixture_value(Trak::read({
      let Ok(explicit_value) = FontData::new(&table_data);
      explicit_value
    }), "fixture value")?;
    let fixture_value = crate::test_support::fixture_value(trak.horiz(), "fixture value")?;
    let horiz = crate::test_support::fixture_value(fixture_value, "fixture value")?;
    let Ok(track_table) = horiz.track_table();

    let expected_per_size_tracking_values = [Some([-15i16, -7]), None, Some([50, 20])];

    for (track, expected_values) in track_table.iter().zip(expected_per_size_tracking_values) {
      let values = (match track
        .per_size_values(
          {
            let Ok(explicit_value) = trak.offset_data();
            explicit_value
          },
          {
            let Ok(explicit_value) = horiz.n_sizes();
            explicit_value
          },
        ) { Ok(value) => Some(value), Err(_invalid_track_values) => None })
        .map(|values| {
          values
            .iter()
            .map(|v| {
              let Ok(value) = v.get();
              value
            })
            .collect::<Vec<_>>()
        });
      assert_eq!(
        values,
        expected_values.map(|value| value.into_iter().collect())
      );
    }
  ;

    Ok(())
  }

  #[test]
  fn bad_offset() -> ::core::result::Result<(), crate::test_support::Failure> {
    let ::core::result::Result::Ok(mut table_data) = example_track_table();

    *match <[_]>::get_mut(&mut table_data, 26) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(crate::read::ReadError::OutOfBounds));
      }
    } = 255;
    let trak = Trak::read({
      let Ok(explicit_value) = FontData::new(&table_data);
      explicit_value
    })?;
    let horiz = crate::test_support::fixture_value(match trak.horiz() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }, "fixture value")?;
    let Ok(track_table) = horiz.track_table();
    assert!(matches!(
      (*match <[_]>::get(&*track_table, 0) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None =>
          return ::core::result::Result::Err(crate::test_support::Failure::from(crate::read::ReadError::OutOfBounds)),
      })
      .per_size_values(
        {
          let Ok(explicit_value) = trak.offset_data();
          explicit_value
        },
        {
          let Ok(explicit_value) = horiz.n_sizes();
          explicit_value
        }
      ),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  fn example_track_table() -> ::core::result::Result<Vec<u8>, ::console_core_never::Never> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    buf.extend([MajorMinor::VERSION_1_0]);
    buf.extend([0u16, 12, 0, 0]);

    buf.extend([3u16, 2]);
    buf.extend([44u32]);

    {
      buf.extend([0xFFFF0000u32]);
      buf.extend([256u16, 52]);
    };

    {
      buf.extend([0x00000000u32]);
      buf.extend([258u16, 60]);
    };

    {
      buf.extend([0x00010000u32]);
      buf.extend([257u16, 56]);
    };

    buf.extend([0x000C0000u32]);
    buf.extend([0x00180000u32]);

    buf.extend([-15i16, -7, 50, 20, 0, 0]);
    ::core::result::Result::Ok(buf.to_vec())
  }
}
