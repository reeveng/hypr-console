use console_core_never::Never;
#[rustfmt::skip]
#[allow(clippy::identity_op, reason = "the generated tables write every offset the generator computed, zero included")]
pub(crate) mod generated;
mod constants;

use crate::keysym::generated::CHAR_TO_BESPOKE_IDX;
use crate::keysym::generated::DATAS;
use crate::keysym::generated::KEYSYM_TO_CHAR;
use crate::keysym::generated::KEYSYM_TO_IDX;
use crate::keysym::generated::KEYSYM_TO_UPPER_KEYSYM;
use crate::keysym::generated::KEYSYM32_TO_CHAR;
use crate::keysym::generated::LONGEST_NAME;
use crate::keysym::generated::LOWER_NAME_TO_IDX;
use crate::keysym::generated::NAMES;
use crate::keysym::hidden::Keysym;
use crate::syms;
use console_core_number_conversion::FromBytes;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use console_core_perfect_hash::Hash as PhfHash;
use console_core_perfect_hash::Map as PhfMap;
use generated::NAME_TO_IDX;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;

pub(crate) mod hidden {
  #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
  pub struct Keysym(pub u32);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Lettercase {
  Lower,
  Upper,
  Uncased,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Region {
  Keypad,
  Main,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Encoding {
  Unicode,
  Legacy,
}

const HAS_CHAR: u8 = 1 << 0;
const KEYSYM_IS_CHAR: u8 = 1 << 1;
const IS_LOWER: u8 = 1 << 2;
const IS_UPPER: u8 = 1 << 3;
const IS_SECONDARY_IDX: u8 = 1 << 4;
const IS_DEPRECATED: u8 = 1 << 5;

const LONGEST_SPELLING: u32 = LONGEST_NAME + 1;
const _: () = assert!(LONGEST_NAME >= 10);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct KeysymData {
  keysym_or_definitive_idx: u32,
  name_start: u16,
  name_len: u8,
  flags: u8,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct KeysymChar {
  keysym: u16,
  char: char,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Keysym32Char {
  keysym: u32,
  char: char,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct KeysymCaseMapping {
  keysym: u16,
  other: u32,
}

impl KeysymData {
  fn name(&self) -> Result<Option<&'static str>, Never> {
    let start = u32::from(self.name_start);
    let end = start.saturating_add(u32::from(self.name_len));
    let Ok(from) = index(start);
    let Ok(to) = index(end);

    Ok(NAMES.get(from..to))
  }

  fn definitive(&'static self) -> Result<Option<&'static KeysymData>, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.flags & IS_SECONDARY_IDX != 0, ()) {
        ::core::option::Option::None => Some(self),
        ::core::option::Option::Some(()) => {
          let Ok(at) = index(self.keysym_or_definitive_idx);

          DATAS.get(at)
        }
      },
    )
  }
}

fn looked_up<K>(map: &PhfMap<K, u16>, key: &K) -> Result<Option<&'static KeysymData>, Never>
where
  K: ?Sized + PhfHash,
{
  let Ok(found) = map.candidate(key);

  Ok(match found {
    None => None,
    Some(&position) => {
      let Ok(at) = index(position);

      DATAS.get(at)
    }
  })
}

impl Keysym {
  pub fn from_char(char: char) -> Result<Self, Never> {
    let c = u32::from(char);

    match c {
      0x20..=0x7e | 0xa0..=0xff => return Ok(Self(c)),
      0x08..=0x0b | 0x0d | 0x1b => return Ok(Self(c | 0xff00)),
      0 => return Ok(syms::NoSymbol),
      0x7f => return Ok(syms::Delete),
      _ => {}
    }

    let Ok(written) = Self::written_as(char);

    Ok(match written {
      Some(sym) => sym,
      None => Self(0x01_00_00_00 | c),
    })
  }

  fn written_as(char: char) -> Result<Option<Self>, Never> {
    let Ok(data) = looked_up(&CHAR_TO_BESPOKE_IDX, &char);

    Ok(match data {
      None => None,
      Some(found) => match ::core::primitive::bool::then_some(
        found.flags & (HAS_CHAR | IS_DEPRECATED) == HAS_CHAR,
        (),
      ) {
        ::core::option::Option::None => None,
        ::core::option::Option::Some(()) => {
          let Ok(spelled) = Self::spelled_by(found, char);

          spelled
        }
      },
    })
  }

  fn spelled_by(found: &KeysymData, char: char) -> Result<Option<Self>, Never> {
    let sym = Self(found.keysym_or_definitive_idx);
    let is_the_char = found.flags & KEYSYM_IS_CHAR != 0 && sym.0 == u32::from(char);

    Ok(match ::core::primitive::bool::then_some(is_the_char, ()) {
      ::core::option::Option::Some(()) => Some(sym),
      ::core::option::Option::None => {
        let Ok(charted) = sym.charted();

        match ::core::primitive::bool::then_some(charted == Some(char), ()) {
          ::core::option::Option::Some(()) => Some(sym),
          ::core::option::Option::None => None,
        }
      }
    })
  }

  fn charted(self) -> Result<Option<char>, Never> {
    let held = self.0;

    Ok(
      match ::core::primitive::bool::then_some(held > 0xffff, ()) {
        ::core::option::Option::Some(()) => {
          match KEYSYM32_TO_CHAR.binary_search_by(|k| k.keysym.cmp(&held)) {
            Ok(at) => KEYSYM32_TO_CHAR.get(at).map(|k| k.char),
            Err(_not_in_the_table) => None,
          }
        }
        ::core::option::Option::None => {
          let Ok(narrow) = fitted::<u32, u16>(held);

          match KEYSYM_TO_CHAR.binary_search_by(|k| k.keysym.cmp(&narrow)) {
            Ok(at) => KEYSYM_TO_CHAR.get(at).map(|k| k.char),
            Err(_not_in_the_table) => None,
          }
        }
      },
    )
  }

  fn data(self) -> Result<Option<&'static KeysymData>, Never> {
    let Ok(data) = looked_up(&KEYSYM_TO_IDX, &self.0);

    Ok(data.filter(|found| found.keysym_or_definitive_idx == self.0))
  }

  pub fn name(self) -> Result<Option<&'static str>, Never> {
    let Ok(data) = self.data();

    Ok(match data {
      None => None,
      Some(found) => {
        let Ok(name) = found.name();

        name
      }
    })
  }

