use super::ucd_table::ucd::*;
use crate::Script;
use crate::hb::algs::*;

pub type Codepoint = u32;

pub mod hb_unicode_funcs_t {
  pub type space_t = u8;
  pub const NOT_SPACE: u8 = 0;
  pub const SPACE_EM: u8 = 1;
  pub const SPACE_EM_2: u8 = 2;
  pub const SPACE_EM_3: u8 = 3;
  pub const SPACE_EM_4: u8 = 4;
  pub const SPACE_EM_5: u8 = 5;
  pub const SPACE_EM_6: u8 = 6;
  pub const SPACE_EM_16: u8 = 16;
  pub const SPACE_4_EM_18: u8 = 17;
  pub const SPACE: u8 = 18;
  pub const SPACE_FIGURE: u8 = 19;
  pub const SPACE_PUNCTUATION: u8 = 20;
  pub const SPACE_NARROW: u8 = 21;
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct GeneralCategory(pub u8);

#[allow(unused)]
impl GeneralCategory {
  pub const CONTROL: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_CONTROL as _);
  pub const FORMAT: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_FORMAT as _);
  pub const UNASSIGNED: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_UNASSIGNED as _);
  pub const PRIVATE_USE: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_PRIVATE_USE as _);
  pub const SURROGATE: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_SURROGATE as _);
  pub const LOWERCASE_LETTER: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_LOWERCASE_LETTER as _);
  pub const MODIFIER_LETTER: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_MODIFIER_LETTER as _);
  pub const OTHER_LETTER: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_OTHER_LETTER as _);
  pub const TITLECASE_LETTER: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_TITLECASE_LETTER as _);
  pub const UPPERCASE_LETTER: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_UPPERCASE_LETTER as _);
  pub const SPACING_MARK: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_SPACING_MARK as _);
  pub const ENCLOSING_MARK: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_ENCLOSING_MARK as _);
  pub const NON_SPACING_MARK: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_NON_SPACING_MARK as _);
  pub const DECIMAL_NUMBER: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_DECIMAL_NUMBER as _);
  pub const LETTER_NUMBER: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_LETTER_NUMBER as _);
  pub const OTHER_NUMBER: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_OTHER_NUMBER as _);
  pub const CONNECT_PUNCTUATION: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_CONNECT_PUNCTUATION as _);
  pub const DASH_PUNCTUATION: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_DASH_PUNCTUATION as _);
  pub const CLOSE_PUNCTUATION: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_CLOSE_PUNCTUATION as _);
  pub const FINAL_PUNCTUATION: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_FINAL_PUNCTUATION as _);
  pub const INITIAL_PUNCTUATION: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_INITIAL_PUNCTUATION as _);
  pub const OTHER_PUNCTUATION: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_OTHER_PUNCTUATION as _);
  pub const OPEN_PUNCTUATION: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_OPEN_PUNCTUATION as _);
  pub const CURRENCY_SYMBOL: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_CURRENCY_SYMBOL as _);
  pub const MODIFIER_SYMBOL: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_MODIFIER_SYMBOL as _);
  pub const MATH_SYMBOL: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_MATH_SYMBOL as _);
  pub const OTHER_SYMBOL: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_OTHER_SYMBOL as _);
  pub const LINE_SEPARATOR: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_LINE_SEPARATOR as _);
  pub const PARAGRAPH_SEPARATOR: Self =
    Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_PARAGRAPH_SEPARATOR as _);
  pub const SPACE_SEPARATOR: Self = Self(hb_gc::HB_UNICODE_GENERAL_CATEGORY_SPACE_SEPARATOR as _);
}

impl GeneralCategory {
  pub fn to_u8(self) -> u8 {
    self.0
  }

  pub fn is_mark(&self) -> bool {
    matches!(
      *self,
      Self::SPACING_MARK | Self::ENCLOSING_MARK | Self::NON_SPACING_MARK
    )
  }

  pub fn is_letter(&self) -> bool {
    matches!(
      *self,
      Self::LOWERCASE_LETTER
        | Self::MODIFIER_LETTER
        | Self::OTHER_LETTER
        | Self::TITLECASE_LETTER
        | Self::UPPERCASE_LETTER
    )
  }
}

