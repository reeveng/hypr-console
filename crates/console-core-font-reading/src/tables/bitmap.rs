//! Common bitmap (EBLC/EBDT/CBLC/CBDT) types.

include!("../../generated/generated_bitmap.rs");

impl BitmapSize {
  pub fn location(
    &self,
    offset_data: FontData,
    glyph_id: GlyphId,
  ) -> Result<BitmapLocation, ReadError> {
    let Ok(start_glyph_index) = self.start_glyph_index();
    let Ok(end_glyph_index) = self.end_glyph_index();

    match ::core::primitive::bool::then_some(
      (start_glyph_index..=end_glyph_index).contains(&glyph_id),
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(ReadError::OutOfBounds),
    }

    let subtable_list = self.index_subtable_list(offset_data)?;
    let mut location = BitmapLocation {
      bit_depth: self.bit_depth,
      ..BitmapLocation::default()
    };

    let Ok(index_subtable_records) = subtable_list.index_subtable_records();

    for record in index_subtable_records {
      let Ok(value_1) = subtable_list.offset_data();

      let subtable = record.index_subtable(value_1)?;
      let Ok(first_glyph_index) = record.first_glyph_index();
      let Ok(last_glyph_index) = record.last_glyph_index();

      match ::core::primitive::bool::then_some(
        (first_glyph_index..=last_glyph_index).contains(&glyph_id),
        (),
      ) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => continue,
      }

      let Ok(value_2) = glyph_id.to_u32();
      let Ok(value_3) = first_glyph_index.to_u32();

      let glyph_ix = ::core::primitive::u64::wrapping_sub(
        {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_2);
          migrated_value
        },
        {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_3);
          migrated_value
        },
      );

      match &subtable {
        IndexSubtable::Format1(st) => {
          let Ok(image_format) = st.image_format();
          location.format = image_format;
          let Ok(value_16) = console_core_number_conversion::index(glyph_ix);

          let Ok(sbit_offsets) = st.sbit_offsets();
          let sbit_offset = sbit_offsets.get(value_16).ok_or(ReadError::OutOfBounds)?;
          let Ok(value_4) = sbit_offset.get();

          let start = ::core::primitive::u64::wrapping_add(
            {
              let Ok(image_data_offset) = st.image_data_offset();
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(image_data_offset);
              migrated_value
            },
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_4);
              migrated_value
            },
          );
          let Ok(value_17) = console_core_number_conversion::index(
            ::core::primitive::u64::wrapping_add(glyph_ix, 1),
          );

          let Ok(st_sbit_offsets) = st.sbit_offsets();
          let st_sbit_offset = st_sbit_offsets
            .get(value_17)
            .ok_or(ReadError::OutOfBounds)?;
          let Ok(value_5) = st_sbit_offset.get();

          let end = ::core::primitive::u64::wrapping_add(
            {
              let Ok(image_data_offset) = st.image_data_offset();
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(image_data_offset);
              migrated_value
            },
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_5);
              migrated_value
            },
          );
          location.data_offset = start;

          match ::core::primitive::bool::then_some(end < start, ()) {
            ::core::option::Option::Some(()) => {
              return Err(ReadError::OutOfBounds);
            }
            ::core::option::Option::None => {}
          }

          location.data_size = ::core::primitive::u64::wrapping_sub(end, start);
        }
        IndexSubtable::Format2(st) => {
          let Ok(image_format) = st.image_format();
          location.format = image_format;
          let data_size = {
            let Ok(image_size) = st.image_size();
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u64>(image_size);
            migrated_value
          };
          location.data_size = data_size;
          location.data_offset = ::core::primitive::u64::wrapping_add(
            {
              let Ok(image_data_offset) = st.image_data_offset();
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(image_data_offset);
              migrated_value
            },
            ::core::primitive::u64::wrapping_mul(glyph_ix, data_size),
          );
          let Ok(big_metrics) = st.big_metrics();
          location.metrics = Some(*match <[_]>::get(&*big_metrics, 0) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
            }
          });
        }
        IndexSubtable::Format3(st) => {
          let Ok(image_format) = st.image_format();
          location.format = image_format;
          let Ok(value_22) = console_core_number_conversion::index(glyph_ix);

          let Ok(sbit_offsets) = st.sbit_offsets();
          let sbit_offset = sbit_offsets.get(value_22).ok_or(ReadError::OutOfBounds)?;
          let Ok(value_6) = sbit_offset.get();

          let start = ::core::primitive::u64::wrapping_add(
            {
              let Ok(image_data_offset) = st.image_data_offset();
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(image_data_offset);
              migrated_value
            },
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_6);
              migrated_value
            },
          );
          let Ok(value_23) = console_core_number_conversion::index(
            ::core::primitive::u64::wrapping_add(glyph_ix, 1),
          );

          let Ok(st_sbit_offsets) = st.sbit_offsets();
          let st_sbit_offset = st_sbit_offsets
            .get(value_23)
            .ok_or(ReadError::OutOfBounds)?;
          let Ok(value_7) = st_sbit_offset.get();

          let end = ::core::primitive::u64::wrapping_add(
            {
              let Ok(image_data_offset) = st.image_data_offset();
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(image_data_offset);
              migrated_value
            },
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_7);
              migrated_value
            },
          );
          location.data_offset = start;

          match ::core::primitive::bool::then_some(end < start, ()) {
            ::core::option::Option::Some(()) => {
              return Err(ReadError::OutOfBounds);
            }
            ::core::option::Option::None => {}
          }

          location.data_size = ::core::primitive::u64::wrapping_sub(end, start);
        }
        IndexSubtable::Format4(st) => {
          let Ok(image_format) = st.image_format();
          location.format = image_format;
          let Ok(records) = st.glyph_array();
          let Ok(number) = glyph_id.to_u32();
          let Ok(array_index) = console_core_number_conversion::fixed_index(
            match records.binary_search_by(|record_2| {
              let Ok(glyph) = record_2.glyph_id();
              let Ok(record_number) = glyph.to_u32();
              record_number.cmp(&number)
            }) {
              Ok(index) => index,
              Err(_missing_glyph) => return Err(ReadError::InvalidCollectionIndex(number)),
            },
          );
          let Ok(start_position) = console_core_number_conversion::index(array_index);
          let Ok(end_position) =
            console_core_number_conversion::index(array_index.wrapping_add(1));
          let first = records.get(start_position).ok_or(ReadError::OutOfBounds)?;
          let last = records.get(end_position).ok_or(ReadError::OutOfBounds)?;
          let Ok(start) = first.sbit_offset();
          let Ok(end) = last.sbit_offset();
          let size = end.checked_sub(start).ok_or(ReadError::OutOfBounds)?;
          location.data_offset = u64::from(start);
          location.data_size = u64::from(size);
        }
        IndexSubtable::Format5(st) => {
          let Ok(image_format) = st.image_format();
          location.format = image_format;
          let Ok(array) = st.glyph_array();
          let ::core::result::Result::Ok(array_ix) =
            ::console_core_number_conversion::fixed_index(
              match array.binary_search_by(|gid| {
                let Ok(value_11) = gid.get();

                let Ok(value_18) = value_11.to_u32();

                let Ok(value_15) = glyph_id.to_u32();

                value_18.cmp(&value_15)
              }) {
                Ok(ix) => ix,
                ::core::result::Result::Err(_insertion_position) => {
                  let Ok(value_12) = glyph_id.to_u32();

                  return Err(ReadError::InvalidCollectionIndex(value_12));
                }
              },
            );
          let data_size = {
            let Ok(image_size) = st.image_size();
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u64>(image_size);
            migrated_value
          };
          location.data_size = data_size;
          let Ok(value_25) = console_core_number_conversion::index(data_size);

          let Ok(value_19) = console_core_number_conversion::fitted::<_, u64>(
            (::core::num::Wrapping(match ::console_core_number_conversion::index(array_ix) {
              ::core::result::Result::Ok(native_position) => native_position,
            }) * ::core::num::Wrapping(value_25))
            .0,
          );

          location.data_offset = ::core::primitive::u64::wrapping_add(
            {
              let Ok(image_data_offset) = st.image_data_offset();
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(image_data_offset);
              migrated_value
            },
            value_19,
          );
          let Ok(big_metrics) = st.big_metrics();
          location.metrics = Some(*match <[_]>::get(&*big_metrics, 0) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
            }
          });
        }
      }

      return Ok(location);
    }

    Err(ReadError::OutOfBounds)
  }

  pub fn index_subtable_list<'a>(
    &self,
    offset_data: FontData<'a>,
  ) -> Result<IndexSubtableList<'a>, ReadError> {
    let start = {
      let Ok(index_subtable_list_offset) = self.index_subtable_list_offset();
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(index_subtable_list_offset);
      migrated_value
    };
    let end = start
      .checked_add({
        let Ok(index_subtable_list_size) = self.index_subtable_list_size();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(index_subtable_list_size);
        migrated_value
      })
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_13) = offset_data.slice(start..end);

    let data = value_13.ok_or(ReadError::OutOfBounds)?;
    let Ok(number_of_index_subtables) = self.number_of_index_subtables();

    IndexSubtableList::read(data, number_of_index_subtables)
  }
}

