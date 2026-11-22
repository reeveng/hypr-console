//! Test data for the IFT table
//!
//! Used for incremental font transfer. Specification:
//! <https://w3c.github.io/IFT/Overview.html>

use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::iter;

use font_types::{Int24, Tag, Uint24};

use crate::{be_buffer, bebuffer::BeBuffer};

pub static IFT_BASE: &[u8] = include_bytes!("../test_data/ttf/ift_base.ttf");

pub static CFF_FONT: &[u8] = include_bytes!("../test_data/ttf/NotoSansJP-Regular.subset.otf");
pub static CFF2_FONT: &[u8] = include_bytes!("../test_data/ttf/NotoSansJP-VF.subset.otf");

pub const CFF_FONT_CHARSTRINGS_OFFSET: u32 = 0x1b9;
pub const CFF2_FONT_CHARSTRINGS_OFFSET: u32 = 0x8f;

pub const RELATIVE_URL_TEMPLATE: &[u8] = b"\x04foo/\x80";
pub const ABSOLUTE_URL_TEMPLATE: &[u8] = b"\x0a//foo.bar/\x80";

pub fn simple_format1() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {

      {1u8: "format"},
      0u32,
      [1u32, 2, 3, 4],
      2u16,
      {2u16: "max_glyph_map_entry_id"},
      (match Uint24::new(7) { Ok(scalar) => scalar }),
      {0u32: "glyph_map_offset"},
      0u32,
      0b00000010u8,

      6u16,
      4u8,
      {b'f': "url_template[1]"},
      {b'o': "url_template[2]"},
      [b'o', b'/', 128u8],

      {3u8: "patch_format"},


      {1u16: "glyph_map"},
      {2u8: "entry_index[1]"},
      [1u8, 0, 1, 0, 0]
  };

  let glyph_map_position = buffer.offset_for("glyph_map")?;
  let Ok(glyph_map_offset_value) = fitted::<_, u32>(glyph_map_position);
  buffer.write_at("glyph_map_offset", glyph_map_offset_value)?;

  Ok(buffer)
}

pub fn format1_with_dup_urls() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {

      1u8,
      0u32,
      [1u32, 2, 3, 4],
      4u16,
      {4u16: "max_glyph_map_entry_id"},
      (match Uint24::new(7) { Ok(scalar) => scalar }),
      {0u32: "glyph_map_offset"},
      0u32,
      0b00000010u8,

      9u16,
      [8u8, b'f', b'o', b'o', b'/', b'b', b'a', b'a', b'r'],

      {3u8: "patch_format"},


      {1u16: "glyph_map"},
      {2u8: "entry_index[1]"},
      [3u8, 4, 0, 0, 0]
  };

  let glyph_map_position = buffer.offset_for("glyph_map")?;
  let Ok(glyph_map_offset_value) = fitted::<_, u32>(glyph_map_position);
  buffer.write_at("glyph_map_offset", glyph_map_offset_value)?;

  Ok(buffer)
}

enum CharstringsOffsets {
  One,
  Two,
}

pub fn simple_format1_with_one_charstrings_offset()
-> Result<BeBuffer, crate::bebuffer::BufferError> {
  simple_format1_with_charstrings(CharstringsOffsets::One)
}

pub fn simple_format1_with_two_charstrings_offsets()
-> Result<BeBuffer, crate::bebuffer::BufferError> {
  simple_format1_with_charstrings(CharstringsOffsets::Two)
}

fn simple_format1_with_charstrings(
  offsets: CharstringsOffsets,
) -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let (field_flags, written): (u8, &[u32]) = match offsets {
    CharstringsOffsets::One => (0b00000001, &[456]),
    CharstringsOffsets::Two => (0b00000011, &[456, 789]),
  };
  let mut buffer = be_buffer! {

      1u8,
      0u8, 0u8, 0u8,
      field_flags,
      [1u32, 2, 3, 4],
      2u16,
      {2u16: "max_glyph_map_entry_id"},
      (match Uint24::new(7) { Ok(scalar) => scalar }),
      {0u32: "glyph_map_offset"},
      0u32,
      0b00000010u8,

      8u16,
      {b'A': "url_template[0]"},
      {b'B': "url_template[1]"},
      [b'C', b'D', b'E', b'F', 0xc9, 0xa4],

      {3u8: "patch_format"},

      (..written.iter().copied()),


      {1u16: "glyph_map"},
      {2u8: "entry_index[1]"},
      [1u8, 0, 1, 0, 0]
  };

  let glyph_map_position = buffer.offset_for("glyph_map")?;
  let Ok(glyph_map_offset_value) = fitted::<_, u32>(glyph_map_position);
  buffer.write_at("glyph_map_offset", glyph_map_offset_value)?;

  Ok(buffer)
}

pub fn u16_entries_format1() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    1u8,
    0u32,
    [1, 2, 3, 4u32],

    300u16,
    300u16,

    (match Uint24::new(7) { Ok(scalar) => scalar }),

    {0u32: "glyph_map_offset"},
    {0u32: "feature_map_offset"},


    {0u8: "applied_entry_bitmap"},
    [
      0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0,
      0, 0u8
    ],

    6u16,
    [4, b'f', b'o', b'o', b'/', 128u8],

    3u8,


    {2u16: "glyph_map"},


    [80, 81, 300, 300, 80u16]
  };

  let glyph_map_position = buffer.offset_for("glyph_map")?;
  let Ok(glyph_map_offset_value) = fitted::<_, u32>(glyph_map_position);
  buffer.write_at("glyph_map_offset", glyph_map_offset_value)?;

  Ok(buffer)
}

