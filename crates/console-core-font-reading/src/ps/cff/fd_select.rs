//! Parsing for CFF FDSelect tables.

use types::GlyphId;

#[doc(inline)]
pub use super::v1::{
  FdSelect, FdSelectFormat0 as Format0, FdSelectFormat3 as Format3, FdSelectFormat4 as Format4,
  FdSelectRange3 as Range3, FdSelectRange4 as Range4,
};

impl FdSelect<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `FdSelect::font_index` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn font_index(&self, glyph_id: GlyphId) -> Option<u16> {
    match self {
      Self::Format0(fds) => {
        let Ok(value_1) = glyph_id.to_u32();

        let Ok(value_4) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_1);
          migrated_value
        });

        let Ok(fds_fds) = fds.fds();

        fds_fds.get(value_4).map(|fd| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(*fd);
          migrated_value
        })
      }

      Self::Format3(fds) => {
        let Ok(ranges) = fds.ranges();
        let Ok(gid) = glyph_id.to_u32();
        let ::core::result::Result::Ok(ix) = ::console_core_number_conversion::fixed_index(
          match ranges.binary_search_by(|range| {
            let value_2 = &gid;

            ({
              let Ok(first_range) = range.first();
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(first_range);
              migrated_value
            })
            .cmp(value_2)
          }) {
            Ok(ix) => ix,
            Err(ix) => ix.saturating_sub(1),
          },
        );
        Some({
          let checked_value = ranges.get(match ::console_core_number_conversion::index(ix) {
            ::core::result::Result::Ok(native_position) => native_position,
          })?;
          let Ok(ranges_fd) = checked_value.fd();
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ranges_fd);
          migrated_value
        })
      }

      Self::Format4(fds) => {
        let Ok(ranges) = fds.ranges();
        let Ok(gid) = glyph_id.to_u32();
        let ::core::result::Result::Ok(ix) = ::console_core_number_conversion::fixed_index(
          match ranges.binary_search_by(|range| {
            let value_3 = &gid;

            let Ok(first_range) = range.first();

            first_range.cmp(value_3)
          }) {
            Ok(ix) => ix,
            Err(ix) => ix.saturating_sub(1),
          },
        );
        let checked_value_2 = ranges.get(match ::console_core_number_conversion::index(ix) {
          ::core::result::Result::Ok(native_position) => native_position,
        })?;
        let Ok(ranges_fd) = checked_value_2.fd();

        Some(ranges_fd)
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use font_test_data::bebuffer::BeBuffer;

  use super::{FdSelect, GlyphId};
  use crate::FontRead;
  use std::ops::Range;

  #[test]
  fn select_font_index() -> ::core::result::Result<(), crate::test_support::Failure> {
    let map = &[
      (0..10, 0),
      (10..32, 4),
      (32..34, 1),
      (34..128, 12),
      (128..1024, 2),
    ];
    let fixtures = make_fd_selects(map)?;

    for data in fixtures {
      let fd_select = crate::test_support::fixture_value(
        FdSelect::read(<crate::font_data::FontData<'_> as ::core::convert::From<
          &[::core::primitive::u8],
        >>::from(&*data)),
        "fixture value",
      )?;

      for (range, font_index) in map {
        for gid in range.clone() {
          assert_eq!(
            ({
              let fixture_value = crate::test_support::fixture_value(
                fd_select.font_index(GlyphId::from(gid)),
                "fixture value",
              )?;
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u8>(fixture_value);
              migrated_value
            }),
            *font_index
          )
        }
      }
    }

    Ok(())
  }

  #[must_use]
  fn make_fd_selects(
    map: &[(Range<u16>, u8)],
  ) -> Result<[BeBuffer; 3], crate::test_support::Failure> {
    let (last, _) = crate::test_support::fixture_value(map.last(), "last FDSelect range")?;
    let glyph_count = last.end;
    Ok({
      let format0 = {
        let mut buf = font_test_data::bebuffer::BeBuffer::default();
        buf.extend([0u8]);

        let Ok(length) = console_core_number_conversion::index(glyph_count);
        let mut fds = vec![0u8; length.min(65_535)];

        for (range, font_index) in map {
          for gid in range.clone() {
            let slot = crate::test_support::fixture_value(
              fds.get_mut(
                match ::console_core_number_conversion::index::<::core::primitive::u16>(gid) {
                  ::core::result::Result::Ok(converted_position) => converted_position,
                },
              ),
              "FDSelect fixture glyph index",
            )?;
            *slot = *font_index;
          }
        }

        buf.extend(fds);
        buf
      };
      let format3 = {
        let mut buf = font_test_data::bebuffer::BeBuffer::default();
        buf.extend([3u8]);

        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(map.len());
          migrated_value
        })]);
        for (range, font_index) in map {
          buf.extend([range.start]);
          buf.extend([*font_index]);
        }

        buf.extend([glyph_count]);
        buf
      };
      let format4 = {
        let mut buf = font_test_data::bebuffer::BeBuffer::default();
        buf.extend([4u8]);

        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(map.len());
          migrated_value
        })]);
        for (range, font_index) in map {
          buf.extend([({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(range.start);
            migrated_value
          })]);
          buf.extend([({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u16>(*font_index);
            migrated_value
          })]);
        }

        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(glyph_count);
          migrated_value
        })]);
        buf
      };
      [format0, format3, format4]
    })
  }
}