#[derive(Clone, Default)]
pub struct BitmapLocation {
  pub format: u16,
  pub data_offset: u64,
  pub data_size: u64,
  pub bit_depth: u8,
  pub metrics: Option<BigGlyphMetrics>,
}

impl BitmapLocation {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `BitmapLocation::is_empty` method returns `bool` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `BitmapLocation::is_empty` method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  pub fn is_empty(&self) -> bool {
    self.data_size == 0
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BitmapDataFormat {
  BitAligned,
  ByteAligned,
  Png,
}

#[derive(Clone)]
pub enum BitmapMetrics {
  Small(SmallGlyphMetrics),
  Big(BigGlyphMetrics),
}

#[derive(Clone)]
pub struct BitmapData<'a> {
  pub metrics: BitmapMetrics,
  pub content: BitmapContent<'a>,
}

#[derive(Clone)]
pub enum BitmapContent<'a> {
  Data(BitmapDataFormat, &'a [u8]),
  Composite(&'a [BdtComponent]),
}

pub(crate) enum BitmapTable {
  Color,
  Embedded,
}

pub(crate) fn bitmap_data<'a>(
  offset_data: FontData<'a>,
  location: &BitmapLocation,
  table: BitmapTable,
) -> Result<BitmapData<'a>, ReadError> {
  let Ok(value_14) = offset_data.slice(
    location.data_offset
      ..::core::primitive::u64::wrapping_add(location.data_offset, location.data_size),
  );