pub fn feature_map_format1() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {

    1u8,

    0u32,

    [1u32, 2u32, 3u32, 4u32],

    400u16,
    300u16,
    (match Uint24::new(7) { Ok(scalar) => scalar }),

    {0u32: "glyph_map_offset"},
    {0u32: "feature_map_offset"},


    {0u8: "applied_entries"},
    [
      0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0u8
    ],
    {0b00001000u8: "applied_entries_296"},
    [
      0, 0,
      0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0u8
    ],

    6u16,
    [4, b'f', b'o', b'o', b'/', 128u8],

    {3u8: "patch_format"},


    {2u16: "glyph_map"},


    [
      80,
      81,
      300u16
    ],
    {299u16: "gid5_entry"},
    {80u16:  "gid6_entry"},


    {3u16: "feature_map"},


    {(match Tag::new(b"dlig") { Ok(scalar) => scalar }): "FeatureRecord[0]"},
    400u16,
    1u16,


    {(match Tag::new(b"liga") { Ok(scalar) => scalar }): "FeatureRecord[1]"},
    384u16,
    2u16,


    [b'n', b'u', b'l', b'l'],
    301u16,
    1u16,


    81u16,
    81u16,


    80u16,
    81u16,


    299u16,
    300u16,


    0u16,
    0u16
  };

  let glyph_map_position = buffer.offset_for("glyph_map")?;
  let Ok(glyph_map_offset_value) = fitted::<_, u32>(glyph_map_position);
  buffer.write_at("glyph_map_offset", glyph_map_offset_value)?;

  let feature_map_position = buffer.offset_for("feature_map")?;
  let Ok(feature_map_offset_value) = fitted::<_, u32>(feature_map_position);
  buffer.write_at("feature_map_offset", feature_map_offset_value)?;

  Ok(buffer)
}

pub fn codepoints_only_format2() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    {2u8: "format"},

    0u32,

    {1u32: "compat_id[0]"},
    {2u32: "compat_id[1]"},
    {3u32: "compat_id[2]"},
    {4u32: "compat_id[3]"},

    3u8,
    {(match Uint24::new(4) { Ok(scalar) => scalar }): "entry_count"},
    {0u32: "entries_offset"},
    0u32,

    6u16,
    [4, b'f', b'o', b'o', b'/', 128u8],



    {0b00010000u8: "entries[0]"},
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b01100000u8: "entries[1]"},
    5u16,
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00100000u8: "entries[2]"},
    5u16,
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00110000u8: "entries[3]"},
    (match Uint24::new(80_000) { Ok(scalar) => scalar }),
    [0b00001101, 0b00000011, 0b00110001u8]
  };

  let entries_0_position = buffer.offset_for("entries[0]")?;
  let Ok(entries_offset_value) = fitted::<_, u32>(entries_0_position);
  buffer.write_at("entries_offset", entries_offset_value)?;

  Ok(buffer)
}

pub fn format2_with_one_charstrings_offset() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  format2_with_charstrings(CharstringsOffsets::One)
}

pub fn format2_with_two_charstrings_offset() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  format2_with_charstrings(CharstringsOffsets::Two)
}

fn format2_with_charstrings(
  offsets: CharstringsOffsets,
) -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let (field_flags, first, rest): (u8, u32, &[u32]) = match offsets {
    CharstringsOffsets::One => (0b00000001, 456, &[]),
    CharstringsOffsets::Two => (0b00000011, 456, &[789]),
  };
  let mut buffer = be_buffer! {
    2u8,

    0u8, 0u8, 0u8,
    {field_flags: "field_flags"},

    {1u32: "compat_id[0]"},
    {2u32: "compat_id[1]"},
    {3u32: "compat_id[2]"},
    {4u32: "compat_id[3]"},

    3u8,
    (match Uint24::new(1) { Ok(scalar) => scalar }),
    {0u32: "entries_offset"},
    0u32,

    8u16,
    [b'A', b'B', b'C', b'D', b'E', b'F', 0xc9, 0xa4],

    {first: "charstrings_offset"},
    (..rest.iter().copied()),



    {0b00010000u8: "entries[0]"},
    [0b00001101, 0b00000011, 0b00110001u8]
  };

  let entries_0_position = buffer.offset_for("entries[0]")?;
  let Ok(entries_offset_value) = fitted::<_, u32>(entries_0_position);
  buffer.write_at("entries_offset", entries_offset_value)?;

  Ok(buffer)
}

pub fn features_and_design_space_format2() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    2u8,

    0u32,

    [1, 2, 3, 4u32],

    {3u8: "patch_format"},
    (match Uint24::new(3) { Ok(scalar) => scalar }),
    {0u32: "entries_offset"},
    0u32,

    6u16,
    [4, b'f', b'o', b'o', b'/', 128u8],



    {0b00010001u8: "entries[0]"},

    2u8,
    (match Tag::new(b"liga") { Ok(scalar) => scalar }),
    (match Tag::new(b"smcp") { Ok(scalar) => scalar }),

    1u16,
    (match Tag::new(b"wdth") { Ok(scalar) => scalar }),
    {0x8000u32: "wdth start"},
    0x10000u32,

    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00010001u8: "entries[1]"},

    1u8,
    (match Tag::new(b"rlig") { Ok(scalar) => scalar }),

    0u16,

    [0b00001101, 0b00000011, 0b00110001u8],


    {0b000100001u8: "entries[2]"},

    1u8,
    (match Tag::new(b"smcp") { Ok(scalar) => scalar }),

    3u16,
    (match Tag::new(b"wght") { Ok(scalar) => scalar }),
    0x00C8_0000u32,
    0x02BC_0000u32,

    (match Tag::new(b"wdth") { Ok(scalar) => scalar }),
    0x0u32,
    0x8000u32,

    (match Tag::new(b"wdth") { Ok(scalar) => scalar }),
    0x0002_0000u32,
    0x0002_8000u32,

    5u16,
    [0b00001101, 0b00000011, 0b00110001u8]
  };

  let entries_0_position = buffer.offset_for("entries[0]")?;
  let Ok(entries_offset_value) = fitted::<_, u32>(entries_0_position);
  buffer.write_at("entries_offset", entries_offset_value)?;

  Ok(buffer)
}

