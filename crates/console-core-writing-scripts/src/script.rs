//! A script, the writing system a character belongs to, as Unicode Standard Annex #24 names it.
//!
//! The variants are Unicode 17.0's scripts by the long names Scripts.txt spells them with, so a
//! variant is written the way the Character Database writes it, underscores and all, and each
//! carries the four letters of ISO 15924 that PropertyValueAliases.txt gives it. `Unknown` is the
//! script of a character no script claims, `Common` of one many scripts share, and `Inherited` of
//! one that takes the script of the character before it.
//!
//! <https://www.unicode.org/reports/tr24/>

use console_core_words::Words;

#[allow(
  non_camel_case_types,
  reason = "a script is named the way the Character Database spells it, and the long names hold underscores"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Words)]
pub enum Script {
  #[words(short_name = "Zzzz")]
  Unknown,
  #[words(short_name = "Zyyy")]
  Common,
  #[words(short_name = "Zinh")]
  Inherited,
  #[words(short_name = "Adlm")]
  Adlam,
  #[words(short_name = "Aghb")]
  Caucasian_Albanian,
  #[words(short_name = "Ahom")]
  Ahom,
  #[words(short_name = "Arab")]
  Arabic,
  #[words(short_name = "Armi")]
  Imperial_Aramaic,
  #[words(short_name = "Armn")]
  Armenian,
  #[words(short_name = "Avst")]
  Avestan,
  #[words(short_name = "Bali")]
  Balinese,
  #[words(short_name = "Bamu")]
  Bamum,
  #[words(short_name = "Bass")]
  Bassa_Vah,
  #[words(short_name = "Batk")]
  Batak,
  #[words(short_name = "Beng")]
  Bengali,
  #[words(short_name = "Berf")]
  Beria_Erfe,
  #[words(short_name = "Bhks")]
  Bhaiksuki,
  #[words(short_name = "Bopo")]
  Bopomofo,
  #[words(short_name = "Brah")]
  Brahmi,
  #[words(short_name = "Brai")]
  Braille,
  #[words(short_name = "Bugi")]
  Buginese,
  #[words(short_name = "Buhd")]
  Buhid,
  #[words(short_name = "Cakm")]
  Chakma,
  #[words(short_name = "Cans")]
  Canadian_Aboriginal,
  #[words(short_name = "Cari")]
  Carian,
  #[words(short_name = "Cham")]
  Cham,
  #[words(short_name = "Cher")]
  Cherokee,
  #[words(short_name = "Chrs")]
  Chorasmian,
  #[words(short_name = "Copt")]
  Coptic,
  #[words(short_name = "Cpmn")]
  Cypro_Minoan,
  #[words(short_name = "Cprt")]
  Cypriot,
  #[words(short_name = "Cyrl")]
  Cyrillic,
  #[words(short_name = "Deva")]
  Devanagari,
  #[words(short_name = "Diak")]
  Dives_Akuru,
  #[words(short_name = "Dogr")]
  Dogra,
  #[words(short_name = "Dsrt")]
  Deseret,
  #[words(short_name = "Dupl")]
  Duployan,
  #[words(short_name = "Egyp")]
  Egyptian_Hieroglyphs,
  #[words(short_name = "Elba")]
  Elbasan,
  #[words(short_name = "Elym")]
  Elymaic,
  #[words(short_name = "Ethi")]
  Ethiopic,
  #[words(short_name = "Gara")]
  Garay,
  #[words(short_name = "Geor")]
  Georgian,
  #[words(short_name = "Glag")]
  Glagolitic,
  #[words(short_name = "Gong")]
  Gunjala_Gondi,
  #[words(short_name = "Gonm")]
  Masaram_Gondi,
  #[words(short_name = "Goth")]
  Gothic,
  #[words(short_name = "Gran")]
  Grantha,
  #[words(short_name = "Grek")]
  Greek,
  #[words(short_name = "Gujr")]
  Gujarati,
  #[words(short_name = "Gukh")]
  Gurung_Khema,
  #[words(short_name = "Guru")]
  Gurmukhi,
  #[words(short_name = "Hang")]
  Hangul,
  #[words(short_name = "Hani")]
  Han,
  #[words(short_name = "Hano")]
  Hanunoo,
  #[words(short_name = "Hatr")]
  Hatran,
  #[words(short_name = "Hebr")]
  Hebrew,
  #[words(short_name = "Hira")]
  Hiragana,
  #[words(short_name = "Hluw")]
  Anatolian_Hieroglyphs,
  #[words(short_name = "Hmng")]
  Pahawh_Hmong,
  #[words(short_name = "Hmnp")]
  Nyiakeng_Puachue_Hmong,
  #[words(short_name = "Hung")]
  Old_Hungarian,
  #[words(short_name = "Ital")]
  Old_Italic,
  #[words(short_name = "Java")]
  Javanese,
  #[words(short_name = "Kali")]
  Kayah_Li,
  #[words(short_name = "Kana")]
  Katakana,
  #[words(short_name = "Kawi")]
  Kawi,
  #[words(short_name = "Khar")]
  Kharoshthi,
  #[words(short_name = "Khmr")]
  Khmer,
  #[words(short_name = "Khoj")]
  Khojki,
  #[words(short_name = "Kits")]
  Khitan_Small_Script,
  #[words(short_name = "Knda")]
  Kannada,
  #[words(short_name = "Krai")]
  Kirat_Rai,
  #[words(short_name = "Kthi")]
  Kaithi,
  #[words(short_name = "Lana")]
  Tai_Tham,
  #[words(short_name = "Laoo")]
  Lao,
  #[words(short_name = "Latn")]
  Latin,
  #[words(short_name = "Lepc")]
  Lepcha,
  #[words(short_name = "Limb")]
  Limbu,
  #[words(short_name = "Lina")]
  Linear_A,
  #[words(short_name = "Linb")]
  Linear_B,
  #[words(short_name = "Lisu")]
  Lisu,
  #[words(short_name = "Lyci")]
  Lycian,
  #[words(short_name = "Lydi")]
  Lydian,
  #[words(short_name = "Mahj")]
  Mahajani,
  #[words(short_name = "Maka")]
  Makasar,
  #[words(short_name = "Mand")]
  Mandaic,
  #[words(short_name = "Mani")]
  Manichaean,
  #[words(short_name = "Marc")]
  Marchen,
  #[words(short_name = "Medf")]
  Medefaidrin,
  #[words(short_name = "Mend")]
  Mende_Kikakui,
  #[words(short_name = "Merc")]
  Meroitic_Cursive,
  #[words(short_name = "Mero")]
  Meroitic_Hieroglyphs,
  #[words(short_name = "Mlym")]
  Malayalam,
  #[words(short_name = "Modi")]
  Modi,
  #[words(short_name = "Mong")]
  Mongolian,
  #[words(short_name = "Mroo")]
  Mro,
  #[words(short_name = "Mtei")]
  Meetei_Mayek,
  #[words(short_name = "Mult")]
  Multani,
  #[words(short_name = "Mymr")]
  Myanmar,
  #[words(short_name = "Nagm")]
  Nag_Mundari,
  #[words(short_name = "Nand")]
  Nandinagari,
  #[words(short_name = "Narb")]
  Old_North_Arabian,
  #[words(short_name = "Nbat")]
  Nabataean,
  #[words(short_name = "Newa")]
  Newa,
  #[words(short_name = "Nkoo")]
  Nko,
  #[words(short_name = "Nshu")]
  Nushu,
  #[words(short_name = "Ogam")]
  Ogham,
  #[words(short_name = "Olck")]
  Ol_Chiki,
  #[words(short_name = "Onao")]
  Ol_Onal,
  #[words(short_name = "Orkh")]
  Old_Turkic,
  #[words(short_name = "Orya")]
  Oriya,
  #[words(short_name = "Osge")]
  Osage,
  #[words(short_name = "Osma")]
  Osmanya,
  #[words(short_name = "Ougr")]
  Old_Uyghur,
  #[words(short_name = "Palm")]
  Palmyrene,
  #[words(short_name = "Pauc")]
  Pau_Cin_Hau,
  #[words(short_name = "Perm")]
  Old_Permic,
  #[words(short_name = "Phag")]
  Phags_Pa,
  #[words(short_name = "Phli")]
  Inscriptional_Pahlavi,
  #[words(short_name = "Phlp")]
  Psalter_Pahlavi,
  #[words(short_name = "Phnx")]
  Phoenician,
  #[words(short_name = "Plrd")]
  Miao,
  #[words(short_name = "Prti")]
  Inscriptional_Parthian,
  #[words(short_name = "Rjng")]
  Rejang,
  #[words(short_name = "Rohg")]
  Hanifi_Rohingya,
  #[words(short_name = "Runr")]
  Runic,
  #[words(short_name = "Samr")]
  Samaritan,
  #[words(short_name = "Sarb")]
  Old_South_Arabian,
  #[words(short_name = "Saur")]
  Saurashtra,
  #[words(short_name = "Sgnw")]
  SignWriting,
  #[words(short_name = "Shaw")]
  Shavian,
  #[words(short_name = "Shrd")]
  Sharada,
  #[words(short_name = "Sidd")]
  Siddham,
  #[words(short_name = "Sidt")]
  Sidetic,
  #[words(short_name = "Sind")]
  Khudawadi,
  #[words(short_name = "Sinh")]
  Sinhala,
  #[words(short_name = "Sogd")]
  Sogdian,
  #[words(short_name = "Sogo")]
  Old_Sogdian,
  #[words(short_name = "Sora")]
  Sora_Sompeng,
  #[words(short_name = "Soyo")]
  Soyombo,
  #[words(short_name = "Sund")]
  Sundanese,
  #[words(short_name = "Sunu")]
  Sunuwar,
  #[words(short_name = "Sylo")]
  Syloti_Nagri,
  #[words(short_name = "Syrc")]
  Syriac,
  #[words(short_name = "Tagb")]
  Tagbanwa,
  #[words(short_name = "Takr")]
  Takri,
  #[words(short_name = "Tale")]
  Tai_Le,
  #[words(short_name = "Talu")]
  New_Tai_Lue,
  #[words(short_name = "Taml")]
  Tamil,
  #[words(short_name = "Tang")]
  Tangut,
  #[words(short_name = "Tavt")]
  Tai_Viet,
  #[words(short_name = "Tayo")]
  Tai_Yo,
  #[words(short_name = "Telu")]
  Telugu,
  #[words(short_name = "Tfng")]
  Tifinagh,
  #[words(short_name = "Tglg")]
  Tagalog,
  #[words(short_name = "Thaa")]
  Thaana,
  #[words(short_name = "Thai")]
  Thai,
  #[words(short_name = "Tibt")]
  Tibetan,
  #[words(short_name = "Tirh")]
  Tirhuta,
  #[words(short_name = "Tnsa")]
  Tangsa,
  #[words(short_name = "Todr")]
  Todhri,
  #[words(short_name = "Tols")]
  Tolong_Siki,
  #[words(short_name = "Toto")]
  Toto,
  #[words(short_name = "Tutg")]
  Tulu_Tigalari,
  #[words(short_name = "Ugar")]
  Ugaritic,
  #[words(short_name = "Vaii")]
  Vai,
  #[words(short_name = "Vith")]
  Vithkuqi,
  #[words(short_name = "Wara")]
  Warang_Citi,
  #[words(short_name = "Wcho")]
  Wancho,
  #[words(short_name = "Xpeo")]
  Old_Persian,
  #[words(short_name = "Xsux")]
  Cuneiform,
  #[words(short_name = "Yezi")]
  Yezidi,
  #[words(short_name = "Yiii")]
  Yi,
  #[words(short_name = "Zanb")]
  Zanabazar_Square,
}
