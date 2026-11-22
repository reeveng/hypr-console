use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Post<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(max_mem_type1_byte_range) = self.max_mem_type1_byte_range();

        Ok(0..max_mem_type1_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Post<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"post"); value });
}

impl<'a> FontRead<'a> for Post<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Post<'a> {
    data: FontData<'a>,
}

impl<'a> Post<'a> {
  pub const MIN_SIZE: u64 = (Version16Dot16::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<Version16Dot16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn italic_angle(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.italic_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn underline_position(&self) -> Result<FWord, Never> {
        let Ok(range) = self.underline_position_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn underline_thickness(&self) -> Result<FWord, Never> {
        let Ok(range) = self.underline_thickness_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn is_fixed_pitch(&self) -> Result<u32, Never> {
        let Ok(range) = self.is_fixed_pitch_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn min_mem_type42(&self) -> Result<u32, Never> {
        let Ok(range) = self.min_mem_type42_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn max_mem_type42(&self) -> Result<u32, Never> {
        let Ok(range) = self.max_mem_type42_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn min_mem_type1(&self) -> Result<u32, Never> {
        let Ok(range) = self.min_mem_type1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn max_mem_type1(&self) -> Result<u32, Never> {
        let Ok(range) = self.max_mem_type1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_glyphs(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.num_glyphs_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn glyph_name_index(&self) -> Result<Option<&'a [BigEndian<u16>]>, Never> {
        let Ok(range) = self.glyph_name_index_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn string_data(&self) -> Result<Option<VarLenArray<'a, PString<'a>>>, Never> {
        let Ok(range) = self.string_data_byte_range();
        let value_7 = (!range.is_empty())
            .then(|| {
                let Ok(value_9) = self.data
                    .split_off(range.start);

                value_9
                    .and_then(|d| match VarLenArray::read(d) { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None })
            });

        Ok(value_7
            .flatten())
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Version16Dot16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn italic_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn underline_position_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.italic_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn underline_thickness_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.underline_position_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn is_fixed_pitch_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.underline_thickness_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn min_mem_type42_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.is_fixed_pitch_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn max_mem_type42_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.min_mem_type42_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn min_mem_type1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.max_mem_type42_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn max_mem_type1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.min_mem_type1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_glyphs_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.max_mem_type1_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((2u16, 0u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn glyph_name_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(own_num_glyphs) = self.num_glyphs();
        let num_glyphs = match own_num_glyphs {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 0,
        };
        let Ok(previous) = self.num_glyphs_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((2u16, 0u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                let Ok(value_4) = transforms::to_u64(num_glyphs);

            start.saturating_add(value_4.saturating_mul(u16::RAW_BYTE_LEN))
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn string_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_name_index_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((2u16, 0u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                let Ok(value_8) = self.data.len();

            start.saturating_add(value_8.saturating_sub(start))
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Post::MIN_SIZE); held }));

impl Default for Post<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Post<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Post")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "italic_angle", value: FieldType::from(self.italic_angle()) }),
            2u64 => Some(Field { name: "underline_position", value: FieldType::from(self.underline_position()) }),
            3u64 => Some(Field { name: "underline_thickness", value: FieldType::from(self.underline_thickness()) }),
            4u64 => Some(Field { name: "is_fixed_pitch", value: FieldType::from(self.is_fixed_pitch()) }),
            5u64 => Some(Field { name: "min_mem_type42", value: FieldType::from(self.min_mem_type42()) }),
            6u64 => Some(Field { name: "max_mem_type42", value: FieldType::from(self.max_mem_type42()) }),
            7u64 => Some(Field { name: "min_mem_type1", value: FieldType::from(self.min_mem_type1()) }),
            8u64 => Some(Field { name: "max_mem_type1", value: FieldType::from(self.max_mem_type1()) }),
            9u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((2u16, 0u16));
                let Ok(num_glyphs) = self.num_glyphs();

                match compatibility {
                    Compatibility::Compatible => num_glyphs.map(|num_glyphs_2| Field { name: "num_glyphs", value: FieldType::from(num_glyphs_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            10u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((2u16, 0u16));
                let Ok(glyph_name_index) = self.glyph_name_index();

                match compatibility {
                    Compatibility::Compatible => glyph_name_index.map(|glyph_name_index_2| Field { name: "glyph_name_index", value: FieldType::from(glyph_name_index_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            11u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((2u16, 0u16));

                match compatibility {
                    Compatibility::Compatible => {
                    let Ok(value) = self.traverse_string_data();

                    Some(Field { name: "string_data", value })
                },
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Post<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