  pub fn char(self) -> Result<Option<char>, Never> {
    let c = self.0;

    match c {
      0x20..=0x7e | 0xa0..=0xff => return Ok(char::from_u32(c)),
      _ => {}
    }

    let Ok(function) = self.function_char();

    match function {
      Some(written) => {
        return Ok(Some(written));
      }
      None => {}
    }

    let Ok(encoding) = self.encoding();

    match encoding {
      Encoding::Unicode => self.unicode_char(),
      Encoding::Legacy => self.charted(),
    }
  }

  fn function_char(self) -> Result<Option<char>, Never> {
    let low = self.0 & 0xff;

    Ok(
      match ::core::primitive::bool::then_some(self.0.wrapping_shr(8) == 0xff, ()) {
        ::core::option::Option::None => None,
        ::core::option::Option::Some(()) => match low {
          0x80 => Some(' '),
          0x08..=0x0b | 0x0d | 0x1b | 0x89 | 0x8d | 0xaa..=0xb9 | 0xbd | 0xff => {
            char::from_u32(low & 0x7f)
          }
          _ => None,
        },
      },
    )
  }

  #[allow(
    clippy::should_implement_trait,
    reason = "symbol parsing retains the keymap API and its infallible error contract"
  )]
  pub fn from_str(name: &(impl AsRef<[u8]> + ?Sized)) -> Result<Option<Self>, Never> {
    let Ok(from_str) = from_str::<false>(name.as_ref());
    Ok(from_str)
  }

  pub fn to_uppercase(self) -> Result<Self, Never> {
    let held = self.0;

    match ::core::primitive::bool::then_some(held <= 0xffff, ()) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some(held < 0x80, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(byte) = fitted::<u32, u8>(held);

            Ok(Keysym(u32::from(byte.to_ascii_uppercase())))
          }
          ::core::option::Option::None => self.legacy_uppercase(),
        }
      }
      ::core::option::Option::None => {
        let Ok(encoding) = self.encoding();

        match encoding {
          Encoding::Unicode => self.unicode_uppercase(),
          Encoding::Legacy => Ok(self),
        }
      }
    }
  }

  fn legacy_uppercase(self) -> Result<Self, Never> {
    let Ok(mapping) = KEYSYM_TO_UPPER_KEYSYM.candidate(&self.0);
    let exact = mapping.filter(|found| u32::from(found.keysym) == self.0);

    Ok(match exact {
      Some(found) => Self(found.other),
      None => self,
    })
  }

  #[cold]
  fn unicode_uppercase(self) -> Result<Self, Never> {
    let Ok(unicode_char) = self.unicode_char();

    Ok(match unicode_char {
      None => self,
      Some(c) => {
        let mut cased = c.to_uppercase();

        match (cased.next(), cased.next()) {
          (Some(only), None) => {
            let code = u32::from(only);

            match ::core::primitive::bool::then_some(code < 0x100, ()) {
              ::core::option::Option::Some(()) => Keysym(code),
              ::core::option::Option::None => Keysym(0x01_00_00_00 | code),
            }
          }
          (None, _) | (Some(_), Some(_)) => self,
        }
      }
    })
  }

  fn unicode_char(self) -> Result<Option<char>, Never> {
    Ok(char::from_u32(self.0 & 0xff_ff_ff))
  }

  fn encoding(self) -> Result<Encoding, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.0.wrapping_shr(24) == 0x01, ()) {
        ::core::option::Option::Some(()) => Encoding::Unicode,
        ::core::option::Option::None => Encoding::Legacy,
      },
    )
  }

  pub fn lettercase(self) -> Result<Lettercase, Never> {
    let Ok(encoding) = self.encoding();

    let (lower, upper) = match encoding {
      Encoding::Unicode => {
        let Ok(unicode_char) = self.unicode_char();

        (
          unicode_char.is_some_and(char::is_lowercase),
          unicode_char.is_some_and(char::is_uppercase),
        )
      }
      Encoding::Legacy => {
        let Ok(data) = self.data();

        (
          data.is_some_and(|found| found.flags & IS_LOWER != 0),
          data.is_some_and(|found| found.flags & IS_UPPER != 0),
        )
      }
    };

    Ok(match (lower, upper) {
      (true, false) => Lettercase::Lower,
      (false, true) => Lettercase::Upper,
      (false, false) | (true, true) => Lettercase::Uncased,
    })
  }

  pub fn region(self) -> Result<Region, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self >= syms::KP_Space && self <= syms::KP_Equal,
        (),
      ) {
        ::core::option::Option::Some(()) => Region::Keypad,
        ::core::option::Option::None => Region::Main,
      },
    )
  }
}

