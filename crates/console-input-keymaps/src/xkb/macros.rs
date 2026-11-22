#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  allow(
    explicit080_macro_budget,
    reason = "a keyed mask is a word of bits whose every bit has a keymap's name, and the constants, the reading from a name, the writing back and the operators are written once from one table of bits and names rather than once per mask, where every copy of the table would drift from the others"
  )
)]
macro_rules! keyed_bitfield {
    (
        $name:ident:
        $($idx:expr => $ident:ident => $s:ident $(| $so:ident)*,)*
    ) => {
        const _: () = {
            use std::fmt::{self, Display, Formatter, Debug};
            use std::ops::{BitOr, Not, BitAnd, BitOrAssign};
            use crate::xkb::meaning::Meaning;
            use console_core_never::Never;

            impl $name {
                pub(crate) const NONE: Self = Self(0);
                pub(crate) const ALL: Self = Self($((1 << $idx))|*);
                $(
                    pub(crate) const $ident: Self = Self(1 << $idx);
                )*

                pub(crate) fn from_meaning(meaning: Meaning) -> Result<Option<Self>, Never> {
                    let named = [
                        (Meaning::None, Self::NONE),
                        (Meaning::All, Self::ALL),
                        $(
                            (Meaning::$s, Self::$ident),
                            $((Meaning::$so, Self::$ident),)*
                        )*
                    ];

                    Ok(named.into_iter().find(|&(said, _)| said == meaning).map(|(_, mask)| mask))
                }
            }

            impl Display for $name {
                fn fmt(&self, f: &mut Formatter) -> fmt::Result {
                    let held = self.0;
                    let names = [$((Self::$ident, stringify!($s)),)*];
                    let mut set = names.into_iter().filter(|&(mask, _)| held & mask.0 != 0).map(|(_, name)| name);

                    match held {
                        0 => f.write_str("none"),
                        _ => match set.next() {
                            None => Ok(()),
                            Some(first) => {
                                f.write_str(first)?;

                                for name in set {
                                    f.write_str("+")?;
                                    f.write_str(name)?;
                                }

                                Ok(())
                            }
                        },
                    }
                }
            }

            impl Debug for $name {
                fn fmt(&self, f: &mut Formatter) -> fmt::Result {
                    Display::fmt(self, f)
                }
            }

            impl BitOr for $name {
                type Output = Self;

                fn bitor(self, rhs: Self) -> Self::Output {
                    Self(self.0 | rhs.0)
                }
            }

            impl BitOrAssign for $name {
                fn bitor_assign(&mut self, rhs: Self) {
                    self.0 |= rhs.0;
                }
            }

            impl BitAnd for $name {
                type Output = Self;

                fn bitand(self, rhs: Self) -> Self::Output {
                    Self(self.0 & rhs.0)
                }
            }

            impl Not for $name {
                type Output = Self;

                fn not(self) -> Self::Output {
                    Self(Self::ALL.0 & !self.0)
                }
            }
        };

    };
}