pub fn child_indices_format2() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    2u8,

    0u32,

    [1, 2, 3, 4u32],

    {3u8: "encoding"},
    (match Uint24::new(9) { Ok(scalar) => scalar }),
    {0u32: "entries_offset"},
    0u32,

    6u16,
    [4, b'f', b'o', b'o', b'/', 128u8],




    {0b01100000u8: "entries[0]"},
    5u16,
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00100000u8: "entries[1]"},
    50u16,
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00000001u8: "entries[2]"},

    1u8,
    (match Tag::new(b"rlig") { Ok(scalar) => scalar }),

    1u16,
    (match Tag::new(b"wght") { Ok(scalar) => scalar }),
    0x00C8_0000u32,
    0x02BC_0000u32,


    {0b00000001u8: "entries[3]"},

    1u8,
    (match Tag::new(b"liga") { Ok(scalar) => scalar }),

    1u16,
    (match Tag::new(b"wght") { Ok(scalar) => scalar }),
    0x0032_0000,
    0x0064_0000,


    {0b00000010u8: "entries[4]"},
    1u8,
    (match Uint24::new(0) { Ok(scalar) => scalar }),


    {0b00000010u8: "entries[5]"},
    1u8,
    (match Uint24::new(2) { Ok(scalar) => scalar }),


    {0b00000010u8: "entries[6]"},
    {4u8: "entries[6]_child_count"},
    (match Uint24::new(3) { Ok(scalar) => scalar }),
    {(match Uint24::new(2) { Ok(scalar) => scalar }): "entries[6]_child"},
    (match Uint24::new(1) { Ok(scalar) => scalar }),
    (match Uint24::new(0) { Ok(scalar) => scalar }),


    {0b00000010u8: "entries[7]"},
    2u8,
    (match Uint24::new(4) { Ok(scalar) => scalar }),
    (match Uint24::new(5) { Ok(scalar) => scalar }),


    {0b00100010u8: "entries[8]"},
    1u8,
    (match Uint24::new(0) { Ok(scalar) => scalar }),
    100u16,
    [0b00001101, 0b00000011, 0b00110001u8]
  };

  let entries_0_position = buffer.offset_for("entries[0]")?;
  let Ok(entries_offset_value) = fitted::<_, u32>(entries_0_position);
  buffer.write_at("entries_offset", entries_offset_value)?;

  Ok(buffer)
}

pub fn custom_ids_format2() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    2u8,

    0u32,

    {1u32: "compat_id[0]"},
    {2u32: "compat_id[1]"},
    {3u32: "compat_id[2]"},
    {4u32: "compat_id[3]"},

    3u8,
    {(match Uint24::new(4) { Ok(scalar) => scalar }): "entry_count"},
    {0u32: "entries_offset"},
    0u32,

    6u16,
    [4, b'f', b'o', b'o', b'/', 128u8],



    {0b00010100u8: "entries[0]"},
    {(match Int24::new(-2) { Ok(scalar) => scalar }): "entries[0].id_delta"},
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00100100u8: "entries[1]"},
    {(match Int24::new(10) { Ok(scalar) => scalar }): "entries[1].id_delta"},
    5u16,
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b01000100u8: "entries[2]"},
    {(match Int24::new(14) { Ok(scalar) => scalar }): "id delta - ignored entry"},


    {0b00101000u8: "entries[3]"},
    {3u8: "entry[4] encoding"},
    10u16,
    [0b00001101, 0b00000011, 0b00110001u8]
  };

  let entries_0_position = buffer.offset_for("entries[0]")?;
  let Ok(entries_offset_value) = fitted::<_, u32>(entries_0_position);
  buffer.write_at("entries_offset", entries_offset_value)?;

  Ok(buffer)
}

enum Preloads {
  Without,
  With,
}

pub fn string_ids_format2() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  string_ids_format2_of(Preloads::Without)
}

pub fn string_ids_format2_with_preloads() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  string_ids_format2_of(Preloads::With)
}

fn string_ids_format2_of(preloads: Preloads) -> Result<BeBuffer, crate::bebuffer::BufferError> {
  const CONTINUE_MASK: u32 = 1 << 23;

  let (count, leading) = match preloads {
    Preloads::Without => (
      6,
      be_buffer! {
        0b00000100u8,
        (match Uint24::new(3) { Ok(scalar) => scalar }),


        0b00000100u8,
        (match Uint24::new(4) { Ok(scalar) => scalar })
      },
    ),
    Preloads::With => (
      5,
      be_buffer! {
        0b00000100u8,
        (match Uint24::new(CONTINUE_MASK | 3) { Ok(scalar) => scalar }),
        (match Uint24::new(CONTINUE_MASK) { Ok(scalar) => scalar }),
        (match Uint24::new(4) { Ok(scalar) => scalar })
      },
    ),
  };
  let Ok(entry_count) = Uint24::new(count);
  let mut buffer = be_buffer! {
    2u8,

    0u32,

    [1, 2, 3, 4u32],

    3u8,
    entry_count,
    {0u32: "entries_offset"},
    {0u32: "string_data_offset"},

    6u16,
    [4, b'f', b'o', b'o', b'/', 128u8],




    {0b00000000u8: "entries"},


    (..leading.iter().copied()),


    0b00000000u8,


    0b00000100u8,
    {(match Uint24::new(3) { Ok(scalar) => scalar }): "entry[4] id length"},


    0b00000100u8,
    (match Uint24::new(0) { Ok(scalar) => scalar }),


    {b'a': "string_data"},
    [b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j']
  };

  let string_data_position = buffer.offset_for("string_data")?;
  let Ok(string_data_offset_value) = fitted::<_, u32>(string_data_position);
  buffer.write_at("string_data_offset", string_data_offset_value)?;

  let entries_position = buffer.offset_for("entries")?;
  let Ok(entries_offset_value) = fitted::<_, u32>(entries_position);
  buffer.write_at("entries_offset", entries_offset_value)?;

  Ok(buffer)
}

pub fn table_keyed_format2() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    2u8,

    0u32,

    {1u32: "compat_id[0]"},
    {2u32: "compat_id[1]"},
    {3u32: "compat_id[2]"},
    {4u32: "compat_id[3]"},

    {1u8: "encoding"},
    {(match Uint24::new(1) { Ok(scalar) => scalar }): "entry_count"},
    {0u32: "entries_offset"},
    0u32,

    6u16,
    [4u8, b'f', b'o', b'o', b'/'],
    {128u8: "url_template_var_end"},



    {0b00100100u8: "entries"},
    {(match Int24::new(0) { Ok(scalar) => scalar }): "id_delta"},
    {0u16: "bias"},
    [0b00001101, 0b00000011, 0b00110001u8]
  };

  let entries_position = buffer.offset_for("entries")?;
  let Ok(entries_offset_value) = fitted::<_, u32>(entries_position);
  buffer.write_at("entries_offset", entries_offset_value)?;

  Ok(buffer)
}

