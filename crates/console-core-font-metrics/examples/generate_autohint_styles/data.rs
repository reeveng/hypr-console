//! Script coverage and blue zones from FreeType autofit.
//!
// In relation to FreeType, this combines the AF_ScriptClass,
// AF_Script_UniRangeRec and AF_BlueStringset.
// Script definitions: https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afscript.h
// Unicode ranges: https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afranges.c
// Blues: https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afblue.h

use super::{Feature, Group, Script};

pub(super) const FEATURES: &[Feature] = &[
  Feature {
    tag: "c2cp",
    name: "petite capitals from capitals",
  },
  Feature {
    tag: "c2sc",
    name: "small capitals from capitals",
  },
  Feature {
    tag: "ordn",
    name: "ordinals",
  },
  Feature {
    tag: "pcap",
    name: "petite capitals",
  },
  Feature {
    tag: "ruby",
    name: "ruby",
  },
  Feature {
    tag: "sinf",
    name: "scientific inferiors",
  },
  Feature {
    tag: "smcp",
    name: "small capitals",
  },
  Feature {
    tag: "subs",
    name: "subscript",
  },
  Feature {
    tag: "sups",
    name: "superscript",
  },
  Feature {
    tag: "titl",
    name: "titling",
  },
];

pub(super) const SCRIPTS: &[Script] = &[
  Script {
    tag: "ADLM",
    name: "Adlam",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𞤌 𞤮",
    base: &[
      (0x1E900, 0x1E95F), // Adlam
    ],
    non_base: &[(0x1D944, 0x1E94A)],
    blues: &[
      ("𞤌 𞤅 𞤈 𞤏 𞤔 𞤚", &["TOP"]),
      ("𞤂 𞤖", &["NONE"]),
      ("𞤬 𞤮 𞤻 𞤼 𞤾", &["TOP", "X_HEIGHT"]),
      ("𞤤 𞤨 𞤩 𞤭 𞤴 𞤸 𞤺 𞥀", &["NONE"]),
    ],
  },
  Script {
    tag: "ARAB",
    name: "Arabic",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ل ح ـ",
    base: &[
      (0x0600, 0x06FF),   // Arabic
      (0x0750, 0x07FF),   // Arabic Supplement
      (0x08A0, 0x08FF),   // Arabic Extended-A
      (0xFB50, 0xFDFF),   // Arabic Presentation Forms-A
      (0xFE70, 0xFEFF),   // Arabic Presentation Forms-B
      (0x1EE00, 0x1EEFF), // Arabic Mathematical Alphabetic Symbols
    ],
    non_base: &[
      (0x0600, 0x0605),
      (0x0610, 0x061A),
      (0x064B, 0x065F),
      (0x0670, 0x0670),
      (0x06D6, 0x06DC),
      (0x06DF, 0x06E4),
      (0x06E7, 0x06E8),
      (0x06EA, 0x06ED),
      (0x08D4, 0x08E1),
      (0x08D3, 0x08FF),
      (0xFBB2, 0xFBC1),
      (0xFE70, 0xFE70),
      (0xFE72, 0xFE72),
      (0xFE74, 0xFE74),
      (0xFE76, 0xFE76),
      (0xFE78, 0xFE78),
      (0xFE7A, 0xFE7A),
      (0xFE7C, 0xFE7C),
      (0xFE7E, 0xFE7E),
    ],
    blues: &[
      ("ا إ ل ك ط ظ", &["TOP"]),
      ("ت ث ط ظ ك", &["NONE"]),
      ("ـ", &["NEUTRAL"]),
    ],
  },
  Script {
    tag: "ARMN",
    name: "Armenian",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ս Ս",
    base: &[
      (0x0530, 0x058F), // Armenian
      (0xFB13, 0xFB17), // Alphab. Present. Forms (Armenian)
    ],
    non_base: &[(0x0559, 0x055F)],
    blues: &[
      ("Ա Մ Ւ Ս Բ Գ Դ Օ", &["TOP"]),
      ("Ւ Ո Դ Ճ Շ Ս Տ Օ", &["NONE"]),
      ("ե է ի մ վ ֆ ճ", &["TOP"]),
      ("ա յ ւ ս գ շ ր օ", &["TOP", "X_HEIGHT"]),
      ("հ ո ճ ա ե ծ ս օ", &["NONE"]),
      ("բ ը ի լ ղ պ փ ց", &["NONE"]),
    ],
  },
  Script {
    tag: "AVST",
    name: "Avestan",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐬚",
    base: &[
      (0x10B00, 0x10B3F), // Avestan
    ],
    non_base: &[(0x10B39, 0x10B3F)],
    blues: &[("𐬀 𐬁 𐬐 𐬛", &["TOP"]), ("𐬀 𐬁", &["NONE"])],
  },
  Script {
    tag: "BAMU",
    name: "Bamum",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ꛁ ꛯ",
    base: &[
      (0xA6A0, 0xA6FF), // Bamum
    ],
    non_base: &[(0xA6F0, 0xA6F1)],
    blues: &[
      ("ꚧ ꚨ ꛛ ꛉ ꛁ ꛈ ꛫ ꛯ", &["TOP"]),
      ("ꚭ ꚳ ꚶ ꛬ ꚢ ꚽ ꛯ ꛲", &["NONE"]),
    ],
  },
  Script {
    tag: "BENG",
    name: "Bengali",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: true,
    standard: "০ ৪",
    base: &[
      (0x0980, 0x09FF), // Bengali
    ],
    non_base: &[
      (0x0981, 0x0981),
      (0x09BC, 0x09BC),
      (0x09C1, 0x09C4),
      (0x09CD, 0x09CD),
      (0x09E2, 0x09E3),
      (0x09FE, 0x09FE),
    ],
    blues: &[
      ("ই ট ঠ ি ী ৈ ৗ", &["TOP"]),
      ("ও এ ড ত ন ব ল ক", &["TOP"]),
      ("অ ড ত ন ব ভ ল ক", &["TOP", "NEUTRAL", "X_HEIGHT"]),
      ("অ ড ত ন ব ভ ল ক", &["NONE"]),
    ],
  },
  Script {
    tag: "BUHD",
    name: "Buhid",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ᝋ ᝏ",
    base: &[
      (0x1740, 0x175F), // Buhid
    ],
    non_base: &[(0x1752, 0x1753)],
    blues: &[
      ("ᝐ ᝈ", &["TOP"]),
      ("ᝅ ᝊ ᝎ", &["TOP"]),
      ("ᝂ ᝃ ᝉ ᝌ", &["TOP", "X_HEIGHT"]),
      ("ᝀ ᝃ ᝆ ᝉ ᝋ ᝏ ᝑ", &["NONE"]),
    ],
  },
  Script {
    tag: "CAKM",
    name: "Chakma",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𑄤 𑄉 𑄛",
    base: &[
      (0x11100, 0x1114F), // Chakma
    ],
    non_base: &[(0x11100, 0x11102), (0x11127, 0x11134), (0x11146, 0x11146)],
    blues: &[
      ("𑄃 𑄅 𑄉 𑄙 𑄗", &["TOP"]),
      ("𑄅 𑄛 𑄝 𑄗 𑄓", &["NONE"]),
      ("𑄖𑄳𑄢 𑄘𑄳𑄢 𑄙𑄳𑄢 𑄤𑄳𑄢 𑄥𑄳𑄢", &["NONE"]),
    ],
  },
  Script {
    tag: "CANS",
    name: "Canadian Syllabics",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ᑌ ᓚ",
    base: &[
      (0x1400, 0x167F), // Unified Canadian Aboriginal Syllabics
      (0x18B0, 0x18FF), // Unified Canadian Aboriginal Syllabics Extended
    ],
    non_base: &[],
    blues: &[
      ("ᗜ ᖴ ᐁ ᒣ ᑫ ᑎ ᔑ ᗰ", &["TOP"]),
      ("ᗶ ᖵ ᒧ ᐃ ᑌ ᒍ ᔑ ᗢ", &["NONE"]),
      ("ᓓ ᓕ ᓀ ᓂ ᓄ ᕄ ᕆ ᘣ", &["TOP", "X_HEIGHT"]),
      ("ᕃ ᓂ ᓀ ᕂ ᓗ ᓚ ᕆ ᘣ", &["NONE"]),
      ("ᐪ ᙆ ᣘ ᐢ ᒾ ᣗ ᔆ", &["TOP"]),
      ("ᙆ ᗮ ᒻ ᐞ ᔆ ᒡ ᒢ ᓑ", &["NONE"]),
    ],
  },
  Script {
    tag: "CARI",
    name: "Carian",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐊫 𐋉",
    base: &[
      (0x102A0, 0x102DF), // Carian
    ],
    non_base: &[],
    blues: &[("𐊧 𐊫 𐊬 𐊭 𐊱 𐊺 𐊼 𐊿", &["TOP"]), ("𐊣 𐊧 𐊷 𐋀 𐊫 𐊸 𐋉", &["NONE"])],
  },
  Script {
    tag: "CHER",
    name: "Cherokee",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "Ꭴ Ꮕ ꮕ",
    base: &[
      (0x13A0, 0x13FF), // Cherokee
      (0xAB70, 0xABBF), // Cherokee Supplement
    ],
    non_base: &[],
    blues: &[
      ("Ꮖ Ꮋ Ꭼ Ꮓ Ꭴ Ꮳ Ꭶ Ꮥ", &["TOP"]),
      ("Ꮖ Ꮋ Ꭼ Ꮓ Ꭴ Ꮳ Ꭶ Ꮥ", &["NONE"]),
      ("ꮒ ꮤ ꮶ ꭴ ꭾ ꮗ ꮝ ꮿ", &["TOP"]),
      ("ꮖ ꭼ ꮓ ꮠ ꮳ ꭶ ꮥ ꮻ", &["TOP", "X_HEIGHT"]),
      ("ꮖ ꭼ ꮓ ꮠ ꮳ ꭶ ꮥ ꮻ", &["NONE"]),
      ("ᏸ ꮐ ꭹ ꭻ", &["NONE"]),
    ],
  },
  Script {
    tag: "COPT",
    name: "Coptic",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "Ⲟ ⲟ",
    base: &[
      (0x2C80, 0x2CFF), // Coptic
    ],
    non_base: &[(0x2CEF, 0x2CF1)],
    blues: &[
      ("Ⲍ Ⲏ Ⲡ Ⳟ Ⲟ Ⲑ Ⲥ Ⳋ", &["TOP"]),
      ("Ⳑ Ⳙ Ⳟ Ⲏ Ⲟ Ⲑ Ⳝ Ⲱ", &["NONE"]),
      ("ⲍ ⲏ ⲡ ⳟ ⲟ ⲑ ⲥ ⳋ", &["TOP", "X_HEIGHT"]),
      ("ⳑ ⳙ ⳟ ⲏ ⲟ ⲑ ⳝ Ⳓ", &["NONE"]),
    ],
  },
  Script {
    tag: "CPRT",
    name: "Cypriot",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐠅 𐠣",
    base: &[
      (0x10800, 0x1083F), // Cypriot
    ],
    non_base: &[],
    blues: &[
      ("𐠍 𐠙 𐠳 𐠱 𐠅 𐠓 𐠣 𐠦", &["TOP"]),
      ("𐠃 𐠊 𐠛 𐠣 𐠳 𐠵 𐠐", &["NONE"]),
      ("𐠈 𐠏 𐠖", &["TOP"]),
      ("𐠈 𐠏 𐠖", &["NONE"]),
    ],
  },
  Script {
    tag: "CYRL",
    name: "Cyrillic",
    group: Group::Default,
    features: true,
    hint_top_to_bottom: false,
    standard: "о О",
    base: &[
      (0x0400, 0x04FF), // Cyrillic
      (0x0500, 0x052F), // Cyrillic Supplement
      (0x2DE0, 0x2DFF), // Cyrillic Extended-A
      (0xA640, 0xA69F), // Cyrillic Extended-B
      (0x1C80, 0x1C8F), // Cyrillic Extended-C
    ],
    non_base: &[
      (0x0483, 0x0489),
      (0x2DE0, 0x2DFF),
      (0xA66F, 0xA67F),
      (0xA69E, 0xA69F),
    ],
    blues: &[
      ("Б В Е П З О С Э", &["TOP"]),
      ("Б В Е Ш З О С Э", &["NONE"]),
      ("х п н ш е з о с", &["TOP", "X_HEIGHT"]),
      ("х п н ш е з о с", &["NONE"]),
      ("р у ф", &["NONE"]),
    ],
  },
  Script {
    tag: "DEVA",
    name: "Devanagari",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: true,
    standard: "ठ व ट",
    base: &[
      (0x0900, 0x093B), // Devanagari
      (0x093D, 0x0950), // ... continued
      (0x0953, 0x0963), // ... continued
      (0x0966, 0x097F), // ... continued
      (0x20B9, 0x20B9), // (new) Rupee sign
      (0xA8E0, 0xA8FF), // Devanagari Extended
    ],
    non_base: &[
      (0x0900, 0x0902),
      (0x093A, 0x093A),
      (0x0941, 0x0948),
      (0x094D, 0x094D),
      (0x0953, 0x0957),
      (0x0962, 0x0963),
      (0xA8E0, 0xA8F1),
      (0xA8FF, 0xA8FF),
    ],
    blues: &[
      ("ई ऐ ओ औ ि ी ो ौ", &["TOP"]),
      ("क म अ आ थ ध भ श", &["TOP"]),
      ("क न म उ छ ट ठ ड", &["TOP", "NEUTRAL", "X_HEIGHT"]),
      ("क न म उ छ ट ठ ड", &["NONE"]),
      ("ु ृ", &["NONE"]),
    ],
  },
  Script {
    tag: "DSRT",
    name: "Deseret",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐐄 𐐬",
    base: &[
      (0x10400, 0x1044F), // Deseret
    ],
    non_base: &[],
    blues: &[
      ("𐐂 𐐄 𐐋 𐐗 𐐑", &["TOP"]),
      ("𐐀 𐐂 𐐄 𐐗 𐐛", &["NONE"]),
      ("𐐪 𐐬 𐐳 𐐿 𐐹", &["TOP", "X_HEIGHT"]),
      ("𐐨 𐐪 𐐬 𐐿 𐑃", &["NONE"]),
    ],
  },
  Script {
    tag: "ETHI",
    name: "Ethiopic",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ዐ",
    base: &[
      (0x1200, 0x137F), // Ethiopic
      (0x1380, 0x139F), // Ethiopic Supplement
      (0x2D80, 0x2DDF), // Ethiopic Extended
      (0xAB00, 0xAB2F), // Ethiopic Extended-A
    ],
    non_base: &[(0x135D, 0x135F)],
    blues: &[
      ("ሀ ሃ ዘ ፐ ማ በ ዋ ዐ", &["TOP"]),
      ("ለ ሐ በ ዘ ሀ ሪ ዐ ጨ", &["NONE"]),
    ],
  },
  Script {
    tag: "GEOR",
    name: "Georgian (Mkhedruli)",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ი ე ა Ჿ",
    base: &[
      (0x10D0, 0x10FF), // Georgian (Mkhedruli)
      (0x1C90, 0x1CBF), // Georgian Extended (Mtavruli)
    ],
    non_base: &[],
    blues: &[
      ("გ დ ე ვ თ ი ო ღ", &["TOP", "X_HEIGHT"]),
      ("ა ზ მ ს შ ძ ხ პ", &["NONE"]),
      ("ს ხ ქ ზ მ შ ჩ წ", &["TOP"]),
      ("ე ვ ჟ ტ უ ფ ქ ყ", &["NONE"]),
      ("Ნ Ჟ Ჳ Ჸ Გ Ე Ო Ჴ", &["TOP"]),
      ("Ი Ჲ Ო Ჩ Მ Შ Ჯ Ჽ", &["NONE"]),
    ],
  },
  Script {
    tag: "GEOK",
    name: "Georgian (Khutsuri)",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "Ⴖ Ⴑ ⴙ",
    base: &[
      (0x10A0, 0x10CD), // Georgian (Asomtavruli)
      (0x2D00, 0x2D2D), // Georgian Supplement (Nuskhuri)
    ],
    non_base: &[],
    blues: &[
      ("Ⴑ Ⴇ Ⴙ Ⴜ Ⴄ Ⴅ Ⴓ Ⴚ", &["TOP"]),
      ("Ⴄ Ⴅ Ⴇ Ⴈ Ⴆ Ⴑ Ⴊ Ⴋ", &["NONE"]),
      ("ⴁ ⴗ ⴂ ⴄ ⴅ ⴇ ⴔ ⴖ", &["TOP", "X_HEIGHT"]),
      ("ⴈ ⴌ ⴖ ⴎ ⴃ ⴆ ⴋ ⴢ", &["NONE"]),
      ("ⴐ ⴑ ⴓ ⴕ ⴙ ⴛ ⴡ ⴣ", &["TOP"]),
      ("ⴄ ⴅ ⴔ ⴕ ⴁ ⴂ ⴘ ⴝ", &["NONE"]),
    ],
  },
  Script {
    tag: "GLAG",
    name: "Glagolitic",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "Ⱅ ⱅ",
    base: &[
      (0x2C00, 0x2C5F),   // Glagolitic
      (0x1E000, 0x1E02F), // Glagolitic Supplement
    ],
    non_base: &[(0x1E000, 0x1E02F)],
    blues: &[
      ("Ⰵ Ⱄ Ⱚ Ⰴ Ⰲ Ⰺ Ⱛ Ⰻ", &["TOP"]),
      ("Ⰵ Ⰴ Ⰲ Ⱚ Ⱎ Ⱑ Ⰺ Ⱄ", &["NONE"]),
      ("ⰵ ⱄ ⱚ ⰴ ⰲ ⰺ ⱛ ⰻ", &["TOP", "X_HEIGHT"]),
      ("ⰵ ⰴ ⰲ ⱚ ⱎ ⱑ ⰺ ⱄ", &["NONE"]),
    ],
  },
  Script {
    tag: "GOTH",
    name: "Gothic",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: true,
    standard: "𐌴 𐌾 𐍃",
    base: &[
      (0x10330, 0x1034F), // Gothic
    ],
    non_base: &[],
    blues: &[("𐌲 𐌶 𐍀 𐍄 𐌴 𐍃 𐍈 𐌾", &["TOP"]), ("𐌶 𐌴 𐍃 𐍈", &["NONE"])],
  },
  Script {
    tag: "GREK",
    name: "Greek",
    group: Group::Default,
    features: true,
    hint_top_to_bottom: false,
    standard: "ο Ο",
    base: &[
      (0x0370, 0x03FF), // Greek and Coptic
      (0x1F00, 0x1FFF), // Greek Extended
    ],
    non_base: &[
      (0x037A, 0x037A),
      (0x0384, 0x0385),
      (0x1FBD, 0x1FC1),
      (0x1FCD, 0x1FCF),
      (0x1FDD, 0x1FDF),
      (0x1FED, 0x1FEF),
      (0x1FFD, 0x1FFE),
    ],
    blues: &[
      ("Γ Β Ε Ζ Θ Ο Ω", &["TOP"]),
      ("Β Δ Ζ Ξ Θ Ο", &["NONE"]),
      ("β θ δ ζ λ ξ", &["TOP"]),
      ("α ε ι ο π σ τ ω", &["TOP", "X_HEIGHT"]),
      ("α ε ι ο π σ τ ω", &["NONE"]),
      ("β γ η μ ρ φ χ ψ", &["NONE"]),
    ],
  },
  Script {
    tag: "GUJR",
    name: "Gujarati",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ટ ૦",
    base: &[
      (0x0A80, 0x0AFF), // Gujarati
    ],
    non_base: &[
      (0x0A81, 0x0A82),
      (0x0ABC, 0x0ABC),
      (0x0AC1, 0x0AC8),
      (0x0ACD, 0x0ACD),
      (0x0AE2, 0x0AE3),
      (0x0AFA, 0x0AFF),
    ],
    blues: &[
      ("ત ન ઋ ઌ છ ટ ર ૦", &["TOP", "X_HEIGHT"]),
      ("ખ ગ ઘ ઞ ઇ ઈ ઠ જ", &["NONE"]),
      ("ઈ ઊ િ ી લી શ્ચિ જિ સી", &["TOP"]),
      ("ુ ૃ ૄ ખુ છૃ છૄ", &["NONE"]),
      ("૦ ૧ ૨ ૩ ૭", &["TOP"]),
    ],
  },
  Script {
    tag: "GURU",
    name: "Gurmukhi",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: true,
    standard: "ਠ ਰ ੦",
    base: &[
      (0x0A00, 0x0A7F), // Gurmukhi
    ],
    non_base: &[
      (0x0A01, 0x0A02),
      (0x0A3C, 0x0A3C),
      (0x0A41, 0x0A51),
      (0x0A70, 0x0A71),
      (0x0A75, 0x0A75),
    ],
    blues: &[
      ("ਇ ਈ ਉ ਏ ਓ ੳ ਿ ੀ", &["TOP"]),
      ("ਕ ਗ ਙ ਚ ਜ ਤ ਧ ਸ", &["TOP"]),
      ("ਕ ਗ ਙ ਚ ਜ ਤ ਧ ਸ", &["TOP", "NEUTRAL", "X_HEIGHT"]),
      ("ਅ ਏ ਓ ਗ ਜ ਠ ਰ ਸ", &["NONE"]),
      ("੦ ੧ ੨ ੩ ੭", &["TOP"]),
    ],
  },
  Script {
    tag: "HEBR",
    name: "Hebrew",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ם",
    base: &[
      (0x0590, 0x05FF), // Hebrew
      (0xFB1D, 0xFB4F), // Alphab. Present. Forms (Hebrew)
    ],
    non_base: &[
      (0x0591, 0x05BF),
      (0x05C1, 0x05C2),
      (0x05C4, 0x05C5),
      (0x05C7, 0x05C7),
      (0xFB1E, 0xFB1E),
    ],
    blues: &[
      ("ב ד ה ח ך כ ם ס", &["TOP", "LONG"]),
      ("ב ט כ ם ס צ", &["NONE"]),
      ("ק ך ן ף ץ", &["NONE"]),
    ],
  },
  Script {
    tag: "HMNP",
    name: "Nyiakeng Puachue Hmong",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𞄨",
    base: &[
      (0x1E100, 0x1E14F), // Nyiakeng Puachue Hmong
    ],
    non_base: &[],
    blues: &[("𞄀 𞄁 𞄈 𞄑 𞄧 𞄬", &["TOP"]), ("𞄁 𞄜 𞄠 𞄡 𞄤 𞅂", &["NONE"])],
  },
  Script {
    tag: "KALI",
    name: "Kayah Li",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ꤍ ꤀",
    base: &[
      (0xA900, 0xA92F), // Kayah Li
    ],
    non_base: &[(0xA926, 0xA92D)],
    blues: &[
      ("꤅ ꤏ ꤁ ꤋ ꤀ ꤍ", &["TOP", "X_HEIGHT"]),
      ("꤈ ꤘ ꤀ ꤍ ꤢ", &["NONE"]),
      ("ꤖ ꤡ", &["TOP"]),
      ("ꤑ ꤜ ꤞ", &["NONE"]),
      ("ꤑ꤬ ꤜ꤭ ꤔ꤬", &["NONE"]),
    ],
  },
  Script {
    tag: "KHMR",
    name: "Khmer",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "០",
    base: &[
      (0x1780, 0x17FF), // Khmer
    ],
    non_base: &[
      (0x17B7, 0x17BD),
      (0x17C6, 0x17C6),
      (0x17C9, 0x17D3),
      (0x17DD, 0x17DD),
    ],
    blues: &[
      ("ខ ទ ន ឧ ឩ ា", &["TOP", "X_HEIGHT"]),
      ("ក្ក ក្ខ ក្គ ក្ថ", &["SUB_TOP"]),
      ("ខ ឃ ច ឋ ប ម យ ឲ", &["NONE"]),
      ("ត្រ រៀ ឲ្យ អឿ", &["NONE"]),
      ("ន្ត្រៃ ង្ខ្យ ក្បៀ ច្រៀ ន្តឿ ល្បឿ", &["NONE"]),
    ],
  },
  Script {
    tag: "KHMS",
    name: "Khmer Symbols",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "᧡ ᧪",
    base: &[
      (0x19E0, 0x19FF), // Khmer Symbols
    ],
    non_base: &[],
    blues: &[("᧠ ᧡", &["TOP", "X_HEIGHT"]), ("᧶ ᧹", &["NONE"])],
  },
  Script {
    tag: "KNDA",
    name: "Kannada",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "೦ ಬ",
    base: &[
      (0x0C80, 0x0CFF), // Kannada
    ],
    non_base: &[
      (0x0C81, 0x0C81),
      (0x0CBC, 0x0CBC),
      (0x0CBF, 0x0CBF),
      (0x0CC6, 0x0CC6),
      (0x0CCC, 0x0CCD),
      (0x0CE2, 0x0CE3),
    ],
    blues: &[
      ("ಇ ಊ ಐ ಣ ಸಾ ನಾ ದಾ ರಾ", &["TOP"]),
      ("ಅ ಉ ಎ ಲ ೦ ೨ ೬ ೭", &["NONE"]),
    ],
  },
  Script {
    tag: "LAOO",
    name: "Lao",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "໐",
    base: &[
      (0x0E80, 0x0EFF), // Lao
    ],
    non_base: &[(0x0EB1, 0x0EB1), (0x0EB4, 0x0EBC), (0x0EC8, 0x0ECD)],
    blues: &[
      ("າ ດ ອ ມ ລ ວ ຣ ງ", &["TOP", "X_HEIGHT"]),
      ("າ ອ ບ ຍ ຣ ຮ ວ ຢ", &["NONE"]),
      ("ປ ຢ ຟ ຝ", &["TOP"]),
      ("ໂ ໄ ໃ", &["TOP"]),
      ("ງ ຊ ຖ ຽ ໆ ຯ", &["NONE"]),
    ],
  },
  Script {
    tag: "LATN",
    name: "Latin",
    group: Group::Default,
    features: true,
    hint_top_to_bottom: false,
    standard: "o O 0",
    base: &[
      (0x0020, 0x007F),   // Basic Latin (no control chars)
      (0x00A0, 0x00A9),   // Latin-1 Supplement (no control chars)
      (0x00AB, 0x00B1),   // ... continued
      (0x00B4, 0x00B8),   // ... continued
      (0x00BB, 0x00FF),   // ... continued
      (0x0100, 0x017F),   // Latin Extended-A
      (0x0180, 0x024F),   // Latin Extended-B
      (0x0250, 0x02AF),   // IPA Extensions
      (0x02B9, 0x02DF),   // Spacing Modifier Letters
      (0x02E5, 0x02FF),   // ... continued
      (0x0300, 0x036F),   // Combining Diacritical Marks
      (0x1AB0, 0x1ABE),   // Combining Diacritical Marks Extended
      (0x1D00, 0x1D2B),   // Phonetic Extensions
      (0x1D6B, 0x1D77),   // ... continued
      (0x1D79, 0x1D7F),   // ... continued
      (0x1D80, 0x1D9A),   // Phonetic Extensions Supplement
      (0x1DC0, 0x1DFF),   // Combining Diacritical Marks Supplement
      (0x1E00, 0x1EFF),   // Latin Extended Additional
      (0x2000, 0x206F),   // General Punctuation
      (0x20A0, 0x20B8),   // Currency Symbols ...
      (0x20BA, 0x20CF),   // ... except new Rupee sign
      (0x2150, 0x218F),   // Number Forms
      (0x2C60, 0x2C7B),   // Latin Extended-C
      (0x2C7E, 0x2C7F),   // ... continued
      (0x2E00, 0x2E7F),   // Supplemental Punctuation
      (0xA720, 0xA76F),   // Latin Extended-D
      (0xA771, 0xA7F7),   // ... continued
      (0xA7FA, 0xA7FF),   // ... continued
      (0xAB30, 0xAB5B),   // Latin Extended-E
      (0xAB60, 0xAB6F),   // ... continued
      (0xFB00, 0xFB06),   // Alphab. Present. Forms (Latin Ligs)
      (0x1D400, 0x1D7FF), // Mathematical Alphanumeric Symbols
    ],
    non_base: &[
      (0x005E, 0x0060),
      (0x007E, 0x007E),
      (0x00A8, 0x00A9),
      (0x00AE, 0x00B0),
      (0x00B4, 0x00B4),
      (0x00B8, 0x00B8),
      (0x00BC, 0x00BE),
      (0x02B9, 0x02DF),
      (0x02E5, 0x02FF),
      (0x0300, 0x036F),
      (0x1AB0, 0x1ABE),
      (0x1DC0, 0x1DFF),
      (0x2017, 0x2017),
      (0x203E, 0x203E),
      (0xA788, 0xA788),
      (0xA7F8, 0xA7FA),
    ],
    blues: &[
      ("T H E Z O C Q S", &["TOP"]),
      ("H E Z L O C U S", &["NONE"]),
      ("f i j k d b h", &["TOP"]),
      ("u v x z o e s c", &["TOP", "X_HEIGHT"]),
      ("n r x z o e s c", &["NONE"]),
      ("p q g j y", &["NONE"]),
    ],
  },
  Script {
    tag: "LATB",
    name: "Latin Subscript Fallback",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ₒ ₀",
    base: &[
      (0x1D62, 0x1D6A), // some small subscript letters
      (0x2080, 0x209C), // subscript digits and letters
      (0x2C7C, 0x2C7C), // latin subscript small letter j
    ],
    non_base: &[],
    blues: &[
      ("₀ ₃ ₅ ₇ ₈", &["TOP"]),
      ("₀ ₁ ₂ ₃ ₈", &["NONE"]),
      ("ᵢ ⱼ ₕ ₖ ₗ", &["TOP"]),
      ("ₐ ₑ ₒ ₓ ₙ ₛ ᵥ ᵤ ᵣ", &["TOP", "X_HEIGHT"]),
      ("ₐ ₑ ₒ ₓ ₙ ₛ ᵥ ᵤ ᵣ", &["NONE"]),
      ("ᵦ ᵧ ᵨ ᵩ ₚ", &["NONE"]),
    ],
  },
  Script {
    tag: "LATP",
    name: "Latin Superscript Fallback",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ᵒ ᴼ ⁰",
    base: &[
      (0x00AA, 0x00AA), // feminine ordinal indicator
      (0x00B2, 0x00B3), // superscript two and three
      (0x00B9, 0x00BA), // superscript one, masc. ord. indic.
      (0x02B0, 0x02B8), // some latin superscript mod. letters
      (0x02E0, 0x02E4), // some IPA modifier letters
      (0x1D2C, 0x1D61), // latin superscript modifier letters
      (0x1D78, 0x1D78), // modifier letter cyrillic en
      (0x1D9B, 0x1DBF), // more modifier letters
      (0x2070, 0x207F), // superscript digits and letters
      (0x2C7D, 0x2C7D), // modifier letter capital v
      (0xA770, 0xA770), // modifier letter us
      (0xA7F8, 0xA7F9), // more modifier letters
      (0xAB5C, 0xAB5F), // more modifier letters
    ],
    non_base: &[],
    blues: &[
      ("⁰ ³ ⁵ ⁷ ᵀ ᴴ ᴱ ᴼ", &["TOP"]),
      ("⁰ ¹ ² ³ ᴱ ᴸ ᴼ ᵁ", &["NONE"]),
      ("ᵇ ᵈ ᵏ ʰ ʲ ᶠ ⁱ", &["TOP"]),
      ("ᵉ ᵒ ʳ ˢ ˣ ᶜ ᶻ", &["TOP", "X_HEIGHT"]),
      ("ᵉ ᵒ ʳ ˢ ˣ ᶜ ᶻ", &["NONE"]),
      ("ᵖ ʸ ᵍ", &["NONE"]),
    ],
  },
  Script {
    tag: "LISU",
    name: "Lisu",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ꓳ",
    base: &[
      (0xA4D0, 0xA4FF), // Lisu
    ],
    non_base: &[],
    blues: &[
      ("ꓡ ꓧ ꓱ ꓶ ꓩ ꓚ ꓵ ꓳ", &["TOP"]),
      ("ꓕ ꓜ ꓞ ꓡ ꓛ ꓢ ꓳ ꓴ", &["NONE"]),
    ],
  },
  Script {
    tag: "MLYM",
    name: "Malayalam",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ഠ റ",
    base: &[
      (0x0D00, 0x0D7F), // Malayalam
    ],
    non_base: &[
      (0x0D00, 0x0D01),
      (0x0D3B, 0x0D3C),
      (0x0D4D, 0x0D4E),
      (0x0D62, 0x0D63),
    ],
    blues: &[
      ("ഒ ട ഠ റ ച പ ച്ച പ്പ", &["TOP"]),
      ("ട ഠ ധ ശ ഘ ച ഥ ല", &["NONE"]),
    ],
  },
  Script {
    tag: "MEDF",
    name: "Medefaidrin",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𖹡 𖹛 𖹯",
    base: &[
      (0x16E40, 0x16E9F), // Medefaidrin
    ],
    non_base: &[],
    blues: &[
      ("𖹀 𖹁 𖹂 𖹃 𖹏 𖹚 𖹟", &["TOP"]),
      ("𖹀 𖹁 𖹂 𖹃 𖹏 𖹚 𖹒 𖹓", &["NONE"]),
      ("𖹤 𖹬 𖹧 𖹴 𖹶 𖹾", &["TOP"]),
      ("𖹠 𖹡 𖹢 𖹹 𖹳 𖹮", &["TOP", "X_HEIGHT"]),
      ("𖹠 𖹡 𖹢 𖹳 𖹭 𖹽", &["NONE"]),
      ("𖹥 𖹨 𖹩", &["NONE"]),
      ("𖺀 𖺅 𖺈 𖺄 𖺍", &["TOP"]),
    ],
  },
  Script {
    tag: "MONG",
    name: "Mongolian",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: true,
    standard: "ᡂ ᠪ",
    base: &[
      (0x1800, 0x18AF),   // Mongolian
      (0x11660, 0x1167F), // Mongolian Supplement
    ],
    non_base: &[(0x1885, 0x1886), (0x18A9, 0x18A9)],
    blues: &[
      ("ᠳ ᠴ ᠶ ᠽ ᡂ ᡊ \u{200d}ᡡ\u{200d} \u{200d}ᡳ\u{200d}", &["TOP"]),
      ("ᡃ", &["NONE"]),
    ],
  },
  Script {
    tag: "MYMR",
    name: "Myanmar",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ဝ င ဂ",
    base: &[
      (0x1000, 0x109F), // Myanmar
      (0xA9E0, 0xA9FF), // Myanmar Extended-B
      (0xAA60, 0xAA7F), // Myanmar Extended-A
    ],
    non_base: &[
      (0x102D, 0x1030),
      (0x1032, 0x1037),
      (0x103A, 0x103A),
      (0x103D, 0x103E),
      (0x1058, 0x1059),
      (0x105E, 0x1060),
      (0x1071, 0x1074),
      (0x1082, 0x1082),
      (0x1085, 0x1086),
      (0x108D, 0x108D),
      (0xA9E5, 0xA9E5),
      (0xAA7C, 0xAA7C),
    ],
    blues: &[
      ("ခ ဂ င ဒ ဝ ၥ ၊ ။", &["TOP", "X_HEIGHT"]),
      ("င ဎ ဒ ပ ဗ ဝ ၊ ။", &["NONE"]),
      ("ဩ ြ ၍ ၏ ၆ ါ ိ", &["TOP"]),
      ("ဉ ည ဥ ဩ ဨ ၂ ၅ ၉", &["NONE"]),
    ],
  },
  Script {
    tag: "NKOO",
    name: "N'Ko",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ߋ ߀",
    base: &[
      (0x07C0, 0x07FF), // N'Ko
    ],
    non_base: &[(0x07EB, 0x07F5), (0x07FD, 0x07FD)],
    blues: &[
      ("ߐ ߉ ߒ ߟ ߖ ߜ ߠ ߥ", &["TOP"]),
      ("߀ ߘ ߡ ߠ ߥ", &["NONE"]),
      ("ߏ ߛ ߋ", &["TOP", "X_HEIGHT"]),
      ("ߎ ߏ ߛ ߋ", &["NONE"]),
    ],
  },
  Script {
    tag: "NONE",
    name: "no script",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "",
    base: &[],
    non_base: &[],
    blues: &[],
  },
  Script {
    tag: "OLCK",
    name: "Ol Chiki",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ᱛ",
    base: &[
      (0x1C50, 0x1C7F), // Ol Chiki
    ],
    non_base: &[],
    blues: &[("ᱛ ᱜ ᱝ ᱡ ᱢ ᱥ", &["TOP"]), ("ᱛ ᱜ ᱝ ᱡ ᱢ ᱥ", &["NONE"])],
  },
  Script {
    tag: "ORKH",
    name: "Old Turkic",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐰗",
    base: &[
      (0x10C00, 0x10C4F), // Old Turkic
    ],
    non_base: &[],
    blues: &[("𐰗 𐰘 𐰧", &["TOP"]), ("𐰉 𐰗 𐰦 𐰧", &["NONE"])],
  },
  Script {
    tag: "OSGE",
    name: "Osage",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐓂 𐓪",
    base: &[
      (0x104B0, 0x104FF), // Osage
    ],
    non_base: &[],
    blues: &[
      ("𐒾 𐓍 𐓒 𐓓 𐒻 𐓂 𐒵 𐓆", &["TOP"]),
      ("𐒰 𐓍 𐓂 𐒿 𐓎 𐒹", &["NONE"]),
      ("𐒼 𐒽 𐒾", &["NONE"]),
      ("𐓵 𐓶 𐓺 𐓻 𐓝 𐓣 𐓪 𐓮", &["TOP", "X_HEIGHT"]),
      ("𐓘 𐓚 𐓣 𐓵 𐓡 𐓧 𐓪 𐓶", &["NONE"]),
      ("𐓤 𐓦 𐓸 𐓹 𐓛", &["TOP"]),
      ("𐓤 𐓥 𐓦", &["NONE"]),
    ],
  },
  Script {
    tag: "OSMA",
    name: "Osmanya",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐒆 𐒠",
    base: &[
      (0x10480, 0x104AF), // Osmanya
    ],
    non_base: &[],
    blues: &[
      ("𐒆 𐒉 𐒐 𐒒 𐒘 𐒛 𐒠 𐒣", &["TOP"]),
      ("𐒀 𐒂 𐒆 𐒈 𐒊 𐒒 𐒠 𐒩", &["NONE"]),
    ],
  },
  Script {
    tag: "ROHG",
    name: "Hanifi Rohingya",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐴰",
    base: &[
      (0x10D00, 0x10D3F), // Hanifi Rohingya
    ],
    non_base: &[],
    blues: &[
      ("𐴃 𐴀 𐴆 𐴖 𐴕", &["TOP"]),
      ("𐴔 𐴖 𐴕 𐴑 𐴐", &["NONE"]),
      ("ـ", &["NEUTRAL"]),
    ],
  },
  Script {
    tag: "SAUR",
    name: "Saurashtra",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ꢝ ꣐",
    base: &[
      (0xA880, 0xA8DF), // Saurashtra
    ],
    non_base: &[(0xA880, 0xA881), (0xA8B4, 0xA8C5)],
    blues: &[("ꢜ ꢞ ꢳ ꢂ ꢖ ꢒ ꢝ ꢛ", &["TOP"]), ("ꢂ ꢨ ꢺ ꢤ ꢎ", &["NONE"])],
  },
  Script {
    tag: "SHAW",
    name: "Shavian",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "𐑴",
    base: &[
      (0x10450, 0x1047F), // Shavian
    ],
    non_base: &[],
    blues: &[
      ("𐑕 𐑙", &["TOP"]),
      ("𐑔 𐑖 𐑗 𐑹 𐑻", &["NONE"]),
      ("𐑟 𐑣", &["NONE"]),
      ("𐑱 𐑲 𐑳 𐑴 𐑸 𐑺 𐑼", &["TOP", "X_HEIGHT"]),
      ("𐑴 𐑻 𐑹", &["NONE"]),
    ],
  },
  Script {
    tag: "SINH",
    name: "Sinhala",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ට",
    base: &[
      (0x0D80, 0x0DFF), // Sinhala
    ],
    non_base: &[(0x0DCA, 0x0DCA), (0x0DD2, 0x0DD6)],
    blues: &[
      ("ඉ ක ඝ ඳ ප ය ල ෆ", &["TOP"]),
      ("එ ඔ ඝ ජ ට ථ ධ ර", &["NONE"]),
      ("ද ඳ උ ල තූ තු බු දු", &["NONE"]),
    ],
  },
  Script {
    tag: "SUND",
    name: "Sundanese",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "᮰",
    base: &[
      (0x1B80, 0x1BBF), // Sundanese
      (0x1CC0, 0x1CCF), // Sundanese Supplement
    ],
    non_base: &[(0x1B80, 0x1B82), (0x1BA1, 0x1BAD)],
    blues: &[
      ("ᮋ ᮞ ᮮ ᮽ ᮰ ᮈ", &["TOP"]),
      ("ᮄ ᮔ ᮕ ᮗ ᮰ ᮆ ᮈ ᮉ", &["NONE"]),
      ("ᮼ ᳄", &["NONE"]),
    ],
  },
  Script {
    tag: "TAML",
    name: "Tamil",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "௦",
    base: &[
      (0x0B80, 0x0BFF), // Tamil
    ],
    non_base: &[(0x0B82, 0x0B82), (0x0BC0, 0x0BC2), (0x0BCD, 0x0BCD)],
    blues: &[
      ("உ ஒ ஓ ற ஈ க ங ச", &["TOP"]),
      ("க ச ல ஶ உ ங ட ப", &["NONE"]),
    ],
  },
  Script {
    tag: "TAVT",
    name: "Tai Viet",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ꪒ ꪫ",
    base: &[
      (0xAA80, 0xAADF), // Tai Viet
    ],
    non_base: &[
      (0xAAB0, 0xAAB0),
      (0xAAB2, 0xAAB4),
      (0xAAB7, 0xAAB8),
      (0xAABE, 0xAABF),
      (0xAAC1, 0xAAC1),
    ],
    blues: &[("ꪆ ꪔ ꪒ ꪖ ꪫ", &["TOP"]), ("ꪉ ꪫ ꪮ", &["NONE"])],
  },
  Script {
    tag: "TELU",
    name: "Telugu",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "౦ ౧",
    base: &[
      (0x0C00, 0x0C7F), // Telugu
    ],
    non_base: &[
      (0x0C00, 0x0C00),
      (0x0C04, 0x0C04),
      (0x0C3E, 0x0C40),
      (0x0C46, 0x0C56),
      (0x0C62, 0x0C63),
    ],
    blues: &[("ఇ ఌ ఙ ఞ ణ ఱ ౯", &["TOP"]), ("అ క చ ర ఽ ౨ ౬", &["NONE"])],
  },
  Script {
    tag: "TFNG",
    name: "Tifinagh",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ⵔ",
    base: &[
      (0x2D30, 0x2D7F), // Tifinagh
    ],
    non_base: &[],
    blues: &[
      ("ⵔ ⵙ ⵛ ⵞ ⴵ ⴼ ⴹ ⵎ", &["TOP"]),
      ("ⵔ ⵙ ⵛ ⵞ ⴵ ⴼ ⴹ ⵎ", &["NONE"]),
    ],
  },
  Script {
    tag: "THAI",
    name: "Thai",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "า ๅ ๐",
    base: &[
      (0x0E00, 0x0E7F), // Thai
    ],
    non_base: &[(0x0E31, 0x0E31), (0x0E34, 0x0E3A), (0x0E47, 0x0E4E)],
    blues: &[
      ("บ เ แ อ ก า", &["TOP", "X_HEIGHT"]),
      ("บ ป ษ ฯ อ ย ฮ", &["NONE"]),
      ("ป ฝ ฟ", &["TOP"]),
      ("โ ใ ไ", &["TOP"]),
      ("ฎ ฏ ฤ ฦ", &["NONE"]),
      ("ญ ฐ", &["NONE"]),
      ("๐ ๑ ๓", &["NONE"]),
    ],
  },
  Script {
    tag: "VAII",
    name: "Vai",
    group: Group::Default,
    features: false,
    hint_top_to_bottom: false,
    standard: "ꘓ ꖜ ꖴ",
    base: &[
      (0xA500, 0xA63F), // Vai
    ],
    non_base: &[],
    blues: &[
      ("ꗍ ꘖ ꘙ ꘜ ꖜ ꖝ ꔅ ꕢ", &["TOP"]),
      ("ꗍ ꘖ ꘙ ꗞ ꔅ ꕢ ꖜ ꔆ", &["NONE"]),
    ],
  },
  Script {
    tag: "LIMB",
    name: "Limbu",
    group: Group::Indic,
    features: false,
    hint_top_to_bottom: false,
    standard: "o",
    base: &[
      (0x1900, 0x194F), // Limbu
    ],
    non_base: &[(0x1920, 0x1922), (0x1927, 0x1934), (0x1937, 0x193B)],
    blues: &[],
  },
  Script {
    tag: "ORYA",
    name: "Oriya",
    group: Group::Indic,
    features: false,
    hint_top_to_bottom: false,
    standard: "o",
    base: &[
      (0x0B00, 0x0B7F), // Oriya
    ],
    non_base: &[
      (0x0B01, 0x0B02),
      (0x0B3C, 0x0B3C),
      (0x0B3F, 0x0B3F),
      (0x0B41, 0x0B44),
      (0x0B4D, 0x0B56),
      (0x0B62, 0x0B63),
    ],
    blues: &[],
  },
  Script {
    tag: "SYLO",
    name: "Syloti Nagri",
    group: Group::Indic,
    features: false,
    hint_top_to_bottom: false,
    standard: "o",
    base: &[
      (0xA800, 0xA82F), // Syloti Nagri
    ],
    non_base: &[
      (0xA802, 0xA802),
      (0xA806, 0xA806),
      (0xA80B, 0xA80B),
      (0xA825, 0xA826),
    ],
    blues: &[],
  },
  Script {
    tag: "TIBT",
    name: "Tibetan",
    group: Group::Indic,
    features: false,
    hint_top_to_bottom: false,
    standard: "o",
    base: &[
      (0x0F00, 0x0FFF), // Tibetan
    ],
    non_base: &[
      (0x0F18, 0x0F19),
      (0x0F35, 0x0F35),
      (0x0F37, 0x0F37),
      (0x0F39, 0x0F39),
      (0x0F3E, 0x0F3F),
      (0x0F71, 0x0F7E),
      (0x0F80, 0x0F84),
      (0x0F86, 0x0F87),
      (0x0F8D, 0x0FBC),
    ],
    blues: &[],
  },
  Script {
    tag: "HANI",
    name: "CJKV ideographs",
    group: Group::Cjk,
    features: false,
    hint_top_to_bottom: false,
    standard: "田 囗",
    base: &[
      (0x1100, 0x11FF),   // Hangul Jamo
      (0x2E80, 0x2EFF),   // CJK Radicals Supplement
      (0x2F00, 0x2FDF),   // Kangxi Radicals
      (0x2FF0, 0x2FFF),   // Ideographic Description Characters
      (0x3000, 0x303F),   // CJK Symbols and Punctuation
      (0x3040, 0x309F),   // Hiragana
      (0x30A0, 0x30FF),   // Katakana
      (0x3100, 0x312F),   // Bopomofo
      (0x3130, 0x318F),   // Hangul Compatibility Jamo
      (0x3190, 0x319F),   // Kanbun
      (0x31A0, 0x31BF),   // Bopomofo Extended
      (0x31C0, 0x31EF),   // CJK Strokes
      (0x31F0, 0x31FF),   // Katakana Phonetic Extensions
      (0x3300, 0x33FF),   // CJK Compatibility
      (0x3400, 0x4DBF),   // CJK Unified Ideographs Extension A
      (0x4DC0, 0x4DFF),   // Yijing Hexagram Symbols
      (0x4E00, 0x9FFF),   // CJK Unified Ideographs
      (0xA960, 0xA97F),   // Hangul Jamo Extended-A
      (0xAC00, 0xD7AF),   // Hangul Syllables
      (0xD7B0, 0xD7FF),   // Hangul Jamo Extended-B
      (0xF900, 0xFAFF),   // CJK Compatibility Ideographs
      (0xFE10, 0xFE1F),   // Vertical forms
      (0xFE30, 0xFE4F),   // CJK Compatibility Forms
      (0xFF00, 0xFFEF),   // Halfwidth and Fullwidth Forms
      (0x1B000, 0x1B0FF), // Kana Supplement
      (0x1B100, 0x1B12F), // Kana Extended-A
      (0x1D300, 0x1D35F), // Tai Xuan Hing Symbols
      (0x20000, 0x2A6DF), // CJK Unified Ideographs Extension B
      (0x2A700, 0x2B73F), // CJK Unified Ideographs Extension C
      (0x2B740, 0x2B81F), // CJK Unified Ideographs Extension D
      (0x2B820, 0x2CEAF), // CJK Unified Ideographs Extension E
      (0x2CEB0, 0x2EBEF), // CJK Unified Ideographs Extension F
      (0x2F800, 0x2FA1F), // CJK Compatibility Ideographs Supplement
    ],
    non_base: &[(0x302A, 0x302F), (0x3190, 0x319F)],
    blues: &[
      (
        "他 们 你 來 們 到 和 地 对 對 就 席 我 时 時 會 来 為 能 舰 說 说 这 這 齊 | 军 同 已 愿 既 星 是 景 民 照 现 現 理 用 置 要 軍 那 配 里 開 雷 露 面 顾",
        &["TOP"],
      ),
      (
        "个 为 人 他 以 们 你 來 個 們 到 和 大 对 對 就 我 时 時 有 来 為 要 說 说 | 主 些 因 它 想 意 理 生 當 看 着 置 者 自 著 裡 过 还 进 進 過 道 還 里 面",
        &["NONE"],
      ),
      (
        " 些 们 你 來 們 到 和 地 她 将 將 就 年 得 情 最 样 樣 理 能 說 说 这 這 通 | 即 吗 吧 听 呢 品 响 嗎 师 師 收 断 斷 明 眼 間 间 际 陈 限 除 陳 随 際 隨",
        &["HORIZONTAL"],
      ),
      (
        "事 前 學 将 將 情 想 或 政 斯 新 样 樣 民 沒 没 然 特 现 現 球 第 經 谁 起 | 例 別 别 制 动 動 吗 嗎 增 指 明 朝 期 构 物 确 种 調 调 費 费 那 都 間 间",
        &["HORIZONTAL", "RIGHT"],
      ),
    ],
  },
];
