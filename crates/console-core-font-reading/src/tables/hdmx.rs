//! The [Horizontal Device Metrics](https://learn.microsoft.com/en-us/typography/opentype/spec/hdmx) table.

include!("../../generated/generated_hdmx.rs");

use std::cmp::Ordering;

impl<'a> Hdmx<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Hdmx::record_for_size` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn record_for_size(&self, size: u8) -> Option<DeviceRecord<'a>> {
    let Ok(records) = self.records();

    let mut lo = 0u64;
    let Ok(mut hi) = records.len();

    for _ in 0..u64::BITS {
      match lo.cmp(&hi) {
        Ordering::Less => {}
        Ordering::Equal | Ordering::Greater => break,
      }

      let mid = lo.midpoint(hi);
      let record = match records.get(mid) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;

      match record.pixel_size.cmp(&size) {
        Ordering::Less => lo = ::core::primitive::u64::wrapping_add(mid, 1),
        Ordering::Greater => hi = mid,
        Ordering::Equal => return Some(record),
      }
    }

    None
  }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceRecord<'a> {
  pub pixel_size: u8,
  pub max_width: u8,
  pub widths: &'a [u8],
}

impl<'a> DeviceRecord<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `DeviceRecord::pixel_size` method returns `u8` directly in the existing parser interface"
    )
  )]
  pub fn pixel_size(&self) -> u8 {
    self.pixel_size
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `DeviceRecord::max_width` method returns `u8` directly in the existing parser interface"
    )
  )]
  pub fn max_width(&self) -> u8 {
    self.max_width
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `DeviceRecord::widths` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn widths(&self) -> &'a [u8] {
    self.widths
  }
}

impl ReadArgs for DeviceRecord<'_> {
  type Arguments = (u16, u32);
}

impl ComputeSize for DeviceRecord<'_> {
  fn compute_size(args: &(u16, u32)) -> Result<u64, ReadError> {
    let (_num_glyphs, size_device_record) = *args;

    Ok({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(size_device_record);
      migrated_value
    })
  }
}

impl<'a> FontReadWithArgs<'a> for DeviceRecord<'a> {
  fn read_with_args(data: FontData<'a>, args: &(u16, u32)) -> Result<Self, ReadError> {
    let Ok(mut cursor) = data.cursor();
    let (num_glyphs, _size_device_record) = *args;
    let pixel_size = cursor.read()?;
    let max_width = cursor.read()?;
    let widths = cursor.read_array({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(num_glyphs);
      migrated_value
    })?;

    Ok(Self {
      pixel_size,
      max_width,
      widths,
    })
  }
}

impl<'a> DeviceRecord<'a> {
  pub fn read(
    data: FontData<'a>,
    num_glyphs: u16,
    size_device_record: u32,
  ) -> Result<Self, ReadError> {
    let args = (num_glyphs, size_device_record);
    Self::read_with_args(data, &args)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for DeviceRecord<'a> {
  fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
    Ok(RecordResolver {
      name: "DeviceRecord",
      get_field: Box::new(move |idx, _data| match idx {
        0u64 => {
          let Ok(field) = Field::new("pixel_size", self.pixel_size());

          Some(field)
        }
        1u64 => {
          let Ok(field) = Field::new("max_width", self.max_width());

          Some(field)
        }
        2u64 => {
          let Ok(field) = Field::new("widths", self.widths());

          Some(field)
        }
        _ => None,
      }),
      data,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use font_test_data::{be_buffer, bebuffer::BeBuffer};

  #[test]
  fn read_hdmx() -> ::core::result::Result<(), crate::read::ReadError> {
    let Ok(buf) = make_hdmx();
    let hdmx = Hdmx::read(
      <crate::font_data::FontData<'_> as ::core::convert::From<&[::core::primitive::u8]>>::from(
        &*buf,
      ),
      3,
    )?;
    assert_eq!(hdmx.version(), Ok(0));
    assert_eq!(hdmx.num_records(), Ok(3));

    assert_eq!(hdmx.size_device_record(), Ok(8));
    let Ok(hdmx_records) = hdmx.records();
    let Ok(iter) = hdmx_records.iter();
    let records = iter.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(records.len(), 3);
    let expected_records = [
      DeviceRecord {
        pixel_size: 8,
        max_width: 13,
        widths: &[10, 12, 13],
      },
      DeviceRecord {
        pixel_size: 16,
        max_width: 21,
        widths: &[18, 20, 21],
      },
      DeviceRecord {
        pixel_size: 32,
        max_width: 52,
        widths: &[38, 40, 52],
      },
    ];
    assert_eq!(records, expected_records);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn find_by_size() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(buf) = make_hdmx();
    let hdmx = Hdmx::read(
      <crate::font_data::FontData<'_> as ::core::convert::From<&[::core::primitive::u8]>>::from(
        &*buf,
      ),
      3,
    )?;
    let fixture_value =
      crate::test_support::fixture_value(hdmx.record_for_size(8), "fixture value")?;
    assert_eq!(fixture_value.pixel_size, 8);
    assert_eq!(
      {
        let medium_device_record =
          crate::test_support::fixture_value(hdmx.record_for_size(16), "fixture value")?;
        medium_device_record
      }
      .pixel_size,
      16
    );
    assert_eq!(
      {
        let large_device_record =
          crate::test_support::fixture_value(hdmx.record_for_size(32), "fixture value")?;
        large_device_record
      }
      .pixel_size,
      32
    );
    assert!(hdmx.record_for_size(7).is_none());
    assert!(hdmx.record_for_size(20).is_none());
    assert!(hdmx.record_for_size(72).is_none());
    ::core::result::Result::Ok(())
  }

  #[must_use]
  fn make_hdmx() -> core::result::Result<BeBuffer, console_core_never::Never> {
    Ok(be_buffer! {
        0u16,
        3u16,
        8u32,

        [8u8, 13, 10, 12, 13, 0, 0, 0],
        [16u8, 21, 18, 20, 21, 0, 0, 0],
        [32u8, 52, 38, 40, 52, 0, 0, 0]
    })
  }
}