fn from_str<const CASE_INSENSITIVE: bool>(s: &[u8]) -> Result<Option<Keysym>, Never> {
  let Ok(length) = fitted::<_, u32>(s.len());

  match ::core::primitive::bool::then_some(length > LONGEST_SPELLING, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let Ok(named) = by_name::<CASE_INSENSITIVE>(s);

  match named {
    Some(sym) => {
      return Ok(Some(sym));
    }
    None => {}
  }

  let Ok(unicode) = after::<CASE_INSENSITIVE>(s, "U");

  match unicode {
    Some(digits) => {
      return match u32::from_bytes_hex(digits) {
        Some(cp) => keysym_from_cp(cp),
        None => Ok(None),
      };
    }
    None => {}
  }

  let Ok(hexadecimal) = after::<CASE_INSENSITIVE>(s, "0x");

  match hexadecimal.and_then(u32::from_bytes_hex) {
    Some(cp) => {
      return Ok(Some(Keysym(cp)));
    }
    None => {}
  }

  let Ok(xf86) = after::<CASE_INSENSITIVE>(s, "XF86_");

  match xf86 {
    Some(rest @ [_, ..]) => respelled::<CASE_INSENSITIVE>(rest),
    Some([]) | None => Ok(None),
  }
}

fn by_name<const CASE_INSENSITIVE: bool>(spelled: &[u8]) -> Result<Option<Keysym>, Never> {
  let lowered;
  let (asked, map) = match ::core::primitive::bool::then_some(CASE_INSENSITIVE, ()) {
    ::core::option::Option::Some(()) => {
      lowered = spelled.to_ascii_lowercase();
      (lowered.as_slice(), &LOWER_NAME_TO_IDX)
    }
    ::core::option::Option::None => (spelled, &NAME_TO_IDX),
  };
  let Ok(data) = looked_up(map, asked);

  let named = match data {
    None => None,
    Some(found) => {
      let Ok(name) = found.name();

      match name.map(str::as_bytes) {
        None => None,
        Some(written) => {
          let good = match ::core::primitive::bool::then_some(CASE_INSENSITIVE, ()) {
            ::core::option::Option::Some(()) => written.eq_ignore_ascii_case(spelled),
            ::core::option::Option::None => written == spelled,
          };

          match ::core::primitive::bool::then_some(good, ()) {
            ::core::option::Option::Some(()) => Some(found),
            ::core::option::Option::None => None,
          }
        }
      }
    }
  };

  Ok(match named {
    None => None,
    Some(found) => {
      let Ok(definitive) = found.definitive();

      definitive.map(|held| Keysym(held.keysym_or_definitive_idx))
    }
  })
}

fn after<'a, const CASE_INSENSITIVE: bool>(
  spelled: &'a [u8],
  prefix: &'static str,
) -> Result<Option<&'a [u8]>, Never> {
  let written = prefix.as_bytes();

  Ok(match spelled.split_at_checked(written.len()) {
    None => None,
    Some((head, rest)) => {
      let starts = match ::core::primitive::bool::then_some(CASE_INSENSITIVE, ()) {
        ::core::option::Option::Some(()) => head.eq_ignore_ascii_case(written),
        ::core::option::Option::None => head == written,
      };

      match ::core::primitive::bool::then_some(starts, ()) {
        ::core::option::Option::Some(()) => Some(rest),
        ::core::option::Option::None => None,
      }
    }
  })
}

