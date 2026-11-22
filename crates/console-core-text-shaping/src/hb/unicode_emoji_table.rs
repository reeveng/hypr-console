


#![allow(unused_parens)]
#![allow(clippy::unnecessary_cast, clippy::unreadable_literal, clippy::double_parens)]

use crate::hb::unicode::Codepoint;

#[allow(dead_code, clippy::allow_attributes_without_reason, missing_docs)]
pub(crate) static _hb_emoji_u8: [u8; 624]=
[
    1,  0,  0,  0, 50,  4,  5,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,118,152,
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  2,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    3,  4,  0,  0,  5,  0,  6,  0,  0,  0,  0,  0,  7,  0,  0,  8,
    0,  0,  0,  9,  0,  0, 10, 11, 12, 13, 14, 15, 16, 17, 18,  0,
    0,  0,  0,  0, 19,  0,  0,  0,  0,  0,  0,  0, 20, 21,  0,  0,
   22,  0,  0,  0,  0,  0,  0,  0,  0,  0, 23,  0,  0,  0,  0,  0,
   24,  0, 25, 26,  0, 27, 28, 29, 30, 31,  1,  1, 32,  1, 33, 34,
    1,  1,  1, 35, 36, 37, 38, 39,  1, 40,  1, 41,  0,  0,  0, 42,
   43, 44, 45, 46, 47, 48,  1,  1,  0, 49,  1,  1,  0,  0,  0,  0,
    1,  1,  1,  1,  1,  1,  1,  1,  1,  1,  1,  1,  1,  1,  1,  1,
    0,  0,  0,  0,  0,  0,  0,  0,255,255,255,255,255,255,255,255,
    0,  0,  0,  0,  0, 66,  0,  0,  0,  0,  0,  0,  0,  0,  0, 16,
    0,  2,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  4,  0,  0,  2,
    0,  0,240,  3,  0,  6,  0,  0,  0,  0,  0, 12,  0,  1,  0,  0,
    0,128,  0,  0,  0,254, 15,  7,  4,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0, 12, 64,  0,  1,  0,  0,  0,  0,  0,  0,120,
   31, 64, 50, 33, 77,196,  0,  7,  5,255, 15,128,105,  1,  0,200,
    0,  0,252, 26,131, 12,  3, 96, 48,193, 26,  0,  0,  6,191, 39,
   36,191, 84, 32,  2,  1, 24,  0,144, 80,184,  0, 24,  0,  0,  0,
    0,  0,224,  0,  2,  0,  1,128,  0,  0,  0,  0,  0,  0, 48,  0,
  224,  0,  0, 24,  0,  0,  0,  0,  0,  0, 33,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  1, 32,  0,  0,128,  2,  0,  0,  0,  0,
   16,  0,  0,  0,  0,240,  0,  0,  0,  0,240,255,  0,128,  1,  0,
    1,128,  1,  0,  0,  0,192,255,  0,  0,  0,  0,  0,  0,  3,192,
    0, 64,254,  7,  0,192,255,255,255,255,255,255, 63,  0,  0,  0,
  254,255,  0,  4,  0,128,252,247,  0,254,255,255,192,255,255,255,
  255,255,255,255,243,255,255,255,255,255,207,206,255,255,255,255,
  255,255,255,255,255,255,185,  7,255,255,255,255,255,255,255,191,
  255,255,255,255,255,255,255, 63,  0,126,255,255,255,128,249,  7,
  128, 60, 97,  0, 48,  1,  6, 16, 28,  0, 14,112, 10,129,  8,252,
  255,255,  0,  0,  0,  0,  0,  0, 63,248,231,255, 63,250,249,255,
    0,  0,  0,252,255,255,255,255,  0,240,  0,  0,  0,  0,  0,  0,
    0,255,  0,252,  0,  0,  0,  0,  0,255,  0,  0,  0,192,  0,240,
  252,255,  0,254,255,255,255,255,  0,240,255,255,255,255,255,247,
  191,255,255,255,255,255,255,255,  0,  0,  0,255,  0,192,255,255,
];

#[allow(dead_code, trivial_numeric_casts, clippy::allow_attributes_without_reason, clippy::unseparated_literal_suffix)]
#[inline(always)]
fn _hb_emoji_b4 (a: &[u8], i: usize) -> u8
{
  (a[i>>1]>>((i&1)<<2))&15
}
#[allow(dead_code, trivial_numeric_casts, clippy::allow_attributes_without_reason, clippy::unseparated_literal_suffix)]
#[inline(always)]
fn _hb_emoji_b1 (a: &[u8], i: usize) -> u8
{
  (a[i>>3]>>((i&7)<<0))&1
}
#[allow(dead_code, trivial_numeric_casts, clippy::allow_attributes_without_reason, clippy::unseparated_literal_suffix, missing_docs)]
#[inline]
pub(crate) fn _hb_emoji_is_Extended_Pictographic_u8 (u: usize) -> u8
{

  match u<131070usize { true => { ({ let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>((_hb_emoji_b1(&_hb_emoji_u8[224usize..],({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index((({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index((_hb_emoji_u8[64usize+({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index((({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index((({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index((_hb_emoji_b4(&_hb_emoji_u8,({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index(({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index(((((u)>>6))>>4)); migrated_value })); migrated_value })))); migrated_value })<<4)); migrated_value })+({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index(((((u)>>6))&15)); migrated_value }))); migrated_value })])); migrated_value })<<6)); migrated_value })+({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index(((u)&63)); migrated_value })))); migrated_value }) }, false => { 0 } }
}

#[inline]
pub(crate) fn is_Extended_Pictographic (u: Codepoint) -> bool
{
  _hb_emoji_is_Extended_Pictographic_u8 (({ let Ok(migrated_value) = console_core_number_conversion::wrapped_index(u); migrated_value })) != 0
}




