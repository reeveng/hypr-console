use crate::byte_reader::Reader;
use core::str::FromStr;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct DateTime {
  pub year: u16,

  pub month: u8,

  pub day: u8,

  pub hour: u8,

  pub minute: u8,

  pub second: u8,

  pub utc_offset_hour: i8,

  pub utc_offset_minute: u8,
}

impl DateTime {
  pub(crate) fn from_bytes(
    bytes: &[u8],
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(bytes);

    let Ok(explicit_value) = reader.forward_tag(b"D:");

    match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    let read_num = |reader_2: &mut Reader<'_>, bytes_2: u8, min: u16, max: u16| -> Option<u16> {
      let Ok(peek_byte) = reader_2.peek_byte();

      let checked_value_3 = peek_byte?;

      match checked_value_3 {
        b'-' | b'+' | b'Z' => return None,
        _ => {}
      }

      let Ok(reader_bytes) = reader_2.read_bytes({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(bytes_2);
        numeric_value
      });
      let checked_value = reader_bytes?;
      let checked_value_2 = match core::str::from_utf8(checked_value) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_invalid_utf8) => ::core::option::Option::None,
      }?;
      let num = match u16::from_str(checked_value_2) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None,
      }?;

      match ::core::primitive::bool::then_some(num < min || num > max, ()) {
        ::core::option::Option::Some(()) => {
          return None;
        }
        ::core::option::Option::None => {}
      }

      Some(num)
    };

    let year = match read_num(&mut reader, 4, 0, 9999) {
      Some(value) => value,
      None => return Ok(None),
    };
    let month = match read_num(&mut reader, 2, 1, 12).map(|n| {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(n);
      numeric_value
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 1,
    };
    let day = match read_num(&mut reader, 2, 1, 31).map(|n| {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(n);
      numeric_value
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 1,
    };
    let hour = match read_num(&mut reader, 2, 0, 23).map(|n| {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(n);
      numeric_value
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };
    let minute = match read_num(&mut reader, 2, 0, 59).map(|n| {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(n);
      numeric_value
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };
    let second = match read_num(&mut reader, 2, 0, 59).map(|n| {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(n);
      numeric_value
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };

    let Ok(at_end) = reader.at_end();
    let (utc_offset_hour, utc_offset_minute) =
      match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(byte) = reader.read_byte();
          let multiplier = match match byte {
            Some(value) => value,
            None => return Ok(None),
          } {
            b'-' => -1,
            _ => 1,
          };

          let hour_2 = i8::wrapping_mul(
            multiplier,
            match read_num(&mut reader, 2, 0, 23).map(|n| {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i8>(n);
              numeric_value
            }) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            },
          );
          let Ok(_) = reader.forward_tag(b"\'");
          let minute_2 = match read_num(&mut reader, 2, 0, 59).map(|n| {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(n);
            numeric_value
          }) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
          };

          (hour_2, minute_2)
        }
        ::core::option::Option::None => (0, 0),
      };

    Ok(Some(Self {
      year,
      month,
      day,
      hour,
      minute,
      second,
      utc_offset_hour,
      utc_offset_minute,
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::DateTime;

  fn parse(str: &str) -> Result<Option<DateTime>, console_core_never::Never> {
    DateTime::from_bytes(str.as_bytes())
  }

  #[test]
  fn year_only_defaults() {
    assert_eq!(
      parse("D:2023"),
      Ok(Some(DateTime {
        year: 2023,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn year_month_defaults() {
    assert_eq!(
      parse("D:202312"),
      Ok(Some(DateTime {
        year: 2023,
        month: 12,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn year_month_day_defaults() {
    assert_eq!(
      parse("D:20231225"),
      Ok(Some(DateTime {
        year: 2023,
        month: 12,
        day: 25,
        hour: 0,
        minute: 0,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn ymdh() {
    assert_eq!(
      parse("D:2023122514"),
      Ok(Some(DateTime {
        year: 2023,
        month: 12,
        day: 25,
        hour: 14,
        minute: 0,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn ymdhm() {
    assert_eq!(
      parse("D:202312251430"),
      Ok(Some(DateTime {
        year: 2023,
        month: 12,
        day: 25,
        hour: 14,
        minute: 30,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn full_local_time() {
    assert_eq!(
      parse("D:20231225143015"),
      Ok(Some(DateTime {
        year: 2023,
        month: 12,
        day: 25,
        hour: 14,
        minute: 30,
        second: 15,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn example_from_spec() {
    assert_eq!(
      parse("D:199812231952-08'00"),
      Ok(Some(DateTime {
        year: 1998,
        month: 12,
        day: 23,
        hour: 19,
        minute: 52,
        second: 0,
        utc_offset_hour: -8,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn positive_offset_with_minutes() {
    assert_eq!(
      parse("D:20230701120000+05'30"),
      Ok(Some(DateTime {
        year: 2023,
        month: 7,
        day: 1,
        hour: 12,
        minute: 0,
        second: 0,
        utc_offset_hour: 5,
        utc_offset_minute: 30
      }))
    );
  }

  #[test]
  fn utc_z() {
    assert_eq!(
      parse("D:20230701120000Z"),
      Ok(Some(DateTime {
        year: 2023,
        month: 7,
        day: 1,
        hour: 12,
        minute: 0,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn utc_z_with_zero_offsets() {
    assert_eq!(
      parse("D:20230701120000Z00'00"),
      Ok(Some(DateTime {
        year: 2023,
        month: 7,
        day: 1,
        hour: 12,
        minute: 0,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn negative_offset_with_minutes() {
    assert_eq!(
      parse("D:20230701120000-03'15"),
      Ok(Some(DateTime {
        year: 2023,
        month: 7,
        day: 1,
        hour: 12,
        minute: 0,
        second: 0,
        utc_offset_hour: -3,
        utc_offset_minute: 15
      }))
    );
  }

  #[test]
  fn leap_year() {
    assert_eq!(
      parse("D:20000229010203+01'00"),
      Ok(Some(DateTime {
        year: 2000,
        month: 2,
        day: 29,
        hour: 1,
        minute: 2,
        second: 3,
        utc_offset_hour: 1,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn max_values() {
    assert_eq!(
      parse("D:99991231235959+14'00"),
      Ok(Some(DateTime {
        year: 9999,
        month: 12,
        day: 31,
        hour: 23,
        minute: 59,
        second: 59,
        utc_offset_hour: 14,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn min_values() {
    assert_eq!(
      parse("D:00000101000000+00'00"),
      Ok(Some(DateTime {
        year: 0,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn offset_hour_only() {
    assert_eq!(
      parse("D:202307011200+02"),
      Ok(Some(DateTime {
        year: 2023,
        month: 7,
        day: 1,
        hour: 12,
        minute: 0,
        second: 0,
        utc_offset_hour: 2,
        utc_offset_minute: 0
      }))
    );
  }

  #[test]
  fn offset_negative_zero_hour() {
    assert_eq!(
      parse("D:202307011200-00'45"),
      Ok(Some(DateTime {
        year: 2023,
        month: 7,
        day: 1,
        hour: 12,
        minute: 0,
        second: 0,
        utc_offset_hour: 0,
        utc_offset_minute: 45
      }))
    );
  }
}