#[allow(dead_code)]
pub mod combining_class {
  pub const NotReordered: u8 = 0;
  pub const Overlay: u8 = 1;
  pub const Nukta: u8 = 7;
  pub const KanaVoicing: u8 = 8;
  pub const Virama: u8 = 9;

  pub const CCC10: u8 = 10;
  pub const CCC11: u8 = 11;
  pub const CCC12: u8 = 12;
  pub const CCC13: u8 = 13;
  pub const CCC14: u8 = 14;
  pub const CCC15: u8 = 15;
  pub const CCC16: u8 = 16;
  pub const CCC17: u8 = 17;
  pub const CCC18: u8 = 18;
  pub const CCC19: u8 = 19;
  pub const CCC20: u8 = 20;
  pub const CCC21: u8 = 21;
  pub const CCC22: u8 = 22;
  pub const CCC23: u8 = 23;
  pub const CCC24: u8 = 24;
  pub const CCC25: u8 = 25;
  pub const CCC26: u8 = 26;

  pub const CCC27: u8 = 27;
  pub const CCC28: u8 = 28;
  pub const CCC29: u8 = 29;
  pub const CCC30: u8 = 30;
  pub const CCC31: u8 = 31;
  pub const CCC32: u8 = 32;
  pub const CCC33: u8 = 33;
  pub const CCC34: u8 = 34;
  pub const CCC35: u8 = 35;

  pub const CCC36: u8 = 36;

  pub const CCC84: u8 = 84;
  pub const CCC91: u8 = 91;

  pub const CCC103: u8 = 103;
  pub const CCC107: u8 = 107;

  pub const CCC118: u8 = 118;
  pub const CCC122: u8 = 122;

  pub const CCC129: u8 = 129;
  pub const CCC130: u8 = 130;
  pub const CCC132: u8 = 132;

  pub const AttachedBelowLeft: u8 = 200;
  pub const AttachedBelow: u8 = 202;
  pub const AttachedAbove: u8 = 214;
  pub const AttachedAboveRight: u8 = 216;
  pub const BelowLeft: u8 = 218;
  pub const Below: u8 = 220;
  pub const BelowRight: u8 = 222;
  pub const Left: u8 = 224;
  pub const Right: u8 = 226;
  pub const AboveLeft: u8 = 228;
  pub const Above: u8 = 230;
  pub const AboveRight: u8 = 232;
  pub const DoubleBelow: u8 = 233;
  pub const DoubleAbove: u8 = 234;

  pub const IotaSubscript: u8 = 240;

  pub const Invalid: u8 = 255;
}

#[allow(dead_code)]
pub mod modified_combining_class {

  pub const CCC10: u8 = 22;
  pub const CCC11: u8 = 15;
  pub const CCC12: u8 = 16;
  pub const CCC13: u8 = 17;
  pub const CCC14: u8 = 23;
  pub const CCC15: u8 = 18;
  pub const CCC16: u8 = 19;
  pub const CCC17: u8 = 20;
  pub const CCC18: u8 = 21;
  pub const CCC19: u8 = 14;
  pub const CCC20: u8 = 24;
  pub const CCC21: u8 = 12;
  pub const CCC22: u8 = 25;
  pub const CCC23: u8 = 13;
  pub const CCC24: u8 = 10;
  pub const CCC25: u8 = 11;
  pub const CCC26: u8 = 26;

  pub const CCC27: u8 = 28;
  pub const CCC28: u8 = 29;
  pub const CCC29: u8 = 30;
  pub const CCC30: u8 = 31;
  pub const CCC31: u8 = 32;
  pub const CCC32: u8 = 33;
  pub const CCC33: u8 = 27;
  pub const CCC34: u8 = 34;
  pub const CCC35: u8 = 35;

  pub const CCC36: u8 = 36;

  pub const CCC84: u8 = 0;
  pub const CCC91: u8 = 0;

  pub const CCC103: u8 = 3;
  pub const CCC107: u8 = 107;

  pub const CCC118: u8 = 118;
  pub const CCC122: u8 = 122;