pub fn table_keyed_format2_with_preload_urls() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    2u8,

    0u32,

    {1u32: "compat_id[0]"},
    {2u32: "compat_id[1]"},
    {3u32: "compat_id[2]"},
    {4u32: "compat_id[3]"},

    {3u8: "encoding"},
    (match Uint24::new(4) { Ok(scalar) => scalar }),
    {0u32: "entries_offset"},
    0u32,

    6u16,
    [4, b'f', b'o', b'o', b'/', 128u8],




    {0b00010000u8: "entries[0]"},
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00010100u8: "entries[1]"},
    (match Int24::new(15) { Ok(scalar) => scalar }),
    (match Int24::new(1) { Ok(scalar) => scalar }),
    (match Int24::new(-10) { Ok(scalar) => scalar }),
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00010100u8: "entries[2]"},
    (match Int24::new(-11) { Ok(scalar) => scalar }),
    (match Int24::new(0) { Ok(scalar) => scalar }),
    [0b00001101, 0b00000011, 0b00110001u8],


    {0b00010000u8: "entries[3]"},
    [0b00001101, 0b00000011, 0b00110001u8]
  };

  let entries_0_position = buffer.offset_for("entries[0]")?;
  let Ok(entries_offset_value) = fitted::<_, u32>(entries_0_position);
  buffer.write_at("entries_offset", entries_offset_value)?;

  Ok(buffer)
}

pub fn table_keyed_patch() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
      {(match Tag::new(b"iftk") { Ok(scalar) => scalar }): "tag"},
      0u32,
      {1u32: "compat_id"},
      [2, 3, 4u32],
      3u16,


      {0u32: "patch_off[0]"},
      {0u32: "patch_off[1]"},
      {0u32: "patch_off[2]"},
      {0u32: "patch_off[3]"},


      {(match Tag::new(b"tab1") { Ok(scalar) => scalar }): "patch[0]"},
      0u8,
      {29u32: "decompressed_len[0]"},

      [0xa1, 0xe0, 0x00, 0xc0, 0x2f, 0x3a, 0x38, 0xf4, 0x01, 0xd1, 0xaf, 0x54, 0x84, 0x14, 0x71,
       0x2a, 0x80, 0x04, 0xa2, 0x1c, 0xd3, 0xdd, 0x07u8],


      {(match Tag::new(b"tab2") { Ok(scalar) => scalar }): "patch[1]"},
      {1u8: "flags[1]"},
      30u32,

      [0xa1, 0xe8, 0x00, 0xc0, 0xef, 0x48, 0x9d, 0xfa, 0xdc, 0xf1, 0xc2, 0xac, 0xc5, 0xde, 0xe4, 0xf4,
       0xb4, 0x02, 0x48, 0x98, 0x98, 0x52, 0x64, 0xa8, 0x50, 0x20, 0x29, 0x75, 0x0bu8],


      {(match Tag::new(b"tab3") { Ok(scalar) => scalar }): "patch[2]"},
      {2u8: "flags[2]"},
      {0u32: "end"}
  };

  let patch_0_position = buffer.offset_for("patch[0]")?;
  let Ok(patch_off_0_value) = fitted::<_, u32>(patch_0_position);
  buffer.write_at("patch_off[0]", patch_off_0_value)?;

  let patch_1_position = buffer.offset_for("patch[1]")?;
  let Ok(patch_off_1_value) = fitted::<_, u32>(patch_1_position);
  buffer.write_at("patch_off[1]", patch_off_1_value)?;

  let patch_2_position = buffer.offset_for("patch[2]")?;
  let Ok(patch_off_2_value) = fitted::<_, u32>(patch_2_position);
  buffer.write_at("patch_off[2]", patch_off_2_value)?;

  let end_position = buffer.offset_for("end")?;
  let Ok(patch_off_3_value) = fitted::<_, u32>(end_position.saturating_add(4));
  buffer.write_at("patch_off[3]", patch_off_3_value)?;

  Ok(buffer)
}

pub fn noop_table_keyed_patch() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  Ok(be_buffer! {
      {(match Tag::new(b"iftk") { Ok(scalar) => scalar }): "tag"},
      0u32,
      [1, 2, 3, 4u32],
      0u16,


      {0u32: "patch_off[0]"}
  })
}

pub fn glyph_keyed_patch_header() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  Ok(be_buffer! {
    {(match Tag::new(b"ifgk") { Ok(scalar) => scalar }): "format"},
    0u32,
    0u8,
    {6u32: "compatibility_id"},
    [7, 8, 9u32],
    {0u32: "max_uncompressed_length"}
  })
}

pub fn noop_glyf_glyph_patches() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  Ok(be_buffer! {
    0u32,
    {1u8: "table_count"},

    (match Tag::new(b"glyf") { Ok(scalar) => scalar }),


    0u32
  })
}

