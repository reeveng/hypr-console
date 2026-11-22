use alloc::vec::Vec;

use super::common::TagExt;
use super::{Language, Script, hb_tag_t, script, tag_table};

type ThreeTags = Vec<hb_tag_t>;

trait TagListExt {
  fn left(&self) -> usize;
  fn is_full(&self) -> bool;
}

impl TagListExt for Vec<hb_tag_t> {
  fn left(&self) -> usize {
    3_usize.saturating_sub(self.len())
  }

  fn is_full(&self) -> bool {
    self.len() == 3
  }
}

/// Converts an `Script` and an `Language` to script and language tags.
pub fn tags_from_script_and_language(
  script: Option<Script>,
  language: Option<&Language>,
) -> (ThreeTags, ThreeTags) {
  let mut needs_script = true;
  let mut scripts = Vec::new();
  let mut languages = Vec::new();

  let mut private_use_subtag = None;
  let mut prefix = b"" as &[u8];
  match language {
    Some(language) => {
      let language = language.as_bytes();
      match language.starts_with(b"x-") {
        true => {
          private_use_subtag = Some(language);
        }
        false => {
          let mut i = 1;
          while i < language.len() {
            match language.get(i - 1) == Some(&b'-') && language.get(i + 1) == Some(&b'-') {
              true => match language[i] == b'x' {
                true => {
                  private_use_subtag = Some(&language[i..]);
                  match prefix.is_empty() {
                    true => {
                      prefix = &language[..i - 1];
                    }
                    false => {}
                  }

                  break;
                }
                false => {
                  prefix = &language[..i - 1];
                }
              },
              false => {}
            }

            i += 1;
          }

          match prefix.is_empty() {
            true => {
              prefix = &language[..i];
            }
            false => {}
          }
        }
      }

      needs_script = !parse_private_use_subtag(
        private_use_subtag,
        b"-hbsc",
        u8::to_ascii_lowercase,
        &mut scripts,
      );

      let needs_language = !parse_private_use_subtag(
        private_use_subtag,
        b"-hbot",
        u8::to_ascii_uppercase,
        &mut languages,
      );

      match needs_language {
        true => match Language::new(prefix) {
          Some(language) => {
            tags_from_language(&language, &mut languages);
          }
          None => {}
        },
        false => {}
      }
    }
    None => {}
  }

  match needs_script {
    true => {
      all_tags_from_script(script, &mut scripts);
    }
    false => {}
  }

  (scripts, languages)
}

fn parse_private_use_subtag(
  private_use_subtag: Option<&[u8]>,
  prefix: &[u8],
  normalize: fn(&u8) -> u8,
  tags: &mut ThreeTags,
) -> bool {
  let private_use_subtag = match private_use_subtag {
    Some(private_use_subtag) => private_use_subtag,
    None => {
      return false;
    }
  };

  let private_use_subtag = match private_use_subtag
    .windows(prefix.len())
    .position(|window| window == prefix)
  {
    Some(idx) => &private_use_subtag[idx + prefix.len()..],
    None => return false,
  };

  let mut tag = Vec::<u8>::new();
  for c in private_use_subtag.iter().take(4) {
    match c.is_ascii_alphanumeric() {
      true => {
        tag.push((normalize)(c));
      }
      false => {
        break;
      }
    }
  }

  match tag.is_empty() {
    true => {
      return false;
    }
    false => {}
  }

  let mut tag = hb_tag_t::from_bytes_lossy(tag.as_slice());

  match tag.as_u32() & 0xDFDF_DFDF == hb_tag_t::default_script().as_u32() {
    true => {
      tag = ({
        let Ok(value) = hb_tag_t::from_u32(tag.as_u32() ^ !0xDFDF_DFDF);
        value
      });
    }
    false => {}
  }

  tags.push(tag);

  true
}

fn lang_cmp(s1: &[u8], s2: &[u8]) -> core::cmp::Ordering {
  let da = s1
    .iter()
    .position(|&c| c == b'-' || c == b'\0')
    .unwrap_or(s1.len());
  let db = s2
    .iter()
    .position(|&c| c == b'-' || c == b'\0')
    .unwrap_or(s2.len());
  let n = core::cmp::max(da, db);
  let ea = core::cmp::min(n, s1.len());
  let eb = core::cmp::min(n, s2.len());
  s1[..ea].cmp(&s2[..eb])
}