  pub const CCC129: u8 = 129;
  pub const CCC130: u8 = 132;
  pub const CCC132: u8 = 131;
}

#[rustfmt::skip]
static MODIFIED_COMBINING_CLASS: &[u8; 256] = &[
    combining_class::NotReordered,
    combining_class::Overlay,
    2, 3, 4, 5, 6,
    combining_class::Nukta,
    combining_class::KanaVoicing,
    combining_class::Virama,


    modified_combining_class::CCC10,
    modified_combining_class::CCC11,
    modified_combining_class::CCC12,
    modified_combining_class::CCC13,
    modified_combining_class::CCC14,
    modified_combining_class::CCC15,
    modified_combining_class::CCC16,
    modified_combining_class::CCC17,
    modified_combining_class::CCC18,
    modified_combining_class::CCC19,
    modified_combining_class::CCC20,
    modified_combining_class::CCC21,
    modified_combining_class::CCC22,
    modified_combining_class::CCC23,
    modified_combining_class::CCC24,
    modified_combining_class::CCC25,
    modified_combining_class::CCC26,


    modified_combining_class::CCC27,
    modified_combining_class::CCC28,
    modified_combining_class::CCC29,
    modified_combining_class::CCC30,
    modified_combining_class::CCC31,
    modified_combining_class::CCC32,
    modified_combining_class::CCC33,
    modified_combining_class::CCC34,
    modified_combining_class::CCC35,


    modified_combining_class::CCC36,

    37, 38, 39,
    40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59,
    60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79,
    80, 81, 82, 83,


    modified_combining_class::CCC84,
    85, 86, 87, 88, 89, 90,
    modified_combining_class::CCC91,
    92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102,


    modified_combining_class::CCC103,
    104, 105, 106,
    modified_combining_class::CCC107,
    108, 109, 110, 111, 112, 113, 114, 115, 116, 117,


    modified_combining_class::CCC118,
    119, 120, 121,
    modified_combining_class::CCC122,
    123, 124, 125, 126, 127, 128,


    modified_combining_class::CCC129,
    modified_combining_class::CCC130,
    131,
    modified_combining_class::CCC132,
    133, 134, 135, 136, 137, 138, 139,


    140, 141, 142, 143, 144, 145, 146, 147, 148, 149,
    150, 151, 152, 153, 154, 155, 156, 157, 158, 159,
    160, 161, 162, 163, 164, 165, 166, 167, 168, 169,
    170, 171, 172, 173, 174, 175, 176, 177, 178, 179,
    180, 181, 182, 183, 184, 185, 186, 187, 188, 189,
    190, 191, 192, 193, 194, 195, 196, 197, 198, 199,

    combining_class::AttachedBelowLeft,
    201,
    combining_class::AttachedBelow,
    203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213,
    combining_class::AttachedAbove,
    215,
    combining_class::AttachedAboveRight,
    217,
    combining_class::BelowLeft,
    219,
    combining_class::Below,
    221,
    combining_class::BelowRight,
    223,
    combining_class::Left,
    225,
    combining_class::Right,
    227,
    combining_class::AboveLeft,
    229,
    combining_class::Above,
    231,
    combining_class::AboveRight,
    combining_class::DoubleBelow,
    combining_class::DoubleAbove,
    235, 236, 237, 238, 239,
    combining_class::IotaSubscript,
    241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254,
    combining_class::Invalid,
];

pub trait CharExt {
  fn script(self) -> Script;
  fn general_category(self) -> GeneralCategory;
  fn space_fallback(self) -> hb_unicode_funcs_t::space_t;
  fn combining_class(self) -> u8;
  fn modified_combining_class(self) -> u8;
  fn mirrored(self) -> Option<Codepoint>;
  fn is_emoji_extended_pictographic(self) -> bool;
  fn is_default_ignorable(self) -> bool;
  fn is_variation_selector(self) -> bool;
  fn vertical(self) -> Option<Codepoint>;
}