fn respelled<const CASE_INSENSITIVE: bool>(rest: &[u8]) -> Result<Option<Keysym>, Never> {
  let Ok(underscores) = fitted::<_, u32>(rest.iter().take_while(|&&c| c == b'_').count());

  Ok((0..=underscores).find_map(|skipped| {
    let Ok(at) = index(skipped);

    match rest.get(at..) {
      Some(tail @ [_, ..]) => {
        let candidate = [b"XF86".as_slice(), tail].concat();
        let Ok(named) = by_name::<CASE_INSENSITIVE>(&candidate);

        named
      }
      Some([]) | None => None,
    }
  }))
}

fn keysym_from_cp(cp: u32) -> Result<Option<Keysym>, Never> {
  Ok(match cp {
    0x000100..=0x10ffff => Some(Keysym(cp | 0x01_00_00_00)),
    _ => char::from_u32(cp).map(|c| {
      let Ok(sym) = Keysym::from_char(c);
      sym
    }),
  })
}

impl Debug for Keysym {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let Ok(named) = self.name();

    match named {
      Some(name) => {
        return f.write_str(name);
      }
      None => {}
    }

    let Ok(written) = self.char();

    match written {
      Some(char) => {
        return Debug::fmt(&char, f);
      }
      None => {}
    }

    write!(f, "0x{:08x}", self.0)
  }
}

impl Display for Keysym {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let Ok(named) = self.name();

    match named {
      Some(name) => {
        return f.write_str(name);
      }
      None => {}
    }

    let Ok(encoding) = self.encoding();

    match encoding {
      Encoding::Unicode => {
        let d = self.0 & 0xff_ff_ff;
        let Ok(from_cp) = keysym_from_cp(d);

        match ::core::primitive::bool::then_some(from_cp == Some(*self), ()) {
          ::core::option::Option::Some(()) => {
            return write!(f, "U{d:x}");
          }
          ::core::option::Option::None => {}
        }
      }
      Encoding::Legacy => {}
    }

    write!(f, "0x{:08x}", self.0)
  }
}