pub fn glyf_u16_glyph_patches() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    5u32,
    {1u8: "table_count"},


    [2, 7u16],
    {8u16: "gid_8"},
    [9u16],
    {13u16: "gid_13"},

    (match Tag::new(b"glyf") { Ok(scalar) => scalar }),


    {0u32: "gid_2_offset"},
    {0u32: "gid_7_offset"},
    {0u32: "gid_8_offset"},
    {0u32: "gid_9_offset"},
    {0u32: "gid_13_offset"},
    {0u32: "end_offset"},


    {b'a': "gid_2_data"},
    [b'b', b'c'],

    {b'd': "gid_7_data"},
    [b'e', b'f', b'g'],

    {b'h': "gid_8_and_9_data"},
    [b'i', b'j', b'k', b'l'],

    {b'm': "gid_13_data"},
    [b'n']
  };

  let gid_2_data_position = buffer.offset_for("gid_2_data")?;
  let Ok(gid_2_offset_value) = fitted::<_, u32>(gid_2_data_position);
  buffer.write_at("gid_2_offset", gid_2_offset_value)?;

  let gid_7_data_position = buffer.offset_for("gid_7_data")?;
  let Ok(gid_7_offset_value) = fitted::<_, u32>(gid_7_data_position);
  buffer.write_at("gid_7_offset", gid_7_offset_value)?;

  let gid_8_and_9_data_position = buffer.offset_for("gid_8_and_9_data")?;
  let Ok(gid_8_offset_value) = fitted::<_, u32>(gid_8_and_9_data_position);
  buffer.write_at("gid_8_offset", gid_8_offset_value)?;

  let gid_8_and_9_data_position_2 = buffer.offset_for("gid_8_and_9_data")?;
  let Ok(gid_9_offset_value) = fitted::<_, u32>(gid_8_and_9_data_position_2);
  buffer.write_at("gid_9_offset", gid_9_offset_value)?;

  let gid_13_data_position = buffer.offset_for("gid_13_data")?;
  let Ok(gid_13_offset_value) = fitted::<_, u32>(gid_13_data_position);
  buffer.write_at("gid_13_offset", gid_13_offset_value)?;
  buffer.write_at("end_offset", gid_13_offset_value.saturating_add(2))?;

  Ok(buffer)
}

pub fn glyf_u16_glyph_patches_2() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    3u32,
    {1u8: "table_count"},


    7u16,
    12u16,
    14u16,

    (match Tag::new(b"glyf") { Ok(scalar) => scalar }),


    {0u32: "gid_7_offset"},
    {0u32: "gid_12_offset"},
    {0u32: "gid_14_offset"},
    {0u32: "end_offset"},


    {b'q': "gid_7_data"},
    [b'r'],

    {b's': "gid_12_data"},
    [b't', b'u'],

    {b'v': "gid_14_data"}
  };

  let gid_7_data_position = buffer.offset_for("gid_7_data")?;
  let Ok(gid_7_offset_value) = fitted::<_, u32>(gid_7_data_position);
  buffer.write_at("gid_7_offset", gid_7_offset_value)?;

  let gid_12_data_position = buffer.offset_for("gid_12_data")?;
  let Ok(gid_12_offset_value) = fitted::<_, u32>(gid_12_data_position);
  buffer.write_at("gid_12_offset", gid_12_offset_value)?;

  let gid_14_data_position = buffer.offset_for("gid_14_data")?;
  let Ok(gid_14_offset_value) = fitted::<_, u32>(gid_14_data_position);
  buffer.write_at("gid_14_offset", gid_14_offset_value)?;
  buffer.write_at("end_offset", gid_14_offset_value.saturating_add(1))?;

  Ok(buffer)
}

pub fn glyf_u24_glyph_patches() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    5u32,
    1u8,
    (match Uint24::new(2) { Ok(scalar) => scalar }), (match Uint24::new(7) { Ok(scalar) => scalar }), (match Uint24::new(8) { Ok(scalar) => scalar }), (match Uint24::new(9) { Ok(scalar) => scalar }), (match Uint24::new(13) { Ok(scalar) => scalar }),
    (match Tag::new(b"glyf") { Ok(scalar) => scalar }),


    {0u32: "gid_2_offset"},
    {0u32: "gid_7_offset"},
    {0u32: "gid_8_offset"},
    {0u32: "gid_9_offset"},
    {0u32: "gid_13_offset"},
    {0u32: "end_offset"},


    {b'a': "gid_2_data"},
    [b'b', b'c'],

    {b'd': "gid_7_data"},
    [b'e', b'f', b'g'],

    {b'h': "gid_8_and_9_data"},
    [b'i', b'j', b'k', b'l'],

    {b'm': "gid_13_data"},
    [b'n']
  };

  let gid_2_data_position = buffer.offset_for("gid_2_data")?;
  let Ok(gid_2_offset_value) = fitted::<_, u32>(gid_2_data_position);
  buffer.write_at("gid_2_offset", gid_2_offset_value)?;

  let gid_7_data_position = buffer.offset_for("gid_7_data")?;
  let Ok(gid_7_offset_value) = fitted::<_, u32>(gid_7_data_position);
  buffer.write_at("gid_7_offset", gid_7_offset_value)?;

  let gid_8_and_9_data_position = buffer.offset_for("gid_8_and_9_data")?;
  let Ok(gid_8_offset_value) = fitted::<_, u32>(gid_8_and_9_data_position);
  buffer.write_at("gid_8_offset", gid_8_offset_value)?;

  let gid_8_and_9_data_position_2 = buffer.offset_for("gid_8_and_9_data")?;
  let Ok(gid_9_offset_value) = fitted::<_, u32>(gid_8_and_9_data_position_2);
  buffer.write_at("gid_9_offset", gid_9_offset_value)?;

  let gid_13_data_position = buffer.offset_for("gid_13_data")?;
  let Ok(gid_13_offset_value) = fitted::<_, u32>(gid_13_data_position);
  buffer.write_at("gid_13_offset", gid_13_offset_value)?;
  buffer.write_at("end_offset", gid_13_offset_value.saturating_add(2))?;

  Ok(buffer)
}