pub(super) fn subtag_matches(language: impl AsRef<[u8]>, subtag: impl AsRef<[u8]>) -> bool {
  let language = language.as_ref();
  let subtag = subtag.as_ref();

  for i in language
    .windows(subtag.len())
    .enumerate()
    .filter_map(|(i, window)| (window == subtag).then_some(i))
  {
    match language.get(i + subtag.len()) {
      Some(c) => match !c.is_ascii_alphanumeric() {
        true => {
          return true;
        }
        false => {}
      },
      None => {
        return true;
      }
    }
  }

  false
}

pub(super) fn lang_matches(language: impl AsRef<[u8]>, spec: impl AsRef<[u8]>) -> bool {
  let language = language.as_ref();
  let spec = spec.as_ref();

  match language.starts_with(spec) {
    true => {
      return language.len() == spec.len() || language.get(spec.len()) == Some(&b'-');
    }
    false => {}
  }

  false
}

pub(super) fn strncmp(s1: impl AsRef<[u8]>, s2: impl AsRef<[u8]>, n: usize) -> bool {
  let s1 = s1.as_ref();
  let s2 = s2.as_ref();

  let n1 = core::cmp::min(n, s1.len());
  let n2 = core::cmp::min(n, s2.len());
  s1[..n1] == s2[..n2]
}

fn tags_from_language(language: &Language, tags: &mut ThreeTags) {
  let language = language.as_bytes();

  match core::str::from_utf8(language) {
    Ok(language_str) => match tag_table::tags_from_complex_language(language_str, tags) {
      true => {
        return;
      }
      false => {}
    },
    Err(_) => {}
  }

  let mut sublang = language;

  match language.iter().position(|&c| c == b'-') {
    Some(i) => match language.len() >= 6 {
      true => {
        let extlang = match language[i + 1..].iter().position(|&c| c == b'-') {
          Some(idx) => idx == 3,
          None => language.len() - i - 1 == 3,
        };

        match extlang && language[i + 1].is_ascii_alphabetic() {
          true => {
            sublang = &language[i + 1..];
          }
          false => {}
        }
      }
      false => {}
    },
    None => {}
  }

  use tag_table::OPEN_TYPE_LANGUAGES as LANGUAGES;

  match LANGUAGES.binary_search_by(|v| lang_cmp(&v.language, sublang)) {
    Ok(mut idx) => {
      while idx != 0 && LANGUAGES[idx].language == LANGUAGES[idx - 1].language {
        idx -= 1;
      }

      let len = core::cmp::min(tags.left(), LANGUAGES.len() - idx - 1);
      for i in 0..len {
        match LANGUAGES[idx + i].language != LANGUAGES[idx].language {
          true => {
            break;
          }
          false => {}
        }

        match LANGUAGES[idx + i].tag.is_null() {
          true => {
            break;
          }
          false => {}
        }

        match tags.is_full() {
          true => {
            break;
          }
          false => {}
        }

        tags.push(LANGUAGES[idx + i].tag);
      }

      return;
    }
    Err(_) => {}
  }

  match language.len() == 3 {
    true => {
      tags.push(hb_tag_t::from_bytes_lossy(language).to_uppercase());
    }
    false => {}
  }
}

fn all_tags_from_script(script: Option<Script>, tags: &mut ThreeTags) {
  match script {
    Some(script) => {
      match new_tag_from_script(script) {
        Some(tag) => {
          if tag
            != ({
              let Ok(value) = hb_tag_t::new(b"mym2");
              value
            })
          {
            let Ok(mut tag3) = tag.to_be_bytes();
            (tag3)[3] = b'3';
            tags.push({
              let Ok(value) = hb_tag_t::new(&tag3);
              value
            });
          }

          match !tags.is_full() {
            true => {
              tags.push(tag);
            }
            false => {}
          }
        }
        None => {}
      }

      match !tags.is_full() {
        true => {
          tags.push(old_tag_from_script(script));
        }
        false => {}
      }
    }
    None => {}
  }
}