impl CharExt for Codepoint {
  fn script(self) -> Script {
    _hb_ucd_sc_map[({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(_hb_ucd_sc(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self);
          migrated_value
        }),
      ));
      migrated_value
    })]
  }

  fn general_category(self) -> GeneralCategory {
    GeneralCategory(_hb_ucd_gc(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self);
        migrated_value
      }),
    ))
  }

  fn space_fallback(self) -> hb_unicode_funcs_t::space_t {
    use hb_unicode_funcs_t::*;

    match self {
      0x0020 => SPACE,
      0x00A0 => SPACE,
      0x2000 => SPACE_EM_2,
      0x2001 => SPACE_EM,
      0x2002 => SPACE_EM_2,
      0x2003 => SPACE_EM,
      0x2004 => SPACE_EM_3,
      0x2005 => SPACE_EM_4,
      0x2006 => SPACE_EM_6,
      0x2007 => SPACE_FIGURE,
      0x2008 => SPACE_PUNCTUATION,
      0x2009 => SPACE_EM_5,
      0x200A => SPACE_EM_16,
      0x202F => SPACE_NARROW,
      0x205F => SPACE_4_EM_18,
      0x3000 => SPACE_EM,
      _ => NOT_SPACE,
    }
  }

  fn combining_class(self) -> u8 {
    _hb_ucd_ccc(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self);
        migrated_value
      }),
    )
  }

  fn modified_combining_class(self) -> u8 {
    let u = self;

    match u == 0x1A60 {
      true => {
        return 254;
      }
      false => {}
    }

    match u == 0x0FC6 {
      true => {
        return 254;
      }
      false => {}
    }

    match u == 0x0F39 {
      true => {
        return 127;
      }
      false => {}
    }

    let k = u.combining_class();

    MODIFIED_COMBINING_CLASS[({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(k);
      migrated_value
    })]
  }

  fn mirrored(self) -> Option<Codepoint> {
    let delta = _hb_ucd_bmg(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self);
        migrated_value
      }),
    );
    match delta == 0 {
      true => None,
      false => Some(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
            (({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(self);
              migrated_value
            })
            .wrapping_add(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(delta);
                migrated_value
              }),
            )),
          );
          migrated_value
        }),
      ),
    }
  }

  fn is_emoji_extended_pictographic(self) -> bool {
    super::unicode_emoji_table::is_Extended_Pictographic(self)
  }

  /// Default_Ignorable codepoints:
  ///
  /// Note: While U+115F, U+1160, U+3164 and U+FFA0 are Default_Ignorable,
  /// we do NOT want to hide them, as the way Uniscribe has implemented them
  /// is with regular spacing glyphs, and that's the way fonts are made to work.
  /// As such, we make exceptions for those four.
  /// Also ignoring U+1BCA0..1BCA3. https://github.com/harfbuzz/harfbuzz/issues/503
  ///
  /// Unicode 14.0:
  /// $ grep '; Default_Ignorable_Code_Point ' DerivedCoreProperties.txt | sed 's/;.*#/#/'
  /// 00AD          # Cf       SOFT HYPHEN
  /// 034F          # Mn       COMBINING GRAPHEME JOINER
  /// 061C          # Cf       ARABIC LETTER MARK
  /// 115F..1160    # Lo   [2] HANGUL CHOSEONG FILLER..HANGUL JUNGSEONG FILLER
  /// 17B4..17B5    # Mn   [2] KHMER VOWEL INHERENT AQ..KHMER VOWEL INHERENT AA
  /// 180B..180D    # Mn   [3] MONGOLIAN FREE VARIATION SELECTOR ONE..MONGOLIAN FREE VARIATION SELECTOR THREE
  /// 180E          # Cf       MONGOLIAN VOWEL SEPARATOR
  /// 180F          # Mn       MONGOLIAN FREE VARIATION SELECTOR FOUR
  /// 200B..200F    # Cf   [5] ZERO WIDTH SPACE..RIGHT-TO-LEFT MARK
  /// 202A..202E    # Cf   [5] LEFT-TO-RIGHT EMBEDDING..RIGHT-TO-LEFT OVERRIDE
  /// 2060..2064    # Cf   [5] WORD JOINER..INVISIBLE PLUS
  /// 2065          # Cn       <reserved-2065>
  /// 2066..206F    # Cf  [10] LEFT-TO-RIGHT ISOLATE..NOMINAL DIGIT SHAPES
  /// 3164          # Lo       HANGUL FILLER
  /// FE00..FE0F    # Mn  [16] VARIATION SELECTOR-1..VARIATION SELECTOR-16
  /// FEFF          # Cf       ZERO WIDTH NO-BREAK SPACE
  /// FFA0          # Lo       HALFWIDTH HANGUL FILLER
  /// FFF0..FFF8    # Cn   [9] <reserved-FFF0>..<reserved-FFF8>
  /// 1BCA0..1BCA3  # Cf   [4] SHORTHAND FORMAT LETTER OVERLAP..SHORTHAND FORMAT UP STEP
  /// 1D173..1D17A  # Cf   [8] MUSICAL SYMBOL BEGIN BEAM..MUSICAL SYMBOL END PHRASE
  /// E0000         # Cn       <reserved-E0000>
  /// E0001         # Cf       LANGUAGE TAG
  /// E0002..E001F  # Cn  [30] <reserved-E0002>..<reserved-E001F>
  /// E0020..E007F  # Cf  [96] TAG SPACE..CANCEL TAG
  /// E0080..E00FF  # Cn [128] <reserved-E0080>..<reserved-E00FF>
  /// E0100..E01EF  # Mn [240] VARIATION SELECTOR-17..VARIATION SELECTOR-256
  /// E01F0..E0FFF  # Cn [3600] <reserved-E01F0>..<reserved-E0FFF>
  fn is_default_ignorable(self) -> bool {
    let ch = self;
    let plane = ch >> 16;
    match plane == 0 {
      true => {
        let page = ch >> 8;
        match page {
          0x00 => ch == 0x00AD,
          0x03 => ch == 0x034F,
          0x06 => ch == 0x061C,
          0x17 => (0x17B4..=0x17B5).contains(&ch),
          0x18 => (0x180B..=0x180E).contains(&ch),
          0x20 => {
            (0x200B..=0x200F).contains(&ch)
              || (0x202A..=0x202E).contains(&ch)
              || (0x2060..=0x206F).contains(&ch)
          }
          0xFE => (0xFE00..=0xFE0F).contains(&ch) || ch == 0xFEFF,
          0xFF => (0xFFF0..=0xFFF8).contains(&ch),
          _ => false,
        }
      }
      false => match plane {
        0x01 => (0x1D173..=0x1D17A).contains(&ch),
        0x0E => (0xE0000..=0xE0FFF).contains(&ch),
        _ => false,
      },
    }
  }

  fn is_variation_selector(self) -> bool {
    (0x0FE00..=0x0FE0F).contains(&self) || (0xE0100..=0xE01EF).contains(&self)
  }

  fn vertical(self) -> Option<Codepoint> {
    Some(match self >> 8 {
      0x20 => match self {
        0x2013 => 0xfe32,
        0x2014 => 0xfe31,
        0x2025 => 0xfe30,
        0x2026 => 0xfe19,
        _ => return None,
      },
      0x30 => match self {
        0x3001 => 0xfe11,
        0x3002 => 0xfe12,
        0x3008 => 0xfe3f,
        0x3009 => 0xfe40,
        0x300a => 0xfe3d,
        0x300b => 0xfe3e,
        0x300c => 0xfe41,
        0x300d => 0xfe42,
        0x300e => 0xfe43,
        0x300f => 0xfe44,
        0x3010 => 0xfe3b,
        0x3011 => 0xfe3c,
        0x3014 => 0xfe39,
        0x3015 => 0xfe3a,
        0x3016 => 0xfe17,
        0x3017 => 0xfe18,
        _ => return None,
      },
      0xfe => match self {
        0xfe4f => 0xfe34,
        _ => return None,
      },
      0xff => match self {
        0xff01 => 0xfe15,
        0xff08 => 0xfe35,
        0xff09 => 0xfe36,
        0xff0c => 0xfe10,
        0xff1a => 0xfe13,
        0xff1b => 0xfe14,
        0xff1f => 0xfe16,
        0xff3b => 0xfe47,
        0xff3d => 0xfe48,
        0xff3f => 0xfe33,
        0xff5b => 0xfe37,
        0xff5d => 0xfe38,
        _ => return None,
      },
      _ => return None,
    })
  }
}

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