pub fn glyf_and_gvar_u16_glyph_patches() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    3u32,
    2u8,
    [2, 7, 8u16],
    {(match Tag::new(b"glyf") { Ok(scalar) => scalar }): "glyf_tag"},
    {(match Tag::new(b"gvar") { Ok(scalar) => scalar }): "gvar_tag"},


    {0u32: "glyf_gid_2_offset"},
    {0u32: "glyf_gid_7_offset"},
    {0u32: "glyf_gid_8_offset"},
    {0u32: "gvar_gid_2_offset"},
    {0u32: "gvar_gid_7_offset"},
    {0u32: "gvar_gid_8_offset"},
    {0u32: "end_offset"},


    {b'a': "glyf_gid_2_data"},
    [b'b', b'c'],

    {b'd': "glyf_gid_7_data"},
    [b'e', b'f', b'g'],

    {b'h': "glyf_gid_8_data"},
    [b'i', b'j', b'k', b'l'],

    {b'm': "gvar_gid_2_data"},
    [b'n'],

    {b'o': "gvar_gid_7_data"},
    [b'p', b'q'],

    {b'r': "gvar_gid_8_data"}
  };

  let glyf_gid_2_data_position = buffer.offset_for("glyf_gid_2_data")?;
  let Ok(glyf_gid_2_offset_value) = fitted::<_, u32>(glyf_gid_2_data_position);
  buffer.write_at("glyf_gid_2_offset", glyf_gid_2_offset_value)?;
  let glyf_gid_7_data_position = buffer.offset_for("glyf_gid_7_data")?;
  let Ok(glyf_gid_7_offset_value) = fitted::<_, u32>(glyf_gid_7_data_position);
  buffer.write_at("glyf_gid_7_offset", glyf_gid_7_offset_value)?;
  let glyf_gid_8_data_position = buffer.offset_for("glyf_gid_8_data")?;
  let Ok(glyf_gid_8_offset_value) = fitted::<_, u32>(glyf_gid_8_data_position);
  buffer.write_at("glyf_gid_8_offset", glyf_gid_8_offset_value)?;

  let gvar_gid_2_data_position = buffer.offset_for("gvar_gid_2_data")?;
  let Ok(gvar_gid_2_offset_value) = fitted::<_, u32>(gvar_gid_2_data_position);
  buffer.write_at("gvar_gid_2_offset", gvar_gid_2_offset_value)?;
  let gvar_gid_7_data_position = buffer.offset_for("gvar_gid_7_data")?;
  let Ok(gvar_gid_7_offset_value) = fitted::<_, u32>(gvar_gid_7_data_position);
  buffer.write_at("gvar_gid_7_offset", gvar_gid_7_offset_value)?;
  let gvar_gid_8_data_position = buffer.offset_for("gvar_gid_8_data")?;
  let Ok(gvar_gid_8_offset_value) = fitted::<_, u32>(gvar_gid_8_data_position);
  buffer.write_at("gvar_gid_8_offset", gvar_gid_8_offset_value)?;
  buffer.write_at("end_offset", gvar_gid_8_offset_value.saturating_add(1))?;

  Ok(buffer)
}

pub fn cff_u16_glyph_patches() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {
    4u32,
    {1u8: "table_count"},


    [1,
     38,
     47,
     59u16],

    {(match Tag::new(b"CFF ") { Ok(scalar) => scalar }): "tag"},


    {0u32: "gid_1_offset"},
    {0u32: "gid_38_offset"},
    {0u32: "gid_47_offset"},
    {0u32: "gid_59_offset"},
    {0u32: "end_offset"},


    {b'a': "gid_1_data"},
    [b'b', b'c'],

    {b'd': "gid_38_data"},
    [b'e', b'f', b'g'],

    {b'h': "gid_47_data"},
    [b'i', b'j', b'k', b'l'],

    {b'm': "gid_59_data"},
    [b'n']
  };

  let gid_1_data_position = buffer.offset_for("gid_1_data")?;
  let Ok(gid_1_offset_value) = fitted::<_, u32>(gid_1_data_position);
  buffer.write_at("gid_1_offset", gid_1_offset_value)?;

  let gid_38_data_position = buffer.offset_for("gid_38_data")?;
  let Ok(gid_38_offset_value) = fitted::<_, u32>(gid_38_data_position);
  buffer.write_at("gid_38_offset", gid_38_offset_value)?;

  let gid_47_data_position = buffer.offset_for("gid_47_data")?;
  let Ok(gid_47_offset_value) = fitted::<_, u32>(gid_47_data_position);
  buffer.write_at("gid_47_offset", gid_47_offset_value)?;

  let gid_59_data_position = buffer.offset_for("gid_59_data")?;
  let Ok(gid_59_offset_value) = fitted::<_, u32>(gid_59_data_position);
  buffer.write_at("gid_59_offset", gid_59_offset_value)?;
  buffer.write_at("end_offset", gid_59_offset_value.saturating_add(2))?;

  Ok(buffer)
}

enum SharedTuplesPlace {
  BeforeGlyphData,
  AfterGlyphData,
}

pub fn short_gvar_with_shared_tuples() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  short_gvar_with_shared_tuples_placed(SharedTuplesPlace::BeforeGlyphData)
}

fn short_gvar_with_shared_tuples_placed(
  place: SharedTuplesPlace,
) -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {

    1u16,
    0u16,
    1u16,
    3u16,
    {0u32: "shared_tuples_offset"},
    15u16,
    0u16,
    {0u32: "glyph_variation_data_offset"},


    {0u16: "glyph_offset[0]"},
    {0u16: "glyph_offset[1]"},
    {0u16: "glyph_offset[2]"},
    {0u16: "glyph_offset[3]"},
    {0u16: "glyph_offset[4]"},
    {0u16: "glyph_offset[5]"},
    {0u16: "glyph_offset[6]"},
    {0u16: "glyph_offset[7]"},
    {0u16: "glyph_offset[8]"},
    {0u16: "glyph_offset[9]"},
    {0u16: "glyph_offset[10]"},
    {0u16: "glyph_offset[11]"},
    {0u16: "glyph_offset[12]"},
    {0u16: "glyph_offset[13]"},
    {0u16: "glyph_offset[14]"},
    {0u16: "glyph_offset[15]"}
  };

  let (first, second): (Pushed, Pushed) = match place {
    SharedTuplesPlace::BeforeGlyphData => (push_shared_tuples, push_glyph_data),
    SharedTuplesPlace::AfterGlyphData => (push_glyph_data, push_shared_tuples),
  };
  let Ok(()) = first(&mut buffer);
  let Ok(()) = second(&mut buffer);

  let sharedtuples_0_position = buffer.offset_for("sharedTuples[0]")?;
  let Ok(shared_tuples_offset_value) = fitted::<_, u32>(sharedtuples_0_position);
  buffer.write_at("shared_tuples_offset", shared_tuples_offset_value)?;

  populate_short_gvar_offsets(&mut buffer)?;

  Ok(buffer)
}

type Pushed = fn(&mut BeBuffer) -> Result<(), Never>;

fn push_shared_tuples(buffer: &mut BeBuffer) -> Result<(), Never> {
  let Ok(()) = buffer.push_with_tag(42u16, "sharedTuples[0]");
  buffer.extend([13u16, 25u16]);

  Ok(())
}