fn new_tag_from_script(script: Script) -> Option<hb_tag_t> {
  match script {
    script::BENGALI => Some({
      let Ok(value) = hb_tag_t::new(b"bng2");
      value
    }),
    script::DEVANAGARI => Some({
      let Ok(value) = hb_tag_t::new(b"dev2");
      value
    }),
    script::GUJARATI => Some({
      let Ok(value) = hb_tag_t::new(b"gjr2");
      value
    }),
    script::GURMUKHI => Some({
      let Ok(value) = hb_tag_t::new(b"gur2");
      value
    }),
    script::KANNADA => Some({
      let Ok(value) = hb_tag_t::new(b"knd2");
      value
    }),
    script::MALAYALAM => Some({
      let Ok(value) = hb_tag_t::new(b"mlm2");
      value
    }),
    script::ORIYA => Some({
      let Ok(value) = hb_tag_t::new(b"ory2");
      value
    }),
    script::TAMIL => Some({
      let Ok(value) = hb_tag_t::new(b"tml2");
      value
    }),
    script::TELUGU => Some({
      let Ok(value) = hb_tag_t::new(b"tel2");
      value
    }),
    script::MYANMAR => Some({
      let Ok(value) = hb_tag_t::new(b"mym2");
      value
    }),
    _ => None,
  }
}