pub fn compose(a: Codepoint, b: Codepoint) -> Option<Codepoint> {
  match compose_hangul(a, b) {
    Some(ab) => {
      return Some(ab);
    }
    None => {}
  }

  let u: u32;

  match (a & 0xFFFF_F800) == 0x0000 && (b & 0xFFFF_FF80) == 0x0300 {
    true => {
      let k = HB_CODEPOINT_ENCODE3_11_7_14(a, b, 0);
      let v = _hb_ucd_dm2_u32_map
        .binary_search_by(|probe| {
          let key = probe & HB_CODEPOINT_ENCODE3_11_7_14(0x001F_FFFF, 0x001F_FFFF, 0);
          key.cmp(&k)
        })
        .ok()
        .map(|index| _hb_ucd_dm2_u32_map[index]);

      match v {
        Some(value) => {
          u = HB_CODEPOINT_DECODE3_11_7_14_3(value);
        }
        None => {
          return None;
        }
      }
    }
    false => {
      let k = HB_CODEPOINT_ENCODE3(a, b, 0);
      let v = _hb_ucd_dm2_u64_map
        .binary_search_by(|probe| {
          let key = probe & HB_CODEPOINT_ENCODE3(0x001F_FFFF, 0x001F_FFFF, 0);
          key.cmp(&k)
        })
        .ok()
        .map(|index| _hb_ucd_dm2_u64_map[index]);

      match v {
        Some(value) => {
          u = HB_CODEPOINT_DECODE3_3(value);
        }
        None => {
          return None;
        }
      }
    }
  }

  match u == 0 {
    true => None,
    false => Some(u),
  }
}