fn push_glyph_data(buffer: &mut BeBuffer) -> Result<(), Never> {
  let Ok(()) = buffer.push_with_tag(1u8, "glyph_0");
  buffer.extend([2u8, 3, 4]);
  let Ok(()) = buffer.push_with_tag(5u8, "glyph_8");
  buffer.extend([6u8, 7, 8, 9]);

  buffer.push_with_tag(10u8, "end")
}

pub fn long_gvar_with_shared_tuples() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {

    1u16,
    0u16,
    1u16,
    3u16,
    {0u32: "shared_tuples_offset"},
    15u16,
    0b00000000_00000001u16,
    {0u32: "glyph_variation_data_offset"},


    {0u32: "glyph_offset[0]"},
    {0u32: "glyph_offset[1]"},
    {0u32: "glyph_offset[2]"},
    {0u32: "glyph_offset[3]"},
    {0u32: "glyph_offset[4]"},
    {0u32: "glyph_offset[5]"},
    {0u32: "glyph_offset[6]"},
    {0u32: "glyph_offset[7]"},
    {0u32: "glyph_offset[8]"},
    {0u32: "glyph_offset[9]"},
    {0u32: "glyph_offset[10]"},
    {0u32: "glyph_offset[11]"},
    {0u32: "glyph_offset[12]"},
    {0u32: "glyph_offset[13]"},
    {0u32: "glyph_offset[14]"},
    {0u32: "glyph_offset[15]"},


    {42u16: "sharedTuples[0]"},
    13u16,
    25u16,


    {1u8: "glyph_0"}, [2, 3, 4u8],
    {5u8: "glyph_8"}, [6, 7, 8, 9u8], {10u8: "end"}
  };

  let sharedtuples_0_position = buffer.offset_for("sharedTuples[0]")?;
  let Ok(shared_tuples_offset_value) = fitted::<_, u32>(sharedtuples_0_position);
  buffer.write_at("shared_tuples_offset", shared_tuples_offset_value)?;

  let data_offset = buffer.offset_for("glyph_0")?;
  let Ok(encoded_data_offset) = fitted::<_, u32>(data_offset);
  buffer.write_at("glyph_variation_data_offset", encoded_data_offset)?;

  let glyph_0_position = buffer.offset_for("glyph_0")?;
  let Ok(glyph0_offset) = fitted::<_, u32>(glyph_0_position.saturating_sub(data_offset));
  let glyph_8_position = buffer.offset_for("glyph_8")?;
  let Ok(glyph8_offset) = fitted::<_, u32>(glyph_8_position.saturating_sub(data_offset));
  let end_position = buffer.offset_for("end")?;
  let Ok(end_offset) =
    fitted::<_, u32>(end_position.saturating_add(1).saturating_sub(data_offset));

  buffer.write_at("glyph_offset[0]", glyph0_offset)?;
  buffer.write_at("glyph_offset[1]", glyph8_offset)?;
  buffer.write_at("glyph_offset[2]", glyph8_offset)?;
  buffer.write_at("glyph_offset[3]", glyph8_offset)?;
  buffer.write_at("glyph_offset[4]", glyph8_offset)?;
  buffer.write_at("glyph_offset[5]", glyph8_offset)?;
  buffer.write_at("glyph_offset[6]", glyph8_offset)?;
  buffer.write_at("glyph_offset[7]", glyph8_offset)?;
  buffer.write_at("glyph_offset[8]", glyph8_offset)?;
  buffer.write_at("glyph_offset[9]", end_offset)?;
  buffer.write_at("glyph_offset[10]", end_offset)?;
  buffer.write_at("glyph_offset[11]", end_offset)?;
  buffer.write_at("glyph_offset[12]", end_offset)?;
  buffer.write_at("glyph_offset[13]", end_offset)?;
  buffer.write_at("glyph_offset[14]", end_offset)?;
  buffer.write_at("glyph_offset[15]", end_offset)?;

  Ok(buffer)
}

pub fn short_gvar_with_no_shared_tuples() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {

    1u16,
    0u16,
    1u16,
    {0u16: "shared_tuple_count"},
    {0u32: "shared_tuples_offset"},
    15u16,
    0u16,
    {0u32: "glyph_variation_data_offset"},


    {0u16: "glyph_offset[0]"},
    {0u16: "glyph_offset[1]"},
    {0u16: "glyph_offset[2]"},
    {0u16: "glyph_offset[3]"},
    {0u16: "glyph_offset[4]"},
    {0u16: "glyph_offset[5]"},
    {0u16: "glyph_offset[6]"},
    {0u16: "glyph_offset[7]"},
    {0u16: "glyph_offset[8]"},
    {0u16: "glyph_offset[9]"},
    {0u16: "glyph_offset[10]"},
    {0u16: "glyph_offset[11]"},
    {0u16: "glyph_offset[12]"},
    {0u16: "glyph_offset[13]"},
    {0u16: "glyph_offset[14]"},
    {0u16: "glyph_offset[15]"},


    {1u8: "glyph_0"}, [2, 3, 4u8],
    {5u8: "glyph_8"}, [6, 7, 8, 9u8], {10u8: "end"}
  };

  let data_offset = buffer.offset_for("glyph_0")?;
  let Ok(encoded_data_offset) = fitted::<_, u32>(data_offset);
  buffer.write_at("shared_tuples_offset", encoded_data_offset)?;
  buffer.write_at("glyph_variation_data_offset", encoded_data_offset)?;

  let glyph_0_position = buffer.offset_for("glyph_0")?;
  let Ok(glyph0_offset) =
    fitted::<_, u16>(glyph_0_position.saturating_sub(data_offset).wrapping_div(2));
  let glyph_8_position = buffer.offset_for("glyph_8")?;
  let Ok(glyph8_offset) =
    fitted::<_, u16>(glyph_8_position.saturating_sub(data_offset).wrapping_div(2));
  let end_position = buffer.offset_for("end")?;
  let Ok(end_offset) = fitted::<_, u16>(
    end_position
      .saturating_add(1)
      .saturating_sub(data_offset)
      .wrapping_div(2),
  );

  buffer.write_at("glyph_offset[0]", glyph0_offset)?;
  buffer.write_at("glyph_offset[1]", glyph8_offset)?;
  buffer.write_at("glyph_offset[2]", glyph8_offset)?;
  buffer.write_at("glyph_offset[3]", glyph8_offset)?;
  buffer.write_at("glyph_offset[4]", glyph8_offset)?;
  buffer.write_at("glyph_offset[5]", glyph8_offset)?;
  buffer.write_at("glyph_offset[6]", glyph8_offset)?;
  buffer.write_at("glyph_offset[7]", glyph8_offset)?;
  buffer.write_at("glyph_offset[8]", glyph8_offset)?;
  buffer.write_at("glyph_offset[9]", end_offset)?;
  buffer.write_at("glyph_offset[10]", end_offset)?;
  buffer.write_at("glyph_offset[11]", end_offset)?;
  buffer.write_at("glyph_offset[12]", end_offset)?;
  buffer.write_at("glyph_offset[13]", end_offset)?;
  buffer.write_at("glyph_offset[14]", end_offset)?;
  buffer.write_at("glyph_offset[15]", end_offset)?;

  Ok(buffer)
}

