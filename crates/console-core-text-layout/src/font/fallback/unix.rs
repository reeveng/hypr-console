use unicode_script::Script;

use super::Fallback;

#[derive(Debug)]
pub struct PlatformFallback;

impl Fallback for PlatformFallback {
  fn common_fallback(&self) -> &'static [&'static str] {
    match common_fallback() {
      Ok(value) => value,
    }
  }

  fn forbidden_fallback(&self) -> &'static [&'static str] {
    match forbidden_fallback() {
      Ok(value) => value,
    }
  }

  fn script_fallback(
    &self,
    script: unicode_script::Script,
    locale: &str,
  ) -> &'static [&'static str] {
    match script_fallback(script, locale) {
      Ok(value) => value,
    }
  }
}

const fn common_fallback() -> Result<&'static [&'static str], console_core_never::Never> {
  {
    Ok(&[
      "Noto Sans",
      "DejaVu Sans",
      "FreeSans",
      "Noto Sans Mono",
      "DejaVu Sans Mono",
      "FreeMono",
      "Noto Sans Symbols",
      "Noto Sans Symbols2",
      "Noto Color Emoji",
    ])
  }
}

const fn forbidden_fallback() -> Result<&'static [&'static str], console_core_never::Never> {
  Ok(&[])
}

fn han_unification(locale: &str) -> Result<&'static [&'static str], console_core_never::Never> {
  {
    Ok(match locale {
      "ja" => &["Noto Sans CJK JP"],

      "ko" => &["Noto Sans CJK KR"],

      "zh-HK" => &["Noto Sans CJK HK"],

      "zh-TW" => &["Noto Sans CJK TC"],

      _ => &["Noto Sans CJK SC"],
    })
  }
}

const SCRIPT_FONTS: &[(Script, &[&str])] = &[
  (
    Script::Adlam,
    &["Noto Sans Adlam", "Noto Sans Adlam Unjoined"],
  ),
  (Script::Arabic, &["Noto Sans Arabic"]),
  (Script::Armenian, &["Noto Sans Armenian"]),
  (Script::Bengali, &["Noto Sans Bengali"]),
  (Script::Braille, &["FreeMono"]),
  (Script::Buhid, &["Noto Sans Buhid"]),
  (Script::Chakma, &["Noto Sans Chakma"]),
  (Script::Cherokee, &["Noto Sans Cherokee"]),
  (Script::Deseret, &["Noto Sans Deseret"]),
  (Script::Devanagari, &["Noto Sans Devanagari"]),
  (Script::Ethiopic, &["Noto Sans Ethiopic"]),
  (Script::Georgian, &["Noto Sans Georgian"]),
  (Script::Gothic, &["Noto Sans Gothic"]),
  (Script::Grantha, &["Noto Sans Grantha"]),
  (Script::Gujarati, &["Noto Sans Gujarati"]),
  (Script::Gurmukhi, &["Noto Sans Gurmukhi"]),
  (Script::Hanunoo, &["Noto Sans Hanunoo"]),
  (Script::Hebrew, &["Noto Sans Hebrew"]),
  (Script::Javanese, &["Noto Sans Javanese"]),
  (Script::Kannada, &["Noto Sans Kannada"]),
  (Script::Khmer, &["Noto Sans Khmer"]),
  (Script::Lao, &["Noto Sans Lao"]),
  (Script::Malayalam, &["Noto Sans Malayalam"]),
  (Script::Mongolian, &["Noto Sans Mongolian"]),
  (Script::Myanmar, &["Noto Sans Myanmar"]),
  (Script::Oriya, &["Noto Sans Oriya"]),
  (Script::Runic, &["Noto Sans Runic"]),
  (Script::Sinhala, &["Noto Sans Sinhala"]),
  (Script::Syriac, &["Noto Sans Syriac"]),
  (Script::Tagalog, &["Noto Sans Tagalog"]),
  (Script::Tagbanwa, &["Noto Sans Tagbanwa"]),
  (Script::Tai_Le, &["Noto Sans Tai Le"]),
  (Script::Tai_Tham, &["Noto Sans Tai Tham"]),
  (Script::Tai_Viet, &["Noto Sans Tai Viet"]),
  (Script::Tamil, &["Noto Sans Tamil"]),
  (Script::Telugu, &["Noto Sans Telugu"]),
  (Script::Thaana, &["Noto Sans Thaana"]),
  (Script::Thai, &["Noto Sans Thai"]),
  (Script::Tibetan, &["Noto Serif Tibetan"]),
  (Script::Tifinagh, &["Noto Sans Tifinagh"]),
  (Script::Vai, &["Noto Sans Vai"]),
  (Script::Yi, &["Noto Sans Yi", "Noto Sans CJK SC"]),
];

fn script_fallback(
  script: Script,
  locale: &str,
) -> Result<&'static [&'static str], console_core_never::Never> {
  {
    let Ok(mapped_fallback) = super::mapped_fallback(
      script,
      locale,
      &[Script::Han, Script::Bopomofo],
      |argument_0| match han_unification(argument_0) {
        Ok(value) => value,
      },
      SCRIPT_FONTS,
    );

    Ok(mapped_fallback)
  }
}