fn compose_hangul(a: Codepoint, b: Codepoint) -> Option<Codepoint> {
  let l = a;
  let v = b;
  match L_BASE <= l && l < (L_BASE + L_COUNT) && V_BASE <= v && v < (V_BASE + V_COUNT) {
    true => {
      let r = S_BASE + (l - L_BASE) * N_COUNT + (v - V_BASE) * T_COUNT;
      Some(r)
    }
    false => match S_BASE <= l
      && l <= (S_BASE + S_COUNT - T_COUNT)
      && T_BASE <= v
      && v < (T_BASE + T_COUNT)
      && (l - S_BASE) % T_COUNT == 0
    {
      true => {
        let r = l + (v - T_BASE);
        Some(r)
      }
      false => None,
    },
  }
}

pub fn decompose(ab: Codepoint) -> Option<(Codepoint, Codepoint)> {
  if let Some((a, b)) = decompose_hangul(ab) {
    return Some((a, b));
  }

  let mut i = ({
    let Ok(migrated_value) = console_core_number_conversion::wrapped_index(_hb_ucd_dm(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ab);
        migrated_value
      }),
    ));
    migrated_value
  });

  match i == 0 {
    true => {
      return None;
    }
    false => {}
  }
  i -= 1;

  match i < _hb_ucd_dm1_p0_map.len() + _hb_ucd_dm1_p2_map.len() {
    true => {
      let a = match i < _hb_ucd_dm1_p0_map.len() {
        true => {
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(_hb_ucd_dm1_p0_map[i]);
            migrated_value
          })
        }
        false => {
          let j = i - _hb_ucd_dm1_p0_map.len();
          0x20000
            | ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(_hb_ucd_dm1_p2_map[j]);
              migrated_value
            })
        }
      };
      return Some((a, 0));
    }
    false => {}
  }

  i -= _hb_ucd_dm1_p0_map.len() + _hb_ucd_dm1_p2_map.len();

  match i < _hb_ucd_dm2_u32_map.len() {
    true => {
      let v = _hb_ucd_dm2_u32_map[i];
      let a = HB_CODEPOINT_DECODE3_11_7_14_1(v);
      let b = HB_CODEPOINT_DECODE3_11_7_14_2(v);
      return Some((a, b));
    }
    false => {}
  }

  i -= _hb_ucd_dm2_u32_map.len();

  let v = _hb_ucd_dm2_u64_map[i];
  let a = HB_CODEPOINT_DECODE3_1(v);
  let b = HB_CODEPOINT_DECODE3_2(v);
  Some((a, b))
}