pub fn short_gvar_near_maximum_offset_size() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  let mut buffer = be_buffer! {

    1u16,
    0u16,
    1u16,
    {0u16: "shared_tuple_count"},
    {0u32: "shared_tuples_offset"},
    15u16,
    0u16,
    {0u32: "glyph_variation_data_offset"},


    {0u16: "glyph_offset[0]"},
    {0u16: "glyph_offset[1]"},
    {0u16: "glyph_offset[2]"},
    {0u16: "glyph_offset[3]"},
    {0u16: "glyph_offset[4]"},
    {0u16: "glyph_offset[5]"},
    {0u16: "glyph_offset[6]"},
    {0u16: "glyph_offset[7]"},
    {0u16: "glyph_offset[8]"},
    {0u16: "glyph_offset[9]"},
    {0u16: "glyph_offset[10]"},
    {0u16: "glyph_offset[11]"},
    {0u16: "glyph_offset[12]"},
    {0u16: "glyph_offset[13]"},
    {0u16: "glyph_offset[14]"},
    {0u16: "glyph_offset[15]"},


    {1u8: "glyph_0"}
  };

  buffer.extend(iter::repeat_n(1u8, 131065));

  let data_offset = buffer.offset_for("glyph_0")?;
  let Ok(encoded_data_offset) = fitted::<_, u32>(data_offset);
  buffer.write_at("shared_tuples_offset", encoded_data_offset)?;
  buffer.write_at("glyph_variation_data_offset", encoded_data_offset)?;

  buffer.write_at("glyph_offset[0]", 0u16)?;
  buffer.write_at("glyph_offset[1]", 65533u16)?;
  buffer.write_at("glyph_offset[2]", 65533u16)?;
  buffer.write_at("glyph_offset[3]", 65533u16)?;
  buffer.write_at("glyph_offset[4]", 65533u16)?;
  buffer.write_at("glyph_offset[5]", 65533u16)?;
  buffer.write_at("glyph_offset[6]", 65533u16)?;
  buffer.write_at("glyph_offset[7]", 65533u16)?;
  buffer.write_at("glyph_offset[8]", 65533u16)?;
  buffer.write_at("glyph_offset[9]", 65533u16)?;
  buffer.write_at("glyph_offset[10]", 65533u16)?;
  buffer.write_at("glyph_offset[11]", 65533u16)?;
  buffer.write_at("glyph_offset[12]", 65533u16)?;
  buffer.write_at("glyph_offset[13]", 65533u16)?;
  buffer.write_at("glyph_offset[14]", 65533u16)?;
  buffer.write_at("glyph_offset[15]", 65533u16)?;

  Ok(buffer)
}

pub fn out_of_order_gvar_with_shared_tuples() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  short_gvar_with_shared_tuples_placed(SharedTuplesPlace::AfterGlyphData)
}

fn populate_short_gvar_offsets(
  buffer: &mut BeBuffer,
) -> Result<(), crate::bebuffer::BufferError> {
  let data_offset = buffer.offset_for("glyph_0")?;
  let Ok(encoded_data_offset) = fitted::<_, u32>(data_offset);
  buffer.write_at("glyph_variation_data_offset", encoded_data_offset)?;

  let glyph_0_position = buffer.offset_for("glyph_0")?;
  let Ok(glyph0_offset) =
    fitted::<_, u16>(glyph_0_position.saturating_sub(data_offset).wrapping_div(2));
  let glyph_8_position = buffer.offset_for("glyph_8")?;
  let Ok(glyph8_offset) =
    fitted::<_, u16>(glyph_8_position.saturating_sub(data_offset).wrapping_div(2));
  let end_position = buffer.offset_for("end")?;
  let Ok(end_offset) = fitted::<_, u16>(
    end_position
      .saturating_add(1)
      .saturating_sub(data_offset)
      .wrapping_div(2),
  );

  buffer.write_at("glyph_offset[0]", glyph0_offset)?;
  buffer.write_at("glyph_offset[1]", glyph8_offset)?;
  buffer.write_at("glyph_offset[2]", glyph8_offset)?;
  buffer.write_at("glyph_offset[3]", glyph8_offset)?;
  buffer.write_at("glyph_offset[4]", glyph8_offset)?;
  buffer.write_at("glyph_offset[5]", glyph8_offset)?;
  buffer.write_at("glyph_offset[6]", glyph8_offset)?;
  buffer.write_at("glyph_offset[7]", glyph8_offset)?;
  buffer.write_at("glyph_offset[8]", glyph8_offset)?;
  buffer.write_at("glyph_offset[9]", end_offset)?;
  buffer.write_at("glyph_offset[10]", end_offset)?;
  buffer.write_at("glyph_offset[11]", end_offset)?;
  buffer.write_at("glyph_offset[12]", end_offset)?;
  buffer.write_at("glyph_offset[13]", end_offset)?;
  buffer.write_at("glyph_offset[14]", end_offset)?;
  buffer.write_at("glyph_offset[15]", end_offset)?;

  Ok(())
}