  let bitmap_data_slice = value_14.ok_or(ReadError::OutOfBounds)?;
  let Ok(mut image_data) = bitmap_data_slice.cursor();

  match (location.format, table) {
    (1, _) => {
      let metrics = read_small_metrics(&mut image_data)?;

      let pitch = ::core::primitive::u64::wrapping_mul(
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(metrics.width);
          migrated_value
        },
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(location.bit_depth);
          migrated_value
        },
      )
      .div_ceil(8);
      let height = {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(metrics.height);
        migrated_value
      };
      let data =
        image_data.read_array::<u8>(::core::primitive::u64::wrapping_mul(pitch, height))?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Small(metrics),
        content: BitmapContent::Data(BitmapDataFormat::ByteAligned, data),
      })
    }

    (2, _) => {
      let metrics = read_small_metrics(&mut image_data)?;
      let width = ::core::primitive::u64::wrapping_mul(
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(metrics.width);
          migrated_value
        },
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(location.bit_depth);
          migrated_value
        },
      );
      let height = {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(metrics.height);
        migrated_value
      };

      let data = image_data
        .read_array::<u8>(::core::primitive::u64::wrapping_mul(width, height).div_ceil(8))?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Small(metrics),
        content: BitmapContent::Data(BitmapDataFormat::BitAligned, data),
      })
    }

    (5, _) => {
      let metrics = location.metrics.ok_or(ReadError::MalformedData(
        "expected metrics from location table",
      ))?;
      let width = ::core::primitive::u64::wrapping_mul(
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(metrics.width);
          migrated_value
        },
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(location.bit_depth);
          migrated_value
        },
      );
      let height = {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(metrics.height);
        migrated_value
      };

      let data = image_data
        .read_array::<u8>(::core::primitive::u64::wrapping_mul(width, height).div_ceil(8))?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Big(metrics),
        content: BitmapContent::Data(BitmapDataFormat::BitAligned, data),
      })
    }

    (6, _) => {
      let metrics = read_big_metrics(&mut image_data)?;

      let pitch = ::core::primitive::u64::wrapping_mul(
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(metrics.width);
          migrated_value
        },
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(location.bit_depth);
          migrated_value
        },
      )
      .div_ceil(8);
      let height = {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(metrics.height);
        migrated_value
      };
      let data =
        image_data.read_array::<u8>(::core::primitive::u64::wrapping_mul(pitch, height))?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Big(metrics),
        content: BitmapContent::Data(BitmapDataFormat::ByteAligned, data),
      })
    }

    (7, _) => {
      let metrics = read_big_metrics(&mut image_data)?;
      let width = ::core::primitive::u64::wrapping_mul(
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(metrics.width);
          migrated_value
        },
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(location.bit_depth);
          migrated_value
        },
      );
      let height = {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(metrics.height);
        migrated_value
      };

      let data = image_data
        .read_array::<u8>(::core::primitive::u64::wrapping_mul(width, height).div_ceil(8))?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Big(metrics),
        content: BitmapContent::Data(BitmapDataFormat::BitAligned, data),
      })
    }

    (8, _) => {
      let metrics = read_small_metrics(&mut image_data)?;
      let _pad = image_data.read::<u8>()?;
      let count = {
        let read = image_data.read::<u16>()?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
        migrated_value
      };
      let components = image_data.read_array::<BdtComponent>(count)?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Small(metrics),
        content: BitmapContent::Composite(components),
      })
    }

    (9, _) => {
      let metrics = read_big_metrics(&mut image_data)?;
      let count = {
        let read = image_data.read::<u16>()?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
        migrated_value
      };
      let components = image_data.read_array::<BdtComponent>(count)?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Big(metrics),
        content: BitmapContent::Composite(components),
      })
    }

    (17, BitmapTable::Color) => {
      let metrics = read_small_metrics(&mut image_data)?;
      let data_len = {
        let read = image_data.read::<u32>()?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
        migrated_value
      };
      let data = image_data.read_array::<u8>(data_len)?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Small(metrics),
        content: BitmapContent::Data(BitmapDataFormat::Png, data),
      })
    }

    (18, BitmapTable::Color) => {
      let metrics = read_big_metrics(&mut image_data)?;
      let data_len = {
        let read = image_data.read::<u32>()?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
        migrated_value
      };
      let data = image_data.read_array::<u8>(data_len)?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Big(metrics),
        content: BitmapContent::Data(BitmapDataFormat::Png, data),
      })
    }

    (19, BitmapTable::Color) => {
      let metrics = location.metrics.ok_or(ReadError::MalformedData(
        "expected metrics from location table",
      ))?;
      let data_len = {
        let read = image_data.read::<u32>()?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
        migrated_value
      };
      let data = image_data.read_array::<u8>(data_len)?;
      Ok(BitmapData {
        metrics: BitmapMetrics::Big(metrics),
        content: BitmapContent::Data(BitmapDataFormat::Png, data),
      })
    }
    (_, _) => Err(ReadError::MalformedData("unexpected bitmap data format")),
  }
}