fn old_tag_from_script(script: Script) -> hb_tag_t {
  match script {
    script::MATH => {
      ({
        let Ok(value) = hb_tag_t::new(b"math");
        value
      })
    }

    script::HIRAGANA => {
      ({
        let Ok(value) = hb_tag_t::new(b"kana");
        value
      })
    }

    script::LAO => {
      ({
        let Ok(value) = hb_tag_t::new(b"lao ");
        value
      })
    }
    script::YI => {
      ({
        let Ok(value) = hb_tag_t::new(b"yi  ");
        value
      })
    }

    script::NKO => {
      ({
        let Ok(value) = hb_tag_t::new(b"nko ");
        value
      })
    }

    script::VAI => {
      ({
        let Ok(value) = hb_tag_t::new(b"vai ");
        value
      })
    }

    _ => {
      ({
        let Ok(value) = hb_tag_t::from_u32(script.tag().as_u32() | 0x2000_0000);
        value
      })
    }
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]

    use super::*;
    use alloc::vec::Vec;

    fn new_tag_to_script(tag: hb_tag_t) -> Option<Script> {
        let Ok(be_bytes) = tag.to_be_bytes();

        match &be_bytes {
            b"bng2" => Some(script::BENGALI),
            b"dev2" => Some(script::DEVANAGARI),
            b"gjr2" => Some(script::GUJARATI),
            b"gur2" => Some(script::GURMUKHI),
            b"knd2" => Some(script::KANNADA),
            b"mlm2" => Some(script::MALAYALAM),
            b"ory2" => Some(script::ORIYA),
            b"tml2" => Some(script::TAMIL),
            b"tel2" => Some(script::TELUGU),
            b"mym2" => Some(script::MYANMAR),
            _ => Some(script::UNKNOWN),
        }
    }

    fn old_tag_to_script(tag: hb_tag_t) -> Option<Script> {
        match tag == hb_tag_t::default_script() { true => {
            return None;
        }, false => {} }

        let Ok(mut bytes) = tag.to_be_bytes();





        match ((bytes))[2] == b' ' { true => {
            ((bytes))[2] = ((bytes))[1];
        }, false => {} }
        match ((bytes))[3] == b' ' { true => {
            ((bytes))[3] = ((bytes))[2];
        }, false => {} }


        ((bytes))[0] = ((bytes))[0].to_ascii_uppercase();

        Some(Script(({ let Ok(value) = hb_tag_t::new(&bytes); value })))
    }

    fn tag_to_script(tag: hb_tag_t) -> Option<Script> {
        let Ok(bytes) = tag.to_be_bytes();
        match ((bytes))[3] == b'2' || ((bytes))[3] == b'3' { true => {
            let mut tag2 = bytes;
            ((tag2))[3] = b'2';
            return new_tag_to_script(({ let Ok(value) = hb_tag_t::new(&tag2); value }));
        }, false => {} }

        old_tag_to_script(tag)
    }

    fn test_simple_tags(tag: &str, script: Script) {
        let tag = hb_tag_t::from_bytes_lossy(tag.as_bytes());

        let (scripts, _) = tags_from_script_and_language(Some(script), None);
        match !scripts.is_empty() { true => {
            assert_eq!(tag, scripts[0]);
        }, false => {
            assert_eq!(tag, hb_tag_t::default_script());
        } }

        assert_eq!(tag_to_script(tag), Some(script));
    }

    #[test]
    fn tag_to_uppercase() {
        assert_eq!(({ let Ok(value) = hb_tag_t::new(b"abcd"); value }).to_uppercase(), ({ let Ok(value) = hb_tag_t::new(b"ABCD"); value }));
        assert_eq!(({ let Ok(value) = hb_tag_t::new(b"abc "); value }).to_uppercase(), ({ let Ok(value) = hb_tag_t::new(b"ABC "); value }));
        assert_eq!(({ let Ok(value) = hb_tag_t::new(b"ABCD"); value }).to_uppercase(), ({ let Ok(value) = hb_tag_t::new(b"ABCD"); value }));
    }

    #[test]
    fn tag_to_lowercase() {
        assert_eq!(({ let Ok(value) = hb_tag_t::new(b"abcd"); value }).to_lowercase(), ({ let Ok(value) = hb_tag_t::new(b"abcd"); value }));
        assert_eq!(({ let Ok(value) = hb_tag_t::new(b"abc "); value }).to_lowercase(), ({ let Ok(value) = hb_tag_t::new(b"abc "); value }));
        assert_eq!(({ let Ok(value) = hb_tag_t::new(b"ABCD"); value }).to_lowercase(), ({ let Ok(value) = hb_tag_t::new(b"abcd"); value }));
    }

    #[test]
    fn script_degenerate() {
        assert_eq!(({ let Ok(value) = hb_tag_t::new(b"DFLT"); value }), hb_tag_t::default_script());


        test_simple_tags("kana", script::KATAKANA);

        let (scripts, _) = tags_from_script_and_language(Some(script::HIRAGANA), None);
        assert_eq!(scripts.as_slice(), &[({ let Ok(value) = hb_tag_t::new(b"kana"); value })]);


        assert_eq!(tag_to_script(({ let Ok(value) = hb_tag_t::new(b"be  "); value })), Script::from_iso15924_tag(({ let Ok(value) = hb_tag_t::new(b"Beee"); value })));
    }

    #[test]
    fn script_simple() {

        test_simple_tags("wwyz", Script::from_iso15924_tag(({ let Ok(value) = hb_tag_t::new(b"wWyZ"); value })).unwrap());


        test_simple_tags("zyyy", script::COMMON);
        test_simple_tags("zinh", script::INHERITED);
        test_simple_tags("zzzz", script::UNKNOWN);

        test_simple_tags("arab", script::ARABIC);
        test_simple_tags("copt", script::COPTIC);
        test_simple_tags("kana", script::KATAKANA);
        test_simple_tags("latn", script::LATIN);


        test_simple_tags("lao ", script::LAO);
        test_simple_tags("yi  ", script::YI);

        test_simple_tags("nko ", script::NKO);

        test_simple_tags("vai ", script::VAI);




        test_simple_tags("mtei", script::MEETEI_MAYEK);

        test_simple_tags("mand", script::MANDAIC);
    }

    macro_rules! test_script_from_language {
        ($name:ident, $tag:expr, $lang:expr, $script:expr) => {
            #[test]
            fn $name() {
                let tag = hb_tag_t::from_bytes_lossy($tag.as_bytes());
                let (scripts, _) =
                    tags_from_script_and_language($script, Language::new($lang).as_ref());
                if !scripts.is_empty() {
                    assert_eq!(scripts.as_slice(), &[tag]);
                }
            }
        };
    }

    test_script_from_language!(script_from_language_01, "", "", None);
    test_script_from_language!(script_from_language_02, "", "en", None);
    test_script_from_language!(script_from_language_03, "copt", "en", Some(script::COPTIC));
    test_script_from_language!(script_from_language_04, "", "x-hbsc", None);
    test_script_from_language!(script_from_language_05, "copt", "x-hbsc", Some(script::COPTIC));
    test_script_from_language!(script_from_language_06, "abc ", "x-hbscabc", None);
    test_script_from_language!(script_from_language_07, "deva", "x-hbscdeva", None);
    test_script_from_language!(script_from_language_08, "dev2", "x-hbscdev2", None);
    test_script_from_language!(script_from_language_09, "dev3", "x-hbscdev3", None);
    test_script_from_language!(script_from_language_10, "copt", "x-hbotpap0-hbsccopt", None);
    test_script_from_language!(script_from_language_11, "", "en-x-hbsc", None);
    test_script_from_language!(script_from_language_12, "copt", "en-x-hbsc", Some(script::COPTIC));
    test_script_from_language!(script_from_language_13, "abc ", "en-x-hbscabc", None);
    test_script_from_language!(script_from_language_14, "deva", "en-x-hbscdeva", None);
    test_script_from_language!(script_from_language_15, "dev2", "en-x-hbscdev2", None);
    test_script_from_language!(script_from_language_16, "dev3", "en-x-hbscdev3", None);
    test_script_from_language!(script_from_language_17, "copt", "en-x-hbotpap0-hbsccopt", None);

    #[test]
    fn script_indic() {
        fn check(tag1: &str, tag2: &str, tag3: &str, script: Script) {
            let tag1 = hb_tag_t::from_bytes_lossy(tag1.as_bytes());
            let tag2 = hb_tag_t::from_bytes_lossy(tag2.as_bytes());
            let tag3 = hb_tag_t::from_bytes_lossy(tag3.as_bytes());

            let (scripts, _) = tags_from_script_and_language(Some(script), None);
            assert_eq!(scripts.as_slice(), &[tag1, tag2, tag3]);
            assert_eq!(tag_to_script(tag1), Some(script));
            assert_eq!(tag_to_script(tag2), Some(script));
            assert_eq!(tag_to_script(tag3), Some(script));
        }

        check("bng3", "bng2", "beng", script::BENGALI);
        check("dev3", "dev2", "deva", script::DEVANAGARI);
        check("gjr3", "gjr2", "gujr", script::GUJARATI);
        check("gur3", "gur2", "guru", script::GURMUKHI);
        check("knd3", "knd2", "knda", script::KANNADA);
        check("mlm3", "mlm2", "mlym", script::MALAYALAM);
        check("ory3", "ory2", "orya", script::ORIYA);
        check("tml3", "tml2", "taml", script::TAMIL);
        check("tel3", "tel2", "telu", script::TELUGU);
    }


    macro_rules! test_tag_from_language {
        ($name:ident, $tag:expr, $lang:expr) => {
            #[test]
            fn $name() {
                let tag = hb_tag_t::from_bytes_lossy($tag.as_bytes());
                let (_, languages) = tags_from_script_and_language(
                    None,
                    Language::new(&$lang.to_lowercase()).as_ref(),
                );
                if !languages.is_empty() {
                    assert_eq!(languages[0], tag);
                }
            }
        };
    }

    test_tag_from_language!(tag_from_language_dflt, "dflt", "");
    test_tag_from_language!(tag_from_language_ALT, "ALT", "alt");
    test_tag_from_language!(tag_from_language_ARA, "ARA", "ar");
    test_tag_from_language!(tag_from_language_AZE, "AZE", "az");
    test_tag_from_language!(tag_from_language_az_ir, "AZE", "az-ir");
    test_tag_from_language!(tag_from_language_az_az, "AZE", "az-az");
    test_tag_from_language!(tag_from_language_ENG, "ENG", "en");
    test_tag_from_language!(tag_from_language_en_US, "ENG", "en_US");
    test_tag_from_language!(tag_from_language_CJA, "CJA", "cja");
    test_tag_from_language!(tag_from_language_CJM, "CJM", "cjm");
    test_tag_from_language!(tag_from_language_ENV, "EVN", "eve");
    test_tag_from_language!(tag_from_language_HAL, "HAL", "cfm");
    test_tag_from_language!(tag_from_language_flm, "HAL", "flm");
    test_tag_from_language!(tag_from_language_hy, "HYE0", "hy");
    test_tag_from_language!(tag_from_language_hyw, "HYE", "hyw");
    test_tag_from_language!(tag_from_language_bgr, "QIN", "bgr");
    test_tag_from_language!(tag_from_language_cbl, "QIN", "cbl");
    test_tag_from_language!(tag_from_language_cka, "QIN", "cka");
    test_tag_from_language!(tag_from_language_cmr, "QIN", "cmr");
    test_tag_from_language!(tag_from_language_cnb, "QIN", "cnb");
    test_tag_from_language!(tag_from_language_cnh, "QIN", "cnh");
    test_tag_from_language!(tag_from_language_cnk, "QIN", "cnk");
    test_tag_from_language!(tag_from_language_cnw, "QIN", "cnw");
    test_tag_from_language!(tag_from_language_csh, "QIN", "csh");
    test_tag_from_language!(tag_from_language_csy, "QIN", "csy");
    test_tag_from_language!(tag_from_language_ctd, "QIN", "ctd");
    test_tag_from_language!(tag_from_language_czt, "QIN", "czt");
    test_tag_from_language!(tag_from_language_dao, "QIN", "dao");
    test_tag_from_language!(tag_from_language_htl, "QIN", "hlt");
    test_tag_from_language!(tag_from_language_mrh, "QIN", "mrh");
    test_tag_from_language!(tag_from_language_pck, "QIN", "pck");
    test_tag_from_language!(tag_from_language_sez, "QIN", "sez");
    test_tag_from_language!(tag_from_language_tcp, "QIN", "tcp");
    test_tag_from_language!(tag_from_language_tcz, "QIN", "tcz");
    test_tag_from_language!(tag_from_language_yos, "QIN", "yos");
    test_tag_from_language!(tag_from_language_zom, "QIN", "zom");
    test_tag_from_language!(tag_from_language_FAR, "FAR", "fa");
    test_tag_from_language!(tag_from_language_fa_IR, "FAR", "fa_IR");
    test_tag_from_language!(tag_from_language_man, "MNK", "man");
    test_tag_from_language!(tag_from_language_SWA, "SWA", "aii");
    test_tag_from_language!(tag_from_language_SYR, "SYR", "syr");
    test_tag_from_language!(tag_from_language_amw, "SYR", "amw");
    test_tag_from_language!(tag_from_language_cld, "SYR", "cld");
    test_tag_from_language!(tag_from_language_syc, "SYR", "syc");
    test_tag_from_language!(tag_from_language_TUA, "TUA", "tru");
    test_tag_from_language!(tag_from_language_zh, "ZHS", "zh");
    test_tag_from_language!(tag_from_language_zh_cn, "ZHS", "zh-cn");
    test_tag_from_language!(tag_from_language_zh_sg, "ZHS", "zh-sg");
    test_tag_from_language!(tag_from_language_zh_mo, "ZHTM", "zh-mo");
    test_tag_from_language!(tag_from_language_zh_hant_mo, "ZHTM", "zh-hant-mo");
    test_tag_from_language!(tag_from_language_zh_hans_mo, "ZHS", "zh-hans-mo");
    test_tag_from_language!(tag_from_language_ZHH, "ZHH", "zh-HK");
    test_tag_from_language!(tag_from_language_zh_HanT_hK, "ZHH", "zH-HanT-hK");
    test_tag_from_language!(tag_from_language_zh_HanS_hK, "ZHS", "zH-HanS-hK");
    test_tag_from_language!(tag_from_language_zh_tw, "ZHT", "zh-tw");
    test_tag_from_language!(tag_from_language_ZHS, "ZHS", "zh-Hans");
    test_tag_from_language!(tag_from_language_ZHT, "ZHT", "zh-Hant");
    test_tag_from_language!(tag_from_language_zh_xx, "ZHS", "zh-xx");
    test_tag_from_language!(tag_from_language_zh_Hans_TW, "ZHS", "zh-Hans-TW");
    test_tag_from_language!(tag_from_language_yue, "ZHH", "yue");
    test_tag_from_language!(tag_from_language_yue_Hant, "ZHH", "yue-Hant");
    test_tag_from_language!(tag_from_language_yue_Hans, "ZHS", "yue-Hans");
    test_tag_from_language!(tag_from_language_ABC, "ABC", "abc");
    test_tag_from_language!(tag_from_language_ABCD, "ABCD", "x-hbotabcd");
    test_tag_from_language!(tag_from_language_asdf_asdf_wer_x_hbotabc_zxc, "ABC", "asdf-asdf-wer-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_asdf_asdf_wer_x_hbotabc, "ABC", "asdf-asdf-wer-x-hbotabc");
    test_tag_from_language!(tag_from_language_asdf_asdf_wer_x_hbotabcd, "ABCD", "asdf-asdf-wer-x-hbotabcd");
    test_tag_from_language!(tag_from_language_asdf_asdf_wer_x_hbot_zxc, "dflt", "asdf-asdf-wer-x-hbot-zxc");
    test_tag_from_language!(tag_from_language_xy, "dflt", "xy");
    test_tag_from_language!(tag_from_language_xyz, "XYZ", "xyz");
    test_tag_from_language!(tag_from_language_xyz_qw, "XYZ", "xyz-qw");


    test_tag_from_language!(tag_from_language__fonipa, "IPPH", "-fonipa");


    test_tag_from_language!(tag_from_language_en_fonipax, "ENG", "en-fonipax");
    test_tag_from_language!(tag_from_language_en_x_fonipa, "ENG", "en-x-fonipa");
    test_tag_from_language!(tag_from_language_en_a_fonipa, "ENG", "en-a-fonipa");
    test_tag_from_language!(tag_from_language_en_a_qwe_b_fonipa, "ENG", "en-a-qwe-b-fonipa");


    test_tag_from_language!(tag_from_language_en_fonipa, "IPPH", "en-fonipa");
    test_tag_from_language!(tag_from_language_en_fonipax_fonipa, "IPPH", "en-fonipax-fonipa");
    test_tag_from_language!(tag_from_language_rm_ch_fonipa_sursilv_x_foobar, "IPPH", "rm-CH-fonipa-sursilv-x-foobar");
    test_tag_from_language!(tag_from_language_IPPH, "IPPH", "und-fonipa");
    test_tag_from_language!(tag_from_language_zh_fonipa, "IPPH", "zh-fonipa");


    test_tag_from_language!(tag_from_language_en_fonnapa, "APPH", "en-fonnapa");
    test_tag_from_language!(tag_from_language_chr_fonnapa, "APPH", "chr-fonnapa");
    test_tag_from_language!(tag_from_language_APPH, "APPH", "und-fonnapa");


    test_tag_from_language!(tag_from_language_ka_geok, "KGE", "ka-Geok");
    test_tag_from_language!(tag_from_language_KGE, "KGE", "und-Geok");


    test_tag_from_language!(tag_from_language_IRT, "IRT", "ga-Latg");


    test_tag_from_language!(tag_from_language_MOL, "MOL", "ro-MD");


    test_tag_from_language!(tag_from_language_PGR, "PGR", "el-polyton");
    test_tag_from_language!(tag_from_language_el_CY_polyton, "PGR", "el-CY-polyton");


    test_tag_from_language!(tag_from_language_aii_Syre, "SYRE", "aii-Syre");
    test_tag_from_language!(tag_from_language_de_Syre, "SYRE", "de-Syre");
    test_tag_from_language!(tag_from_language_syr_Syre, "SYRE", "syr-Syre");
    test_tag_from_language!(tag_from_language_und_Syre, "SYRE", "und-Syre");


    test_tag_from_language!(tag_from_language_aii_Syrj, "SYRJ", "aii-Syrj");
    test_tag_from_language!(tag_from_language_de_Syrj, "SYRJ", "de-Syrj");
    test_tag_from_language!(tag_from_language_syr_Syrj, "SYRJ", "syr-Syrj");
    test_tag_from_language!(tag_from_language_SYRJ, "SYRJ", "und-Syrj");


    test_tag_from_language!(tag_from_language_aii_Syrn, "SYRN", "aii-Syrn");
    test_tag_from_language!(tag_from_language_de_Syrn, "SYRN", "de-Syrn");
    test_tag_from_language!(tag_from_language_syr_Syrn, "SYRN", "syr-Syrn");
    test_tag_from_language!(tag_from_language_SYRN, "SYRN", "und-Syrn");


    test_tag_from_language!(tag_from_language_fa_x_hbotabc_zxc, "ABC", "fa-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_fa_ir_x_hbotabc_zxc, "ABC", "fa-ir-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_zh_x_hbotabc_zxc, "ABC", "zh-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_zh_cn_x_hbotabc_zxc, "ABC", "zh-cn-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_zh_xy_x_hbotabc_zxc, "ABC", "zh-xy-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_xyz_xy_x_hbotabc_zxc, "ABC", "xyz-xy-x-hbotabc-zxc");


    test_tag_from_language!(tag_from_language_ar_aao, "ARA", "ar-aao");
    test_tag_from_language!(tag_from_language_art_lojban, "JBO", "art-lojban");
    test_tag_from_language!(tag_from_language_kok_gom, "KOK", "kok-gom");
    test_tag_from_language!(tag_from_language_i_lux, "LTZ", "i-lux");
    test_tag_from_language!(tag_from_language_drh, "MNG", "drh");
    test_tag_from_language!(tag_from_language_ar_ary1, "MOR", "ar-ary");
    test_tag_from_language!(tag_from_language_ar_ary_DZ, "MOR", "ar-ary-DZ");
    test_tag_from_language!(tag_from_language_no_bok, "NOR", "no-bok");
    test_tag_from_language!(tag_from_language_no_nyn, "NYN", "no-nyn");
    test_tag_from_language!(tag_from_language_i_hak, "ZHS", "i-hak");
    test_tag_from_language!(tag_from_language_zh_guoyu, "ZHS", "zh-guoyu");
    test_tag_from_language!(tag_from_language_zh_min, "ZHS", "zh-min");
    test_tag_from_language!(tag_from_language_zh_min_nan, "ZHS", "zh-min-nan");
    test_tag_from_language!(tag_from_language_zh_xiang, "ZHS", "zh-xiang");


    test_tag_from_language!(tag_from_language_als, "SQI", "als");
    test_tag_from_language!(tag_from_language_far, "dflt", "far");


    test_tag_from_language!(tag_from_language_ar_001, "ARA", "ar-001");


    test_tag_from_language!(tag_from_language_invalid, "TRK", "tr@foo=bar");

    macro_rules! test_tags {
        ($name:ident, $script:expr, $lang:expr, $scripts:expr, $langs:expr) => {
            #[test]
            fn $name() {
                let (scripts, languages) =
                    tags_from_script_and_language($script, Language::new($lang).as_ref());

                let exp_scripts: Vec<hb_tag_t> = $scripts.iter().map(|v| hb_tag_t::from_bytes_lossy(*v)).collect();
                let exp_langs: Vec<hb_tag_t> = $langs.iter().map(|v| hb_tag_t::from_bytes_lossy(*v)).collect();

                assert_eq!(exp_scripts, scripts.as_slice());
                assert_eq!(exp_langs, languages.as_slice());
            }
        };
    }

    test_tags!(tag_full_en, None, "en", &[], &[b"ENG"]);
    test_tags!(tag_full_en_x_hbscdflt, None, "en-x-hbscdflt", &[b"DFLT"], &[b"ENG"]);
    test_tags!(tag_full_en_latin, Some(script::LATIN), "en", &[b"latn"], &[b"ENG"]);
    test_tags!(tag_full_und_fonnapa, None, "und-fonnapa", &[], &[b"APPH"]);
    test_tags!(tag_full_en_fonnapa, None, "en-fonnapa", &[], &[b"APPH"]);
    test_tags!(tag_full_x_hbot1234_hbsc5678, None, "x-hbot1234-hbsc5678", &[b"5678"], &[b"1234"]);
    test_tags!(tag_full_x_hbsc5678_hbot1234, None, "x-hbsc5678-hbot1234", &[b"5678"], &[b"1234"]);
    test_tags!(tag_full_ml, Some(script::MALAYALAM), "ml", &[b"mlm3", b"mlm2", b"mlym"], &[b"MAL", b"MLR"]);
    test_tags!(tag_full_xyz, None, "xyz", &[], &[b"XYZ"]);
    test_tags!(tag_full_xy, None, "xy", &[], &[]);
}