pub fn decompose_hangul(ab: Codepoint) -> Option<(Codepoint, Codepoint)> {
  let si = ab.wrapping_sub(S_BASE);
  match si >= S_COUNT {
    true => {
      return None;
    }
    false => {}
  }

  let (a, b) = match si % T_COUNT != 0 {
    true => (S_BASE + (si / T_COUNT) * T_COUNT, T_BASE + (si % T_COUNT)),
    false => (L_BASE + (si / N_COUNT), V_BASE + (si % N_COUNT) / T_COUNT),
  };
  Some((a, b))
}

pub mod hb_gc {
  pub const HB_UNICODE_GENERAL_CATEGORY_CONTROL: u32 = 0;
  pub const HB_UNICODE_GENERAL_CATEGORY_FORMAT: u32 = 1;
  pub const HB_UNICODE_GENERAL_CATEGORY_UNASSIGNED: u32 = 2;
  pub const HB_UNICODE_GENERAL_CATEGORY_PRIVATE_USE: u32 = 3;
  pub const HB_UNICODE_GENERAL_CATEGORY_SURROGATE: u32 = 4;
  pub const HB_UNICODE_GENERAL_CATEGORY_LOWERCASE_LETTER: u32 = 5;
  pub const HB_UNICODE_GENERAL_CATEGORY_MODIFIER_LETTER: u32 = 6;
  pub const HB_UNICODE_GENERAL_CATEGORY_OTHER_LETTER: u32 = 7;
  pub const HB_UNICODE_GENERAL_CATEGORY_TITLECASE_LETTER: u32 = 8;
  pub const HB_UNICODE_GENERAL_CATEGORY_UPPERCASE_LETTER: u32 = 9;
  pub const HB_UNICODE_GENERAL_CATEGORY_SPACING_MARK: u32 = 10;
  pub const HB_UNICODE_GENERAL_CATEGORY_ENCLOSING_MARK: u32 = 11;
  pub const HB_UNICODE_GENERAL_CATEGORY_NON_SPACING_MARK: u32 = 12;
  pub const HB_UNICODE_GENERAL_CATEGORY_DECIMAL_NUMBER: u32 = 13;
  pub const HB_UNICODE_GENERAL_CATEGORY_LETTER_NUMBER: u32 = 14;
  pub const HB_UNICODE_GENERAL_CATEGORY_OTHER_NUMBER: u32 = 15;
  pub const HB_UNICODE_GENERAL_CATEGORY_CONNECT_PUNCTUATION: u32 = 16;
  pub const HB_UNICODE_GENERAL_CATEGORY_DASH_PUNCTUATION: u32 = 17;
  pub const HB_UNICODE_GENERAL_CATEGORY_CLOSE_PUNCTUATION: u32 = 18;
  pub const HB_UNICODE_GENERAL_CATEGORY_FINAL_PUNCTUATION: u32 = 19;
  pub const HB_UNICODE_GENERAL_CATEGORY_INITIAL_PUNCTUATION: u32 = 20;
  pub const HB_UNICODE_GENERAL_CATEGORY_OTHER_PUNCTUATION: u32 = 21;
  pub const HB_UNICODE_GENERAL_CATEGORY_OPEN_PUNCTUATION: u32 = 22;
  pub const HB_UNICODE_GENERAL_CATEGORY_CURRENCY_SYMBOL: u32 = 23;
  pub const HB_UNICODE_GENERAL_CATEGORY_MODIFIER_SYMBOL: u32 = 24;
  pub const HB_UNICODE_GENERAL_CATEGORY_MATH_SYMBOL: u32 = 25;
  pub const HB_UNICODE_GENERAL_CATEGORY_OTHER_SYMBOL: u32 = 26;
  pub const HB_UNICODE_GENERAL_CATEGORY_LINE_SEPARATOR: u32 = 27;
  pub const HB_UNICODE_GENERAL_CATEGORY_PARAGRAPH_SEPARATOR: u32 = 28;
  pub const HB_UNICODE_GENERAL_CATEGORY_SPACE_SEPARATOR: u32 = 29;
}