fn read_small_metrics(cursor: &mut Cursor) -> Result<SmallGlyphMetrics, ReadError> {
  let array = cursor.read_array::<SmallGlyphMetrics>(1)?;

  Ok(*match <[_]>::get(&*array, 0) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => {
      return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
    }
  })
}

fn read_big_metrics(cursor: &mut Cursor) -> Result<BigGlyphMetrics, ReadError> {
  let array = cursor.read_array::<BigGlyphMetrics>(1)?;

  Ok(*match <[_]>::get(&*array, 0) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => {
      return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
    }
  })
}

#[cfg(feature = "experimental_traverse")]
impl SbitLineMetrics {
  #[must_use]
  pub(crate) fn traversal_type<'a>(
    &self,
    data: FontData<'a>,
  ) -> core::result::Result<FieldType<'a>, console_core_never::Never> {
    let Ok(traverse) = self.traverse(data);

    Ok(FieldType::Record(traverse))
  }
}

#[derive(Clone)]
pub enum IndexSubtable<'a> {
  Format1(IndexSubtable1<'a>),
  Format2(IndexSubtable2<'a>),
  Format3(IndexSubtable3<'a>),
  Format4(IndexSubtable4<'a>),
  Format5(IndexSubtable5<'a>),
}

impl<'a> IndexSubtable<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `IndexSubtable::offset_data` method returns `FontData` directly in the existing parser interface"
    )
  )]
  pub fn offset_data(&self) -> FontData<'a> {
    match self {
      Self::Format1(item) => {
        let Ok(value_20) = item.offset_data();

        value_20
      }
      Self::Format2(item) => {
        let Ok(value_21) = item.offset_data();

        value_21
      }
      Self::Format3(item) => {
        let Ok(value_24) = item.offset_data();

        value_24
      }
      Self::Format4(item) => {
        let Ok(value_27) = item.offset_data();

        value_27
      }
      Self::Format5(item) => {
        let Ok(value_28) = item.offset_data();

        value_28
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `IndexSubtable::index_format` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn index_format(&self) -> u16 {
    match self {
      Self::Format1(item) => {
        let Ok(index_format) = item.index_format();

        index_format
      }
      Self::Format2(item) => {
        let Ok(index_format) = item.index_format();

        index_format
      }
      Self::Format3(item) => {
        let Ok(index_format) = item.index_format();

        index_format
      }
      Self::Format4(item) => {
        let Ok(index_format) = item.index_format();

        index_format
      }
      Self::Format5(item) => {
        let Ok(index_format) = item.index_format();

        index_format
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `IndexSubtable::image_format` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn image_format(&self) -> u16 {
    match self {
      Self::Format1(item) => {
        let Ok(image_format) = item.image_format();

        image_format
      }
      Self::Format2(item) => {
        let Ok(image_format) = item.image_format();

        image_format
      }
      Self::Format3(item) => {
        let Ok(image_format) = item.image_format();

        image_format
      }
      Self::Format4(item) => {
        let Ok(image_format) = item.image_format();

        image_format
      }
      Self::Format5(item) => {
        let Ok(image_format) = item.image_format();

        image_format
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `IndexSubtable::image_data_offset` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn image_data_offset(&self) -> u32 {
    match self {
      Self::Format1(item) => {
        let Ok(image_data_offset) = item.image_data_offset();

        image_data_offset
      }
      Self::Format2(item) => {
        let Ok(image_data_offset) = item.image_data_offset();

        image_data_offset
      }
      Self::Format3(item) => {
        let Ok(image_data_offset) = item.image_data_offset();

        image_data_offset
      }
      Self::Format4(item) => {
        let Ok(image_data_offset) = item.image_data_offset();

        image_data_offset
      }
      Self::Format5(item) => {
        let Ok(image_data_offset) = item.image_data_offset();

        image_data_offset
      }
    }
  }
}

impl ReadArgs for IndexSubtable<'_> {
  type Arguments = (GlyphId16, GlyphId16);
}

impl<'a> FontReadWithArgs<'a> for IndexSubtable<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Arguments) -> Result<Self, ReadError> {
    let format: u16 = data.read_at(0u64)?;

    match format {
      IndexSubtable1::FORMAT => FontReadWithArgs::read_with_args(data, args).map(Self::Format1),
      IndexSubtable2::FORMAT => FontRead::read(data).map(Self::Format2),
      IndexSubtable3::FORMAT => FontReadWithArgs::read_with_args(data, args).map(Self::Format3),
      IndexSubtable4::FORMAT => FontRead::read(data).map(Self::Format4),
      IndexSubtable5::FORMAT => FontRead::read(data).map(Self::Format5),
      other => Err(ReadError::InvalidFormat(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other),
      )),
    }
  }
}

impl<'a> MinByteRange<'a> for IndexSubtable<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, console_core_never::Never> {
    match self {
      Self::Format1(item) => item.min_byte_range(),
      Self::Format2(item) => item.min_byte_range(),
      Self::Format3(item) => item.min_byte_range(),
      Self::Format4(item) => item.min_byte_range(),
      Self::Format5(item) => item.min_byte_range(),
    }
  }

  fn min_table_bytes(&self) -> Result<&'a [u8], ReadError> {
    match self {
      Self::Format1(item) => item.min_table_bytes(),
      Self::Format2(item) => item.min_table_bytes(),
      Self::Format3(item) => item.min_table_bytes(),
      Self::Format4(item) => item.min_table_bytes(),
      Self::Format5(item) => item.min_table_bytes(),
    }
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> IndexSubtable<'a> {
  #[must_use]
  fn dyn_inner<'b>(
    &'b self,
  ) -> core::result::Result<&'b dyn SomeTable<'a>, console_core_never::Never> {
    Ok(match self {
      Self::Format1(table) => table,
      Self::Format2(table) => table,
      Self::Format3(table) => table,
      Self::Format4(table) => table,
      Self::Format5(table) => table,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for IndexSubtable<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Ok(dyn_inner) = self.dyn_inner();

    dyn_inner.fmt(f)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for IndexSubtable<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    let Ok(dyn_inner) = self.dyn_inner();
    let Ok(explicit_value) = dyn_inner.type_name();

    Ok(explicit_value)
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    let Ok(dyn_inner) = self.dyn_inner();
    let Ok(explicit_value) = dyn_inner.get_field(idx);

    Ok(explicit_value)
  }
}
